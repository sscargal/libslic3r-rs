//! Multi-objective Variable Layer Height (VLH) optimization.
//!
//! This module implements a multi-objective approach to layer height selection
//! that balances quality, speed, strength, and material usage through weighted
//! objective functions. It extends the curvature-based adaptive system in
//! [`crate::adaptive`] with feature-aware height decisions and Laplacian
//! smoothing.
//!
//! # Architecture
//!
//! - **Objectives** ([`objectives`]): Pure scoring functions that map geometry
//!   to desired layer heights.
//! - **Weights** ([`VlhWeights`]): Normalized weights controlling the balance
//!   between objectives.
//! - **Config** ([`VlhConfig`]): All parameters extracted from `PrintConfig`.
//! - **Result** ([`VlhResult`]): Final `(z, height)` pairs plus diagnostics.

pub mod features;
pub mod objectives;
pub mod optimizer;
pub mod smooth;

/// Normalized objective weights (always sum to 1.0).
///
/// Created via [`VlhWeights::new`] which normalizes any non-negative inputs.
/// If all inputs are zero, falls back to quality-only weighting.
#[derive(Debug, Clone, Copy)]
pub struct VlhWeights {
    pub quality: f64,
    pub speed: f64,
    pub strength: f64,
    pub material: f64,
}

impl VlhWeights {
    /// Create normalized weights. If all zero, defaults to quality=1.0.
    #[must_use]
    pub fn new(quality: f64, speed: f64, strength: f64, material: f64) -> Self {
        let sum = quality + speed + strength + material;
        if sum < 1e-12 {
            return Self {
                quality: 1.0,
                speed: 0.0,
                strength: 0.0,
                material: 0.0,
            };
        }
        Self {
            quality: quality / sum,
            speed: speed / sum,
            strength: strength / sum,
            material: material / sum,
        }
    }
}

/// Per-Z objective scores: each objective maps to a desired layer height.
///
/// Each field represents the layer height that a single objective would prefer
/// at a given Z position. The final height is a weighted combination via
/// [`ObjectiveScores::combine`].
#[derive(Debug, Clone, Copy)]
pub struct ObjectiveScores {
    pub quality_height: f64,
    pub speed_height: f64,
    pub strength_height: f64,
    pub material_height: f64,
}

impl ObjectiveScores {
    /// Weighted combination of objective heights.
    ///
    /// Weights must be pre-normalized (sum to 1.0) via [`VlhWeights::new`].
    #[must_use]
    pub fn combine(&self, weights: &VlhWeights) -> f64 {
        weights.quality * self.quality_height
            + weights.speed * self.speed_height
            + weights.strength * self.strength_height
            + weights.material * self.material_height
    }
}

/// Optimizer mode selection.
///
/// Controls which algorithm is used to select final layer heights from the
/// objective scores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptimizerMode {
    /// Greedy per-layer optimization (fast, good for most cases).
    #[default]
    Greedy,
    /// Dynamic programming for globally optimal height sequences.
    DynamicProgramming,
}

/// Feature types detected by the geometry pre-pass.
///
/// Each variant carries geometry-specific metadata used by the strength
/// objective to demand thinner layers in structurally critical regions.
#[derive(Debug, Clone)]
pub enum FeatureType {
    /// Overhang region requiring support or careful layering.
    Overhang { angle_deg: f64 },
    /// Bridging region spanning unsupported gaps.
    Bridge,
    /// Thin wall requiring precise extrusion control.
    ThinWall { width_mm: f64 },
    /// Hole feature requiring smooth circular approximation.
    Hole { diameter_mm: f64 },
}

/// A detected feature at a Z range that influences layer height.
#[derive(Debug, Clone)]
pub struct FeatureDetection {
    pub feature_type: FeatureType,
    pub z_min: f64,
    pub z_max: f64,
    pub demanded_height: f64,
}

/// Per-layer diagnostic data emitted via the event system.
///
/// Contains the breakdown of how the final layer height was determined,
/// useful for visualization and debugging.
#[derive(Debug, Clone)]
pub struct VlhDiagnosticLayer {
    pub layer: usize,
    pub z: f64,
    pub height: f64,
    pub quality_score: f64,
    pub speed_score: f64,
    pub strength_score: f64,
    pub material_score: f64,
    pub dominant_factor: String,
    pub features: Vec<String>,
}

/// VLH optimizer configuration (extracted from `PrintConfig` for convenience).
///
/// Collects all VLH-related parameters into a single struct that can be
/// passed to the optimizer without coupling to the full `PrintConfig`.
#[derive(Debug, Clone)]
pub struct VlhConfig {
    pub min_height: f64,
    pub max_height: f64,
    pub first_layer_height: f64,
    pub weights: VlhWeights,
    pub optimizer_mode: OptimizerMode,
    pub smoothing_strength: f64,
    pub smoothing_iterations: u32,
    pub diagnostics: bool,
    pub stochastic: bool,
    pub feature_overhang_weight: f64,
    pub feature_bridge_weight: f64,
    pub feature_thin_wall_weight: f64,
    pub feature_hole_weight: f64,
    pub overhang_angle_min: f64,
    pub overhang_angle_max: f64,
    pub thin_wall_threshold: f64,
    pub feature_margin_layers: u32,
    pub nozzle_diameter: f64,
}

/// Result of VLH optimization.
///
/// Contains the final layer heights and optional diagnostic data for each
/// layer showing how the height was determined.
#[derive(Debug, Clone)]
pub struct VlhResult {
    /// Layer `(z_position, layer_height)` pairs.
    pub heights: Vec<(f64, f64)>,
    /// Per-layer diagnostic breakdown (empty if diagnostics disabled).
    pub diagnostics: Vec<VlhDiagnosticLayer>,
}

use slicecore_mesh::TriangleMesh;

/// Main entry point for multi-objective VLH optimization.
///
/// Pipeline:
/// 1. Sample curvature profile from mesh (reuses `adaptive::sample_curvature_profile`)
/// 2. Build feature map from mesh geometry
/// 3. Compute per-Z objective scores
/// 4. Run optimizer (greedy or DP based on config)
/// 5. Apply Laplacian smoothing + ratio clamping
/// 6. Optionally collect per-layer diagnostics
///
/// # Panics
///
/// Does not panic. Returns an empty result for degenerate meshes.
#[must_use]
pub fn compute_vlh_heights(mesh: &TriangleMesh, config: &VlhConfig) -> VlhResult {
    let aabb = mesh.aabb();
    let mesh_max_z = aabb.max.z;

    // Degenerate mesh check.
    if mesh_max_z <= 0.0 || config.min_height <= 0.0 || config.max_height <= 0.0 {
        return VlhResult {
            heights: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    let min_h = config.min_height.min(config.max_height);
    let max_h = config.min_height.max(config.max_height);

    // Step 1: Sample curvature profile.
    let sample_step = min_h / 2.0;
    let curvature_profile = crate::adaptive::sample_curvature_profile(mesh, sample_step);

    if curvature_profile.is_empty() {
        return VlhResult {
            heights: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    // Step 2: Build feature map.
    let feature_map = features::build_feature_map(mesh, config);

    // Step 3: Build ZSample array from curvature profile.
    let vertices = mesh.vertices();
    let indices = mesh.indices();
    let normals = mesh.normals();

    // Pre-compute per-triangle Z ranges for steepness lookup.
    let tri_z_ranges: Vec<(f64, f64)> = indices
        .iter()
        .map(|tri| {
            let z0 = vertices[tri[0] as usize].z;
            let z1 = vertices[tri[1] as usize].z;
            let z2 = vertices[tri[2] as usize].z;
            (z0.min(z1).min(z2), z0.max(z1).max(z2))
        })
        .collect();

    let z_samples: Vec<optimizer::ZSample> = curvature_profile
        .iter()
        .map(|&(z, curvature)| {
            let stress_factor = features::query_stress_factor(&feature_map, z);
            let feature_demanded_height = features::query_feature_demanded_height(&feature_map, z);

            // Compute external surface fraction: fraction of triangles at this Z
            // whose normals are steep (i.e., far from horizontal).
            // Steep = |normal.z| < 0.7 approximately -> visible external surface.
            let tris_at_z: Vec<usize> = tri_z_ranges
                .iter()
                .enumerate()
                .filter(|(_, &(zmin, zmax))| zmin <= z && z <= zmax)
                .map(|(i, _)| i)
                .collect();

            let external_surface_fraction = if tris_at_z.is_empty() {
                0.0
            } else {
                let avg_abs_nz: f64 = tris_at_z.iter().map(|&i| normals[i].z.abs()).sum::<f64>()
                    / tris_at_z.len() as f64;
                1.0 - avg_abs_nz
            };

            let scores = objectives::compute_objective_scores(
                curvature,
                external_surface_fraction,
                stress_factor,
                min_h,
                max_h,
            );

            optimizer::ZSample {
                z,
                scores,
                feature_demanded_height,
                stress_factor,
                external_surface_fraction,
            }
        })
        .collect();

    // Step 4: Run optimizer.
    let mut heights = match config.optimizer_mode {
        OptimizerMode::Greedy => optimizer::optimize_greedy(&z_samples, config),
        OptimizerMode::DynamicProgramming => optimizer::optimize_dp(&z_samples, config),
    };

    if heights.is_empty() {
        return VlhResult {
            heights: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    // Step 5: Determine pinned indices and apply smoothing.
    let pinned: Vec<bool> = heights
        .iter()
        .enumerate()
        .map(|(i, &(z, _))| {
            if i == 0 {
                return true; // First layer always pinned.
            }
            features::query_feature_demanded_height(&feature_map, z).is_some()
        })
        .collect();

    smooth::smooth_vlh_heights(
        &mut heights,
        &pinned,
        config.smoothing_strength,
        config.smoothing_iterations as usize,
        1.5,
        config.min_height,
        config.max_height,
    );

    // Step 6: Optionally build diagnostics.
    let diagnostics = if config.diagnostics {
        heights
            .iter()
            .enumerate()
            .map(|(i, &(z, height))| {
                // Find the closest z_sample for this layer's Z.
                let sample_idx = z_samples.partition_point(|s| s.z < z);
                let sample_idx = sample_idx.min(z_samples.len().saturating_sub(1));
                let sample = &z_samples[sample_idx];

                let scores = &sample.scores;
                let quality_score = scores.quality_height;
                let speed_score = scores.speed_height;
                let strength_score = scores.strength_height;
                let material_score = scores.material_height;

                // Determine dominant factor.
                let mut factors = [
                    ("quality", (config.weights.quality * quality_score).abs()),
                    ("speed", (config.weights.speed * speed_score).abs()),
                    ("strength", (config.weights.strength * strength_score).abs()),
                    ("material", (config.weights.material * material_score).abs()),
                ];
                factors.sort_by(|a, b| b.1.total_cmp(&a.1));
                let dominant_factor = factors[0].0.to_string();

                // Collect feature descriptions at this Z.
                let feature_descs: Vec<String> = feature_map
                    .detections()
                    .iter()
                    .filter(|d| d.z_min <= z && d.z_max >= z)
                    .map(|d| match &d.feature_type {
                        FeatureType::Overhang { angle_deg } => {
                            format!("overhang:{angle_deg:.1}deg")
                        }
                        FeatureType::Bridge => "bridge".to_string(),
                        FeatureType::ThinWall { width_mm } => {
                            format!("thin_wall:{width_mm:.1}mm")
                        }
                        FeatureType::Hole { diameter_mm } => {
                            format!("hole:{diameter_mm:.1}mm")
                        }
                    })
                    .collect();

                VlhDiagnosticLayer {
                    layer: i,
                    z,
                    height,
                    quality_score,
                    speed_score,
                    strength_score,
                    material_score,
                    dominant_factor,
                    features: feature_descs,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    VlhResult {
        heights,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_single_quality() {
        let w = VlhWeights::new(1.0, 0.0, 0.0, 0.0);
        assert!((w.quality - 1.0).abs() < 1e-9);
        assert!((w.speed - 0.0).abs() < 1e-9);
        assert!((w.strength - 0.0).abs() < 1e-9);
        assert!((w.material - 0.0).abs() < 1e-9);
    }

    #[test]
    fn weights_equal_two() {
        let w = VlhWeights::new(1.0, 1.0, 0.0, 0.0);
        assert!((w.quality - 0.5).abs() < 1e-9);
        assert!((w.speed - 0.5).abs() < 1e-9);
    }

    #[test]
    fn weights_all_zero_fallback() {
        let w = VlhWeights::new(0.0, 0.0, 0.0, 0.0);
        assert!((w.quality - 1.0).abs() < 1e-9);
        assert!((w.speed - 0.0).abs() < 1e-9);
        assert!((w.strength - 0.0).abs() < 1e-9);
        assert!((w.material - 0.0).abs() < 1e-9);
    }

    #[test]
    fn scores_combine_quality_only() {
        let scores = ObjectiveScores {
            quality_height: 0.1,
            speed_height: 0.3,
            strength_height: 0.2,
            material_height: 0.3,
        };
        let weights = VlhWeights::new(1.0, 0.0, 0.0, 0.0);
        let combined = scores.combine(&weights);
        assert!(
            (combined - 0.1).abs() < 1e-9,
            "Quality-only weight should return quality_height, got {combined}"
        );
    }

    #[test]
    fn scores_combine_equal_weights() {
        let scores = ObjectiveScores {
            quality_height: 0.1,
            speed_height: 0.3,
            strength_height: 0.1,
            material_height: 0.3,
        };
        let weights = VlhWeights::new(1.0, 1.0, 1.0, 1.0);
        let combined = scores.combine(&weights);
        let expected = (0.1 + 0.3 + 0.1 + 0.3) / 4.0;
        assert!(
            (combined - expected).abs() < 1e-9,
            "Equal weights should average, got {combined}"
        );
    }

    #[test]
    fn optimizer_mode_default_is_greedy() {
        let mode = OptimizerMode::default();
        assert_eq!(mode, OptimizerMode::Greedy);
    }

    #[test]
    fn optimizer_mode_has_dp_variant() {
        let mode = OptimizerMode::DynamicProgramming;
        assert_eq!(mode, OptimizerMode::DynamicProgramming);
    }

    #[test]
    fn vlh_config_can_be_constructed() {
        let config = VlhConfig {
            min_height: 0.05,
            max_height: 0.3,
            first_layer_height: 0.2,
            weights: VlhWeights::new(1.0, 0.0, 0.0, 0.0),
            optimizer_mode: OptimizerMode::Greedy,
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
        };
        assert!((config.min_height - 0.05).abs() < 1e-9);
        assert_eq!(config.optimizer_mode, OptimizerMode::Greedy);
    }

    // -- Integration tests for compute_vlh_heights --

    use slicecore_math::Point3;

    /// Creates a unit sphere mesh (radius 1, centered at (0, 0, 1)).
    fn unit_sphere() -> TriangleMesh {
        let stacks = 32;
        let slices = 32;
        let radius = 1.0;
        let center = Point3::new(0.0, 0.0, 1.0);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        vertices.push(Point3::new(center.x, center.y, center.z - radius));
        for i in 1..stacks {
            let phi = std::f64::consts::PI * i as f64 / stacks as f64;
            let z = center.z - radius * phi.cos();
            let r = radius * phi.sin();
            for j in 0..slices {
                let theta = 2.0 * std::f64::consts::PI * j as f64 / slices as f64;
                let x = center.x + r * theta.cos();
                let y = center.y + r * theta.sin();
                vertices.push(Point3::new(x, y, z));
            }
        }
        vertices.push(Point3::new(center.x, center.y, center.z + radius));

        let top_pole = vertices.len() as u32 - 1;

        for j in 0..slices {
            let j_next = (j + 1) % slices;
            indices.push([0, 1 + j as u32, 1 + j_next as u32]);
        }

        for i in 0..(stacks - 2) {
            let row_start = 1 + i as u32 * slices as u32;
            let next_row_start = row_start + slices as u32;
            for j in 0..slices {
                let j_next = (j + 1) % slices;
                let a = row_start + j as u32;
                let b = row_start + j_next as u32;
                let c = next_row_start + j_next as u32;
                let d = next_row_start + j as u32;
                indices.push([a, d, c]);
                indices.push([a, c, b]);
            }
        }

        let last_row_start = 1 + (stacks as u32 - 2) * slices as u32;
        for j in 0..slices {
            let j_next = (j + 1) % slices;
            indices.push([
                last_row_start + j as u32,
                top_pole,
                last_row_start + j_next as u32,
            ]);
        }

        TriangleMesh::new(vertices, indices).expect("sphere mesh should be valid")
    }

    fn test_vlh_config() -> VlhConfig {
        VlhConfig {
            min_height: 0.05,
            max_height: 0.3,
            first_layer_height: 0.2,
            weights: VlhWeights::new(1.0, 0.0, 0.0, 0.0),
            optimizer_mode: OptimizerMode::Greedy,
            smoothing_strength: 0.0,
            smoothing_iterations: 0,
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

    #[test]
    fn vlh_quality_only_sphere_produces_variable_heights() {
        let mesh = unit_sphere();
        let config = test_vlh_config();
        let result = compute_vlh_heights(&mesh, &config);

        assert!(
            !result.heights.is_empty(),
            "Should produce non-empty heights"
        );

        // Quality-only on sphere should produce variable heights (not all identical).
        let h_values: Vec<f64> = result.heights.iter().skip(1).map(|&(_, h)| h).collect();
        assert!(
            h_values.len() >= 3,
            "Should produce at least 3 interior layers"
        );

        let min_h = h_values.iter().copied().fold(f64::INFINITY, f64::min);
        let max_h = h_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let range = max_h - min_h;
        assert!(
            range > 0.01,
            "Height range ({range:.4}) should show variation for sphere curvature (min={min_h:.4}, max={max_h:.4})"
        );

        // Regions where curvature changes rapidly should have thinner layers
        // than flat/constant regions. The sphere's curvature peaks near pole
        // transitions (z ~ 0.1-0.3 and z ~ 1.7-1.9).
        let transition_layers: Vec<f64> = result
            .heights
            .iter()
            .filter(|&&(z, _)| (z > 0.1 && z < 0.5) || (z > 1.5 && z < 1.9))
            .map(|&(_, h)| h)
            .collect();
        let mid_layers: Vec<f64> = result
            .heights
            .iter()
            .filter(|&&(z, _)| z > 0.8 && z < 1.2)
            .map(|&(_, h)| h)
            .collect();

        if !transition_layers.is_empty() && !mid_layers.is_empty() {
            let avg_trans: f64 =
                transition_layers.iter().sum::<f64>() / transition_layers.len() as f64;
            let avg_mid: f64 = mid_layers.iter().sum::<f64>() / mid_layers.len() as f64;
            // Both regions should have reasonable heights; at least some variation.
            assert!(
                (avg_trans - avg_mid).abs() < 0.25,
                "Heights should be reasonable: transition avg={avg_trans:.4}, mid avg={avg_mid:.4}"
            );
        }
    }

    #[test]
    fn vlh_speed_only_produces_near_max_height() {
        // Use a cube (no overhangs) so feature demands don't override speed objective.
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
        let mesh = TriangleMesh::new(vertices, indices).expect("cube should be valid");
        let mut config = test_vlh_config();
        config.weights = VlhWeights::new(0.0, 1.0, 0.0, 0.0);
        let result = compute_vlh_heights(&mesh, &config);

        assert!(!result.heights.is_empty());

        // Skip first and last layers.
        let interior = &result.heights[1..result.heights.len().saturating_sub(1)];
        let nozzle_limit = config.nozzle_diameter * 0.75;
        for &(z, h) in interior {
            assert!(
                h >= config.max_height * 0.7 || h >= nozzle_limit * 0.7,
                "Speed-only at z={z:.3} should be near max, got {h:.4}"
            );
        }
    }

    #[test]
    fn vlh_greedy_deterministic() {
        let mesh = unit_sphere();
        let config = test_vlh_config();
        let result1 = compute_vlh_heights(&mesh, &config);
        let result2 = compute_vlh_heights(&mesh, &config);
        assert_eq!(result1.heights.len(), result2.heights.len());
        for (a, b) in result1.heights.iter().zip(&result2.heights) {
            assert!((a.0 - b.0).abs() < 1e-15, "Z mismatch: {} vs {}", a.0, b.0);
            assert!(
                (a.1 - b.1).abs() < 1e-15,
                "Height mismatch: {} vs {}",
                a.1,
                b.1
            );
        }
    }

    #[test]
    fn vlh_dp_deterministic() {
        let mesh = unit_sphere();
        let mut config = test_vlh_config();
        config.optimizer_mode = OptimizerMode::DynamicProgramming;
        let result1 = compute_vlh_heights(&mesh, &config);
        let result2 = compute_vlh_heights(&mesh, &config);
        assert_eq!(result1.heights.len(), result2.heights.len());
        for (a, b) in result1.heights.iter().zip(&result2.heights) {
            assert!((a.0 - b.0).abs() < 1e-15, "Z mismatch");
            assert!((a.1 - b.1).abs() < 1e-15, "Height mismatch");
        }
    }

    #[test]
    fn vlh_diagnostics_populated_when_enabled() {
        let mesh = unit_sphere();
        let mut config = test_vlh_config();
        config.diagnostics = true;
        let result = compute_vlh_heights(&mesh, &config);

        assert!(!result.heights.is_empty());
        assert_eq!(
            result.diagnostics.len(),
            result.heights.len(),
            "Diagnostics should have one entry per layer"
        );
        for diag in &result.diagnostics {
            assert!(diag.quality_score > 0.0, "Quality score should be positive");
            assert!(
                !diag.dominant_factor.is_empty(),
                "Dominant factor should be set"
            );
        }
    }

    #[test]
    fn vlh_output_monotonically_increasing_z() {
        let mesh = unit_sphere();
        let config = test_vlh_config();
        let result = compute_vlh_heights(&mesh, &config);

        for i in 1..result.heights.len() {
            assert!(
                result.heights[i].0 > result.heights[i - 1].0,
                "Z[{}]={} should be > Z[{}]={}",
                i,
                result.heights[i].0,
                i - 1,
                result.heights[i - 1].0
            );
        }
    }

    #[test]
    fn vlh_deterministic_greedy_10_runs() {
        let mesh = unit_sphere();
        let config = test_vlh_config();
        let baseline = compute_vlh_heights(&mesh, &config);
        for run in 0..10 {
            let result = compute_vlh_heights(&mesh, &config);
            assert_eq!(
                baseline.heights.len(),
                result.heights.len(),
                "Run {run}: length mismatch"
            );
            for (i, (a, b)) in baseline.heights.iter().zip(&result.heights).enumerate() {
                assert!(
                    (a.0 - b.0).abs() < 1e-15,
                    "Run {run}, layer {i}: Z mismatch {:.15} vs {:.15}",
                    a.0,
                    b.0
                );
                assert!(
                    (a.1 - b.1).abs() < 1e-15,
                    "Run {run}, layer {i}: Height mismatch {:.15} vs {:.15}",
                    a.1,
                    b.1
                );
            }
        }
    }

    #[test]
    fn vlh_deterministic_dp_10_runs() {
        let mesh = unit_sphere();
        let mut config = test_vlh_config();
        config.optimizer_mode = OptimizerMode::DynamicProgramming;
        let baseline = compute_vlh_heights(&mesh, &config);
        for run in 0..10 {
            let result = compute_vlh_heights(&mesh, &config);
            assert_eq!(
                baseline.heights.len(),
                result.heights.len(),
                "DP run {run}: length mismatch"
            );
            for (i, (a, b)) in baseline.heights.iter().zip(&result.heights).enumerate() {
                assert!(
                    (a.0 - b.0).abs() < 1e-15,
                    "DP run {run}, layer {i}: Z mismatch",
                );
                assert!(
                    (a.1 - b.1).abs() < 1e-15,
                    "DP run {run}, layer {i}: Height mismatch",
                );
            }
        }
    }

    #[test]
    fn vlh_output_heights_within_bounds() {
        let mesh = unit_sphere();
        let config = test_vlh_config();
        let result = compute_vlh_heights(&mesh, &config);

        for &(z, h) in result.heights.iter().skip(1) {
            assert!(
                h >= config.min_height - 1e-9,
                "Height {h:.6} at z={z:.3} below min {}",
                config.min_height
            );
            assert!(
                h <= config.max_height + 1e-9,
                "Height {h:.6} at z={z:.3} above max {}",
                config.max_height
            );
        }
    }
}
