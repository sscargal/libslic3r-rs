//! Sandbox configuration for WASM plugins.
//!
//! Provides configurable resource limits (memory, CPU fuel) that are applied
//! to each WASM plugin instance. Each `generate()` call gets a fresh
//! [`wasmtime::Store`] with the configured limits, preventing resource
//! accumulation across calls.

use serde::{Deserialize, Serialize};

/// Configuration for WASM plugin sandboxing.
///
/// Controls resource limits applied to each WASM plugin execution. The defaults
/// are suitable for typical infill generation workloads.
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
            max_cpu_fuel: 10_000_000,            // ~10M instructions
        }
    }
}
