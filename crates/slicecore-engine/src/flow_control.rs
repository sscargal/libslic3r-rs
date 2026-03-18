//! Per-feature flow multiplier control.
//!
//! Allows fine-tuning extrusion flow rate independently for each feature type
//! (perimeters, infill, support, etc.). Each multiplier defaults to 1.0 (100%)
//! and is applied to E-values during G-code generation.
//!
//! # Example
//!
//! ```ignore
//! let mut flow = PerFeatureFlow::default();
//! flow.outer_perimeter = 0.95; // 5% less flow on outer walls
//! flow.bridge = 1.1;           // 10% more flow on bridges
//! let multiplier = flow.get_multiplier(FeatureType::OuterPerimeter);
//! assert!((multiplier - 0.95).abs() < 1e-9);
//! ```

use serde::{Deserialize, Serialize};
use slicecore_config_derive::SettingSchema;

use crate::toolpath::FeatureType;

/// Per-feature flow multiplier configuration.
///
/// Each field corresponds to a [`FeatureType`] and defaults to 1.0 (no change).
/// Values < 1.0 reduce flow; values > 1.0 increase flow.
///
/// Serialized as a TOML section `[per_feature_flow]` within [`PrintConfig`](crate::config::PrintConfig).
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
#[setting(category = "Advanced")]
pub struct PerFeatureFlow {
    /// Flow multiplier for outer (visible) perimeter walls.
    #[setting(
        tier = 3,
        description = "Flow multiplier for outer perimeter walls",
        min = 0.0,
        max = 2.0
    )]
    pub outer_perimeter: f64,
    /// Flow multiplier for inner perimeter walls.
    #[setting(
        tier = 3,
        description = "Flow multiplier for inner perimeter walls",
        min = 0.0,
        max = 2.0
    )]
    pub inner_perimeter: f64,
    /// Flow multiplier for solid infill (top/bottom surfaces).
    #[setting(
        tier = 3,
        description = "Flow multiplier for solid infill",
        min = 0.0,
        max = 2.0
    )]
    pub solid_infill: f64,
    /// Flow multiplier for sparse infill.
    #[setting(
        tier = 3,
        description = "Flow multiplier for sparse infill",
        min = 0.0,
        max = 2.0
    )]
    pub sparse_infill: f64,
    /// Flow multiplier for support structures.
    #[setting(
        tier = 3,
        description = "Flow multiplier for support structures",
        min = 0.0,
        max = 2.0
    )]
    pub support: f64,
    /// Flow multiplier for support interface layers.
    #[setting(
        tier = 3,
        description = "Flow multiplier for support interface layers",
        min = 0.0,
        max = 2.0
    )]
    pub support_interface: f64,
    /// Flow multiplier for bridge extrusions.
    #[setting(
        tier = 3,
        description = "Flow multiplier for bridge extrusions",
        min = 0.0,
        max = 2.0
    )]
    pub bridge: f64,
    /// Flow multiplier for gap fill extrusions.
    #[setting(
        tier = 3,
        description = "Flow multiplier for gap fill extrusions",
        min = 0.0,
        max = 2.0
    )]
    pub gap_fill: f64,
    /// Flow multiplier for skirt outlines.
    #[setting(
        tier = 3,
        description = "Flow multiplier for skirt outlines",
        min = 0.0,
        max = 2.0
    )]
    pub skirt: f64,
    /// Flow multiplier for brim adhesion aid.
    #[setting(
        tier = 3,
        description = "Flow multiplier for brim adhesion",
        min = 0.0,
        max = 2.0
    )]
    pub brim: f64,
    /// Flow multiplier for variable-width (Arachne) perimeters.
    #[setting(
        tier = 3,
        description = "Flow multiplier for variable-width perimeters",
        min = 0.0,
        max = 2.0
    )]
    pub variable_width_perimeter: f64,
    /// Flow multiplier for ironing passes.
    #[setting(
        tier = 3,
        description = "Flow multiplier for ironing passes",
        min = 0.0,
        max = 2.0
    )]
    pub ironing: f64,
    /// Flow multiplier for purge tower extrusions.
    #[setting(
        tier = 3,
        description = "Flow multiplier for purge tower extrusions",
        min = 0.0,
        max = 2.0
    )]
    pub purge_tower: f64,
}

impl Default for PerFeatureFlow {
    fn default() -> Self {
        Self {
            outer_perimeter: 1.0,
            inner_perimeter: 1.0,
            solid_infill: 1.0,
            sparse_infill: 1.0,
            support: 1.0,
            support_interface: 1.0,
            bridge: 1.0,
            gap_fill: 1.0,
            skirt: 1.0,
            brim: 1.0,
            variable_width_perimeter: 1.0,
            ironing: 1.0,
            purge_tower: 1.0,
        }
    }
}

impl PerFeatureFlow {
    /// Returns the flow multiplier for the given feature type.
    ///
    /// Travel moves always return 1.0 (no flow adjustment for non-extrusion moves).
    pub fn get_multiplier(&self, feature: FeatureType) -> f64 {
        match feature {
            FeatureType::OuterPerimeter => self.outer_perimeter,
            FeatureType::InnerPerimeter => self.inner_perimeter,
            FeatureType::SolidInfill => self.solid_infill,
            FeatureType::SparseInfill => self.sparse_infill,
            FeatureType::Support => self.support,
            FeatureType::SupportInterface => self.support_interface,
            FeatureType::Bridge => self.bridge,
            FeatureType::GapFill => self.gap_fill,
            FeatureType::Skirt => self.skirt,
            FeatureType::Brim => self.brim,
            FeatureType::VariableWidthPerimeter => self.variable_width_perimeter,
            FeatureType::Ironing => self.ironing,
            FeatureType::PurgeTower => self.purge_tower,
            FeatureType::Travel => 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_all_multipliers_are_one() {
        let flow = PerFeatureFlow::default();
        assert!((flow.outer_perimeter - 1.0).abs() < 1e-9);
        assert!((flow.inner_perimeter - 1.0).abs() < 1e-9);
        assert!((flow.solid_infill - 1.0).abs() < 1e-9);
        assert!((flow.sparse_infill - 1.0).abs() < 1e-9);
        assert!((flow.support - 1.0).abs() < 1e-9);
        assert!((flow.support_interface - 1.0).abs() < 1e-9);
        assert!((flow.bridge - 1.0).abs() < 1e-9);
        assert!((flow.gap_fill - 1.0).abs() < 1e-9);
        assert!((flow.skirt - 1.0).abs() < 1e-9);
        assert!((flow.brim - 1.0).abs() < 1e-9);
        assert!((flow.variable_width_perimeter - 1.0).abs() < 1e-9);
        assert!((flow.ironing - 1.0).abs() < 1e-9);
        assert!((flow.purge_tower - 1.0).abs() < 1e-9);
    }

    #[test]
    fn get_multiplier_returns_correct_values() {
        let mut flow = PerFeatureFlow::default();
        flow.outer_perimeter = 0.95;
        flow.bridge = 1.1;
        flow.ironing = 0.1;

        assert!((flow.get_multiplier(FeatureType::OuterPerimeter) - 0.95).abs() < 1e-9);
        assert!((flow.get_multiplier(FeatureType::Bridge) - 1.1).abs() < 1e-9);
        assert!((flow.get_multiplier(FeatureType::Ironing) - 0.1).abs() < 1e-9);
        // Travel always returns 1.0.
        assert!((flow.get_multiplier(FeatureType::Travel) - 1.0).abs() < 1e-9);
        // Unmodified features return default 1.0.
        assert!((flow.get_multiplier(FeatureType::InnerPerimeter) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn serde_round_trip() {
        let mut flow = PerFeatureFlow::default();
        flow.outer_perimeter = 0.95;
        flow.sparse_infill = 1.05;

        let json = serde_json::to_string(&flow).unwrap();
        let deserialized: PerFeatureFlow = serde_json::from_str(&json).unwrap();

        assert!((deserialized.outer_perimeter - 0.95).abs() < 1e-9);
        assert!((deserialized.sparse_infill - 1.05).abs() < 1e-9);
        assert!((deserialized.bridge - 1.0).abs() < 1e-9);
    }

    #[test]
    fn toml_deserialization_partial() {
        let toml_str = r#"
outer_perimeter = 0.95
bridge = 1.1
"#;
        let flow: PerFeatureFlow = toml::from_str(toml_str).unwrap();
        assert!((flow.outer_perimeter - 0.95).abs() < 1e-9);
        assert!((flow.bridge - 1.1).abs() < 1e-9);
        // Unspecified fields use defaults.
        assert!((flow.inner_perimeter - 1.0).abs() < 1e-9);
        assert!((flow.ironing - 1.0).abs() < 1e-9);
    }
}
