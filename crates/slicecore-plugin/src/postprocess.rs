//! Post-processor plugin adapter and pipeline runner.
//!
//! The [`PostProcessorPluginAdapter`] trait provides a uniform host-side
//! interface for post-processor plugins regardless of their loading mechanism
//! (native, WASM, or built-in).
//!
//! The [`run_post_processors`] function executes multiple post-processor
//! plugins in priority order, piping the output of each into the next.

use slicecore_gcode_io::GcodeCommand;
use slicecore_plugin_api::{FfiPrintConfigSnapshot, ProcessingMode};

use crate::error::PluginSystemError;
use crate::registry::PluginKind;

/// Host-side adapter trait for post-processor plugins.
///
/// Wraps native, WASM, and built-in post-processor plugins with a
/// uniform API. Not FFI-safe -- only used within the host process.
pub trait PostProcessorPluginAdapter: Send + Sync {
    /// Returns the unique name of this post-processor.
    fn name(&self) -> String;
    /// Returns a human-readable description.
    fn description(&self) -> String;
    /// Returns the execution priority (lower = earlier).
    fn priority(&self) -> i32;
    /// Processes G-code commands through this post-processor.
    ///
    /// # Errors
    ///
    /// Returns [`PluginSystemError`] if the plugin fails to process.
    fn process(
        &self,
        commands: &[GcodeCommand],
        config: &FfiPrintConfigSnapshot,
    ) -> Result<Vec<GcodeCommand>, PluginSystemError>;
    /// Returns the processing mode for this plugin.
    fn processing_mode(&self) -> ProcessingMode;
    /// Returns the plugin kind (Native, Wasm, Builtin).
    fn plugin_type(&self) -> PluginKind;
}

/// Runs multiple post-processor plugins in priority order.
///
/// Plugins are sorted by `(priority, name)` (lower priority number first,
/// stable sort, alphabetical tie-break). Each plugin's output is piped
/// as input to the next.
///
/// Returns the original commands unchanged when the plugin list is empty.
///
/// # Errors
///
/// Returns the first [`PluginSystemError`] encountered during processing.
pub fn run_post_processors(
    commands: Vec<GcodeCommand>,
    plugins: &[&dyn PostProcessorPluginAdapter],
    config: &FfiPrintConfigSnapshot,
) -> Result<Vec<GcodeCommand>, PluginSystemError> {
    if plugins.is_empty() {
        return Ok(commands);
    }

    // Sort by (priority, name) -- stable sort, lower priority first
    let mut sorted: Vec<&dyn PostProcessorPluginAdapter> = plugins.to_vec();
    sorted.sort_by(|a, b| {
        a.priority()
            .cmp(&b.priority())
            .then(a.name().cmp(&b.name()))
    });

    let mut current = commands;
    for plugin in &sorted {
        current = plugin.process(&current, config)?;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock post-processor that appends a comment.
    struct AppendCommentPlugin {
        name: String,
        priority: i32,
        comment: String,
    }

    impl AppendCommentPlugin {
        fn new(name: &str, priority: i32, comment: &str) -> Self {
            Self {
                name: name.to_string(),
                priority,
                comment: comment.to_string(),
            }
        }
    }

    impl PostProcessorPluginAdapter for AppendCommentPlugin {
        fn name(&self) -> String {
            self.name.clone()
        }
        fn description(&self) -> String {
            format!("Appends comment: {}", self.comment)
        }
        fn priority(&self) -> i32 {
            self.priority
        }
        fn process(
            &self,
            commands: &[GcodeCommand],
            _config: &FfiPrintConfigSnapshot,
        ) -> Result<Vec<GcodeCommand>, PluginSystemError> {
            let mut result = commands.to_vec();
            result.push(GcodeCommand::Comment(self.comment.clone()));
            Ok(result)
        }
        fn processing_mode(&self) -> ProcessingMode {
            ProcessingMode::All
        }
        fn plugin_type(&self) -> PluginKind {
            PluginKind::Builtin
        }
    }

    fn test_config() -> FfiPrintConfigSnapshot {
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

    #[test]
    fn empty_plugin_list_returns_original() {
        let commands = vec![
            GcodeCommand::Comment("original".to_string()),
            GcodeCommand::SetAbsolutePositioning,
        ];
        let result = run_post_processors(commands.clone(), &[], &test_config()).unwrap();
        assert_eq!(result, commands);
    }

    #[test]
    fn single_plugin_modifies_commands() {
        let commands = vec![GcodeCommand::Comment("start".to_string())];
        let plugin = AppendCommentPlugin::new("test", 0, "added by plugin");
        let plugins: Vec<&dyn PostProcessorPluginAdapter> = vec![&plugin];
        let result = run_post_processors(commands, &plugins, &test_config()).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[1],
            GcodeCommand::Comment("added by plugin".to_string())
        );
    }

    #[test]
    fn priority_ordering() {
        let commands = vec![GcodeCommand::Comment("start".to_string())];
        let plugin_a = AppendCommentPlugin::new("alpha", 10, "second");
        let plugin_b = AppendCommentPlugin::new("beta", 1, "first");
        let plugins: Vec<&dyn PostProcessorPluginAdapter> = vec![&plugin_a, &plugin_b];
        let result = run_post_processors(commands, &plugins, &test_config()).unwrap();
        // beta (priority 1) runs first, alpha (priority 10) runs second
        assert_eq!(result.len(), 3);
        assert_eq!(result[1], GcodeCommand::Comment("first".to_string()));
        assert_eq!(result[2], GcodeCommand::Comment("second".to_string()));
    }

    #[test]
    fn name_tiebreak_on_same_priority() {
        let commands = vec![GcodeCommand::Comment("start".to_string())];
        let plugin_b = AppendCommentPlugin::new("bravo", 5, "bravo-comment");
        let plugin_a = AppendCommentPlugin::new("alpha", 5, "alpha-comment");
        let plugins: Vec<&dyn PostProcessorPluginAdapter> = vec![&plugin_b, &plugin_a];
        let result = run_post_processors(commands, &plugins, &test_config()).unwrap();
        // Same priority, alphabetical: alpha before bravo
        assert_eq!(result.len(), 3);
        assert_eq!(
            result[1],
            GcodeCommand::Comment("alpha-comment".to_string())
        );
        assert_eq!(
            result[2],
            GcodeCommand::Comment("bravo-comment".to_string())
        );
    }

    #[test]
    fn pipeline_chains_output() {
        let commands = vec![];
        let plugin_1 = AppendCommentPlugin::new("p1", 1, "from p1");
        let plugin_2 = AppendCommentPlugin::new("p2", 2, "from p2");
        let plugins: Vec<&dyn PostProcessorPluginAdapter> = vec![&plugin_1, &plugin_2];
        let result = run_post_processors(commands, &plugins, &test_config()).unwrap();
        // p1 adds "from p1", p2 receives that and adds "from p2"
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], GcodeCommand::Comment("from p1".to_string()));
        assert_eq!(result[1], GcodeCommand::Comment("from p2".to_string()));
    }
}
