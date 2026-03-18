//! Sandbox configuration for WASM plugins.
//!
//! Provides configurable resource limits (memory, CPU fuel) that are applied
//! to each WASM plugin instance. Each `generate()` call gets a fresh
//! [`wasmtime::Store`] with the configured limits, preventing resource
//! accumulation across calls.

use serde::{Deserialize, Serialize};
use slicecore_plugin_api::ResourceLimits;

/// Configuration for WASM plugin sandboxing.
///
/// Controls resource limits applied to each WASM plugin execution. The defaults
/// are suitable for typical infill generation workloads.
///
/// Can be constructed from a [`ResourceLimits`] found in a plugin manifest,
/// or use the sensible defaults (64 MiB memory, 10M fuel).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Maximum memory in bytes for the WASM plugin (default: 64 MiB).
    pub max_memory_bytes: usize,
    /// Maximum CPU fuel units (default: 10_000_000, roughly 10M wasm instructions).
    pub max_cpu_fuel: u64,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024, // 64 MiB
            max_cpu_fuel: 10_000_000,           // ~10M instructions
        }
    }
}

impl SandboxConfig {
    /// Creates a `SandboxConfig` from manifest `ResourceLimits`.
    ///
    /// Converts the manifest's `max_memory_mb` (megabytes) to bytes.
    /// Uses the manifest's `max_cpu_fuel` directly.
    pub fn from_resource_limits(limits: &ResourceLimits) -> Self {
        Self {
            max_memory_bytes: (limits.max_memory_mb as usize) * 1024 * 1024,
            max_cpu_fuel: limits.max_cpu_fuel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_config_defaults() {
        let config = SandboxConfig::default();
        assert_eq!(config.max_memory_bytes, 64 * 1024 * 1024); // 64 MiB
        assert_eq!(config.max_cpu_fuel, 10_000_000); // 10M fuel
    }

    #[test]
    fn sandbox_config_from_resource_limits() {
        let limits = ResourceLimits {
            max_memory_mb: 128,
            max_cpu_fuel: 5_000_000,
        };
        let config = SandboxConfig::from_resource_limits(&limits);
        assert_eq!(config.max_memory_bytes, 128 * 1024 * 1024); // 128 MiB
        assert_eq!(config.max_cpu_fuel, 5_000_000);
    }

    #[test]
    fn sandbox_config_from_small_limits() {
        let limits = ResourceLimits {
            max_memory_mb: 1,
            max_cpu_fuel: 100,
        };
        let config = SandboxConfig::from_resource_limits(&limits);
        assert_eq!(config.max_memory_bytes, 1 * 1024 * 1024); // 1 MiB
        assert_eq!(config.max_cpu_fuel, 100);
    }

    #[test]
    fn sandbox_config_serde_roundtrip() {
        let config = SandboxConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SandboxConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.max_memory_bytes, config.max_memory_bytes);
        assert_eq!(deserialized.max_cpu_fuel, config.max_cpu_fuel);
    }
}
