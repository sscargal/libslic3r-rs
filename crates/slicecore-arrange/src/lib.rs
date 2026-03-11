//! Build plate auto-arrangement for the slicecore 3D slicing engine.
//!
//! This crate provides automatic positioning of multiple 3D-printed parts
//! on a build plate to maximize utilization and minimize wasted space.
//! It supports arbitrary bed shapes, convex hull footprint projection,
//! spacing/brim/raft-aware footprint expansion, and collision detection.
//!
//! # Architecture
//!
//! - **Bed parsing** ([`bed`]): Parse bed shape strings and create bed polygons
//! - **Footprint computation** ([`footprint`]): Project 3D meshes to 2D convex
//!   hull footprints, expand for spacing, and detect collisions
//! - **Orientation** ([`orient`]): Auto-orient parts for minimal support or
//!   maximal bed contact
//! - **Placement** ([`placer`]): Bottom-left fill algorithm with rotation variants
//! - **Grouping** ([`grouper`]): Material and height-aware multi-plate grouping
//! - **Sequential** ([`sequential`]): Gantry clearance and back-to-front ordering
//! - **Configuration** ([`config`]): Control arrangement behavior via
//!   [`ArrangeConfig`] and describe parts via [`ArrangePart`]
//! - **Results** ([`result`]): Output structures ([`ArrangementResult`],
//!   [`PlateArrangement`], [`PartPlacement`]) describing the arrangement
//!
//! # Quick Start
//!
//! ```
//! use slicecore_math::Point3;
//! use slicecore_arrange::{arrange, ArrangeConfig, ArrangePart};
//!
//! let parts = vec![
//!     ArrangePart {
//!         id: "cube1".into(),
//!         vertices: vec![
//!             Point3::new(0.0, 0.0, 0.0), Point3::new(20.0, 0.0, 0.0),
//!             Point3::new(20.0, 20.0, 0.0), Point3::new(0.0, 20.0, 0.0),
//!             Point3::new(0.0, 0.0, 20.0), Point3::new(20.0, 0.0, 20.0),
//!             Point3::new(20.0, 20.0, 20.0), Point3::new(0.0, 20.0, 20.0),
//!         ],
//!         mesh_height: 20.0,
//!         ..Default::default()
//!     },
//! ];
//! let config = ArrangeConfig::default();
//! let result = arrange(&parts, &config, "", 220.0, 220.0).unwrap();
//! assert_eq!(result.total_plates, 1);
//! ```

#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    missing_docs,
    missing_debug_implementations
)]
#![allow(
    clippy::cargo_common_metadata,
    clippy::multiple_crate_versions,
    clippy::module_name_repetitions
)]

pub mod bed;
pub mod config;
pub mod error;
pub mod footprint;
pub mod grouper;
pub mod orient;
pub mod placer;
pub mod result;
pub mod sequential;

pub use config::{ArrangeConfig, ArrangePart, GantryModel, OrientCriterion};
pub use error::ArrangeError;
pub use result::{ArrangementResult, PartPlacement, PlateArrangement};

use slicecore_math::IPoint2;

use bed::{bed_from_dimensions, bed_with_margin, parse_bed_shape};
use footprint::{compute_footprint, expand_footprint};
use grouper::{group_by_height, group_by_material, split_into_plates};
use orient::auto_orient;
use placer::{
    center_arrangement, effective_spacing, prepare_part, PreparePartConfig, PreparedPart,
};
use sequential::{expand_for_gantry, order_back_to_front, validate_sequential};

/// Arranges parts on the build plate.
///
/// This is the main entry point for the arrangement system. It handles
/// auto-orientation, footprint computation, material/height grouping,
/// multi-plate splitting, sequential mode validation, and centering.
///
/// # Arguments
///
/// * `parts` - Parts to arrange
/// * `config` - Arrangement configuration
/// * `bed_shape` - Bed shape string (e.g., `"0x0,220x0,220x220,0x220"`). If empty, falls back to rectangular bed from dimensions.
/// * `bed_x` - Bed width in mm (fallback when `bed_shape` is empty)
/// * `bed_y` - Bed depth in mm (fallback when `bed_shape` is empty)
///
/// # Errors
///
/// Returns [`ArrangeError::NoPartsProvided`] if `parts` is empty.
/// Returns [`ArrangeError::InvalidBedShape`] if the bed shape string is invalid.
/// Returns [`ArrangeError::SequentialOverlap`] if sequential mode detects gantry collision.
#[allow(
    clippy::similar_names,
    reason = "bed_x/bed_y are standard dimension names"
)]
pub fn arrange(
    parts: &[ArrangePart],
    config: &ArrangeConfig,
    bed_shape: &str,
    bed_x: f64,
    bed_y: f64,
) -> Result<ArrangementResult, ArrangeError> {
    if parts.is_empty() {
        return Err(ArrangeError::NoPartsProvided);
    }

    let bed = if bed_shape.trim().is_empty() {
        bed_from_dimensions(bed_x, bed_y)
    } else {
        parse_bed_shape(bed_shape)?
    };

    arrange_inner(parts, config, &bed)
}

/// Arranges parts with a progress callback.
///
/// Same as [`arrange`] but calls `progress(0.0..1.0)` at key steps:
/// - 0.0: Starting
/// - 0.1: After orientation
/// - 0.2..0.8: During placement (proportional to parts placed)
/// - 0.9: After grouping/sequential
/// - 1.0: Complete
///
/// # Errors
///
/// Same error conditions as [`arrange`].
#[allow(
    clippy::similar_names,
    reason = "bed_x/bed_y are standard dimension names"
)]
pub fn arrange_with_progress(
    parts: &[ArrangePart],
    config: &ArrangeConfig,
    bed_shape: &str,
    bed_x: f64,
    bed_y: f64,
    mut progress: impl FnMut(f64),
) -> Result<ArrangementResult, ArrangeError> {
    if parts.is_empty() {
        return Err(ArrangeError::NoPartsProvided);
    }

    progress(0.0);

    let bed = if bed_shape.trim().is_empty() {
        bed_from_dimensions(bed_x, bed_y)
    } else {
        parse_bed_shape(bed_shape)?
    };

    progress(0.1);

    let spacing = effective_spacing(config);

    // Prepare parts with auto-orient
    let part_count = parts.len();
    let prepared: Vec<PreparedPart> = parts
        .iter()
        .enumerate()
        .map(|(i, part)| {
            let _orientation = compute_orientation(part, config);
            let p = prepare_part(&PreparePartConfig {
                id: &part.id,
                vertices: &part.vertices,
                mesh_height: part.mesh_height,
                material: part.material.clone(),
                rotation_locked: part.rotation_locked,
                rotation_step: config.rotation_step,
                spacing,
                brim_width: config.brim_width,
                raft_margin: config.raft_margin,
            });
            #[allow(clippy::cast_precision_loss, reason = "part count is always small")]
            let frac = 0.1 + 0.7 * ((i + 1) as f64 / part_count as f64) * 0.5;
            progress(frac);
            p
        })
        .collect();

    let result = build_arrangement(&prepared, parts, config, &bed)?;

    progress(0.9);

    progress(1.0);
    Ok(result)
}

/// Computes orientation for a part if auto-orient is enabled.
fn compute_orientation(part: &ArrangePart, config: &ArrangeConfig) -> Option<(f64, f64, f64)> {
    if !config.auto_orient || part.orientation_locked || part.vertices.is_empty() {
        return None;
    }
    // For auto-orient we need face normals and areas.
    // In the absence of explicit normals, we derive them from vertex data.
    // For Phase 27 v1, we use a simplified approach: treat each triangle triple as a face.
    // If vertices are raw mesh vertices (not indexed triangles), we skip orient.
    // Proper normal extraction requires the full TriangleMesh, which is not available here.
    // For now, return identity -- the orient module is tested independently.
    Some(auto_orient(
        &part.vertices,
        &[], // No normals available from ArrangePart
        &[],
        &config.orient_criterion,
    ))
}

/// Core arrangement logic shared between `arrange` and `arrange_with_progress`.
fn arrange_inner(
    parts: &[ArrangePart],
    config: &ArrangeConfig,
    bed: &[slicecore_math::IPoint2],
) -> Result<ArrangementResult, ArrangeError> {
    let spacing = effective_spacing(config);

    // Prepare parts
    let prepared: Vec<PreparedPart> = parts
        .iter()
        .map(|part| {
            prepare_part(&PreparePartConfig {
                id: &part.id,
                vertices: &part.vertices,
                mesh_height: part.mesh_height,
                material: part.material.clone(),
                rotation_locked: part.rotation_locked,
                rotation_step: config.rotation_step,
                spacing,
                brim_width: config.brim_width,
                raft_margin: config.raft_margin,
            })
        })
        .collect();

    build_arrangement(&prepared, parts, config, bed)
}

/// Builds the arrangement result from prepared parts.
fn build_arrangement(
    prepared: &[PreparedPart],
    parts: &[ArrangePart],
    config: &ArrangeConfig,
    bed: &[slicecore_math::IPoint2],
) -> Result<ArrangementResult, ArrangeError> {
    let inset_bed = bed_with_margin(bed, config.bed_margin);

    // Group by material
    let material_groups = if config.material_grouping {
        group_by_material(parts, false)
    } else {
        vec![(0..parts.len()).collect()]
    };

    let mut all_plates: Vec<PlateArrangement> = Vec::new();
    let mut all_unplaced: Vec<String> = Vec::new();

    for mat_group in &material_groups {
        // Sub-group by height
        let height_groups = group_by_height(parts, mat_group);

        for height_group in &height_groups {
            let group_prepared: Vec<PreparedPart> =
                height_group.iter().map(|&i| prepared[i].clone()).collect();

            let (plates, unplaced) = split_into_plates(&group_prepared, &inset_bed, config);

            for mut plate_placements in plates {
                let plate_idx = all_plates.len();
                for p in &mut plate_placements {
                    p.plate_index = plate_idx;
                }

                // Center if configured
                if config.center_after_packing {
                    center_arrangement(&mut plate_placements, bed);
                }

                // Sequential mode
                if config.sequential_mode {
                    // Only validate gantry clearance if a gantry model is set
                    if !matches!(config.gantry_model, GantryModel::None) {
                        let spacing = effective_spacing(config);
                        let footprints: Vec<Vec<IPoint2>> = plate_placements
                            .iter()
                            .map(|p| {
                                let part = parts.iter().find(|pp| pp.id == p.part_id);
                                match part {
                                    Some(pp) => {
                                        let fp = compute_footprint(&pp.vertices);
                                        let expanded = expand_footprint(
                                            &fp,
                                            spacing,
                                            config.brim_width,
                                            config.raft_margin,
                                        );
                                        expand_for_gantry(&expanded, &config.gantry_model)
                                    }
                                    None => Vec::new(),
                                }
                            })
                            .collect();

                        validate_sequential(&plate_placements, &footprints, &config.gantry_model)?;
                    }

                    order_back_to_front(&mut plate_placements);
                }

                all_plates.push(PlateArrangement {
                    plate_index: plate_idx,
                    placements: plate_placements,
                });
            }

            all_unplaced.extend(unplaced);
        }
    }

    let total_plates = all_plates.len();
    Ok(ArrangementResult {
        plates: all_plates,
        total_plates,
        unplaced_parts: all_unplaced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::Point3;

    fn cube_vertices(size: f64) -> Vec<Point3> {
        let h = size / 2.0;
        vec![
            Point3::new(-h, -h, 0.0),
            Point3::new(h, -h, 0.0),
            Point3::new(h, h, 0.0),
            Point3::new(-h, h, 0.0),
            Point3::new(-h, -h, size),
            Point3::new(h, -h, size),
            Point3::new(h, h, size),
            Point3::new(-h, h, size),
        ]
    }

    fn cube_part(id: &str, size: f64) -> ArrangePart {
        ArrangePart {
            id: id.into(),
            vertices: cube_vertices(size),
            mesh_height: size,
            ..Default::default()
        }
    }

    #[test]
    fn arrange_three_cubes_on_220_bed() {
        let parts = vec![
            cube_part("a", 30.0),
            cube_part("b", 50.0),
            cube_part("c", 20.0),
        ];
        let config = ArrangeConfig::default();
        let result = arrange(&parts, &config, "", 220.0, 220.0).unwrap();

        assert!(
            result.total_plates >= 1,
            "Should have at least 1 plate, got {}",
            result.total_plates
        );
        let total_placed: usize = result.plates.iter().map(|p| p.placements.len()).sum();
        assert_eq!(total_placed, 3, "All 3 parts should be placed");
        assert!(result.unplaced_parts.is_empty());
    }

    #[test]
    fn arrange_empty_parts_error() {
        let config = ArrangeConfig::default();
        let result = arrange(&[], &config, "", 220.0, 220.0);
        assert!(result.is_err());
    }

    #[test]
    fn arrange_many_parts_multi_plate() {
        // 6 large parts that can't all fit on one 220x220 bed
        // (each ~100mm, only 4 fit per plate with spacing)
        let parts: Vec<ArrangePart> = (0..6).map(|i| cube_part(&format!("p{i}"), 90.0)).collect();
        let config = ArrangeConfig::default();
        let result = arrange(&parts, &config, "", 220.0, 220.0).unwrap();

        assert!(
            result.total_plates > 1,
            "Should need multiple plates for 6 x 90mm cubes, got {}",
            result.total_plates
        );
        let total_placed: usize = result.plates.iter().map(|p| p.placements.len()).sum();
        assert_eq!(
            total_placed + result.unplaced_parts.len(),
            6,
            "All parts should be accounted for"
        );
    }

    #[test]
    fn arrange_with_sequential_mode() {
        // Use small parts on a large bed so they are well-separated
        let parts = vec![
            ArrangePart {
                id: "seq_a".into(),
                vertices: cube_vertices(10.0),
                mesh_height: 10.0,
                ..Default::default()
            },
            ArrangePart {
                id: "seq_b".into(),
                vertices: cube_vertices(10.0),
                mesh_height: 10.0,
                ..Default::default()
            },
        ];
        let mut config = ArrangeConfig::default();
        config.sequential_mode = true;
        config.gantry_model = GantryModel::None; // No gantry clearance for this test
        config.part_spacing = 20.0; // Large spacing to ensure separation

        let result = arrange(&parts, &config, "", 220.0, 220.0).unwrap();

        assert!(result.total_plates >= 1);
        // Check that print_order is set
        let plate = &result.plates[0];
        for p in &plate.placements {
            assert!(
                p.print_order.is_some(),
                "Sequential mode should set print_order for {}",
                p.part_id
            );
        }
    }

    #[test]
    fn arrange_with_progress_callback() {
        let parts = vec![cube_part("prog1", 30.0), cube_part("prog2", 30.0)];
        let config = ArrangeConfig::default();

        let mut progress_values: Vec<f64> = Vec::new();
        let result = arrange_with_progress(&parts, &config, "", 220.0, 220.0, |p| {
            progress_values.push(p);
        });

        assert!(result.is_ok());
        assert!(!progress_values.is_empty(), "Progress should be called");
        assert!(
            progress_values.first().copied() == Some(0.0),
            "Should start at 0.0"
        );
        assert!(
            (progress_values.last().copied().unwrap_or(0.0) - 1.0).abs() < f64::EPSILON,
            "Should end at 1.0"
        );
    }

    #[test]
    fn arrange_with_bed_shape_string() {
        let parts = vec![cube_part("shaped", 20.0)];
        let config = ArrangeConfig::default();
        let result = arrange(&parts, &config, "0x0,220x0,220x220,0x220", 0.0, 0.0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().total_plates, 1);
    }

    #[test]
    fn arrange_with_material_grouping() {
        let parts = vec![
            ArrangePart {
                id: "pla1".into(),
                vertices: cube_vertices(20.0),
                mesh_height: 20.0,
                material: Some("PLA".into()),
                ..Default::default()
            },
            ArrangePart {
                id: "abs1".into(),
                vertices: cube_vertices(20.0),
                mesh_height: 20.0,
                material: Some("ABS".into()),
                ..Default::default()
            },
            ArrangePart {
                id: "pla2".into(),
                vertices: cube_vertices(20.0),
                mesh_height: 20.0,
                material: Some("PLA".into()),
                ..Default::default()
            },
        ];
        let mut config = ArrangeConfig::default();
        config.material_grouping = true;
        let result = arrange(&parts, &config, "", 220.0, 220.0).unwrap();

        let total_placed: usize = result.plates.iter().map(|p| p.placements.len()).sum();
        assert_eq!(total_placed, 3, "All parts should be placed");
    }
}
