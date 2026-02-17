//! Custom G-code injection hooks.
//!
//! Provides configurable G-code strings that are injected at specific points
//! during print: before/after layer changes, during tool changes, and at
//! specific Z heights. Placeholders like `{layer_num}` are substituted with
//! actual values at injection time.
//!
//! # Placeholders
//!
//! - `{layer_num}` -- current layer index (0-based)
//! - `{layer_z}` -- current layer Z height in mm
//! - `{total_layers}` -- total number of layers in the print
//!
//! # Example
//!
//! ```ignore
//! let hooks = CustomGcodeHooks {
//!     after_layer_change: "M117 Layer {layer_num}/{total_layers}".to_string(),
//!     ..Default::default()
//! };
//! let result = substitute_placeholders(&hooks.after_layer_change, 5, 1.2, 100);
//! assert_eq!(result, "M117 Layer 5/100");
//! ```

use serde::{Deserialize, Serialize};

/// Custom G-code injection hooks configuration.
///
/// Each field contains a G-code string to inject at the corresponding point
/// in the print. Empty strings are skipped (no injection). Multiple G-code
/// commands can be separated by newlines.
///
/// Serialized as a TOML section `[custom_gcode]` within
/// [`PrintConfig`](crate::config::PrintConfig).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomGcodeHooks {
    /// G-code injected before each layer's Z move.
    pub before_layer_change: String,
    /// G-code injected after each layer's Z move.
    pub after_layer_change: String,
    /// G-code injected during tool changes (multi-material, future use).
    pub tool_change_gcode: String,
    /// Alias for `before_layer_change` (synonym used by some slicer UIs).
    /// If both are non-empty, `before_layer_change` takes precedence.
    pub before_every_layer: String,
    /// G-code injected at specific Z heights.
    /// Each entry is a `(z_height, gcode_string)` pair.
    pub custom_gcode_per_z: Vec<(f64, String)>,
}

impl CustomGcodeHooks {
    /// Returns the effective before-layer-change G-code.
    ///
    /// Prefers `before_layer_change` if non-empty, falls back to
    /// `before_every_layer`.
    pub fn effective_before_layer(&self) -> &str {
        if !self.before_layer_change.is_empty() {
            &self.before_layer_change
        } else {
            &self.before_every_layer
        }
    }
}

/// Substitutes placeholders in a G-code template string.
///
/// # Placeholders
///
/// - `{layer_num}` -- replaced with `layer_num`
/// - `{layer_z}` -- replaced with `layer_z` formatted to 3 decimal places
/// - `{total_layers}` -- replaced with `total_layers`
///
/// # Parameters
///
/// - `gcode`: Template string with optional placeholders.
/// - `layer_num`: Current layer index (0-based).
/// - `layer_z`: Current layer Z height in mm.
/// - `total_layers`: Total number of layers in the print.
///
/// # Returns
///
/// The G-code string with all recognized placeholders replaced.
pub fn substitute_placeholders(
    gcode: &str,
    layer_num: usize,
    layer_z: f64,
    total_layers: usize,
) -> String {
    gcode
        .replace("{layer_num}", &layer_num.to_string())
        .replace("{layer_z}", &format!("{:.3}", layer_z))
        .replace("{total_layers}", &total_layers.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hooks_are_empty() {
        let hooks = CustomGcodeHooks::default();
        assert!(hooks.before_layer_change.is_empty());
        assert!(hooks.after_layer_change.is_empty());
        assert!(hooks.tool_change_gcode.is_empty());
        assert!(hooks.before_every_layer.is_empty());
        assert!(hooks.custom_gcode_per_z.is_empty());
    }

    #[test]
    fn substitute_placeholders_replaces_all() {
        let template = "M117 L{layer_num} Z{layer_z} of {total_layers}";
        let result = substitute_placeholders(template, 5, 1.2, 100);
        assert_eq!(result, "M117 L5 Z1.200 of 100");
    }

    #[test]
    fn substitute_placeholders_no_placeholders() {
        let template = "G28 ; home all";
        let result = substitute_placeholders(template, 0, 0.3, 50);
        assert_eq!(result, "G28 ; home all");
    }

    #[test]
    fn substitute_placeholders_repeated() {
        let template = "{layer_num} {layer_num}";
        let result = substitute_placeholders(template, 3, 0.6, 10);
        assert_eq!(result, "3 3");
    }

    #[test]
    fn effective_before_layer_prefers_before_layer_change() {
        let hooks = CustomGcodeHooks {
            before_layer_change: "BLC".to_string(),
            before_every_layer: "BEL".to_string(),
            ..Default::default()
        };
        assert_eq!(hooks.effective_before_layer(), "BLC");
    }

    #[test]
    fn effective_before_layer_falls_back_to_before_every_layer() {
        let hooks = CustomGcodeHooks {
            before_layer_change: String::new(),
            before_every_layer: "BEL".to_string(),
            ..Default::default()
        };
        assert_eq!(hooks.effective_before_layer(), "BEL");
    }

    #[test]
    fn serde_round_trip() {
        let hooks = CustomGcodeHooks {
            after_layer_change: "M117 Layer {layer_num}".to_string(),
            custom_gcode_per_z: vec![
                (5.0, "M600 ; filament change".to_string()),
                (10.5, "M0 ; pause".to_string()),
            ],
            ..Default::default()
        };

        let json = serde_json::to_string(&hooks).unwrap();
        let deserialized: CustomGcodeHooks = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.after_layer_change, "M117 Layer {layer_num}");
        assert_eq!(deserialized.custom_gcode_per_z.len(), 2);
        assert!((deserialized.custom_gcode_per_z[0].0 - 5.0).abs() < 1e-9);
        assert_eq!(
            deserialized.custom_gcode_per_z[0].1,
            "M600 ; filament change"
        );
    }

    #[test]
    fn toml_deserialization() {
        let toml_str = r#"
after_layer_change = "M117 Layer {layer_num}"
custom_gcode_per_z = [[5.0, "M600"], [10.5, "M0"]]
"#;
        let hooks: CustomGcodeHooks = toml::from_str(toml_str).unwrap();
        assert_eq!(hooks.after_layer_change, "M117 Layer {layer_num}");
        assert_eq!(hooks.custom_gcode_per_z.len(), 2);
        assert!(hooks.before_layer_change.is_empty());
    }
}
