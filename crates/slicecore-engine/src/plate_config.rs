//! Plate-level configuration for multi-object slicing.
//!
//! [`PlateConfig`] is the top-level input to the slicing engine, replacing
//! direct [`PrintConfig`] usage. It contains base profile layers, default
//! object overrides, named override sets, and per-object configurations.
//!
//! Single-object plates are backward compatible via [`PlateConfig::single_object`]
//! and the [`From<PrintConfig>`] implementation.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::PrintConfig;

/// How a mesh is sourced for an object in a plate config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshSource {
    /// Load from a file path (STL, 3MF, OBJ).
    File(PathBuf),
    /// Mesh provided directly in memory (for API/WASM use).
    InMemory,
}

/// Geometric primitive shapes for modifier volumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModifierShape {
    /// Axis-aligned box with position, size, and rotation.
    Box {
        /// Center position `[x, y, z]` in mm.
        position: [f64; 3],
        /// Dimensions `[width, depth, height]` in mm.
        size: [f64; 3],
        /// Euler rotation `[rx, ry, rz]` in degrees.
        rotation: [f64; 3],
    },
    /// Cylinder with position, radius, height, and rotation.
    Cylinder {
        /// Center position `[x, y, z]` in mm.
        position: [f64; 3],
        /// Radius in mm.
        radius: f64,
        /// Height in mm.
        height: f64,
        /// Euler rotation `[rx, ry, rz]` in degrees.
        rotation: [f64; 3],
    },
    /// Sphere with position and radius.
    Sphere {
        /// Center position `[x, y, z]` in mm.
        position: [f64; 3],
        /// Radius in mm.
        radius: f64,
    },
}

/// Source of a modifier mesh volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModifierSource {
    /// Geometric primitive (no mesh file needed).
    Primitive(ModifierShape),
    /// External mesh file (STL).
    File(PathBuf),
}

/// Object-level transform (position, rotation, scale).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    /// Position offset `[x, y, z]` in mm.
    pub position: [f64; 3],
    /// Euler rotation `[rx, ry, rz]` in degrees.
    pub rotation: [f64; 3],
    /// Scale factor `[sx, sy, sz]` (1.0 = identity).
    pub scale: [f64; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

/// A modifier mesh configuration: volume + overrides as TOML partial table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifierConfig {
    /// Source geometry for this modifier volume.
    pub source: ModifierSource,
    /// Unique identifier for this modifier within the object.
    pub modifier_id: String,
    /// Setting overrides applied within this modifier region.
    pub overrides: toml::map::Map<String, toml::Value>,
}

/// Layer-range override: applies overrides within a Z or layer number range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerRangeOverride {
    /// Z height range in mm `(min, max)`. Mutually exclusive with `layer_range`.
    pub z_range: Option<(f64, f64)>,
    /// Layer number range `(start, end)`, inclusive. Mutually exclusive with `z_range`.
    pub layer_range: Option<(u32, u32)>,
    /// Overrides to apply within this range.
    pub overrides: toml::map::Map<String, toml::Value>,
}

/// Per-object configuration within a plate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectConfig {
    /// How to obtain the mesh for this object.
    pub mesh_source: MeshSource,
    /// Human-readable name for this object.
    pub name: Option<String>,
    /// Named override set to apply (references [`PlateConfig::override_sets`]).
    pub override_set: Option<String>,
    /// Inline setting overrides (takes precedence over `override_set`).
    pub inline_overrides: Option<toml::map::Map<String, toml::Value>>,
    /// Modifier volumes with per-region overrides.
    #[serde(default)]
    pub modifiers: Vec<ModifierConfig>,
    /// Layer-range overrides for this object.
    #[serde(default)]
    pub layer_overrides: Vec<LayerRangeOverride>,
    /// Object transform (position, rotation, scale).
    pub transform: Option<Transform>,
    /// Number of copies to print.
    pub copies: u32,
}

impl Default for ObjectConfig {
    fn default() -> Self {
        Self {
            mesh_source: MeshSource::InMemory,
            name: None,
            override_set: None,
            inline_overrides: None,
            modifiers: Vec::new(),
            layer_overrides: Vec::new(),
            transform: None,
            copies: 1,
        }
    }
}

/// Top-level plate configuration: replaces direct `PrintConfig` as engine input.
///
/// Contains base profile layers, default object overrides, named override sets,
/// and per-object configurations. Single-object plates are backward compatible
/// via [`PlateConfig::single_object`] and [`From<PrintConfig>`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlateConfig {
    /// Machine/printer profile name or path.
    pub machine_profile: Option<String>,
    /// Filament/material profile name or path.
    pub filament_profile: Option<String>,
    /// Process/quality profile name or path.
    pub process_profile: Option<String>,
    /// User override TOML file path.
    pub user_override_file: Option<PathBuf>,
    /// CLI `--set key=value` overrides.
    pub cli_set_overrides: Vec<(String, String)>,
    /// Default overrides applied to ALL objects (cascade layer 7).
    pub default_object_overrides: Option<toml::map::Map<String, toml::Value>>,
    /// Named override sets (inline definitions or loaded from files).
    pub override_sets: HashMap<String, toml::map::Map<String, toml::Value>>,
    /// Per-object configurations.
    pub objects: Vec<ObjectConfig>,
}

impl Default for PlateConfig {
    fn default() -> Self {
        Self {
            machine_profile: None,
            filament_profile: None,
            process_profile: None,
            user_override_file: None,
            cli_set_overrides: Vec::new(),
            default_object_overrides: None,
            override_sets: HashMap::new(),
            objects: Vec::new(),
        }
    }
}

impl PlateConfig {
    /// Creates a single-object plate config wrapping a `PrintConfig` (backward compat).
    ///
    /// The resulting plate has no overrides and a single default object.
    #[must_use]
    pub fn single_object(_config: PrintConfig) -> Self {
        Self {
            machine_profile: None,
            filament_profile: None,
            process_profile: None,
            user_override_file: None,
            cli_set_overrides: Vec::new(),
            default_object_overrides: None,
            override_sets: HashMap::new(),
            objects: vec![ObjectConfig::default()],
        }
    }

    /// Returns `true` if this is a single-object plate with no overrides.
    #[must_use]
    pub fn is_simple(&self) -> bool {
        self.objects.len() == 1
            && self.default_object_overrides.is_none()
            && self.objects[0].override_set.is_none()
            && self.objects[0].inline_overrides.is_none()
            && self.objects[0].modifiers.is_empty()
            && self.objects[0].layer_overrides.is_empty()
    }
}

impl From<PrintConfig> for PlateConfig {
    fn from(config: PrintConfig) -> Self {
        Self::single_object(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_object_creates_valid_plate() {
        let config = PrintConfig::default();
        let plate = PlateConfig::single_object(config);
        assert_eq!(plate.objects.len(), 1);
        assert!(plate.machine_profile.is_none());
        assert!(plate.filament_profile.is_none());
        assert!(plate.process_profile.is_none());
        assert!(plate.default_object_overrides.is_none());
        assert!(plate.override_sets.is_empty());
        assert!(plate.cli_set_overrides.is_empty());
    }

    #[test]
    fn is_simple_true_for_single_object() {
        let plate = PlateConfig::from(PrintConfig::default());
        assert!(plate.is_simple());
    }

    #[test]
    fn is_simple_false_with_default_overrides() {
        let mut plate = PlateConfig::from(PrintConfig::default());
        let mut overrides = toml::map::Map::new();
        overrides.insert("infill_density".to_string(), toml::Value::Float(0.5));
        plate.default_object_overrides = Some(overrides);
        assert!(!plate.is_simple());
    }

    #[test]
    fn is_simple_false_with_inline_overrides() {
        let mut plate = PlateConfig::from(PrintConfig::default());
        let mut overrides = toml::map::Map::new();
        overrides.insert("wall_count".to_string(), toml::Value::Integer(4));
        plate.objects[0].inline_overrides = Some(overrides);
        assert!(!plate.is_simple());
    }

    #[test]
    fn from_print_config() {
        let config = PrintConfig::default();
        let plate: PlateConfig = config.into();
        assert_eq!(plate.objects.len(), 1);
        assert!(plate.is_simple());
    }

    #[test]
    fn object_config_default_values() {
        let obj = ObjectConfig::default();
        assert_eq!(obj.copies, 1);
        assert!(obj.modifiers.is_empty());
        assert!(obj.layer_overrides.is_empty());
        assert!(obj.name.is_none());
        assert!(obj.override_set.is_none());
        assert!(obj.inline_overrides.is_none());
        assert!(obj.transform.is_none());
    }

    #[test]
    fn transform_default_identity() {
        let t = Transform::default();
        assert_eq!(t.position, [0.0, 0.0, 0.0]);
        assert_eq!(t.rotation, [0.0, 0.0, 0.0]);
        assert_eq!(t.scale, [1.0, 1.0, 1.0]);
    }
}
