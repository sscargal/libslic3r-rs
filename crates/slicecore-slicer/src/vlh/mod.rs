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

pub mod objectives;

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
}
