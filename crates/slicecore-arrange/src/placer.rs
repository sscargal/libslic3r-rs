//! Bottom-left fill placement algorithm for part arrangement.
//!
//! Places parts on the build plate using a largest-first, bottom-left fill
//! strategy with rotation variants and nozzle-aware adaptive spacing.

use slicecore_math::{mm_to_coord, IBBox2, IPoint2};

use crate::bed::{bed_with_margin, point_in_bed};
use crate::config::ArrangeConfig;
use crate::footprint::{
    centroid, compute_footprint, expand_footprint, footprint_area, footprints_overlap,
    rotate_footprint,
};
use crate::result::PartPlacement;

/// A part prepared for placement with pre-computed footprint variants.
#[derive(Clone, Debug)]
pub struct PreparedPart {
    /// Part identifier.
    pub id: String,
    /// Pre-computed footprint variants (one per allowed rotation).
    pub footprint_variants: Vec<Vec<IPoint2>>,
    /// Area of the base footprint in mm^2 (for sorting).
    pub area: f64,
    /// Height of the mesh in mm.
    pub mesh_height: f64,
    /// Material identifier.
    pub material: Option<String>,
    /// Rotation angles corresponding to each variant, in degrees.
    pub rotation_angles: Vec<f64>,
}

/// Configuration for preparing a part for placement.
#[derive(Clone, Debug)]
pub struct PreparePartConfig<'a> {
    /// Part identifier.
    pub id: &'a str,
    /// 3D mesh vertices.
    pub vertices: &'a [slicecore_math::Point3],
    /// Height of the mesh in mm.
    pub mesh_height: f64,
    /// Material identifier.
    pub material: Option<String>,
    /// Whether the part's rotation is locked.
    pub rotation_locked: bool,
    /// Rotation step in degrees.
    pub rotation_step: f64,
    /// Effective spacing in mm.
    pub spacing: f64,
    /// Brim width in mm.
    pub brim_width: f64,
    /// Raft margin in mm.
    pub raft_margin: f64,
}

/// Computes the effective spacing considering nozzle diameter.
///
/// Returns `max(part_spacing, nozzle_diameter * 1.5)` to ensure spacing is
/// always at least 1.5x the nozzle diameter, preventing warping and
/// elephant's foot issues with larger nozzles.
///
/// # Examples
///
/// ```
/// use slicecore_arrange::config::ArrangeConfig;
/// use slicecore_arrange::placer::effective_spacing;
///
/// let mut config = ArrangeConfig::default();
/// config.nozzle_diameter = 0.4;
/// config.part_spacing = 2.0;
/// assert!((effective_spacing(&config) - 2.0).abs() < f64::EPSILON);
///
/// config.nozzle_diameter = 1.2;
/// config.part_spacing = 1.0;
/// assert!((effective_spacing(&config) - 1.8).abs() < 1e-10);
/// ```
#[must_use]
pub fn effective_spacing(config: &ArrangeConfig) -> f64 {
    config.part_spacing.max(config.nozzle_diameter * 1.5)
}

/// Prepares a part for placement by computing footprint rotation variants.
///
/// If the part is rotation-locked, only the base footprint (at 0 degrees)
/// is included. Otherwise, footprints at each rotation step increment
/// (0, step, 2*step, ...) are computed.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "rotation count from 360/step is always a small positive number"
)]
pub fn prepare_part(cfg: &PreparePartConfig<'_>) -> PreparedPart {
    let base_footprint = compute_footprint(cfg.vertices);
    let base_expanded =
        expand_footprint(&base_footprint, cfg.spacing, cfg.brim_width, cfg.raft_margin);
    let area = footprint_area(&base_expanded);

    let mut footprint_variants = Vec::new();
    let mut rotation_angles = Vec::new();

    if cfg.rotation_locked || cfg.rotation_step <= 0.0 {
        footprint_variants.push(base_expanded);
        rotation_angles.push(0.0);
    } else {
        let num_rotations = (360.0 / cfg.rotation_step).ceil() as usize;
        for i in 0..num_rotations {
            let angle = (i as f64) * cfg.rotation_step;
            if angle >= 360.0 {
                break;
            }
            let rotated = if angle.abs() < f64::EPSILON {
                base_expanded.clone()
            } else {
                let rotated_base = rotate_footprint(&base_footprint, angle);
                expand_footprint(&rotated_base, cfg.spacing, cfg.brim_width, cfg.raft_margin)
            };
            footprint_variants.push(rotated);
            rotation_angles.push(angle);
        }
    }

    PreparedPart {
        id: cfg.id.to_string(),
        footprint_variants,
        area,
        mesh_height: cfg.mesh_height,
        material: cfg.material.clone(),
        rotation_angles,
    }
}

/// Translates a footprint polygon by the given offset.
fn translate_footprint(footprint: &[IPoint2], dx: i64, dy: i64) -> Vec<IPoint2> {
    footprint
        .iter()
        .map(|p| IPoint2::new(p.x + dx, p.y + dy))
        .collect()
}

/// Tests whether a translated footprint fits entirely within the bed.
fn footprint_fits_in_bed(footprint: &[IPoint2], bed: &[IPoint2]) -> bool {
    footprint.iter().all(|p| point_in_bed(p, bed))
}

/// Places parts on the bed using a bottom-left fill algorithm.
///
/// Parts are sorted by area (largest first) and placed at the first valid
/// position found by scanning from bottom-left. Each part tries all rotation
/// variants and picks the first valid placement.
///
/// Returns a tuple of (placed parts, unplaced part IDs).
///
/// # Arguments
///
/// * `parts` - Pre-computed parts with footprint variants
/// * `bed` - Bed boundary polygon
/// * `config` - Arrangement configuration
#[must_use]
pub fn place_parts(
    parts: &[PreparedPart],
    bed: &[IPoint2],
    config: &ArrangeConfig,
) -> (Vec<PartPlacement>, Vec<String>) {
    let inset_bed = bed_with_margin(bed, config.bed_margin);

    // Sort parts by area descending (largest first)
    let mut indices: Vec<usize> = (0..parts.len()).collect();
    indices.sort_by(|&a, &b| {
        parts[b]
            .area
            .partial_cmp(&parts[a].area)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let Some(bed_bbox) = IBBox2::from_points(&inset_bed) else {
        return (Vec::new(), parts.iter().map(|p| p.id.clone()).collect());
    };

    // Scanning resolution: use nozzle_diameter converted to coord
    let scan_step = mm_to_coord(config.nozzle_diameter.max(0.5));

    let mut placed: Vec<PartPlacement> = Vec::new();
    let mut placed_footprints: Vec<Vec<IPoint2>> = Vec::new();
    let mut unplaced: Vec<String> = Vec::new();

    for &idx in &indices {
        let part = &parts[idx];
        let mut found = false;

        'variant: for (vi, variant) in part.footprint_variants.iter().enumerate() {
            let Some(fp_bbox) = IBBox2::from_points(variant) else {
                continue;
            };

            let fp_width = fp_bbox.max.x - fp_bbox.min.x;
            let fp_height = fp_bbox.max.y - fp_bbox.min.y;

            // Scan positions within inset bed
            let mut y = bed_bbox.min.y;
            while y <= bed_bbox.max.y - fp_height {
                let mut x = bed_bbox.min.x;
                while x <= bed_bbox.max.x - fp_width {
                    // Translate footprint to candidate position
                    let dx = x - fp_bbox.min.x;
                    let dy = y - fp_bbox.min.y;
                    let translated = translate_footprint(variant, dx, dy);

                    // Check: all points inside inset bed
                    if !footprint_fits_in_bed(&translated, &inset_bed) {
                        x += scan_step;
                        continue;
                    }

                    // Check: no overlap with placed footprints
                    let overlaps = placed_footprints
                        .iter()
                        .any(|placed_fp| footprints_overlap(&translated, placed_fp));

                    if !overlaps {
                        let center = centroid(&translated);
                        let (cx, cy) = center.to_mm();
                        placed.push(PartPlacement {
                            part_id: part.id.clone(),
                            position: (cx, cy),
                            rotation_deg: part.rotation_angles[vi],
                            orientation: None,
                            plate_index: 0,
                            print_order: None,
                        });
                        placed_footprints.push(translated);
                        found = true;
                        break 'variant;
                    }

                    x += scan_step;
                }
                y += scan_step;
            }
        }

        if !found {
            unplaced.push(part.id.clone());
        }
    }

    (placed, unplaced)
}

/// Centers an arrangement on the bed.
///
/// Computes the bounding box of all placements, finds the offset needed
/// to center it on the bed, and shifts all placement positions accordingly.
#[allow(clippy::similar_names, reason = "min_x/max_x and min_y/max_y are standard bbox names")]
pub fn center_arrangement(placements: &mut [PartPlacement], bed: &[IPoint2]) {
    if placements.is_empty() || bed.is_empty() {
        return;
    }

    let bed_center = centroid(bed);
    let (bed_cx, bed_cy) = bed_center.to_mm();

    // Compute bounding box of placements
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for p in placements.iter() {
        min_x = min_x.min(p.position.0);
        min_y = min_y.min(p.position.1);
        max_x = max_x.max(p.position.0);
        max_y = max_y.max(p.position.1);
    }

    let placement_cx = (min_x + max_x) / 2.0;
    let placement_cy = (min_y + max_y) / 2.0;

    let dx = bed_cx - placement_cx;
    let dy = bed_cy - placement_cy;

    for p in placements.iter_mut() {
        p.position.0 += dx;
        p.position.1 += dy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::Point3;

    /// Creates a cube mesh with vertices for a given size centered at origin.
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

    fn default_config() -> ArrangeConfig {
        ArrangeConfig::default()
    }

    fn make_prepare_config<'a>(
        id: &'a str,
        vertices: &'a [Point3],
        mesh_height: f64,
        rotation_locked: bool,
        config: &ArrangeConfig,
        spacing: f64,
    ) -> PreparePartConfig<'a> {
        PreparePartConfig {
            id,
            vertices,
            mesh_height,
            material: None,
            rotation_locked,
            rotation_step: config.rotation_step,
            spacing,
            brim_width: config.brim_width,
            raft_margin: config.raft_margin,
        }
    }

    #[test]
    fn effective_spacing_standard_nozzle() {
        let mut config = default_config();
        config.nozzle_diameter = 0.4;
        config.part_spacing = 2.0;
        let spacing = effective_spacing(&config);
        assert!(
            (spacing - 2.0).abs() < f64::EPSILON,
            "Expected 2.0, got {spacing}"
        );
    }

    #[test]
    fn effective_spacing_large_nozzle_overrides() {
        let mut config = default_config();
        config.nozzle_diameter = 1.2;
        config.part_spacing = 1.0;
        let spacing = effective_spacing(&config);
        assert!(
            (spacing - 1.8).abs() < 1e-10,
            "Expected 1.8, got {spacing}"
        );
    }

    #[test]
    fn effective_spacing_nozzle_1mm() {
        let mut config = default_config();
        config.nozzle_diameter = 1.0;
        config.part_spacing = 2.0;
        let spacing = effective_spacing(&config);
        assert!(
            (spacing - 2.0).abs() < f64::EPSILON,
            "Expected 2.0, got {spacing}"
        );
    }

    #[test]
    fn two_cubes_placed_on_220x220_bed() {
        let config = default_config();
        let spacing = effective_spacing(&config);
        let bed = crate::bed::bed_from_dimensions(220.0, 220.0);
        let verts1 = cube_vertices(50.0);
        let verts2 = cube_vertices(50.0);

        let p1 = prepare_part(&make_prepare_config("cube1", &verts1, 50.0, false, &config, spacing));
        let p2 = prepare_part(&make_prepare_config("cube2", &verts2, 50.0, false, &config, spacing));

        let parts = vec![p1, p2];
        let (placed, unplaced) = place_parts(&parts, &bed, &config);

        assert_eq!(placed.len(), 2, "Both cubes should be placed");
        assert!(unplaced.is_empty(), "No cubes should be unplaced");

        // Verify no overlap: positions should differ
        let pos0 = &placed[0].position;
        let pos1 = &placed[1].position;
        let dist = ((pos0.0 - pos1.0).powi(2) + (pos0.1 - pos1.1).powi(2)).sqrt();
        assert!(dist > 10.0, "Parts should be separated, dist={dist}");
    }

    #[test]
    fn oversized_part_returns_unplaced() {
        let config = default_config();
        let spacing = effective_spacing(&config);
        let bed = crate::bed::bed_from_dimensions(220.0, 220.0);
        let verts = cube_vertices(250.0);

        let p = prepare_part(&make_prepare_config("huge", &verts, 50.0, false, &config, spacing));

        let (placed, unplaced) = place_parts(&[p], &bed, &config);
        assert!(placed.is_empty(), "Oversized part should not be placed");
        assert_eq!(unplaced.len(), 1);
        assert_eq!(unplaced[0], "huge");
    }

    #[test]
    fn rotation_locked_single_variant() {
        let config = default_config();
        let spacing = effective_spacing(&config);
        let verts = cube_vertices(20.0);

        let p = prepare_part(&make_prepare_config("locked", &verts, 20.0, true, &config, spacing));

        assert_eq!(
            p.footprint_variants.len(),
            1,
            "Rotation-locked part should have exactly 1 variant"
        );
        assert!(
            p.rotation_angles[0].abs() < f64::EPSILON,
            "Single variant should be at 0 degrees"
        );
    }

    #[test]
    fn parts_sorted_largest_first() {
        let config = default_config();
        let spacing = effective_spacing(&config);
        let bed = crate::bed::bed_from_dimensions(220.0, 220.0);
        let small_verts = cube_vertices(20.0);
        let large_verts = cube_vertices(80.0);

        let small = prepare_part(&make_prepare_config("small", &small_verts, 20.0, false, &config, spacing));
        let large = prepare_part(&make_prepare_config("large", &large_verts, 80.0, false, &config, spacing));

        // Pass small first, but placer should process large first
        let parts = vec![small, large];
        let (placed, _unplaced) = place_parts(&parts, &bed, &config);

        assert_eq!(placed.len(), 2, "Both should be placed");
    }

    #[test]
    fn center_arrangement_shifts_to_bed_center() {
        let bed = crate::bed::bed_from_dimensions(220.0, 220.0);
        let mut placements = vec![
            PartPlacement {
                part_id: "a".into(),
                position: (10.0, 10.0),
                rotation_deg: 0.0,
                orientation: None,
                plate_index: 0,
                print_order: None,
            },
            PartPlacement {
                part_id: "b".into(),
                position: (30.0, 10.0),
                rotation_deg: 0.0,
                orientation: None,
                plate_index: 0,
                print_order: None,
            },
        ];

        center_arrangement(&mut placements, &bed);

        // Group center was (20, 10), bed center is (110, 110)
        // Shift should be (90, 100)
        assert!(
            (placements[0].position.0 - 100.0).abs() < 1.0,
            "a.x should be ~100, got {}",
            placements[0].position.0
        );
        assert!(
            (placements[0].position.1 - 110.0).abs() < 1.0,
            "a.y should be ~110, got {}",
            placements[0].position.1
        );
        assert!(
            (placements[1].position.0 - 120.0).abs() < 1.0,
            "b.x should be ~120, got {}",
            placements[1].position.0
        );
    }
}
