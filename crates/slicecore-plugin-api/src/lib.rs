//! # slicecore-plugin-api
//!
//! FFI-safe plugin interface types for the slicecore slicing engine.
//!
//! This crate defines the shared interface between the **host** application
//! (`slicecore-plugin`) and individual **plugin** crates. It is the foundation
//! of a three-crate plugin architecture:
//!
//! 1. **`slicecore-plugin-api`** (this crate) -- Shared types and traits.
//!    Both the host and plugins depend on this crate. Type layout agreement
//!    at load time prevents undefined behavior.
//!
//! 2. **`slicecore-plugin`** -- Plugin registry, native loader (`abi_stable`),
//!    and WASM loader (`wasmtime`). Only the host depends on this crate.
//!
//! 3. **Plugin crates** (e.g., `zigzag-infill`) -- Individual plugin
//!    implementations. Each depends only on `slicecore-plugin-api`.
//!
//! ## FFI Safety
//!
//! All types that cross the plugin boundary derive [`abi_stable::StableAbi`]
//! and use FFI-safe wrappers (`RVec`, `RString`, `RResult`) instead of
//! standard library types. **Never** pass `Vec<T>`, `String`, or `Box<T>`
//! across the FFI boundary.
//!
//! ## Modules
//!
//! - [`types`] -- FFI-safe infill request/result types
//! - [`error`] -- FFI-safe error types
//! - [`metadata`] -- Plugin metadata and manifest (serde-serializable, not FFI)
//! - [`traits`] -- FFI-safe plugin traits (`InfillPatternPlugin`) and root module

// The abi_stable sabi_trait macro generates non-local impl blocks that trigger
// this lint on newer Rust compilers. Suppressed at crate level since the macro
// expansion cannot be controlled from user code.
#![allow(non_local_definitions)]

pub mod error;
pub mod metadata;
pub mod traits;
pub mod types;

// Re-export primary types for convenience
pub use error::PluginError;
pub use metadata::{
    PluginCapability, PluginManifest, PluginMetadata, PluginType, ResourceLimits,
};
pub use traits::{InfillPatternPlugin, InfillPatternPlugin_TO, InfillPluginMod, InfillPluginMod_Ref};
pub use types::{FfiInfillLine, InfillRequest, InfillResult};
