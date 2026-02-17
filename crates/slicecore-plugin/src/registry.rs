//! Plugin registry for managing discovered and loaded plugins.
//!
//! The [`PluginRegistry`] is the central hub for plugin management. It handles
//! discovery (scanning directories for `plugin.toml` manifests), loading
//! (native via `abi_stable`, WASM via `wasmtime`), and lookup (by name).

use std::collections::HashMap;
use std::path::Path;

use slicecore_plugin_api::{InfillRequest, InfillResult, PluginManifest};

use crate::discovery;
use crate::error::PluginSystemError;

/// The kind of plugin (loading mechanism).
///
/// This is the host-side enum, distinct from the FFI-safe
/// [`slicecore_plugin_api::PluginType`] used in manifests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    /// Native dynamic library loaded via `abi_stable`.
    Native,
    /// WebAssembly component loaded via `wasmtime`.
    Wasm,
    /// Built-in plugin (compiled into the host).
    Builtin,
}

/// Information about a registered plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin name.
    pub name: String,
    /// Plugin description.
    pub description: String,
    /// Loading mechanism.
    pub plugin_kind: PluginKind,
    /// Plugin version (from manifest).
    pub version: String,
}

/// Internal trait for infill plugin adapters.
///
/// This is the host-side interface that wraps both native and WASM plugins
/// with a uniform API. It is not FFI-safe -- only used within the host process.
pub trait InfillPluginAdapter: Send + Sync {
    /// Returns the unique name of this infill pattern.
    fn name(&self) -> String;
    /// Returns a human-readable description.
    fn description(&self) -> String;
    /// Generates infill lines for the given request.
    fn generate(&self, request: &InfillRequest) -> Result<InfillResult, PluginSystemError>;
    /// Returns the plugin kind (Native, Wasm, Builtin).
    fn plugin_type(&self) -> PluginKind;
}

/// Central plugin registry managing all loaded plugins.
///
/// Provides discovery, registration, and lookup of infill plugins.
pub struct PluginRegistry {
    /// Loaded infill plugins keyed by name.
    infill_plugins: HashMap<String, Box<dyn InfillPluginAdapter>>,
    /// Discovered manifests (for informational purposes).
    manifests: Vec<PluginManifest>,
}

impl PluginRegistry {
    /// Creates a new empty plugin registry.
    pub fn new() -> Self {
        Self {
            infill_plugins: HashMap::new(),
            manifests: Vec::new(),
        }
    }

    /// Discovers and loads all plugins from a directory.
    ///
    /// Scans the directory for `plugin.toml` manifests, validates version
    /// compatibility, and loads each plugin based on its type (native or WASM).
    ///
    /// Returns information about all successfully loaded plugins.
    #[cfg(not(target_family = "wasm"))]
    pub fn discover_and_load(&mut self, dir: &Path) -> Result<Vec<PluginInfo>, PluginSystemError> {
        let discovered = discovery::discover_plugins(dir)?;
        let mut loaded = Vec::new();

        for (plugin_dir, manifest) in discovered {
            match manifest.plugin_type {
                slicecore_plugin_api::PluginType::Native => {
                    let plugin = crate::native::load_native_plugin(&plugin_dir, &manifest)?;
                    let info = PluginInfo {
                        name: plugin.name(),
                        description: plugin.description(),
                        plugin_kind: PluginKind::Native,
                        version: manifest.metadata.version.clone(),
                    };
                    self.manifests.push(manifest);
                    self.infill_plugins
                        .insert(info.name.clone(), Box::new(plugin));
                    loaded.push(info);
                }
                slicecore_plugin_api::PluginType::Wasm => {
                    // WASM loading will be added in plan 03
                    return Err(PluginSystemError::LoadFailed {
                        path: plugin_dir,
                        reason: "WASM plugin loading not yet implemented".to_string(),
                    });
                }
            }
        }

        Ok(loaded)
    }

    /// Manually registers an infill plugin.
    ///
    /// Useful for built-in plugins or test fixtures.
    pub fn register_infill_plugin(&mut self, plugin: Box<dyn InfillPluginAdapter>) {
        let name = plugin.name();
        self.infill_plugins.insert(name, plugin);
    }

    /// Looks up an infill plugin by name.
    pub fn get_infill_plugin(&self, name: &str) -> Option<&dyn InfillPluginAdapter> {
        self.infill_plugins.get(name).map(|p| p.as_ref())
    }

    /// Returns information about all registered infill plugins.
    pub fn list_infill_plugins(&self) -> Vec<PluginInfo> {
        self.infill_plugins
            .values()
            .map(|plugin| PluginInfo {
                name: plugin.name(),
                description: plugin.description(),
                plugin_kind: plugin.plugin_type(),
                version: String::new(), // Version not tracked on adapter
            })
            .collect()
    }

    /// Checks if an infill plugin with the given name is registered.
    pub fn has_infill_plugin(&self, name: &str) -> bool {
        self.infill_plugins.contains_key(name)
    }

    /// Returns the number of registered infill plugins.
    pub fn infill_plugin_count(&self) -> usize {
        self.infill_plugins.len()
    }

    /// Returns the discovered manifests.
    pub fn manifests(&self) -> &[PluginManifest] {
        &self.manifests
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::RVec;
    use slicecore_plugin_api::InfillResult;

    /// A mock infill plugin for testing.
    struct MockInfillPlugin {
        name: String,
        description: String,
    }

    impl MockInfillPlugin {
        fn new(name: &str, description: &str) -> Self {
            Self {
                name: name.to_string(),
                description: description.to_string(),
            }
        }
    }

    impl InfillPluginAdapter for MockInfillPlugin {
        fn name(&self) -> String {
            self.name.clone()
        }

        fn description(&self) -> String {
            self.description.clone()
        }

        fn generate(&self, _request: &InfillRequest) -> Result<InfillResult, PluginSystemError> {
            Ok(InfillResult { lines: RVec::new() })
        }

        fn plugin_type(&self) -> PluginKind {
            PluginKind::Builtin
        }
    }

    #[test]
    fn registry_new_is_empty() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.infill_plugin_count(), 0);
        assert!(registry.list_infill_plugins().is_empty());
    }

    #[test]
    fn registry_default_is_empty() {
        let registry = PluginRegistry::default();
        assert_eq!(registry.infill_plugin_count(), 0);
    }

    #[test]
    fn registry_register_and_get() {
        let mut registry = PluginRegistry::new();
        let plugin = MockInfillPlugin::new("test-pattern", "A test pattern");
        registry.register_infill_plugin(Box::new(plugin));

        assert!(registry.has_infill_plugin("test-pattern"));
        assert!(!registry.has_infill_plugin("nonexistent"));

        let plugin = registry.get_infill_plugin("test-pattern").unwrap();
        assert_eq!(plugin.name(), "test-pattern");
        assert_eq!(plugin.description(), "A test pattern");
    }

    #[test]
    fn registry_get_nonexistent_returns_none() {
        let registry = PluginRegistry::new();
        assert!(registry.get_infill_plugin("nonexistent").is_none());
    }

    #[test]
    fn registry_list_plugins() {
        let mut registry = PluginRegistry::new();
        registry.register_infill_plugin(Box::new(MockInfillPlugin::new(
            "pattern-a",
            "First pattern",
        )));
        registry.register_infill_plugin(Box::new(MockInfillPlugin::new(
            "pattern-b",
            "Second pattern",
        )));

        let list = registry.list_infill_plugins();
        assert_eq!(list.len(), 2);

        let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"pattern-a"));
        assert!(names.contains(&"pattern-b"));
    }

    #[test]
    fn registry_register_overwrites_existing() {
        let mut registry = PluginRegistry::new();
        registry
            .register_infill_plugin(Box::new(MockInfillPlugin::new("pattern", "First version")));
        registry
            .register_infill_plugin(Box::new(MockInfillPlugin::new("pattern", "Second version")));

        assert_eq!(registry.infill_plugin_count(), 1);
        let plugin = registry.get_infill_plugin("pattern").unwrap();
        assert_eq!(plugin.description(), "Second version");
    }

    #[test]
    fn registry_plugin_generate() {
        let mut registry = PluginRegistry::new();
        registry.register_infill_plugin(Box::new(MockInfillPlugin::new("test", "Test pattern")));

        let plugin = registry.get_infill_plugin("test").unwrap();
        let request = InfillRequest {
            boundary_points: RVec::from(vec![0i64, 0, 100, 0, 100, 100, 0, 100]),
            boundary_lengths: RVec::from(vec![4u32]),
            density: 0.2,
            layer_index: 0,
            layer_z: 0.2,
            line_width: 0.4,
        };

        let result = plugin.generate(&request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().lines.len(), 0);
    }
}
