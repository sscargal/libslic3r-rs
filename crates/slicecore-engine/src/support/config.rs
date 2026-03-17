//! Support structure configuration types.
//!
//! Defines all parameters controlling support generation: type selection,
//! pattern, density, interface layers, gap distances, tree support parameters,
//! bridge detection settings, quality presets, and conflict resolution.
//!
//! All types derive `Serialize`/`Deserialize` for TOML configuration support,
//! following the same pattern as [`ScarfJointConfig`](crate::config::ScarfJointConfig).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Controls which type of support structure to generate.
///
/// - `Auto`: Selects based on geometry (small contact areas -> tree, large -> traditional).
/// - `Traditional`: Column-based support structures.
/// - `Tree`: Tree-branching support structures.
/// - `None`: No support generation even when enabled.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportType {
    /// Automatically select between traditional and tree based on geometry.
    #[default]
    Auto,
    /// Column-based support with configurable pattern fill.
    Traditional,
    /// Tree-branching support for minimal contact area.
    Tree,
    /// Explicitly disable support (overrides `enabled` flag).
    None,
}

/// Fill pattern for the support body (non-interface layers).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportPattern {
    /// Grid pattern (perpendicular crossing lines).
    Grid,
    /// Single-direction lines (easier to remove).
    #[default]
    Line,
    /// Alternating-angle rectilinear fill.
    Rectilinear,
}

/// Fill pattern for support interface layers (contact surfaces).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfacePattern {
    /// Alternating-angle rectilinear (default, good surface quality).
    #[default]
    Rectilinear,
    /// Concentric rings following the support boundary.
    Concentric,
    /// Grid pattern for maximum coverage.
    Grid,
}

/// Tree support branch growth style.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeBranchStyle {
    /// Automatically select between organic and geometric based on model.
    #[default]
    Auto,
    /// Smooth, curved branches (better aesthetics, harder to remove).
    Organic,
    /// Straight, angular branches (easier to remove, faster to generate).
    Geometric,
}

/// Taper method for tree support trunks.
///
/// Controls how the trunk diameter decreases from base to tip.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaperMethod {
    /// Automatically select taper method (defaults to linear).
    #[default]
    Auto,
    /// Linear diameter reduction from base to tip.
    Linear,
    /// Exponential diameter reduction (faster narrowing near tip).
    Exponential,
    /// Load-based taper considering supported weight distribution.
    LoadBased,
}

/// Quality preset that adjusts multiple support parameters at once.
///
/// When applied, overrides density, interface density, z-gap, and
/// interface layer count for a balanced quality-vs-removability trade-off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityPreset {
    /// Fast removal: lower density, fewer interface layers, larger gaps.
    Low,
    /// Balanced defaults matching research recommendations.
    Medium,
    /// Maximum surface quality: higher density, more interface layers.
    High,
}

/// Strategy for resolving conflicting support configuration values.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Warn when conflicting settings are detected (default).
    #[default]
    WarnOnConflict,
    /// Automatically merge conflicting settings using priority rules.
    SmartMerge,
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// Bridge detection and extrusion configuration.
///
/// Controls print parameters for detected bridge regions (unsupported
/// horizontal spans between two support points).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BridgeConfig {
    /// Bridge print speed in mm/s.
    pub speed: f64,
    /// Fan speed during bridging (0-255, 255 = 100%).
    pub fan_speed: u8,
    /// Flow rate ratio during bridging (< 1.0 reduces stringing).
    pub flow_ratio: f64,
    /// Acceleration during bridging in mm/s^2.
    pub acceleration: f64,
    /// Line width ratio relative to standard extrusion width.
    pub line_width_ratio: f64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            speed: 30.0,
            fan_speed: 255,
            flow_ratio: 0.85,
            acceleration: 500.0,
            line_width_ratio: 1.0,
        }
    }
}

/// Tree support-specific configuration.
///
/// Parameters controlling branch growth, merging, and tapering for
/// tree-style support structures.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TreeSupportConfig {
    /// Branch growth style (organic curves vs geometric angles).
    pub branch_style: TreeBranchStyle,
    /// Trunk diameter taper method.
    pub taper_method: TaperMethod,
    /// Maximum branching angle in degrees.
    pub branch_angle: f64,
    /// Minimum branch divergence angle in degrees.
    pub min_branch_angle: f64,
    /// Maximum trunk diameter in mm.
    pub max_trunk_diameter: f64,
    /// Branch merge distance as a factor of trunk diameter.
    pub merge_distance_factor: f64,
    /// Tip diameter at contact points in mm.
    pub tip_diameter: f64,
}

impl Default for TreeSupportConfig {
    fn default() -> Self {
        Self {
            branch_style: TreeBranchStyle::Auto,
            taper_method: TaperMethod::Auto,
            branch_angle: 45.0,
            min_branch_angle: 15.0,
            max_trunk_diameter: 10.0,
            merge_distance_factor: 3.0,
            tip_diameter: 0.8,
        }
    }
}

/// Complete support structure configuration.
///
/// Controls all aspects of support generation including detection thresholds,
/// pattern selection, interface layers, gap distances, and sub-configurations
/// for tree support and bridge detection.
///
/// # Defaults
///
/// Default values are based on research recommendations:
/// - 45-degree overhang threshold (universal FDM standard)
/// - 15% body density, 80% interface density
/// - 0.2mm Z-gap (PLA default), 0.4mm XY-gap (1 extrusion width)
/// - Line pattern for easy removal
/// - 2 interface layers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SupportConfig {
    /// Enable support generation.
    pub enabled: bool,
    /// Support type selection strategy.
    pub support_type: SupportType,
    /// Overhang angle threshold in degrees (surfaces steeper than this
    /// relative to horizontal are considered overhangs needing support).
    pub overhang_angle: f64,
    /// Minimum support region area in mm^2 (regions smaller than this
    /// are discarded as unprintable).
    pub min_support_area: f64,
    /// Support body density as a fraction (0.0 - 1.0).
    pub support_density: f64,
    /// Fill pattern for support body layers.
    pub support_pattern: SupportPattern,
    /// Number of dense interface layers at top/bottom of support.
    pub interface_layers: u32,
    /// Interface layer density as a fraction (0.0 - 1.0).
    pub interface_density: f64,
    /// Fill pattern for interface layers.
    pub interface_pattern: InterfacePattern,
    /// Z-axis gap between support top and model bottom in mm.
    pub z_gap: f64,
    /// XY-axis gap between support and model walls in mm.
    pub xy_gap: f64,
    /// Only generate support touching the build plate (no support-on-model).
    pub build_plate_only: bool,
    /// Enable bridge detection for unsupported horizontal spans.
    pub bridge_detection: bool,
    /// Bridge extrusion configuration.
    pub bridge: BridgeConfig,
    /// Tree support configuration.
    pub tree: TreeSupportConfig,
    /// Optional quality preset (overrides individual settings when applied).
    pub quality_preset: Option<QualityPreset>,
    /// Strategy for resolving conflicting configuration values.
    pub conflict_resolution: ConflictResolution,
    /// Number of dense interface layers at the bottom of support (support floor).
    /// OrcaSlicer: `support_bottom_interface_layers`. Range: 0-10. Default: 0.
    pub support_bottom_interface_layers: u32,
}

impl Default for SupportConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            support_type: SupportType::Auto,
            overhang_angle: 45.0,
            min_support_area: 0.77,
            support_density: 0.15,
            support_pattern: SupportPattern::Line,
            interface_layers: 2,
            interface_density: 0.80,
            interface_pattern: InterfacePattern::Rectilinear,
            z_gap: 0.2,
            xy_gap: 0.4,
            build_plate_only: false,
            bridge_detection: true,
            bridge: BridgeConfig::default(),
            tree: TreeSupportConfig::default(),
            quality_preset: None,
            conflict_resolution: ConflictResolution::WarnOnConflict,
            support_bottom_interface_layers: 0,
        }
    }
}

impl QualityPreset {
    /// Applies this quality preset to a support configuration.
    ///
    /// Overrides density, interface density, z-gap, and interface layer
    /// count to match the preset's quality-vs-removability balance.
    pub fn apply(&self, config: &mut SupportConfig) {
        match self {
            QualityPreset::Low => {
                config.support_density = 0.10;
                config.interface_density = 0.50;
                config.z_gap = 0.30;
                config.interface_layers = 1;
            }
            QualityPreset::Medium => {
                config.support_density = 0.15;
                config.interface_density = 0.80;
                config.z_gap = 0.20;
                config.interface_layers = 2;
            }
            QualityPreset::High => {
                config.support_density = 0.20;
                config.interface_density = 1.0;
                config.z_gap = 0.15;
                config.interface_layers = 3;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_config_defaults() {
        let config = SupportConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.support_type, SupportType::Auto);
        assert!((config.overhang_angle - 45.0).abs() < 1e-9);
        assert!((config.min_support_area - 0.77).abs() < 1e-9);
        assert!((config.support_density - 0.15).abs() < 1e-9);
        assert_eq!(config.support_pattern, SupportPattern::Line);
        assert_eq!(config.interface_layers, 2);
        assert!((config.interface_density - 0.80).abs() < 1e-9);
        assert_eq!(config.interface_pattern, InterfacePattern::Rectilinear);
        assert!((config.z_gap - 0.2).abs() < 1e-9);
        assert!((config.xy_gap - 0.4).abs() < 1e-9);
        assert!(!config.build_plate_only);
        assert!(config.bridge_detection);
        assert!(config.quality_preset.is_none());
        assert_eq!(
            config.conflict_resolution,
            ConflictResolution::WarnOnConflict
        );
    }

    #[test]
    fn bridge_config_defaults() {
        let config = BridgeConfig::default();
        assert!((config.speed - 30.0).abs() < 1e-9);
        assert_eq!(config.fan_speed, 255);
        assert!((config.flow_ratio - 0.85).abs() < 1e-9);
        assert!((config.acceleration - 500.0).abs() < 1e-9);
        assert!((config.line_width_ratio - 1.0).abs() < 1e-9);
    }

    #[test]
    fn tree_support_config_defaults() {
        let config = TreeSupportConfig::default();
        assert_eq!(config.branch_style, TreeBranchStyle::Auto);
        assert_eq!(config.taper_method, TaperMethod::Auto);
        assert!((config.branch_angle - 45.0).abs() < 1e-9);
        assert!((config.min_branch_angle - 15.0).abs() < 1e-9);
        assert!((config.max_trunk_diameter - 10.0).abs() < 1e-9);
        assert!((config.merge_distance_factor - 3.0).abs() < 1e-9);
        assert!((config.tip_diameter - 0.8).abs() < 1e-9);
    }

    #[test]
    fn quality_preset_low_applies() {
        let mut config = SupportConfig::default();
        QualityPreset::Low.apply(&mut config);
        assert!((config.support_density - 0.10).abs() < 1e-9);
        assert!((config.interface_density - 0.50).abs() < 1e-9);
        assert!((config.z_gap - 0.30).abs() < 1e-9);
        assert_eq!(config.interface_layers, 1);
    }

    #[test]
    fn quality_preset_medium_applies() {
        let mut config = SupportConfig::default();
        QualityPreset::Medium.apply(&mut config);
        assert!((config.support_density - 0.15).abs() < 1e-9);
        assert!((config.interface_density - 0.80).abs() < 1e-9);
        assert!((config.z_gap - 0.20).abs() < 1e-9);
        assert_eq!(config.interface_layers, 2);
    }

    #[test]
    fn quality_preset_high_applies() {
        let mut config = SupportConfig::default();
        QualityPreset::High.apply(&mut config);
        assert!((config.support_density - 0.20).abs() < 1e-9);
        assert!((config.interface_density - 1.0).abs() < 1e-9);
        assert!((config.z_gap - 0.15).abs() < 1e-9);
        assert_eq!(config.interface_layers, 3);
    }

    #[test]
    fn support_type_serde_round_trip() {
        let types = [
            SupportType::Auto,
            SupportType::Traditional,
            SupportType::Tree,
            SupportType::None,
        ];
        for t in &types {
            let json = serde_json::to_string(t).unwrap();
            let deserialized: SupportType = serde_json::from_str(&json).unwrap();
            assert_eq!(*t, deserialized, "Serde round-trip failed for {:?}", t);
        }
    }

    #[test]
    fn support_pattern_serde_round_trip() {
        let patterns = [
            SupportPattern::Grid,
            SupportPattern::Line,
            SupportPattern::Rectilinear,
        ];
        for p in &patterns {
            let json = serde_json::to_string(p).unwrap();
            let deserialized: SupportPattern = serde_json::from_str(&json).unwrap();
            assert_eq!(*p, deserialized, "Serde round-trip failed for {:?}", p);
        }
    }

    #[test]
    fn interface_pattern_serde_round_trip() {
        let patterns = [
            InterfacePattern::Rectilinear,
            InterfacePattern::Concentric,
            InterfacePattern::Grid,
        ];
        for p in &patterns {
            let json = serde_json::to_string(p).unwrap();
            let deserialized: InterfacePattern = serde_json::from_str(&json).unwrap();
            assert_eq!(*p, deserialized, "Serde round-trip failed for {:?}", p);
        }
    }

    #[test]
    fn tree_branch_style_serde_round_trip() {
        let styles = [
            TreeBranchStyle::Auto,
            TreeBranchStyle::Organic,
            TreeBranchStyle::Geometric,
        ];
        for s in &styles {
            let json = serde_json::to_string(s).unwrap();
            let deserialized: TreeBranchStyle = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, deserialized, "Serde round-trip failed for {:?}", s);
        }
    }

    #[test]
    fn taper_method_serde_round_trip() {
        let methods = [
            TaperMethod::Auto,
            TaperMethod::Linear,
            TaperMethod::Exponential,
            TaperMethod::LoadBased,
        ];
        for m in &methods {
            let json = serde_json::to_string(m).unwrap();
            let deserialized: TaperMethod = serde_json::from_str(&json).unwrap();
            assert_eq!(*m, deserialized, "Serde round-trip failed for {:?}", m);
        }
    }

    #[test]
    fn conflict_resolution_serde_round_trip() {
        let modes = [
            ConflictResolution::WarnOnConflict,
            ConflictResolution::SmartMerge,
        ];
        for m in &modes {
            let json = serde_json::to_string(m).unwrap();
            let deserialized: ConflictResolution = serde_json::from_str(&json).unwrap();
            assert_eq!(*m, deserialized, "Serde round-trip failed for {:?}", m);
        }
    }
}
