//! Core types for the ConfigSchema setting metadata system.

use serde::{Deserialize, Serialize};

/// A dotted-path key identifying a specific setting (e.g., `"print.layer_height"`).
///
/// # Examples
///
/// ```
/// use slicecore_config_schema::SettingKey;
///
/// let key = SettingKey::new("print.layer_height");
/// assert_eq!(key.to_string(), "print.layer_height");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct SettingKey(pub String);

impl SettingKey {
    /// Creates a new `SettingKey` from any string-like value.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for SettingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for SettingKey {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// Progressive disclosure tier controlling which settings are visible at each experience level.
///
/// Lower tiers are shown first; higher tiers are hidden unless the user opts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Tier {
    /// AI-managed automatic settings, not shown to users.
    AiAuto = 0,
    /// Simple settings for beginners.
    Simple = 1,
    /// Intermediate settings for users who want more control.
    Intermediate = 2,
    /// Advanced settings for experienced users.
    Advanced = 3,
    /// Developer-only settings for debugging and testing.
    Developer = 4,
}

impl Tier {
    /// Returns the string representation of this tier.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AiAuto => "ai_auto",
            Self::Simple => "simple",
            Self::Intermediate => "intermediate",
            Self::Advanced => "advanced",
            Self::Developer => "developer",
        }
    }

    /// Converts a `u8` value to a `Tier`, returning `None` for invalid values.
    #[must_use]
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::AiAuto),
            1 => Some(Self::Simple),
            2 => Some(Self::Intermediate),
            3 => Some(Self::Advanced),
            4 => Some(Self::Developer),
            _ => None,
        }
    }
}

/// UI grouping category for organizing settings in the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettingCategory {
    /// Print quality settings (layer height, resolution).
    Quality,
    /// Print speed settings.
    Speed,
    /// Line width settings.
    LineWidth,
    /// Cooling fan and temperature settings.
    Cooling,
    /// Retraction settings.
    Retraction,
    /// Support structure settings.
    Support,
    /// Infill pattern and density settings.
    Infill,
    /// Bed adhesion settings (brim, raft, skirt).
    Adhesion,
    /// Advanced print settings.
    Advanced,
    /// Machine-specific settings.
    Machine,
    /// Filament material settings.
    Filament,
    /// Acceleration and jerk settings.
    Acceleration,
    /// Post-processing settings.
    PostProcess,
    /// Timelapse recording settings.
    Timelapse,
    /// Multi-material and multi-extruder settings.
    MultiMaterial,
    /// Calibration and tuning settings.
    Calibration,
}

impl SettingCategory {
    /// Returns the lowercase string representation of this category.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Quality => "quality",
            Self::Speed => "speed",
            Self::LineWidth => "line_width",
            Self::Cooling => "cooling",
            Self::Retraction => "retraction",
            Self::Support => "support",
            Self::Infill => "infill",
            Self::Adhesion => "adhesion",
            Self::Advanced => "advanced",
            Self::Machine => "machine",
            Self::Filament => "filament",
            Self::Acceleration => "acceleration",
            Self::PostProcess => "post_process",
            Self::Timelapse => "timelapse",
            Self::MultiMaterial => "multi_material",
            Self::Calibration => "calibration",
        }
    }
}

/// Discriminated union of value types a setting can hold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueType {
    /// Boolean on/off setting.
    Bool,
    /// Integer setting.
    Int,
    /// Floating-point setting.
    Float,
    /// Free-form text setting.
    String,
    /// Percentage value (0-100 or beyond).
    Percent,
    /// Enumerated choice with named variants.
    Enum {
        /// The allowed variants for this enum setting.
        variants: Vec<EnumVariant>,
    },
    /// Vector of floating-point values (e.g., per-extruder values).
    FloatVec,
}

/// A single variant in an enumerated setting value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumVariant {
    /// The snake_case variant name used in configuration files.
    pub value: String,
    /// Human-readable display name for the UI.
    pub display: String,
    /// Description of what this variant does.
    pub description: String,
}

/// Validation constraint applied to a setting value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    /// Numeric range constraint (inclusive on both ends).
    Range {
        /// Minimum allowed value.
        min: f64,
        /// Maximum allowed value.
        max: f64,
    },
    /// Dependency constraint: this setting is only relevant when another setting
    /// meets the specified condition.
    DependsOn {
        /// The key of the setting this depends on.
        key: SettingKey,
        /// A human-readable condition string (e.g., `"== true"`, `"> 0"`).
        condition: String,
    },
}

/// Full metadata definition for a single setting in the schema.
///
/// Contains all information needed for UI rendering, validation, documentation,
/// and JSON Schema generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingDefinition {
    /// Unique dotted-path key for this setting.
    pub key: SettingKey,
    /// Human-readable display name for UI labels.
    pub display_name: String,
    /// Detailed description of what this setting controls.
    pub description: String,
    /// Progressive disclosure tier.
    pub tier: Tier,
    /// UI grouping category.
    pub category: SettingCategory,
    /// The type of value this setting holds.
    pub value_type: ValueType,
    /// Default value as a JSON value.
    pub default_value: serde_json::Value,
    /// Validation constraints.
    pub constraints: Vec<Constraint>,
    /// Keys of settings that this setting affects (forward dependency graph).
    pub affects: Vec<SettingKey>,
    /// Keys of settings that affect this setting (inverse dependency graph).
    pub affected_by: Vec<SettingKey>,
    /// Optional unit string for display (e.g., `"mm"`, `"mm/s"`, `"%"`).
    pub units: Option<String>,
    /// Freeform tags for filtering and search.
    pub tags: Vec<String>,
    /// Version string when this setting was introduced.
    pub since_version: String,
    /// If deprecated, the reason/migration guidance.
    pub deprecated: Option<String>,
}

/// Trait implemented by config structs to provide setting metadata.
///
/// The `#[derive(ConfigSchema)]` macro generates this implementation automatically.
/// Manual implementations are also supported for custom types.
pub trait HasSettingSchema {
    /// Returns setting definitions for this type, prefixed with the given path.
    ///
    /// # Arguments
    ///
    /// * `prefix` - Dotted path prefix to prepend to all setting keys (e.g., `"print"`).
    fn setting_definitions(prefix: &str) -> Vec<SettingDefinition>;
}
