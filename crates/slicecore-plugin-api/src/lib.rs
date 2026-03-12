//! # slicecore-plugin-api
//!
//! Shared interface types for the slicecore plugin system.
//!
//! This crate defines the FFI-safe types and traits that form the contract
//! between the slicecore host application and plugins. Both native (dynamic
//! library) and WASM plugins implement the same logical interface.
//!
//! ## Architecture
//!
//! The plugin system uses a three-crate architecture:
//!
//! 1. **`slicecore-plugin-api`** (this crate) -- Shared types and traits.
//!    Both the host and plugins depend on this crate. Type layout agreement
//!    at load time prevents undefined behavior.
//!
//! 2. **`slicecore-plugin`** -- Host-side registry, loading, and lifecycle.
//!    Handles plugin discovery, native loading via `abi_stable`, and WASM
//!    loading via `wasmtime`.
//!
//! 3. **Plugin crates** -- Individual plugin implementations. Each depends
//!    only on `slicecore-plugin-api`.
//!
//! ## FFI Safety
//!
//! All types that cross the plugin boundary derive [`abi_stable::StableAbi`]
//! and use FFI-safe wrappers (`RVec`, `RString`, `RResult`) instead of
//! standard library types. **Never** pass `Vec<T>`, `String`, or `Box<T>`
//! across the FFI boundary.
//!
//! ## Creating a Native Plugin
//!
//! Native plugins are compiled as dynamic libraries (`.so` / `.dll` / `.dylib`)
//! and loaded at runtime via `abi_stable`'s type-layout-verified FFI.
//!
//! Steps:
//!
//! 1. Create a new crate with `crate-type = ["cdylib"]` in `Cargo.toml`
//! 2. Depend on `slicecore-plugin-api` and `abi_stable = "0.11"`
//! 3. Implement the [`InfillPatternPlugin`] trait on your plugin struct
//! 4. Export the root module via `#[export_root_module]` returning an
//!    [`InfillPluginMod_Ref`]
//! 5. Create a `plugin.toml` manifest alongside the built library
//! 6. Build with `cargo build`
//!
//! See `plugins/examples/native-zigzag-infill/` for a complete working example.
//!
//! ## Creating a WASM Plugin
//!
//! WASM plugins are compiled as WebAssembly components and loaded at runtime
//! via `wasmtime` with sandboxed execution (memory limits, CPU fuel).
//!
//! Steps:
//!
//! 1. Create a new crate with `crate-type = ["cdylib"]` in `Cargo.toml`
//! 2. Depend on `wit-bindgen` for guest binding generation
//! 3. Copy the WIT file from `crates/slicecore-plugin/wit/slicecore-plugin.wit`
//! 4. Use `wit_bindgen::generate!` to create guest bindings
//! 5. Implement the generated `Guest` trait
//! 6. Create a `plugin.toml` manifest alongside the built `.wasm` file
//! 7. Build with `cargo build --target wasm32-wasip2`
//!
//! See `plugins/examples/wasm-spiral-infill/` for a complete working example.
//!
//! ## Modules
//!
//! - [`types`] -- FFI-safe infill request/result types ([`InfillRequest`], [`InfillResult`], [`FfiInfillLine`])
//! - [`error`] -- FFI-safe error types ([`PluginError`])
//! - [`metadata`] -- Plugin metadata and manifest (serde-serializable, not FFI)
//! - [`traits`] -- FFI-safe plugin traits ([`InfillPatternPlugin`]) and root module ([`InfillPluginMod`])
//! - [`postprocess_types`] -- FFI-safe post-processor types ([`FfiGcodeCommand`], [`PostProcessRequest`], [`PostProcessResult`])
//! - [`postprocess_traits`] -- FFI-safe post-processor trait ([`GcodePostProcessorPlugin`]) and root module ([`PostProcessorPluginMod`])

// The abi_stable sabi_trait macro generates non-local impl blocks that trigger
// this lint on newer Rust compilers. Suppressed at crate level since the macro
// expansion cannot be controlled from user code.
#![allow(non_local_definitions)]

pub mod error;
pub mod metadata;
pub mod postprocess_traits;
pub mod postprocess_types;
pub mod traits;
pub mod types;

// Re-export primary types for convenience
pub use error::PluginError;
pub use metadata::{PluginCapability, PluginManifest, PluginMetadata, PluginType, ResourceLimits};
pub use postprocess_traits::{
    GcodePostProcessorPlugin, GcodePostProcessorPlugin_TO, PostProcessorPluginMod,
    PostProcessorPluginMod_Ref,
};
pub use postprocess_types::{
    FfiConfigParam, FfiGcodeCommand, FfiPrintConfigSnapshot, LayerPostProcessRequest,
    PostProcessRequest, PostProcessResult, ProcessingMode,
};
pub use traits::{
    InfillPatternPlugin, InfillPatternPlugin_TO, InfillPluginMod, InfillPluginMod_Ref,
};
pub use types::{FfiInfillLine, InfillRequest, InfillResult};
