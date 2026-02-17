//! # slicecore-plugin
//!
//! Plugin registry, native loader, and discovery for the slicecore slicing engine.
//!
//! This is the **host-side** plugin infrastructure. It discovers plugins on disk,
//! validates version compatibility, loads native dynamic libraries via `abi_stable`,
//! and presents a unified API for the engine to call plugin-provided infill patterns.
//!
//! ## Architecture
//!
//! This crate is part of a three-crate plugin architecture:
//!
//! 1. **`slicecore-plugin-api`** -- Shared FFI-safe types and traits
//! 2. **`slicecore-plugin`** (this crate) -- Host-side registry and loaders
//! 3. **Plugin crates** -- Individual plugin implementations
//!
//! ## Modules
//!
//! - [`error`] -- Plugin system error types
//! - [`registry`] -- Plugin registry with discover, register, get, list operations
//! - [`discovery`] -- Directory scanning and manifest parsing
//! - [`native`] -- Native plugin loader via `abi_stable` (cfg-gated for non-WASM)
//! - [`convert`] -- Type conversion between internal and FFI-safe types

pub mod convert;
pub mod discovery;
pub mod error;
#[cfg(not(target_family = "wasm"))]
pub mod native;
pub mod registry;
pub mod sandbox;
#[cfg(feature = "wasm-plugins")]
pub mod wasm;

// Re-export primary types
pub use convert::{ffi_result_to_lines, regions_to_request, ConvertedInfillLine};
pub use error::PluginSystemError;
pub use registry::{InfillPluginAdapter, PluginInfo, PluginKind, PluginRegistry};

// Re-export key types from the API crate for convenience
pub use slicecore_plugin_api::{
    FfiInfillLine, InfillRequest, InfillResult, PluginManifest, PluginMetadata,
};
