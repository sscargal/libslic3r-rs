//! Built-in post-processor plugins for the G-code pipeline.
//!
//! Four built-in post-processors are provided:
//!
//! - [`PauseAtLayerPlugin`]: Inserts pause commands (M0/M600) at specified layers
//! - [`TimelapseCameraPlugin`]: Inserts retract-park-dwell-unretract for camera snapshots
//! - [`FanSpeedOverridePlugin`]: Overrides fan speed within specified layer ranges
//! - [`CustomGcodeInjectionPlugin`]: Injects raw G-code at configurable trigger points
//!
//! All plugins implement [`PostProcessorPluginAdapter`] with `PluginKind::Builtin`
//! and self-skip when their configuration is empty or disabled.
//!
//! Use [`create_builtin_postprocessors`] to create all enabled built-in
//! post-processors from a [`PostProcessConfig`].

use slicecore_gcode_io::GcodeCommand;
use slicecore_plugin::postprocess::PostProcessorPluginAdapter;
use slicecore_plugin::registry::PluginKind;
use slicecore_plugin_api::{FfiPrintConfigSnapshot, ProcessingMode};

use crate::config::{CustomGcodeTrigger, PostProcessConfig};

// ---------------------------------------------------------------------------
// PauseAtLayerPlugin
// ---------------------------------------------------------------------------

/// Inserts a pause command at specified layer changes.
///
/// When the G-code stream contains a layer comment matching one of the
/// configured layer indices, the plugin inserts the configured pause
/// command (e.g. `M0` or `M600`) after the layer change comment.
///
/// Returns the input unchanged if no layers are specified.
#[derive(Debug)]
pub struct PauseAtLayerPlugin {
    layers: Vec<usize>,
    pause_command: String,
}

impl PauseAtLayerPlugin {
    /// Creates a new `PauseAtLayerPlugin` from the post-process configuration.
    pub fn from_config(config: &PostProcessConfig) -> Self {
        Self {
            layers: config.pause_at_layers.clone(),
            pause_command: config.pause_command.clone(),
        }
    }
}

impl PostProcessorPluginAdapter for PauseAtLayerPlugin {
    fn name(&self) -> String {
        "pause_at_layer".to_string()
    }

    fn description(&self) -> String {
        format!(
            "Inserts {} at layers: {:?}",
            self.pause_command, self.layers
        )
    }

    fn priority(&self) -> i32 {
        50
    }

    fn process(
        &self,
        commands: &[GcodeCommand],
        _config: &FfiPrintConfigSnapshot,
    ) -> Result<Vec<GcodeCommand>, slicecore_plugin::error::PluginSystemError> {
        if self.layers.is_empty() {
            return Ok(commands.to_vec());
        }

        let mut result = Vec::with_capacity(commands.len());
        let mut current_layer: usize = 0;

        for cmd in commands {
            // Track layer changes via layer comments.
            if let GcodeCommand::Comment(text) = cmd {
                if text.starts_with("Layer ") {
                    if let Some(num_str) = text
                        .strip_prefix("Layer ")
                        .and_then(|s| s.split_whitespace().next())
                    {
                        if let Ok(layer) = num_str.parse::<usize>() {
                            current_layer = layer;
                        }
                    }
                }
            }

            result.push(cmd.clone());

            // Insert pause after the layer comment if this layer is in the list.
            if let GcodeCommand::Comment(text) = cmd {
                if text.starts_with("Layer ")
                    && self.layers.contains(&current_layer)
                {
                    result.push(GcodeCommand::Comment(format!(
                        "Pause at layer {}",
                        current_layer
                    )));
                    result.push(GcodeCommand::Raw(self.pause_command.clone()));
                }
            }
        }

        Ok(result)
    }

    fn processing_mode(&self) -> ProcessingMode {
        ProcessingMode::All
    }

    fn plugin_type(&self) -> PluginKind {
        PluginKind::Builtin
    }
}

// ---------------------------------------------------------------------------
// TimelapseCameraPlugin
// ---------------------------------------------------------------------------

/// Inserts timelapse camera snapshot sequences at every layer change.
///
/// At each layer change, inserts:
/// 1. Retract filament
/// 2. Rapid move to park position
/// 3. Dwell for camera capture
/// 4. Rapid move back to last known position
/// 5. Unretract filament
///
/// Returns the input unchanged if timelapse is not enabled.
#[derive(Debug)]
pub struct TimelapseCameraPlugin {
    enabled: bool,
    park_x: f64,
    park_y: f64,
    dwell_ms: u32,
    retract_distance: f64,
    retract_speed: f64,
}

impl TimelapseCameraPlugin {
    /// Creates a new `TimelapseCameraPlugin` from the post-process configuration.
    pub fn from_config(config: &PostProcessConfig) -> Self {
        Self {
            enabled: config.timelapse.enabled,
            park_x: config.timelapse.park_x,
            park_y: config.timelapse.park_y,
            dwell_ms: config.timelapse.dwell_ms,
            retract_distance: config.timelapse.retract_distance,
            retract_speed: config.timelapse.retract_speed,
        }
    }
}

impl PostProcessorPluginAdapter for TimelapseCameraPlugin {
    fn name(&self) -> String {
        "timelapse_camera".to_string()
    }

    fn description(&self) -> String {
        format!(
            "Timelapse camera: park at ({}, {}), dwell {}ms",
            self.park_x, self.park_y, self.dwell_ms
        )
    }

    fn priority(&self) -> i32 {
        60
    }

    fn process(
        &self,
        commands: &[GcodeCommand],
        _config: &FfiPrintConfigSnapshot,
    ) -> Result<Vec<GcodeCommand>, slicecore_plugin::error::PluginSystemError> {
        if !self.enabled {
            return Ok(commands.to_vec());
        }

        let mut result = Vec::with_capacity(commands.len());
        let mut last_x: f64 = 0.0;
        let mut last_y: f64 = 0.0;

        for cmd in commands {
            // Track last known XY position from moves.
            match cmd {
                GcodeCommand::LinearMove { x, y, .. }
                | GcodeCommand::RapidMove { x, y, .. } => {
                    if let Some(xv) = x {
                        last_x = *xv;
                    }
                    if let Some(yv) = y {
                        last_y = *yv;
                    }
                }
                _ => {}
            }

            result.push(cmd.clone());

            // Insert timelapse sequence after layer comments.
            if let GcodeCommand::Comment(text) = cmd {
                if text.starts_with("Layer ") {
                    // 1. Retract
                    result.push(GcodeCommand::Retract {
                        distance: self.retract_distance,
                        feedrate: self.retract_speed,
                    });
                    // 2. Park
                    result.push(GcodeCommand::RapidMove {
                        x: Some(self.park_x),
                        y: Some(self.park_y),
                        z: None,
                        f: None,
                    });
                    // 3. Dwell
                    result.push(GcodeCommand::Dwell {
                        ms: self.dwell_ms,
                    });
                    // 4. Return to last position
                    result.push(GcodeCommand::RapidMove {
                        x: Some(last_x),
                        y: Some(last_y),
                        z: None,
                        f: None,
                    });
                    // 5. Unretract
                    result.push(GcodeCommand::Unretract {
                        distance: self.retract_distance,
                        feedrate: self.retract_speed,
                    });
                }
            }
        }

        Ok(result)
    }

    fn processing_mode(&self) -> ProcessingMode {
        ProcessingMode::All
    }

    fn plugin_type(&self) -> PluginKind {
        PluginKind::Builtin
    }
}

// ---------------------------------------------------------------------------
// FanSpeedOverridePlugin
// ---------------------------------------------------------------------------

/// Overrides fan speed commands within specified layer ranges.
///
/// When a `SetFanSpeed` command is encountered within a rule's layer range,
/// the fan speed value is replaced with the rule's configured value.
///
/// Returns the input unchanged if no rules are configured.
#[derive(Debug)]
pub struct FanSpeedOverridePlugin {
    rules: Vec<crate::config::FanOverrideRule>,
}

impl FanSpeedOverridePlugin {
    /// Creates a new `FanSpeedOverridePlugin` from the post-process configuration.
    pub fn from_config(config: &PostProcessConfig) -> Self {
        Self {
            rules: config.fan_overrides.clone(),
        }
    }
}

impl PostProcessorPluginAdapter for FanSpeedOverridePlugin {
    fn name(&self) -> String {
        "fan_speed_override".to_string()
    }

    fn description(&self) -> String {
        format!("Fan speed override: {} rules", self.rules.len())
    }

    fn priority(&self) -> i32 {
        70
    }

    fn process(
        &self,
        commands: &[GcodeCommand],
        _config: &FfiPrintConfigSnapshot,
    ) -> Result<Vec<GcodeCommand>, slicecore_plugin::error::PluginSystemError> {
        if self.rules.is_empty() {
            return Ok(commands.to_vec());
        }

        let mut result = Vec::with_capacity(commands.len());
        let mut current_layer: usize = 0;

        for cmd in commands {
            // Track layer changes.
            if let GcodeCommand::Comment(text) = cmd {
                if text.starts_with("Layer ") {
                    if let Some(num_str) = text
                        .strip_prefix("Layer ")
                        .and_then(|s| s.split_whitespace().next())
                    {
                        if let Ok(layer) = num_str.parse::<usize>() {
                            current_layer = layer;
                        }
                    }
                }
            }

            // Check if this is a fan speed command within a rule's range.
            if let GcodeCommand::SetFanSpeed(_) = cmd {
                let mut overridden = false;
                for rule in &self.rules {
                    let in_range = current_layer >= rule.start_layer
                        && rule
                            .end_layer
                            .map_or(true, |end| current_layer <= end);
                    if in_range {
                        result.push(GcodeCommand::SetFanSpeed(rule.fan_speed));
                        overridden = true;
                        break;
                    }
                }
                if !overridden {
                    result.push(cmd.clone());
                }
            } else {
                result.push(cmd.clone());
            }
        }

        Ok(result)
    }

    fn processing_mode(&self) -> ProcessingMode {
        ProcessingMode::All
    }

    fn plugin_type(&self) -> PluginKind {
        PluginKind::Builtin
    }
}

// ---------------------------------------------------------------------------
// CustomGcodeInjectionPlugin
// ---------------------------------------------------------------------------

/// Injects raw G-code at configurable trigger points.
///
/// Supports four trigger types:
/// - `EveryNLayers`: Inject after layer change every N layers
/// - `AtLayers`: Inject after layer change at specific layer indices
/// - `BeforeRetraction`: Inject before each `Retract` command
/// - `AfterRetraction`: Inject after each `Unretract` command
///
/// Returns the input unchanged if no rules are configured.
#[derive(Debug)]
pub struct CustomGcodeInjectionPlugin {
    rules: Vec<crate::config::CustomGcodeRule>,
}

impl CustomGcodeInjectionPlugin {
    /// Creates a new `CustomGcodeInjectionPlugin` from the post-process configuration.
    pub fn from_config(config: &PostProcessConfig) -> Self {
        Self {
            rules: config.custom_gcode.clone(),
        }
    }

    /// Converts a multi-line G-code string into individual `Raw` commands.
    fn gcode_to_commands(gcode: &str) -> Vec<GcodeCommand> {
        gcode
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| GcodeCommand::Raw(l.to_string()))
            .collect()
    }
}

impl PostProcessorPluginAdapter for CustomGcodeInjectionPlugin {
    fn name(&self) -> String {
        "custom_gcode_injection".to_string()
    }

    fn description(&self) -> String {
        format!("Custom G-code injection: {} rules", self.rules.len())
    }

    fn priority(&self) -> i32 {
        80
    }

    fn process(
        &self,
        commands: &[GcodeCommand],
        _config: &FfiPrintConfigSnapshot,
    ) -> Result<Vec<GcodeCommand>, slicecore_plugin::error::PluginSystemError> {
        if self.rules.is_empty() {
            return Ok(commands.to_vec());
        }

        let mut result = Vec::with_capacity(commands.len());
        let mut current_layer: usize = 0;

        for cmd in commands {
            // Track layer changes.
            if let GcodeCommand::Comment(text) = cmd {
                if text.starts_with("Layer ") {
                    if let Some(num_str) = text
                        .strip_prefix("Layer ")
                        .and_then(|s| s.split_whitespace().next())
                    {
                        if let Ok(layer) = num_str.parse::<usize>() {
                            current_layer = layer;
                        }
                    }
                }
            }

            // BeforeRetraction: inject before Retract commands.
            if matches!(cmd, GcodeCommand::Retract { .. }) {
                for rule in &self.rules {
                    if matches!(rule.trigger, CustomGcodeTrigger::BeforeRetraction) {
                        result.extend(Self::gcode_to_commands(&rule.gcode));
                    }
                }
            }

            result.push(cmd.clone());

            // AfterRetraction: inject after Unretract commands.
            if matches!(cmd, GcodeCommand::Unretract { .. }) {
                for rule in &self.rules {
                    if matches!(rule.trigger, CustomGcodeTrigger::AfterRetraction) {
                        result.extend(Self::gcode_to_commands(&rule.gcode));
                    }
                }
            }

            // Layer-based triggers: inject after layer comments.
            if let GcodeCommand::Comment(text) = cmd {
                if text.starts_with("Layer ") {
                    for rule in &self.rules {
                        let should_inject = match &rule.trigger {
                            CustomGcodeTrigger::EveryNLayers { n } => {
                                *n > 0 && current_layer % n == 0
                            }
                            CustomGcodeTrigger::AtLayers { layers } => {
                                layers.contains(&current_layer)
                            }
                            _ => false,
                        };
                        if should_inject {
                            result.extend(Self::gcode_to_commands(&rule.gcode));
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    fn processing_mode(&self) -> ProcessingMode {
        ProcessingMode::All
    }

    fn plugin_type(&self) -> PluginKind {
        PluginKind::Builtin
    }
}

// ---------------------------------------------------------------------------
// Factory function
// ---------------------------------------------------------------------------

/// Creates all enabled built-in post-processors from configuration.
///
/// Returns an empty `Vec` when post-processing is disabled or all
/// individual features are unconfigured, enabling self-skip behavior
/// at the pipeline level.
///
/// # Examples
///
/// ```
/// use slicecore_engine::config::PostProcessConfig;
/// use slicecore_engine::postprocess_builtin::create_builtin_postprocessors;
///
/// let config = PostProcessConfig::default();
/// let plugins = create_builtin_postprocessors(&config);
/// assert!(plugins.is_empty(), "default config produces no plugins");
/// ```
pub fn create_builtin_postprocessors(
    config: &PostProcessConfig,
) -> Vec<Box<dyn PostProcessorPluginAdapter>> {
    let mut plugins: Vec<Box<dyn PostProcessorPluginAdapter>> = Vec::new();

    if !config.pause_at_layers.is_empty() {
        plugins.push(Box::new(PauseAtLayerPlugin::from_config(config)));
    }

    if config.timelapse.enabled {
        plugins.push(Box::new(TimelapseCameraPlugin::from_config(config)));
    }

    if !config.fan_overrides.is_empty() {
        plugins.push(Box::new(FanSpeedOverridePlugin::from_config(config)));
    }

    if !config.custom_gcode.is_empty() {
        plugins.push(Box::new(CustomGcodeInjectionPlugin::from_config(config)));
    }

    plugins
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CustomGcodeRule, CustomGcodeTrigger, FanOverrideRule, TimelapseConfig};

    fn test_config_snapshot() -> FfiPrintConfigSnapshot {
        FfiPrintConfigSnapshot {
            nozzle_diameter: 0.4,
            layer_height: 0.2,
            first_layer_height: 0.3,
            bed_x: 220.0,
            bed_y: 220.0,
            print_speed: 60.0,
            travel_speed: 120.0,
            retract_length: 0.8,
            retract_speed: 45.0,
            nozzle_temp: 200.0,
            bed_temp: 60.0,
            fan_speed: 255,
            total_layers: 100,
        }
    }

    fn layer_comment(layer: usize) -> GcodeCommand {
        GcodeCommand::Comment(format!("Layer {} at Z={:.3}", layer, (layer as f64) * 0.2))
    }

    // ---- PauseAtLayerPlugin tests ----

    #[test]
    fn pause_self_skip_empty_layers() {
        let config = PostProcessConfig::default();
        let plugin = PauseAtLayerPlugin::from_config(&config);
        let commands = vec![layer_comment(0), layer_comment(1)];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();
        assert_eq!(result, commands, "should return unchanged when no layers specified");
    }

    #[test]
    fn pause_inserts_at_specified_layer() {
        let mut config = PostProcessConfig::default();
        config.pause_at_layers = vec![1];
        config.pause_command = "M600".to_string();

        let plugin = PauseAtLayerPlugin::from_config(&config);
        let commands = vec![layer_comment(0), layer_comment(1), layer_comment(2)];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();

        // Layer 0: comment only. Layer 1: comment + pause comment + M600. Layer 2: comment only.
        assert_eq!(result.len(), 5);
        assert!(matches!(&result[2], GcodeCommand::Comment(t) if t.contains("Pause at layer 1")));
        assert_eq!(result[3], GcodeCommand::Raw("M600".to_string()));
    }

    // ---- TimelapseCameraPlugin tests ----

    #[test]
    fn timelapse_self_skip_when_disabled() {
        let config = PostProcessConfig::default();
        let plugin = TimelapseCameraPlugin::from_config(&config);
        let commands = vec![layer_comment(0)];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();
        assert_eq!(result, commands);
    }

    #[test]
    fn timelapse_inserts_sequence_at_layer_change() {
        let mut config = PostProcessConfig::default();
        config.timelapse = TimelapseConfig {
            enabled: true,
            park_x: 10.0,
            park_y: 20.0,
            dwell_ms: 300,
            retract_distance: 1.5,
            retract_speed: 3000.0,
        };

        let plugin = TimelapseCameraPlugin::from_config(&config);
        let commands = vec![
            GcodeCommand::LinearMove {
                x: Some(50.0),
                y: Some(60.0),
                z: None,
                e: Some(1.0),
                f: Some(1800.0),
            },
            layer_comment(1),
        ];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();

        // Original 2 commands + 5 timelapse commands
        assert_eq!(result.len(), 7);

        // After layer comment: retract, park, dwell, return, unretract
        assert!(matches!(&result[2], GcodeCommand::Retract { distance, .. } if (*distance - 1.5).abs() < 1e-9));
        assert!(matches!(&result[3], GcodeCommand::RapidMove { x: Some(x), y: Some(y), .. } if (*x - 10.0).abs() < 1e-9 && (*y - 20.0).abs() < 1e-9));
        assert!(matches!(&result[4], GcodeCommand::Dwell { ms: 300 }));
        // Return to last known position (50, 60)
        assert!(matches!(&result[5], GcodeCommand::RapidMove { x: Some(x), y: Some(y), .. } if (*x - 50.0).abs() < 1e-9 && (*y - 60.0).abs() < 1e-9));
        assert!(matches!(&result[6], GcodeCommand::Unretract { distance, .. } if (*distance - 1.5).abs() < 1e-9));
    }

    // ---- FanSpeedOverridePlugin tests ----

    #[test]
    fn fan_override_self_skip_empty_rules() {
        let config = PostProcessConfig::default();
        let plugin = FanSpeedOverridePlugin::from_config(&config);
        let commands = vec![GcodeCommand::SetFanSpeed(200)];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();
        assert_eq!(result, commands);
    }

    #[test]
    fn fan_override_replaces_within_range() {
        let mut config = PostProcessConfig::default();
        config.fan_overrides = vec![FanOverrideRule {
            start_layer: 1,
            end_layer: Some(3),
            fan_speed: 128,
        }];

        let plugin = FanSpeedOverridePlugin::from_config(&config);
        let commands = vec![
            layer_comment(0),
            GcodeCommand::SetFanSpeed(200),
            layer_comment(2),
            GcodeCommand::SetFanSpeed(200),
            layer_comment(5),
            GcodeCommand::SetFanSpeed(200),
        ];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();

        // Layer 0: unchanged (200). Layer 2: overridden (128). Layer 5: unchanged (200).
        assert_eq!(result[1], GcodeCommand::SetFanSpeed(200));
        assert_eq!(result[3], GcodeCommand::SetFanSpeed(128));
        assert_eq!(result[5], GcodeCommand::SetFanSpeed(200));
    }

    #[test]
    fn fan_override_none_end_layer_applies_to_end() {
        let mut config = PostProcessConfig::default();
        config.fan_overrides = vec![FanOverrideRule {
            start_layer: 3,
            end_layer: None,
            fan_speed: 50,
        }];

        let plugin = FanSpeedOverridePlugin::from_config(&config);
        let commands = vec![
            layer_comment(5),
            GcodeCommand::SetFanSpeed(255),
        ];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();
        assert_eq!(result[1], GcodeCommand::SetFanSpeed(50));
    }

    // ---- CustomGcodeInjectionPlugin tests ----

    #[test]
    fn custom_gcode_self_skip_empty_rules() {
        let config = PostProcessConfig::default();
        let plugin = CustomGcodeInjectionPlugin::from_config(&config);
        let commands = vec![layer_comment(0)];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();
        assert_eq!(result, commands);
    }

    #[test]
    fn custom_gcode_every_n_layers() {
        let mut config = PostProcessConfig::default();
        config.custom_gcode = vec![CustomGcodeRule {
            trigger: CustomGcodeTrigger::EveryNLayers { n: 2 },
            gcode: "M400".to_string(),
        }];

        let plugin = CustomGcodeInjectionPlugin::from_config(&config);
        let commands = vec![layer_comment(0), layer_comment(1), layer_comment(2), layer_comment(3)];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();

        // Injection at layers 0 and 2 (every 2 layers)
        assert_eq!(result.len(), 6);
        assert_eq!(result[1], GcodeCommand::Raw("M400".to_string())); // after layer 0
        assert_eq!(result[4], GcodeCommand::Raw("M400".to_string())); // after layer 2
    }

    #[test]
    fn custom_gcode_at_layers() {
        let mut config = PostProcessConfig::default();
        config.custom_gcode = vec![CustomGcodeRule {
            trigger: CustomGcodeTrigger::AtLayers {
                layers: vec![1, 3],
            },
            gcode: "G28 X".to_string(),
        }];

        let plugin = CustomGcodeInjectionPlugin::from_config(&config);
        let commands = vec![layer_comment(0), layer_comment(1), layer_comment(2), layer_comment(3)];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();

        assert_eq!(result.len(), 6);
        assert_eq!(result[2], GcodeCommand::Raw("G28 X".to_string())); // after layer 1
        assert_eq!(result[5], GcodeCommand::Raw("G28 X".to_string())); // after layer 3
    }

    #[test]
    fn custom_gcode_before_retraction() {
        let mut config = PostProcessConfig::default();
        config.custom_gcode = vec![CustomGcodeRule {
            trigger: CustomGcodeTrigger::BeforeRetraction,
            gcode: "M400".to_string(),
        }];

        let plugin = CustomGcodeInjectionPlugin::from_config(&config);
        let commands = vec![
            layer_comment(0),
            GcodeCommand::Retract {
                distance: 0.8,
                feedrate: 2700.0,
            },
        ];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[1], GcodeCommand::Raw("M400".to_string())); // before retract
        assert!(matches!(&result[2], GcodeCommand::Retract { .. }));
    }

    #[test]
    fn custom_gcode_after_retraction() {
        let mut config = PostProcessConfig::default();
        config.custom_gcode = vec![CustomGcodeRule {
            trigger: CustomGcodeTrigger::AfterRetraction,
            gcode: "M400".to_string(),
        }];

        let plugin = CustomGcodeInjectionPlugin::from_config(&config);
        let commands = vec![
            layer_comment(0),
            GcodeCommand::Unretract {
                distance: 0.8,
                feedrate: 2700.0,
            },
        ];
        let result = plugin.process(&commands, &test_config_snapshot()).unwrap();

        assert_eq!(result.len(), 3);
        assert!(matches!(&result[1], GcodeCommand::Unretract { .. }));
        assert_eq!(result[2], GcodeCommand::Raw("M400".to_string())); // after unretract
    }

    // ---- Factory function tests ----

    #[test]
    fn create_builtin_empty_config_returns_empty() {
        let config = PostProcessConfig::default();
        let plugins = create_builtin_postprocessors(&config);
        assert!(plugins.is_empty());
    }

    #[test]
    fn create_builtin_with_pause_layers() {
        let mut config = PostProcessConfig::default();
        config.pause_at_layers = vec![5];
        let plugins = create_builtin_postprocessors(&config);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name(), "pause_at_layer");
    }

    #[test]
    fn create_builtin_with_all_features() {
        let mut config = PostProcessConfig::default();
        config.pause_at_layers = vec![1];
        config.timelapse.enabled = true;
        config.fan_overrides = vec![FanOverrideRule {
            start_layer: 0,
            end_layer: None,
            fan_speed: 128,
        }];
        config.custom_gcode = vec![CustomGcodeRule {
            trigger: CustomGcodeTrigger::EveryNLayers { n: 5 },
            gcode: "M400".to_string(),
        }];

        let plugins = create_builtin_postprocessors(&config);
        assert_eq!(plugins.len(), 4);

        // Verify priority ordering would be correct
        let priorities: Vec<i32> = plugins.iter().map(|p| p.priority()).collect();
        assert_eq!(priorities, vec![50, 60, 70, 80]);
    }

    #[test]
    fn post_process_config_serde_defaults() {
        let config: PostProcessConfig = toml::from_str("").unwrap();
        assert!(!config.enabled);
        assert!(config.pause_at_layers.is_empty());
        assert_eq!(config.pause_command, "M0");
        assert!(!config.timelapse.enabled);
        assert!(config.fan_overrides.is_empty());
        assert!(config.custom_gcode.is_empty());
        assert!(config.plugin_order.is_empty());
    }

    #[test]
    fn post_process_config_roundtrip() {
        let mut config = PostProcessConfig::default();
        config.enabled = true;
        config.pause_at_layers = vec![5, 10];
        config.pause_command = "M600".to_string();

        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: PostProcessConfig = toml::from_str(&toml_str).unwrap();
        assert!(deserialized.enabled);
        assert_eq!(deserialized.pause_at_layers, vec![5, 10]);
        assert_eq!(deserialized.pause_command, "M600");
    }
}
