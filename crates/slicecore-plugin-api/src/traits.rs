//! FFI-safe plugin traits using `abi_stable`'s `sabi_trait` macro.
//!
//! The [`InfillPatternPlugin`] trait defines the interface that all infill
//! pattern plugins must implement. The `#[sabi_trait]` attribute generates
//! FFI-safe trait object types:
//!
//! - `InfillPatternPlugin_TO<'_, Pointer>` -- FFI-safe trait object wrapper
//! - `InfillPatternPlugin_CTO<'_, Pointer>` -- const trait object
//!
//! The [`InfillPluginMod`] struct defines the RootModule entry point for
//! native plugins. Each native plugin exports this module via
//! `#[export_root_module]`, allowing the host to discover and instantiate
//! the plugin.

use abi_stable::library::RootModule;
use abi_stable::package_version_strings;
use abi_stable::sabi_trait;
use abi_stable::sabi_types::version::VersionStrings;
use abi_stable::std_types::{RBox, RResult, RString};
use abi_stable::StableAbi;

use crate::types::{InfillRequest, InfillResult};

/// FFI-safe plugin trait for infill pattern generation.
///
/// Plugins implement this trait to provide custom infill patterns.
/// The `#[sabi_trait]` attribute generates FFI-safe trait object wrappers
/// that can safely cross the dynamic library boundary.
///
/// # Generated Types
///
/// The macro generates `InfillPatternPlugin_TO` (trait object wrapper)
/// which can be used as: `InfillPatternPlugin_TO<'static, RBox<()>>`
///
/// # Example
///
/// ```ignore
/// use slicecore_plugin_api::traits::*;
/// use abi_stable::std_types::{RResult, ROk, RString};
///
/// struct MyInfill;
///
/// impl InfillPatternPlugin for MyInfill {
///     fn name(&self) -> RString { "my-infill".into() }
///     fn description(&self) -> RString { "My custom infill".into() }
///     fn generate(&self, request: &InfillRequest) -> RResult<InfillResult, RString> {
///         ROk(InfillResult { lines: Default::default() })
///     }
/// }
/// ```
#[sabi_trait]
pub trait InfillPatternPlugin: Send + Sync + Debug {
    /// Returns the unique name of this infill pattern (e.g., "zigzag").
    fn name(&self) -> RString;

    /// Returns a human-readable description of this infill pattern.
    fn description(&self) -> RString;

    /// Generate infill lines for the given request.
    ///
    /// Returns `ROk(InfillResult)` on success, or `RErr(RString)` with
    /// an error message on failure.
    #[sabi(last_prefix_field)]
    fn generate(&self, request: &InfillRequest) -> RResult<InfillResult, RString>;
}

/// The root module entry point for native infill plugins.
///
/// Each native plugin crate exports an instance of this struct via
/// `#[export_root_module]`. The host uses [`RootModule::load_from_directory`]
/// to load the plugin and call the `new` function to create plugin instances.
///
/// The `#[sabi(kind(Prefix))]` attribute makes this a prefix type, allowing
/// future fields to be added at the end without breaking binary compatibility.
#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix))]
pub struct InfillPluginMod {
    /// Factory function to create a new plugin instance.
    #[sabi(last_prefix_field)]
    pub new: extern "C" fn() -> InfillPatternPlugin_TO<'static, RBox<()>>,
}

impl RootModule for InfillPluginMod_Ref {
    abi_stable::declare_root_module_statics! { InfillPluginMod_Ref }

    const BASE_NAME: &'static str = "slicecore_infill_plugin";
    const NAME: &'static str = "slicecore_infill_plugin";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::{RResult, ROk, RVec};
    use crate::types::InfillResult;

    /// A test implementation of InfillPatternPlugin.
    #[derive(Debug)]
    struct TestInfillPlugin;

    impl InfillPatternPlugin for TestInfillPlugin {
        fn name(&self) -> RString {
            "test-pattern".into()
        }

        fn description(&self) -> RString {
            "A test infill pattern for unit tests".into()
        }

        fn generate(&self, _request: &InfillRequest) -> RResult<InfillResult, RString> {
            ROk(InfillResult {
                lines: RVec::new(),
            })
        }
    }

    #[test]
    fn test_plugin_trait_object_creation() {
        let plugin = TestInfillPlugin;
        let trait_obj = InfillPatternPlugin_TO::from_value(plugin, abi_stable::sabi_trait::TD_Opaque);
        assert_eq!(trait_obj.name().as_str(), "test-pattern");
        assert_eq!(
            trait_obj.description().as_str(),
            "A test infill pattern for unit tests"
        );
    }

    #[test]
    fn test_plugin_generate_empty_result() {
        let plugin = TestInfillPlugin;
        let trait_obj = InfillPatternPlugin_TO::from_value(plugin, abi_stable::sabi_trait::TD_Opaque);

        let request = InfillRequest {
            boundary_points: RVec::from(vec![0i64, 0, 100, 0, 100, 100, 0, 100]),
            boundary_lengths: RVec::from(vec![4u32]),
            density: 0.2,
            layer_index: 0,
            layer_z: 0.2,
            line_width: 0.4,
        };

        match trait_obj.generate(&request) {
            ROk(result) => assert_eq!(result.lines.len(), 0),
            abi_stable::std_types::RErr(e) => panic!("unexpected error: {}", e),
        }
    }
}
