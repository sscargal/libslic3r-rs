//! # slicecore-plugin
//!
//! Plugin System -- host-side plugin infrastructure for the slicecore slicing engine.
//!
//! This crate provides the [`PluginRegistry`] for discovering, loading, and managing
//! infill pattern plugins at runtime. It supports two plugin backends:
//!
//! - **Native plugins** -- Dynamic libraries (`.so`/`.dll`/`.dylib`) loaded via
//!   `abi_stable` with compile-time ABI verification
//! - **WASM plugins** -- WebAssembly components loaded via `wasmtime` with
//!   sandboxed execution (memory limits, CPU fuel budgets)
//!
//! Both backends present a unified API through the [`InfillPluginAdapter`] trait,
//! so the slicing engine treats all plugins identically regardless of their
//! loading mechanism.
//!
//! ## Quick Start
//!
//! ```ignore
//! use slicecore_plugin::{PluginRegistry, SandboxConfig};
//! use std::path::Path;
//!
//! // Create a registry and discover plugins from a directory
//! let mut registry = PluginRegistry::new();
//! registry.discover_and_load(Path::new("plugins/")).unwrap();
//!
//! // List available plugins
//! for info in registry.list_infill_plugins() {
//!     println!("{}: {} ({:?})", info.name, info.description, info.plugin_kind);
//! }
//!
//! // Generate infill using a specific plugin
//! if let Some(plugin) = registry.get_infill_plugin("zigzag") {
//!     let result = plugin.generate(&request).unwrap();
//! }
//! ```
//!
//! ## Architecture
//!
//! This crate is part of a three-crate plugin architecture:
//!
//! 1. **`slicecore-plugin-api`** -- Shared FFI-safe types and traits
//! 2. **`slicecore-plugin`** (this crate) -- Host-side registry and loaders
//! 3. **Plugin crates** -- Individual plugin implementations
//!
//! ## Feature Flags
//!
//! | Feature          | Default | Description                                    |
//! |------------------|---------|------------------------------------------------|
//! | `native-plugins` | yes     | Enables native (cdylib) plugin loading         |
//! | `wasm-plugins`   | yes     | Enables WASM plugin loading via `wasmtime`     |
//!
//! Disable features to reduce compile times or binary size when only one
//! plugin backend is needed. The `native` module is additionally cfg-gated
//! to exclude it when compiling to WASM targets (`target_family = "wasm"`).
//!
//! ## Modules
//!
//! - [`error`] -- Plugin system error types ([`PluginSystemError`])
//! - [`registry`] -- Plugin registry with discover, register, get, list operations
//! - [`discovery`] -- Directory scanning and manifest parsing
//! - [`native`] -- Native plugin loader via `abi_stable` (cfg-gated for non-WASM targets)
//! - [`convert`] -- Type conversion between internal and FFI-safe types
//! - [`sandbox`] -- WASM plugin sandbox configuration ([`SandboxConfig`])
//! - [`wasm`] -- WASM plugin loader via `wasmtime` Component Model (requires `wasm-plugins` feature)

pub mod convert;
pub mod discovery;
pub mod error;
#[cfg(not(target_family = "wasm"))]
pub mod native;
pub mod postprocess;
pub mod postprocess_convert;
pub mod registry;
pub mod sandbox;
#[cfg(feature = "wasm-plugins")]
pub mod wasm;

// Re-export primary types
pub use convert::{ffi_result_to_lines, regions_to_request, ConvertedInfillLine};
pub use error::PluginSystemError;
pub use postprocess::{run_post_processors, PostProcessorPluginAdapter};
pub use registry::{InfillPluginAdapter, PluginInfo, PluginKind, PluginRegistry};
pub use sandbox::SandboxConfig;

// Re-export key types from the API crate for convenience
pub use slicecore_plugin_api::{
    FfiInfillLine, InfillRequest, InfillResult, PluginManifest, PluginMetadata,
};
