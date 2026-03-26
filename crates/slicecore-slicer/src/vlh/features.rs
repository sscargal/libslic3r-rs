//! Feature map pre-pass for VLH optimization.
//!
//! Detects geometric features (overhangs, bridges, thin walls, holes) from mesh
//! geometry and builds a per-Z influence map. Each feature carries a demanded
//! layer height reflecting its structural requirements.
//!
//! # Architecture
//!
//! The feature map is built once from mesh triangle normals, then queried
//! efficiently via binary search during layer height optimization.
//!
//! Currently implements overhang detection from mesh normals. Hole, thin-wall,
//! and bridge detection require sliced contour data and will be integrated in
//! Plan 04.

use super::{FeatureDetection, FeatureType, VlhConfig};
use slicecore_mesh::TriangleMesh;

/// Sorted collection of feature detections with efficient Z-based lookup.
///
/// Detections are sorted by `z_min` for binary-search queries. Multiple
/// features may overlap at the same Z; query functions resolve conflicts
/// using most-demanding-wins semantics.
#[derive(Debug, Clone)]
pub struct FeatureMap {
    detections: Vec<FeatureDetection>,
}

impl FeatureMap {
    /// Returns a reference to the underlying detections (for testing/diagnostics).
    #[must_use]
    pub fn detections(&self) -> &[FeatureDetection] {
        &self.detections
    }

    /// Returns the number of detected features.
    #[must_use]
    pub fn len(&self) -> usize {
        self.detections.len()
    }

    /// Returns true if no features were detected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.detections.is_empty()
    }
}

/// Build a feature map from mesh geometry.
///
/// Scans triangle normals to detect overhang regions and records them as
/// [`FeatureDetection`] entries. Hole, thin-wall, and bridge detection
/// require sliced contour data and are deferred to Plan 04 integration.
///
/// # Arguments
///
/// * `mesh` - Triangle mesh to analyze
/// * `config` - VLH configuration with angle thresholds and weights
///
/// # Returns
///
/// A [`FeatureMap`] with detections sorted by `z_min` for efficient lookup.
#[must_use]
pub fn build_feature_map(mesh: &TriangleMesh, config: &VlhConfig) -> FeatureMap {
    let mut detections = Vec::new();

    // Overhang detection from triangle normals.
    detect_overhangs(mesh, config, &mut detections);

    // TODO: Hole detection requires sliced contour data (Plan 04).
    // TODO: Thin wall detection requires sliced contour data (Plan 04).
    // TODO: Bridge detection requires sliced contour data (Plan 04).

    // Apply feature margin extension.
    let margin_mm = config.feature_margin_layers as f64 * config.min_height;
    for d in &mut detections {
        d.z_min -= margin_mm;
        d.z_max += margin_mm;
    }

    // Sort by z_min using total_cmp for determinism (NaN-safe).
    detections.sort_by(|a, b| a.z_min.total_cmp(&b.z_min));

    FeatureMap { detections }
}

/// Detect overhang features from mesh triangle normals.
///
/// For each triangle, computes the surface angle from horizontal. Triangles
/// with angles in `[overhang_angle_min, overhang_angle_max]` are recorded
/// as overhang features. The demanded height is scaled by angle sensitivity
/// and the overhang weight.
fn detect_overhangs(
    mesh: &TriangleMesh,
    config: &VlhConfig,
    detections: &mut Vec<FeatureDetection>,
) {
    let vertices = mesh.vertices();
    let indices = mesh.indices();
    let normals = mesh.normals();

    let angle_min_rad = config.overhang_angle_min.to_radians();
    let angle_max_rad = config.overhang_angle_max.to_radians();

    for (tri_idx, tri) in indices.iter().enumerate() {
        let normal = &normals[tri_idx];

        // Surface angle from horizontal = acos(|normal.z|).
        // A horizontal surface has angle = 0 deg, vertical = 90 deg.
        let abs_nz = normal.z.abs().clamp(0.0, 1.0);
        let angle_rad = abs_nz.acos();

        // Only consider triangles in the overhang angle range.
        if angle_rad < angle_min_rad || angle_rad > angle_max_rad {
            continue;
        }

        // Skip if overhang weight is zero.
        if config.feature_overhang_weight < 1e-12 {
            continue;
        }

        let angle_deg = angle_rad.to_degrees();

        // Compute sensitivity: 0.0 at angle_min, 1.0 at angle_max.
        let angle_range = config.overhang_angle_max - config.overhang_angle_min;
        let sensitivity = if angle_range > 1e-12 {
            ((angle_deg - config.overhang_angle_min) / angle_range).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Demanded height: thinner layers for more severe overhangs.
        let demanded_height =
            config.min_height + (config.max_height - config.min_height) * (1.0 - sensitivity);
        let demanded_height = demanded_height * config.feature_overhang_weight;
        // Clamp to valid range.
        let demanded_height = demanded_height.clamp(config.min_height, config.max_height);

        // Triangle Z range.
        let z0 = vertices[tri[0] as usize].z;
        let z1 = vertices[tri[1] as usize].z;
        let z2 = vertices[tri[2] as usize].z;
        let z_min = z0.min(z1).min(z2);
        let z_max = z0.max(z1).max(z2);

        detections.push(FeatureDetection {
            feature_type: FeatureType::Overhang { angle_deg },
            z_min,
            z_max,
            demanded_height,
        });
    }
}

/// Query the aggregate stress factor at a given Z position.
///
/// Returns the maximum stress contribution from all features overlapping
/// the given Z, clamped to `[0.0, 1.0]`. Returns `0.0` if no features
/// overlap.
///
/// Stress factor is computed as `1.0 - (demanded_height - min) / (max - min)`
/// where min/max are the global height bounds. Higher stress = more demanding.
#[must_use]
pub fn query_stress_factor(feature_map: &FeatureMap, z: f64) -> f64 {
    let mut max_stress = 0.0_f64;

    for d in overlapping_features(feature_map, z) {
        // Stress is computed per feature type. Higher stress = more demanding.
        let stress = match &d.feature_type {
            FeatureType::Overhang { angle_deg } => {
                // Stress scales with angle severity.
                (*angle_deg / 90.0).clamp(0.0, 1.0)
            }
            FeatureType::Bridge => 1.0,
            FeatureType::ThinWall { width_mm } => (1.0 - width_mm / 2.0).clamp(0.0, 1.0),
            FeatureType::Hole { diameter_mm } => (1.0 - diameter_mm / 10.0).clamp(0.0, 1.0),
        };

        max_stress = max_stress.max(stress);
    }

    max_stress.clamp(0.0, 1.0)
}

/// Query the demanded layer height at a given Z position.
///
/// Returns the minimum demanded height across all features overlapping
/// the given Z (most-demanding-wins). Returns `None` if no features
/// overlap.
#[must_use]
pub fn query_feature_demanded_height(feature_map: &FeatureMap, z: f64) -> Option<f64> {
    let mut min_demanded: Option<f64> = None;

    for d in overlapping_features(feature_map, z) {
        min_demanded = Some(match min_demanded {
            Some(current) => current.min(d.demanded_height),
            None => d.demanded_height,
        });
    }

    min_demanded
}

/// Find all features whose Z range overlaps the given Z value.
///
/// Uses binary search on the sorted `z_min` values to find a starting
/// point, then scans forward through potentially overlapping detections.
fn overlapping_features(feature_map: &FeatureMap, z: f64) -> Vec<&FeatureDetection> {
    let detections = &feature_map.detections;
    if detections.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();

    // Binary search: find the first detection whose z_min > z.
    // All detections before that point *might* overlap z (if their z_max >= z).
    let upper = detections.partition_point(|d| d.z_min <= z);

    // Check all detections from the start up to `upper` whose z_max >= z.
    for d in &detections[..upper] {
        if d.z_max >= z {
            result.push(d);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::super::VlhWeights;
    use super::*;
    use slicecore_math::Point3;

    /// Helper: create a VlhConfig with sensible defaults for testing.
    fn test_config() -> VlhConfig {
        VlhConfig {
            min_height: 0.05,
            max_height: 0.3,
            first_layer_height: 0.2,
            weights: VlhWeights::new(1.0, 0.0, 0.0, 0.0),
            optimizer_mode: super::super::OptimizerMode::Greedy,
            smoothing_strength: 0.5,
            smoothing_iterations: 3,
            diagnostics: false,
            stochastic: false,
            feature_overhang_weight: 1.0,
            feature_bridge_weight: 1.0,
            feature_thin_wall_weight: 1.0,
            feature_hole_weight: 1.0,
            overhang_angle_min: 40.0,
            overhang_angle_max: 60.0,
            thin_wall_threshold: 0.8,
            feature_margin_layers: 2,
            nozzle_diameter: 0.4,
        }
    }

    /// Creates a mesh with steep overhangs (45-degree angled surfaces).
    /// A truncated cone shape: wider at top, narrower at bottom.
    fn overhang_mesh() -> TriangleMesh {
        let half = 0.5_f64;
        // top_half chosen so side face normal angle from horizontal is ~50 degrees
        // (within the default [40, 60] overhang range).
        let top_half = 1.34_f64;
        let h = 1.0_f64;
        let vertices = vec![
            // Bottom square (z=0)
            Point3::new(-half, -half, 0.0),
            Point3::new(half, -half, 0.0),
            Point3::new(half, half, 0.0),
            Point3::new(-half, half, 0.0),
            // Top square (z=h), wider
            Point3::new(-top_half, -top_half, h),
            Point3::new(top_half, -top_half, h),
            Point3::new(top_half, top_half, h),
            Point3::new(-top_half, top_half, h),
        ];
        let indices = vec![
            // Bottom face
            [0, 2, 1],
            [0, 3, 2],
            // Top face
            [4, 5, 6],
            [4, 6, 7],
            // Side faces (angled ~45 degrees)
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        TriangleMesh::new(vertices, indices).expect("overhang mesh should be valid")
    }

    /// Creates a simple vertical box (no overhangs).
    fn vertical_box() -> TriangleMesh {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 2.0),
            Point3::new(1.0, 0.0, 2.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(0.0, 1.0, 2.0),
        ];
        let indices = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        TriangleMesh::new(vertices, indices).expect("vertical box should be valid")
    }

    #[test]
    fn build_feature_map_detects_overhangs() {
        let mesh = overhang_mesh();
        let config = test_config();
        let fmap = build_feature_map(&mesh, &config);
        let has_overhang = fmap
            .detections
            .iter()
            .any(|d| matches!(d.feature_type, FeatureType::Overhang { .. }));
        assert!(
            has_overhang,
            "Should detect overhang features on angled mesh"
        );
    }

    #[test]
    fn feature_margin_extends_detection_zone() {
        let mesh = overhang_mesh();
        let mut config = test_config();
        config.feature_margin_layers = 3;
        let fmap = build_feature_map(&mesh, &config);
        let margin_mm = config.feature_margin_layers as f64 * config.min_height;
        for d in &fmap.detections {
            assert!(
                d.z_max >= d.z_min,
                "z_max ({}) should be >= z_min ({})",
                d.z_max,
                d.z_min,
            );
        }
        if !fmap.detections.is_empty() {
            let min_z = fmap
                .detections
                .iter()
                .map(|d| d.z_min)
                .fold(f64::INFINITY, f64::min);
            let max_z = fmap
                .detections
                .iter()
                .map(|d| d.z_max)
                .fold(f64::NEG_INFINITY, f64::max);
            let span = max_z - min_z;
            assert!(
                span > 0.5,
                "Feature span ({span}) should be extended by margin ({margin_mm})"
            );
        }
    }

    #[test]
    fn query_stress_factor_no_features_returns_zero() {
        let mesh = vertical_box();
        let mut config = test_config();
        config.overhang_angle_min = 40.0;
        config.overhang_angle_max = 60.0;
        let fmap = build_feature_map(&mesh, &config);
        let stress = query_stress_factor(&fmap, 1.0);
        assert!(
            (stress - 0.0).abs() < 1e-9,
            "Stress factor should be 0.0 for vertical box, got {stress}"
        );
    }

    #[test]
    fn query_stress_factor_overhang_returns_positive() {
        let mesh = overhang_mesh();
        let config = test_config();
        let fmap = build_feature_map(&mesh, &config);
        let stress = query_stress_factor(&fmap, 0.5);
        assert!(
            stress > 0.0,
            "Stress factor should be > 0.0 at overhang region, got {stress}"
        );
    }

    #[test]
    fn query_feature_demanded_height_no_features_returns_none() {
        let mesh = vertical_box();
        let mut config = test_config();
        config.overhang_angle_min = 40.0;
        config.overhang_angle_max = 60.0;
        let fmap = build_feature_map(&mesh, &config);
        let demanded = query_feature_demanded_height(&fmap, 1.0);
        assert!(
            demanded.is_none(),
            "Should return None for no features, got {demanded:?}"
        );
    }

    #[test]
    fn query_feature_demanded_height_hole_returns_min_height() {
        let fmap = FeatureMap {
            detections: vec![FeatureDetection {
                feature_type: FeatureType::Hole { diameter_mm: 3.0 },
                z_min: 0.5,
                z_max: 1.5,
                demanded_height: 0.05,
            }],
        };
        let demanded = query_feature_demanded_height(&fmap, 1.0);
        assert_eq!(
            demanded,
            Some(0.05),
            "Hole feature should demand min_height"
        );
    }

    #[test]
    fn overlapping_features_most_demanding_wins() {
        let fmap = FeatureMap {
            detections: vec![
                FeatureDetection {
                    feature_type: FeatureType::Overhang { angle_deg: 45.0 },
                    z_min: 0.0,
                    z_max: 2.0,
                    demanded_height: 0.15,
                },
                FeatureDetection {
                    feature_type: FeatureType::Hole { diameter_mm: 3.0 },
                    z_min: 0.5,
                    z_max: 1.5,
                    demanded_height: 0.05,
                },
            ],
        };
        let demanded = query_feature_demanded_height(&fmap, 1.0);
        assert_eq!(
            demanded,
            Some(0.05),
            "Most demanding (thinnest) feature should win"
        );
    }

    #[test]
    fn feature_map_sorted_by_z_min() {
        let mesh = overhang_mesh();
        let config = test_config();
        let fmap = build_feature_map(&mesh, &config);
        for i in 1..fmap.detections.len() {
            assert!(
                fmap.detections[i].z_min >= fmap.detections[i - 1].z_min,
                "Detections should be sorted by z_min: {} >= {}",
                fmap.detections[i].z_min,
                fmap.detections[i - 1].z_min,
            );
        }
    }

    #[test]
    fn feature_detection_is_deterministic() {
        let mesh = overhang_mesh();
        let config = test_config();
        let first = build_feature_map(&mesh, &config);
        for _ in 0..10 {
            let other = build_feature_map(&mesh, &config);
            assert_eq!(
                first.detections.len(),
                other.detections.len(),
                "Feature map should be deterministic"
            );
            for (a, b) in first.detections.iter().zip(other.detections.iter()) {
                assert!(
                    (a.z_min - b.z_min).abs() < 1e-12,
                    "z_min should be identical across runs"
                );
                assert!(
                    (a.demanded_height - b.demanded_height).abs() < 1e-12,
                    "demanded_height should be identical across runs"
                );
            }
        }
    }
}
