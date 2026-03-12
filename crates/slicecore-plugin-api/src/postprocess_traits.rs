//! FFI-safe post-processor plugin trait using `abi_stable`'s `sabi_trait` macro.
//!
//! The [`GcodePostProcessorPlugin`] trait defines the interface that all G-code
//! post-processor plugins must implement. The `#[sabi_trait]` attribute generates
//! FFI-safe trait object types:
//!
//! - `GcodePostProcessorPlugin_TO<'_, Pointer>` -- FFI-safe trait object wrapper
//! - `GcodePostProcessorPlugin_CTO<'_, Pointer>` -- const trait object
//!
//! The [`PostProcessorPluginMod`] struct defines the `RootModule` entry point
//! for native post-processor plugins.

use abi_stable::library::RootModule;
use abi_stable::package_version_strings;
use abi_stable::sabi_trait;
use abi_stable::sabi_types::version::VersionStrings;
use abi_stable::std_types::{RBox, RResult, RString};
use abi_stable::StableAbi;

use crate::postprocess_types::{
    LayerPostProcessRequest, PostProcessRequest, PostProcessResult, ProcessingMode,
};

/// FFI-safe plugin trait for G-code post-processing.
///
/// Plugins implement this trait to modify G-code after slicing is complete.
/// The `#[sabi_trait]` attribute generates FFI-safe trait object wrappers
/// that can safely cross the dynamic library boundary.
///
/// # Generated Types
///
/// The macro generates `GcodePostProcessorPlugin_TO` (trait object wrapper)
/// which can be used as: `GcodePostProcessorPlugin_TO<'static, RBox<()>>`
///
/// # Example
///
/// ```ignore
/// use slicecore_plugin_api::postprocess_traits::*;
/// use slicecore_plugin_api::postprocess_types::*;
/// use abi_stable::std_types::{RResult, ROk, RString};
///
/// struct MyPostProcessor;
///
/// impl GcodePostProcessorPlugin for MyPostProcessor {
///     fn name(&self) -> RString { "my-postprocessor".into() }
///     fn description(&self) -> RString { "My custom post-processor".into() }
///     fn process_all(&self, request: &PostProcessRequest) -> RResult<PostProcessResult, RString> {
///         ROk(PostProcessResult { commands: request.commands.clone() })
///     }
///     fn process_layer(&self, request: &LayerPostProcessRequest) -> RResult<PostProcessResult, RString> {
///         ROk(PostProcessResult { commands: request.commands.clone() })
///     }
///     fn processing_mode(&self) -> ProcessingMode {
///         ProcessingMode::All
///     }
/// }
/// ```
#[sabi_trait]
pub trait GcodePostProcessorPlugin: Send + Sync + Debug {
    /// Returns the unique name of this post-processor plugin.
    fn name(&self) -> RString;

    /// Returns a human-readable description of this post-processor.
    fn description(&self) -> RString;

    /// Process all G-code commands at once.
    ///
    /// Called when [`processing_mode()`](Self::processing_mode) returns
    /// [`ProcessingMode::All`] or [`ProcessingMode::Both`].
    ///
    /// # Errors
    ///
    /// Returns `RErr` if the post-processing fails.
    fn process_all(&self, request: &PostProcessRequest) -> RResult<PostProcessResult, RString>;

    /// Process G-code commands for a single layer.
    ///
    /// Called when [`processing_mode()`](Self::processing_mode) returns
    /// [`ProcessingMode::PerLayer`] or [`ProcessingMode::Both`].
    ///
    /// # Errors
    ///
    /// Returns `RErr` if the post-processing fails.
    fn process_layer(
        &self,
        request: &LayerPostProcessRequest,
    ) -> RResult<PostProcessResult, RString>;

    /// Returns the processing mode for this plugin.
    ///
    /// Determines whether the host calls [`process_all()`](Self::process_all),
    /// [`process_layer()`](Self::process_layer), or both.
    #[sabi(last_prefix_field)]
    fn processing_mode(&self) -> ProcessingMode;
}

/// The root module entry point for native post-processor plugins.
///
/// Each native post-processor plugin crate exports an instance of this struct
/// via `#[export_root_module]`. The host uses [`RootModule::load_from_directory`]
/// to load the plugin and call `new_plugin` to create plugin instances.
#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix))]
pub struct PostProcessorPluginMod {
    /// Factory function to create a new post-processor plugin instance.
    #[sabi(last_prefix_field)]
    pub new_plugin: extern "C" fn() -> GcodePostProcessorPlugin_TO<'static, RBox<()>>,
}

impl RootModule for PostProcessorPluginMod_Ref {
    abi_stable::declare_root_module_statics! { PostProcessorPluginMod_Ref }

    const BASE_NAME: &'static str = "slicecore_postprocessor_plugin";
    const NAME: &'static str = "slicecore_postprocessor_plugin";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::{ROk, RVec};

    /// A test implementation of GcodePostProcessorPlugin.
    #[derive(Debug)]
    struct TestPostProcessor;

    impl GcodePostProcessorPlugin for TestPostProcessor {
        fn name(&self) -> RString {
            "test-postprocessor".into()
        }

        fn description(&self) -> RString {
            "A test post-processor for unit tests".into()
        }

        fn process_all(&self, request: &PostProcessRequest) -> RResult<PostProcessResult, RString> {
            ROk(PostProcessResult {
                commands: request.commands.clone(),
            })
        }

        fn process_layer(
            &self,
            request: &LayerPostProcessRequest,
        ) -> RResult<PostProcessResult, RString> {
            ROk(PostProcessResult {
                commands: request.commands.clone(),
            })
        }

        fn processing_mode(&self) -> ProcessingMode {
            ProcessingMode::All
        }
    }

    #[test]
    fn test_postprocessor_trait_object_creation() {
        let plugin = TestPostProcessor;
        let trait_obj =
            GcodePostProcessorPlugin_TO::from_value(plugin, abi_stable::sabi_trait::TD_Opaque);
        assert_eq!(trait_obj.name().as_str(), "test-postprocessor");
        assert_eq!(
            trait_obj.description().as_str(),
            "A test post-processor for unit tests"
        );
        assert_eq!(trait_obj.processing_mode(), ProcessingMode::All);
    }

    #[test]
    fn test_postprocessor_process_all() {
        let plugin = TestPostProcessor;
        let trait_obj =
            GcodePostProcessorPlugin_TO::from_value(plugin, abi_stable::sabi_trait::TD_Opaque);

        let config = crate::postprocess_types::FfiPrintConfigSnapshot {
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
        };

        let request = PostProcessRequest {
            commands: RVec::from(vec![crate::postprocess_types::FfiGcodeCommand::Comment(
                RString::from("test"),
            )]),
            config,
            params: RVec::new(),
        };

        match trait_obj.process_all(&request) {
            ROk(result) => assert_eq!(result.commands.len(), 1),
            abi_stable::std_types::RErr(e) => panic!("unexpected error: {e}"),
        }
    }
}
