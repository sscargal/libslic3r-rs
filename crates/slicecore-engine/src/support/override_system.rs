//! Manual support override system with enforcers, blockers, and volume modifiers.
//!
//! Provides mechanisms for users to manually override automatic support placement:
//! - **Mesh-based enforcers/blockers**: Load STL meshes to define enforce/block regions.
//! - **Volume modifiers**: Programmatic shapes (box, cylinder, sphere) for enforce/block.
//! - **Override application**: Combines auto-support with manual overrides per layer.
//!
//! Enforcer order: enforcers first (union), then blockers (difference).
//! Blockers always win in conflicting regions.

use serde::{Deserialize, Serialize};
use slicecore_geo::polygon::{Polygon, ValidPolygon, Winding};
use slicecore_geo::{polygon_difference, polygon_union};
use slicecore_mesh::TriangleMesh;

use super::conflict::ConflictWarning;

/// Computes the net area in mm^2 of a set of polygons, accounting for holes.
///
/// CCW polygons contribute positive area (outer boundaries), CW polygons
/// contribute negative area (holes). Returns the absolute net area.
fn net_area_mm2(polygons: &[ValidPolygon]) -> f64 {
    let net: f64 = polygons
        .iter()
        .map(|p| match p.winding() {
            Winding::CounterClockwise => p.area_mm2(),
            Winding::Clockwise => -p.area_mm2(),
        })
        .sum();
    net.abs()
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Geometric shape of a volume modifier.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeShape {
    /// Axis-aligned box (width x depth x height).
    Box,
    /// Vertical cylinder (radius x radius x height).
    Cylinder,
    /// Sphere (radius x radius x radius).
    Sphere,
}

/// Role of an override modifier: enforcer adds support, blocker removes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideRole {
    /// Adds support in the specified region.
    Enforcer,
    /// Removes support from the specified region.
    Blocker,
}

// ---------------------------------------------------------------------------
// Volume modifier
// ---------------------------------------------------------------------------

/// A programmatic volume modifier that defines an enforce/block region.
///
/// Volume modifiers are geometric primitives (box, cylinder, sphere) positioned
/// in 3D space. At each layer height, their 2D cross-section is computed and
/// used to add or remove support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeModifier {
    /// Geometric shape type.
    pub shape: VolumeShape,
    /// Whether this modifier adds or removes support.
    pub role: OverrideRole,
    /// Center position in mm (x, y, z).
    pub center: (f64, f64, f64),
    /// Dimensions in mm:
    /// - Box: (width, depth, height)
    /// - Cylinder: (radius, radius, height)
    /// - Sphere: (radius, radius, radius)
    pub size: (f64, f64, f64),
    /// Rotation around Z axis in degrees (for box/cylinder alignment).
    pub rotation: f64,
}

/// Computes the 2D cross-section of a volume modifier at a given Z height.
///
/// Returns `None` if the Z height is outside the volume's vertical extent.
/// Returns a `ValidPolygon` representing the cross-section shape otherwise.
///
/// # Shapes
///
/// - **Box**: Rectangle at heights within `[center.z - h/2, center.z + h/2]`.
///   Applies Z-axis rotation if specified.
/// - **Cylinder**: Circle (32-vertex approximation) at heights within the
///   cylinder's vertical range.
/// - **Sphere**: Circle with radius `sqrt(R^2 - dz^2)` at heights within
///   `[center.z - R, center.z + R]`.
pub fn volume_modifier_at_z(modifier: &VolumeModifier, z: f64) -> Option<ValidPolygon> {
    let (cx, cy, cz) = modifier.center;

    match modifier.shape {
        VolumeShape::Box => {
            let (w, d, h) = modifier.size;
            let half_h = h / 2.0;
            if z < cz - half_h || z > cz + half_h {
                return None;
            }
            // Rectangle centered at (cx, cy) with dimensions w x d.
            let hw = w / 2.0;
            let hd = d / 2.0;
            let corners = if modifier.rotation.abs() > 1e-9 {
                let angle_rad = modifier.rotation.to_radians();
                let cos_a = angle_rad.cos();
                let sin_a = angle_rad.sin();
                // Rotate each corner around center.
                let raw = [(-hw, -hd), (hw, -hd), (hw, hd), (-hw, hd)];
                raw.iter()
                    .map(|&(rx, ry)| {
                        let rotated_x = rx * cos_a - ry * sin_a + cx;
                        let rotated_y = rx * sin_a + ry * cos_a + cy;
                        (rotated_x, rotated_y)
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![
                    (cx - hw, cy - hd),
                    (cx + hw, cy - hd),
                    (cx + hw, cy + hd),
                    (cx - hw, cy + hd),
                ]
            };
            Polygon::from_mm(&corners).validate().ok()
        }
        VolumeShape::Cylinder => {
            let (radius, _, h) = modifier.size;
            let half_h = h / 2.0;
            if z < cz - half_h || z > cz + half_h {
                return None;
            }
            // Circle approximated with 32 vertices.
            Some(make_circle_polygon(cx, cy, radius, 32))
        }
        VolumeShape::Sphere => {
            let radius = modifier.size.0;
            let dz = z - cz;
            if dz.abs() > radius {
                return None;
            }
            // Cross-section radius: sqrt(R^2 - dz^2)
            let cross_r = (radius * radius - dz * dz).sqrt();
            if cross_r < 1e-6 {
                return None; // Too small to be meaningful.
            }
            Some(make_circle_polygon(cx, cy, cross_r, 32))
        }
    }
}

/// Creates a circle polygon centered at (cx, cy) with the given radius and vertex count.
fn make_circle_polygon(cx: f64, cy: f64, radius: f64, n_vertices: usize) -> ValidPolygon {
    let points: Vec<(f64, f64)> = (0..n_vertices)
        .map(|i| {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n_vertices as f64);
            (cx + radius * angle.cos(), cy + radius * angle.sin())
        })
        .collect();
    Polygon::from_mm(&points)
        .validate()
        .expect("Circle polygon with sufficient vertices should always be valid")
}

// ---------------------------------------------------------------------------
// Mesh-based override
// ---------------------------------------------------------------------------

/// A mesh-based enforcer or blocker.
///
/// Contains pre-sliced regions per layer for efficient per-layer override
/// application. The source mesh is consumed during slicing and not retained,
/// since only the per-layer contours are needed for override application.
#[derive(Clone, Debug)]
pub struct MeshOverride {
    /// Whether this mesh acts as an enforcer or blocker.
    pub role: OverrideRole,
    /// Pre-sliced regions per layer. `sliced_regions[i]` contains the
    /// contour polygons at layer `i`.
    pub sliced_regions: Vec<Vec<ValidPolygon>>,
}

/// Slices an enforcer/blocker mesh at each layer height.
///
/// Pre-computes the 2D cross-section of the override mesh at each layer,
/// storing results for efficient per-layer application.
///
/// # Parameters
///
/// - `mesh`: The enforcer or blocker mesh (STL geometry).
/// - `role`: Whether this mesh acts as an enforcer or blocker.
/// - `layer_heights`: Per-layer `(z, layer_height)` pairs.
///
/// # Returns
///
/// A `MeshOverride` with pre-sliced regions ready for application.
pub fn slice_override_mesh(
    mesh: &TriangleMesh,
    role: OverrideRole,
    layer_heights: &[(f64, f64)],
) -> MeshOverride {
    let sliced_regions: Vec<Vec<ValidPolygon>> = layer_heights
        .iter()
        .map(|&(z, _)| slicecore_slicer::slice_at_height(mesh, z))
        .collect();

    MeshOverride {
        role,
        sliced_regions,
    }
}

// ---------------------------------------------------------------------------
// Override application
// ---------------------------------------------------------------------------

/// Applies manual overrides (enforcers, blockers, volume modifiers) to auto-generated support.
///
/// Processing order per layer:
/// 1. Apply mesh-based enforcers (union with auto-support).
/// 2. Apply volume modifier enforcers (union).
/// 3. Apply mesh-based blockers (difference from support).
/// 4. Apply volume modifier blockers (difference).
///
/// Blockers are always applied after enforcers, ensuring blocker priority
/// in conflicting regions.
///
/// # Parameters
///
/// - `auto_support`: Mutable per-layer auto-generated support regions.
/// - `enforcers`: Pre-sliced enforcer meshes.
/// - `blockers`: Pre-sliced blocker meshes.
/// - `volume_modifiers`: Programmatic volume modifiers.
/// - `layer_heights`: Per-layer `(z, layer_height)` pairs.
///
/// # Returns
///
/// List of `ConflictWarning` for cases where blockers removed auto-detected
/// critical support (determined by area threshold).
pub fn apply_overrides(
    auto_support: &mut [Vec<ValidPolygon>],
    enforcers: &[MeshOverride],
    blockers: &[MeshOverride],
    volume_modifiers: &[VolumeModifier],
    layer_heights: &[(f64, f64)],
) -> Vec<ConflictWarning> {
    let mut warnings = Vec::new();

    for (layer_idx, layer_support) in auto_support.iter_mut().enumerate() {
        let z = layer_heights.get(layer_idx).map(|&(z, _)| z).unwrap_or(0.0);

        // Snapshot auto-support area before overrides for conflict detection.
        let auto_area = net_area_mm2(layer_support);

        // --- Step 1: Apply mesh-based enforcers (union) ---
        for enforcer in enforcers {
            if let Some(regions) = enforcer.sliced_regions.get(layer_idx) {
                if !regions.is_empty() && !layer_support.is_empty() {
                    if let Ok(merged) = polygon_union(layer_support, regions) {
                        if !merged.is_empty() {
                            *layer_support = merged;
                        }
                    }
                } else if !regions.is_empty() {
                    // No auto-support at this layer; enforcer creates new support.
                    *layer_support = regions.clone();
                }
            }
        }

        // --- Step 2: Apply volume modifier enforcers (union) ---
        for vm in volume_modifiers
            .iter()
            .filter(|v| v.role == OverrideRole::Enforcer)
        {
            if let Some(cross_section) = volume_modifier_at_z(vm, z) {
                if layer_support.is_empty() {
                    *layer_support = vec![cross_section];
                } else if let Ok(merged) = polygon_union(layer_support, &[cross_section]) {
                    if !merged.is_empty() {
                        *layer_support = merged;
                    }
                }
            }
        }

        // --- Step 3: Apply mesh-based blockers (difference) ---
        for blocker in blockers {
            if let Some(regions) = blocker.sliced_regions.get(layer_idx) {
                if !regions.is_empty() && !layer_support.is_empty() {
                    *layer_support = polygon_difference(layer_support, regions).unwrap_or_default();
                }
            }
        }

        // --- Step 4: Apply volume modifier blockers (difference) ---
        for vm in volume_modifiers
            .iter()
            .filter(|v| v.role == OverrideRole::Blocker)
        {
            if let Some(cross_section) = volume_modifier_at_z(vm, z) {
                if !layer_support.is_empty() {
                    *layer_support =
                        polygon_difference(layer_support, &[cross_section]).unwrap_or_default();
                }
            }
        }

        // --- Conflict detection: check if blocker removed significant auto-support ---
        let post_area = net_area_mm2(layer_support);

        let removed_area = auto_area - post_area;
        // Warn if more than 1 mm^2 of auto-support was removed.
        if removed_area > 1.0 {
            warnings.push(ConflictWarning {
                layer_index: layer_idx,
                conflict_type: super::conflict::ConflictType::BlockerRemovesCritical,
                message: format!(
                    "Blocker removed {:.1} mm^2 of auto-detected support at layer {}",
                    removed_area, layer_idx
                ),
                affected_area_mm2: removed_area,
            });
        }
    }

    warnings
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    /// Helper to create a validated CCW square at a given position and size.
    fn make_square(x: f64, y: f64, size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(x, y), (x + size, y), (x + size, y + size), (x, y + size)])
            .validate()
            .unwrap()
    }

    // --- Volume modifier cross-section tests ---

    #[test]
    fn box_volume_modifier_within_range() {
        let modifier = VolumeModifier {
            shape: VolumeShape::Box,
            role: OverrideRole::Enforcer,
            center: (50.0, 50.0, 5.0),
            size: (10.0, 10.0, 10.0), // 10x10x10 box
            rotation: 0.0,
        };

        // z=5.0 is at the center -- should produce a rectangle.
        let result = volume_modifier_at_z(&modifier, 5.0);
        assert!(
            result.is_some(),
            "Box at center Z should produce cross-section"
        );

        let poly = result.unwrap();
        // Rectangle should be approximately 10x10 = 100 mm^2.
        let area = poly.area_mm2();
        assert!(
            (area - 100.0).abs() < 1.0,
            "Box cross-section area should be ~100 mm^2, got {}",
            area
        );
    }

    #[test]
    fn box_volume_modifier_outside_range() {
        let modifier = VolumeModifier {
            shape: VolumeShape::Box,
            role: OverrideRole::Enforcer,
            center: (50.0, 50.0, 5.0),
            size: (10.0, 10.0, 10.0),
            rotation: 0.0,
        };

        // z=20.0 is far above -- should return None.
        let result = volume_modifier_at_z(&modifier, 20.0);
        assert!(result.is_none(), "Box outside Z range should return None");
    }

    #[test]
    fn sphere_cross_section_at_equator_is_maximum() {
        let modifier = VolumeModifier {
            shape: VolumeShape::Sphere,
            role: OverrideRole::Enforcer,
            center: (50.0, 50.0, 10.0),
            size: (5.0, 5.0, 5.0), // radius=5
            rotation: 0.0,
        };

        // At equator (z=10.0), cross-section radius = full radius = 5.
        let equator = volume_modifier_at_z(&modifier, 10.0).unwrap();
        let equator_area = equator.area_mm2();

        // Expected area = PI * 5^2 = ~78.54 mm^2 (polygon approximation will be slightly less).
        assert!(
            equator_area > 70.0 && equator_area < 85.0,
            "Sphere equator cross-section should be ~78.5 mm^2, got {}",
            equator_area
        );

        // Near the pole (z=14.5), cross-section radius = sqrt(25 - 20.25) = sqrt(4.75) ~ 2.18.
        let near_pole = volume_modifier_at_z(&modifier, 14.5).unwrap();
        let near_pole_area = near_pole.area_mm2();

        assert!(
            near_pole_area < equator_area,
            "Cross-section near pole ({}) should be smaller than equator ({})",
            near_pole_area,
            equator_area
        );

        // At the pole (z=15.0), cross-section radius = 0 -> None.
        let at_pole = volume_modifier_at_z(&modifier, 15.0);
        assert!(
            at_pole.is_none(),
            "Sphere at pole should return None (zero radius)"
        );
    }

    #[test]
    fn cylinder_cross_section_within_range() {
        let modifier = VolumeModifier {
            shape: VolumeShape::Cylinder,
            role: OverrideRole::Blocker,
            center: (50.0, 50.0, 5.0),
            size: (3.0, 3.0, 10.0), // radius=3, height=10
            rotation: 0.0,
        };

        // z=5.0 is center -- should produce a circle.
        let result = volume_modifier_at_z(&modifier, 5.0);
        assert!(
            result.is_some(),
            "Cylinder at center Z should produce circle"
        );

        let poly = result.unwrap();
        // Circle area = PI * 3^2 = ~28.27 mm^2.
        let area = poly.area_mm2();
        assert!(
            area > 25.0 && area < 32.0,
            "Cylinder cross-section should be ~28.3 mm^2, got {}",
            area
        );

        // Outside range.
        let outside = volume_modifier_at_z(&modifier, 20.0);
        assert!(
            outside.is_none(),
            "Cylinder outside Z range should return None"
        );
    }

    // --- Override application tests ---

    #[test]
    fn enforcer_union_adds_support() {
        // Auto-support: a 10x10 square at (50, 50).
        let auto_square = make_square(50.0, 50.0, 10.0);
        let mut auto_support = vec![vec![auto_square.clone()]];

        // Enforcer: a 10x10 square at (60, 50) -- adjacent, no overlap with auto.
        let enforcer_square = make_square(60.0, 50.0, 10.0);
        let enforcer = MeshOverride {
            role: OverrideRole::Enforcer,
            sliced_regions: vec![vec![enforcer_square]],
        };

        let layer_heights = vec![(0.1, 0.2)];
        let warnings = apply_overrides(&mut auto_support, &[enforcer], &[], &[], &layer_heights);

        // Result should be larger than original auto-support.
        let result_area = net_area_mm2(&auto_support[0]);
        let original_area = 100.0; // 10x10 mm.
        assert!(
            result_area > original_area + 50.0,
            "Enforcer should grow support area from {} to at least {}, got {}",
            original_area,
            original_area + 50.0,
            result_area
        );

        // No conflict warnings expected (only enforcers applied).
        assert!(
            warnings.is_empty(),
            "No conflict warnings expected for enforcers only"
        );
    }

    #[test]
    fn blocker_difference_removes_support() {
        // Auto-support: a 20x20 square at (50, 50).
        let auto_square = make_square(50.0, 50.0, 20.0);
        let mut auto_support = vec![vec![auto_square.clone()]];

        // Blocker: a 10x10 square at (55, 55) -- overlaps center of auto-support.
        let blocker_square = make_square(55.0, 55.0, 10.0);
        let blocker = MeshOverride {
            role: OverrideRole::Blocker,
            sliced_regions: vec![vec![blocker_square]],
        };

        let layer_heights = vec![(0.1, 0.2)];
        let warnings = apply_overrides(&mut auto_support, &[], &[blocker], &[], &layer_heights);

        // Result should be smaller than original (using net area to account for holes).
        let result_area = net_area_mm2(&auto_support[0]);
        let original_area = 400.0; // 20x20 mm.
        assert!(
            result_area < original_area - 50.0,
            "Blocker should shrink support area from {} by at least 50 mm^2, got {}",
            original_area,
            result_area
        );

        // Should have conflict warnings (blocker removed >1 mm^2 of auto-support).
        assert!(
            !warnings.is_empty(),
            "Should warn about blocker removing auto-support"
        );
    }

    #[test]
    fn blocker_wins_over_enforcer() {
        // Auto-support: a 10x10 square at (50, 50).
        let auto_square = make_square(50.0, 50.0, 10.0);
        let mut auto_support = vec![vec![auto_square.clone()]];

        // Enforcer and blocker overlap the same 10x10 region at (50, 50).
        let overlap_square = make_square(50.0, 50.0, 10.0);

        let enforcer = MeshOverride {
            role: OverrideRole::Enforcer,
            sliced_regions: vec![vec![overlap_square.clone()]],
        };
        let blocker = MeshOverride {
            role: OverrideRole::Blocker,
            sliced_regions: vec![vec![overlap_square]],
        };

        let layer_heights = vec![(0.1, 0.2)];
        let _warnings = apply_overrides(
            &mut auto_support,
            &[enforcer],
            &[blocker],
            &[],
            &layer_heights,
        );

        // Blocker applied after enforcer -> result should have no support in that region.
        let result_area = net_area_mm2(&auto_support[0]);
        assert!(
            result_area < 1.0,
            "Blocker should win over enforcer: remaining area should be ~0, got {}",
            result_area
        );
    }

    #[test]
    fn box_volume_modifier_with_rotation() {
        let modifier = VolumeModifier {
            shape: VolumeShape::Box,
            role: OverrideRole::Enforcer,
            center: (50.0, 50.0, 5.0),
            size: (10.0, 4.0, 10.0), // 10x4 box
            rotation: 45.0,          // 45-degree rotation
        };

        let result = volume_modifier_at_z(&modifier, 5.0);
        assert!(result.is_some(), "Rotated box should produce cross-section");

        let poly = result.unwrap();
        let area = poly.area_mm2();
        // Area should still be 10x4 = 40 mm^2 (rotation preserves area).
        assert!(
            (area - 40.0).abs() < 2.0,
            "Rotated box area should be ~40 mm^2, got {}",
            area
        );
    }

    #[test]
    fn volume_modifier_enforcer_creates_support_from_nothing() {
        // No auto-support at this layer.
        let mut auto_support = vec![Vec::new()];

        // Volume modifier enforcer: a box at (50, 50, 0.1).
        let vm = VolumeModifier {
            shape: VolumeShape::Box,
            role: OverrideRole::Enforcer,
            center: (50.0, 50.0, 0.5),
            size: (10.0, 10.0, 1.0),
            rotation: 0.0,
        };

        let layer_heights = vec![(0.1, 0.2)];
        let _warnings = apply_overrides(&mut auto_support, &[], &[], &[vm], &layer_heights);

        assert!(
            !auto_support[0].is_empty(),
            "Volume modifier enforcer should create support from nothing"
        );

        let area = net_area_mm2(&auto_support[0]);
        assert!(
            area > 90.0,
            "Enforcer box should create ~100 mm^2 support, got {}",
            area
        );
    }

    #[test]
    fn volume_modifier_blocker_removes_support() {
        // Auto-support: 20x20 square.
        let auto_square = make_square(50.0, 50.0, 20.0);
        let mut auto_support = vec![vec![auto_square]];

        // Volume modifier blocker: cylinder at center of auto-support.
        let vm = VolumeModifier {
            shape: VolumeShape::Cylinder,
            role: OverrideRole::Blocker,
            center: (60.0, 60.0, 0.5),
            size: (5.0, 5.0, 1.0),
            rotation: 0.0,
        };

        let layer_heights = vec![(0.1, 0.2)];
        let _warnings = apply_overrides(&mut auto_support, &[], &[], &[vm], &layer_heights);

        let result_area = net_area_mm2(&auto_support[0]);
        let original_area = 400.0; // 20x20.
        assert!(
            result_area < original_area,
            "Volume modifier blocker should reduce support area from {}, got {}",
            original_area,
            result_area
        );
    }
}
