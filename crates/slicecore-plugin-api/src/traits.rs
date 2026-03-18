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

use crate::types::{CsgMeshData, CsgPrimitiveParams, InfillRequest, InfillResult};

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
    /// Returns the unique name of this infill pattern (e.g., `"zigzag"`).
    ///
    /// This name is used as the identifier in configuration files and as the
    /// lookup key in the [`PluginRegistry`](crate::traits::InfillPluginMod).
    /// It should be lowercase, hyphen-separated, and globally unique among
    /// all loaded plugins.
    fn name(&self) -> RString;

    /// Returns a human-readable description of this infill pattern.
    ///
    /// This is displayed in user interfaces and documentation. It should
    /// briefly describe the visual and structural characteristics of the
    /// generated infill pattern.
    fn description(&self) -> RString;

    /// Generate infill lines for the given boundary region.
    ///
    /// Called once per infill region per layer during the slicing pipeline.
    /// The `request` contains the polygon boundary, fill density, layer
    /// information, and extrusion parameters. All coordinates use the
    /// engine's integer coordinate system (`COORD_SCALE = 1_000_000`,
    /// i.e., 1 unit = 1 nanometer).
    ///
    /// Returns `ROk(InfillResult)` with the generated line segments on
    /// success, or `RErr(RString)` with an error message on failure.
    ///
    /// # Errors
    ///
    /// Should return `RErr` if the boundary is malformed, coordinates are
    /// out of range, or the infill algorithm encounters an unrecoverable
    /// error. The host will report the error and may fall back to a default
    /// infill pattern.
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

// ---------------------------------------------------------------------------
// CSG Operation Plugin
// ---------------------------------------------------------------------------

/// FFI-safe plugin trait for custom CSG (boolean) operations.
///
/// Plugins implementing this trait can create custom mesh primitives and apply
/// boolean operations via the plugin system. The host converts between its
/// internal `TriangleMesh` and [`CsgMeshData`] at the plugin boundary.
///
/// All geometry uses plain `[f64; 3]` vertices and `[u32; 3]` indices for
/// simplicity and FFI safety.
///
/// # Generated Types
///
/// The `#[sabi_trait]` macro generates `CsgOperationPlugin_TO` (trait object
/// wrapper) which can be used as: `CsgOperationPlugin_TO<'static, RBox<()>>`
///
/// # Example
///
/// ```ignore
/// use slicecore_plugin_api::traits::*;
/// use slicecore_plugin_api::{CsgMeshData, CsgPrimitiveParams};
/// use abi_stable::std_types::{RResult, ROk, RString, RVec};
///
/// #[derive(Debug)]
/// struct MyCsgPlugin;
///
/// impl CsgOperationPlugin for MyCsgPlugin {
///     fn name(&self) -> RString { "my-csg".into() }
///
///     fn create_primitive(
///         &self,
///         params: &CsgPrimitiveParams,
///     ) -> RResult<CsgMeshData, RString> {
///         ROk(CsgMeshData {
///             vertices: RVec::new(),
///             indices: RVec::new(),
///         })
///     }
///
///     fn apply_boolean(
///         &self,
///         mesh_a: &CsgMeshData,
///         mesh_b: &CsgMeshData,
///         operation: &RString,
///     ) -> RResult<CsgMeshData, RString> {
///         ROk(CsgMeshData {
///             vertices: mesh_a.vertices.clone(),
///             indices: mesh_a.indices.clone(),
///         })
///     }
/// }
/// ```
#[sabi_trait]
pub trait CsgOperationPlugin: Send + Sync + Debug {
    /// Returns the unique name of this CSG plugin (e.g., `"custom-boolean"`).
    fn name(&self) -> RString;

    /// Creates a custom mesh primitive from the given parameters.
    ///
    /// The returned [`CsgMeshData`] should contain a valid, watertight triangle
    /// mesh. The host may repair minor defects, but degenerate meshes will
    /// cause boolean operations to fail.
    ///
    /// # Errors
    ///
    /// Returns `RErr(RString)` if the primitive type is unsupported or
    /// the parameters are invalid.
    fn create_primitive(&self, params: &CsgPrimitiveParams) -> RResult<CsgMeshData, RString>;

    /// Applies a custom boolean operation to two meshes.
    ///
    /// The `operation` string identifies the operation type (e.g., `"union"`,
    /// `"difference"`, `"intersection"`, `"xor"`, or a plugin-defined operation).
    ///
    /// # Errors
    ///
    /// Returns `RErr(RString)` if the operation is unsupported or fails.
    #[sabi(last_prefix_field)]
    fn apply_boolean(
        &self,
        mesh_a: &CsgMeshData,
        mesh_b: &CsgMeshData,
        operation: &RString,
    ) -> RResult<CsgMeshData, RString>;
}

/// The root module entry point for native CSG plugins.
///
/// Each native CSG plugin crate exports an instance of this struct via
/// `#[export_root_module]`. The host calls the `new` function to create
/// plugin instances.
#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix))]
pub struct CsgPluginMod {
    /// Factory function to create a new CSG plugin instance.
    #[sabi(last_prefix_field)]
    pub new: extern "C" fn() -> CsgOperationPlugin_TO<'static, RBox<()>>,
}

impl RootModule for CsgPluginMod_Ref {
    abi_stable::declare_root_module_statics! { CsgPluginMod_Ref }

    const BASE_NAME: &'static str = "slicecore_csg_plugin";
    const NAME: &'static str = "slicecore_csg_plugin";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::InfillResult;
    use abi_stable::std_types::{ROk, RResult, RVec};

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
            ROk(InfillResult { lines: RVec::new() })
        }
    }

    #[test]
    fn test_plugin_trait_object_creation() {
        let plugin = TestInfillPlugin;
        let trait_obj =
            InfillPatternPlugin_TO::from_value(plugin, abi_stable::sabi_trait::TD_Opaque);
        assert_eq!(trait_obj.name().as_str(), "test-pattern");
        assert_eq!(
            trait_obj.description().as_str(),
            "A test infill pattern for unit tests"
        );
    }

    #[test]
    fn test_plugin_generate_empty_result() {
        let plugin = TestInfillPlugin;
        let trait_obj =
            InfillPatternPlugin_TO::from_value(plugin, abi_stable::sabi_trait::TD_Opaque);

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

    // -----------------------------------------------------------------------
    // CSG plugin tests
    // -----------------------------------------------------------------------

    /// A test implementation of CsgOperationPlugin.
    #[derive(Debug)]
    struct TestCsgPlugin;

    impl CsgOperationPlugin for TestCsgPlugin {
        fn name(&self) -> RString {
            "test-csg".into()
        }

        fn create_primitive(&self, _params: &CsgPrimitiveParams) -> RResult<CsgMeshData, RString> {
            ROk(CsgMeshData {
                vertices: RVec::from(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
                indices: RVec::from(vec![[0u32, 1, 2]]),
            })
        }

        fn apply_boolean(
            &self,
            mesh_a: &CsgMeshData,
            _mesh_b: &CsgMeshData,
            _operation: &RString,
        ) -> RResult<CsgMeshData, RString> {
            // Trivial: return mesh_a unchanged.
            ROk(CsgMeshData {
                vertices: mesh_a.vertices.clone(),
                indices: mesh_a.indices.clone(),
            })
        }
    }

    #[test]
    fn test_csg_plugin_trait_object_creation() {
        let plugin = TestCsgPlugin;
        let trait_obj =
            CsgOperationPlugin_TO::from_value(plugin, abi_stable::sabi_trait::TD_Opaque);
        assert_eq!(trait_obj.name().as_str(), "test-csg");
    }

    #[test]
    fn test_csg_plugin_create_primitive() {
        let plugin = TestCsgPlugin;
        let trait_obj =
            CsgOperationPlugin_TO::from_value(plugin, abi_stable::sabi_trait::TD_Opaque);

        let params = CsgPrimitiveParams {
            primitive_type: "box".into(),
            dimensions: RVec::from(vec![1.0_f64, 1.0, 1.0]),
            segments: 1,
        };

        match trait_obj.create_primitive(&params) {
            ROk(mesh) => {
                assert_eq!(mesh.vertices.len(), 3);
                assert_eq!(mesh.indices.len(), 1);
            }
            abi_stable::std_types::RErr(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn test_csg_plugin_apply_boolean() {
        let plugin = TestCsgPlugin;
        let trait_obj =
            CsgOperationPlugin_TO::from_value(plugin, abi_stable::sabi_trait::TD_Opaque);

        let mesh = CsgMeshData {
            vertices: RVec::from(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
            indices: RVec::from(vec![[0u32, 1, 2]]),
        };

        let op: RString = "union".into();
        match trait_obj.apply_boolean(&mesh, &mesh, &op) {
            ROk(result) => assert_eq!(result.vertices.len(), 3),
            abi_stable::std_types::RErr(e) => panic!("unexpected error: {e}"),
        }
    }
}
