//! Plugin metadata and manifest structures.
//!
//! These types describe plugin identity, capabilities, and configuration.
//! They are plain serde-serializable types (not FFI-safe) used for plugin
//! discovery and manifest parsing before a plugin is loaded.
//!
//! Plugin manifests are typically stored as `plugin.toml` files alongside
//! the plugin binary.

use serde::{Deserialize, Serialize};

/// Metadata identifying a plugin.
///
/// Contains human-readable information and version compatibility requirements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMetadata {
    /// Unique plugin name (e.g., "zigzag-infill").
    pub name: String,
    /// Semantic version of the plugin (e.g., "1.0.0").
    pub version: String,
    /// Human-readable description of the plugin's functionality.
    pub description: String,
    /// Plugin author name or organization.
    pub author: String,
    /// SPDX license identifier (e.g., "MIT", "Apache-2.0").
    pub license: String,
    /// Minimum compatible API version (e.g., "0.1.0").
    pub min_api_version: String,
    /// Maximum compatible API version (e.g., "0.2.0").
    pub max_api_version: String,
}

/// Extension point capability that a plugin provides.
///
/// Currently only infill pattern generation is supported. Additional
/// capabilities (G-code post-processing, support strategies, etc.) will
/// be added in future phases.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    /// The plugin provides a custom infill pattern.
    InfillPattern,
}

/// Plugin type indicating the loading mechanism.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    /// Native dynamic library loaded via `abi_stable`.
    Native,
    /// WebAssembly component loaded via `wasmtime`.
    Wasm,
}

/// Resource limits for sandboxed (WASM) plugins.
///
/// Native plugins run in the host process and cannot be resource-limited
/// at the same granularity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Maximum memory allocation in megabytes.
    pub max_memory_mb: u64,
    /// Maximum CPU fuel (wasmtime fuel units) for execution budgeting.
    pub max_cpu_fuel: u64,
}

/// Complete plugin manifest describing a plugin's identity, type, and capabilities.
///
/// Parsed from `plugin.toml` files during plugin discovery. The manifest is
/// read and validated before the plugin binary is loaded, allowing early
/// rejection of incompatible plugins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    /// Plugin identity and version information.
    pub metadata: PluginMetadata,
    /// Loading mechanism (native or WASM).
    pub plugin_type: PluginType,
    /// Filename of the plugin library (e.g., "libzigzag.so" or "zigzag.wasm").
    pub library_filename: String,
    /// Extension points this plugin implements.
    pub capabilities: Vec<PluginCapability>,
    /// Optional resource limits (primarily for WASM plugins).
    pub resource_limits: Option<ResourceLimits>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> PluginManifest {
        PluginManifest {
            metadata: PluginMetadata {
                name: "zigzag-infill".to_string(),
                version: "1.0.0".to_string(),
                description: "Zigzag infill pattern with configurable angle".to_string(),
                author: "Test Author".to_string(),
                license: "MIT".to_string(),
                min_api_version: "0.1.0".to_string(),
                max_api_version: "0.2.0".to_string(),
            },
            plugin_type: PluginType::Native,
            library_filename: "libzigzag_infill.so".to_string(),
            capabilities: vec![PluginCapability::InfillPattern],
            resource_limits: None,
        }
    }

    fn sample_wasm_manifest() -> PluginManifest {
        PluginManifest {
            metadata: PluginMetadata {
                name: "spiral-infill".to_string(),
                version: "0.1.0".to_string(),
                description: "Spiral infill pattern".to_string(),
                author: "Test Author".to_string(),
                license: "Apache-2.0".to_string(),
                min_api_version: "0.1.0".to_string(),
                max_api_version: "0.1.0".to_string(),
            },
            plugin_type: PluginType::Wasm,
            library_filename: "spiral_infill.wasm".to_string(),
            capabilities: vec![PluginCapability::InfillPattern],
            resource_limits: Some(ResourceLimits {
                max_memory_mb: 64,
                max_cpu_fuel: 1_000_000,
            }),
        }
    }

    #[test]
    fn metadata_serde_json_roundtrip() {
        let manifest = sample_manifest();
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let deserialized: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, deserialized);
    }

    #[test]
    fn wasm_manifest_serde_json_roundtrip() {
        let manifest = sample_wasm_manifest();
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let deserialized: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, deserialized);
    }

    #[test]
    fn plugin_capability_serde() {
        let cap = PluginCapability::InfillPattern;
        let json = serde_json::to_string(&cap).unwrap();
        assert_eq!(json, "\"infill_pattern\"");
        let deserialized: PluginCapability = serde_json::from_str(&json).unwrap();
        assert_eq!(cap, deserialized);
    }

    #[test]
    fn plugin_type_serde() {
        let native = PluginType::Native;
        let wasm = PluginType::Wasm;
        assert_eq!(serde_json::to_string(&native).unwrap(), "\"native\"");
        assert_eq!(serde_json::to_string(&wasm).unwrap(), "\"wasm\"");
    }

    #[test]
    fn resource_limits_serde_roundtrip() {
        let limits = ResourceLimits {
            max_memory_mb: 128,
            max_cpu_fuel: 5_000_000,
        };
        let json = serde_json::to_string(&limits).unwrap();
        let deserialized: ResourceLimits = serde_json::from_str(&json).unwrap();
        assert_eq!(limits, deserialized);
    }

    #[test]
    fn manifest_resource_limits_none_serde() {
        let manifest = sample_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        assert!(json.contains("\"resource_limits\":null"));
        let deserialized: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.resource_limits, None);
    }
}
