# Phase 7: Plugin System - Research

**Researched:** 2026-02-17
**Domain:** Rust plugin systems (native dynamic loading + WASM sandboxing)
**Confidence:** MEDIUM-HIGH

## Summary

Phase 7 implements the core architectural differentiator of libslic3r-rs: a plugin system that allows external developers to extend the slicing engine without modifying or recompiling the core. The system requires two distinct plugin loading mechanisms -- native dynamic libraries via `abi_stable` for maximum performance, and sandboxed WASM plugins via `wasmtime` Component Model for safety and portability.

The existing codebase already has critical foundations in place. `IPoint2` is `#[repr(C)]`, core types like `InfillLine`, `LayerInfill`, and `ValidPolygon` are well-defined, and the infill system uses a dispatch pattern (`generate_infill` with `InfillPattern` enum + per-pattern modules) that naturally maps to a plugin interface. However, the current architecture dispatches on an enum and calls module-level functions rather than trait objects, so the main work is: (1) define FFI-safe plugin traits, (2) create a `slicecore-plugin` crate with PluginRegistry, (3) implement native loading via `abi_stable`, (4) implement WASM loading via `wasmtime` Component Model with WIT interfaces, and (5) wire plugin-provided infill into the engine pipeline.

The three-crate pattern (`slicecore-plugin-api` for shared interface, plugin crates for implementations, `slicecore-plugin` for registry/loading) is the established Rust pattern for abi_stable-based plugins. For WASM, the interface is defined in WIT (WebAssembly Interface Types) files and the host uses `wasmtime::component::bindgen!` to generate type-safe bindings. Both paths converge on a unified `PluginRegistry` that presents a single API to the engine regardless of plugin origin.

**Primary recommendation:** Use the three-crate pattern with `abi_stable` 0.11 for native plugins and `wasmtime` 41+ with Component Model for WASM plugins. Define a `slicecore-plugin-api` crate containing `#[sabi_trait]` trait definitions and FFI-safe types. Keep the plugin interface focused on infill pattern generation for v1 (matching existing `generate_infill` signature), with extension points for G-code post-processing as a secondary goal.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `abi_stable` | 0.11.3 | FFI-safe Rust-to-Rust dynamic library loading with type checking | Only production-grade stable ABI solution for Rust; load-time layout verification prevents UB; `sabi_trait` generates FFI-safe trait objects |
| `wasmtime` | 41.x | WASM Component Model runtime for sandboxed plugin execution | Bytecode Alliance reference runtime; mature, production-proven; Component Model support with `bindgen!` macro |
| `wasmtime-wasi` | 41.x (matching wasmtime) | WASI system interface for WASM plugins | Required for WASM plugins that need basic I/O or logging capabilities |
| `wit-bindgen` | latest 0.x | Generate Rust guest bindings from WIT interface definitions | Official Bytecode Alliance tooling for Component Model guest code generation |
| `semver` | 1.x | Plugin version compatibility checking | Standard Rust semver parsing and matching; already in the ecosystem stack plan |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `cargo-component` | latest | Build tool for compiling WASM component plugins | Plugin developers building WASM plugins (dev tooling, not a runtime dependency) |
| `toml` | 0.8 | Parse plugin manifest files (plugin.toml) | Already a dependency in slicecore-engine; used for plugin metadata |
| `serde` | 1.x | Serialize/deserialize plugin metadata and config | Already a workspace dependency |
| `thiserror` | 2.x | Plugin-specific error types | Already a workspace dependency |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `abi_stable` | `stabby` | stabby (v72.x) has better niche optimization for enums and supports async, but abi_stable has more documentation, more examples, and more production usage for plugin systems specifically |
| `abi_stable` | raw `libloading` + `#[repr(C)]` | Manual approach avoids the abi_stable dependency but requires hand-rolling all type checking, version verification, and panic safety -- exactly the problems abi_stable solves |
| `wasmtime` | `wasmer` | wasmer is an alternative WASM runtime but wasmtime has better Component Model support (the standard for typed plugin interfaces) and is the Bytecode Alliance reference implementation |
| Plugin manifest in TOML | Plugin manifest in JSON | TOML is already used throughout the project for configuration; consistency matters more than format choice |

**Installation:**
```bash
# In workspace Cargo.toml [workspace.dependencies]:
cargo add abi_stable@0.11 --package slicecore-plugin-api
cargo add wasmtime@41 --features component-model,cranelift --package slicecore-plugin
cargo add wasmtime-wasi@41 --package slicecore-plugin
cargo add semver@1 --package slicecore-plugin

# For WASM plugin development (developer tooling):
cargo install cargo-component
rustup target add wasm32-wasip2
```

## Architecture Patterns

### Recommended Project Structure
```
crates/
  slicecore-plugin-api/       # Shared interface crate (FFI-safe types + traits)
    src/
      lib.rs                  # Re-exports
      types.rs                # FFI-safe versions of core types (InfillLine, etc.)
      traits.rs               # #[sabi_trait] InfillPatternPlugin, GcodePostProcessor
      metadata.rs             # PluginMetadata, PluginCapability, version info
      error.rs                # FFI-safe error types
    Cargo.toml                # depends on: abi_stable, serde, semver
  slicecore-plugin/           # Plugin loading, registry, and lifecycle management
    src/
      lib.rs                  # Re-exports
      registry.rs             # PluginRegistry (unified native + WASM)
      native.rs               # Native plugin loader (abi_stable RootModule)
      wasm.rs                 # WASM plugin loader (wasmtime Component Model)
      sandbox.rs              # WASM resource limits and sandboxing config
      discovery.rs            # Directory scanning, manifest parsing
      error.rs                # Plugin system error types
    wit/
      slicecore-plugin.wit    # WIT interface definition for WASM plugins
    Cargo.toml                # depends on: slicecore-plugin-api, abi_stable, wasmtime, wasmtime-wasi
plugins/
  examples/
    native-zigzag-infill/     # Example native plugin (cdylib)
      src/lib.rs
      Cargo.toml
    wasm-spiral-infill/       # Example WASM plugin (component)
      src/lib.rs
      Cargo.toml
```

### Pattern 1: Three-Crate Plugin Architecture
**What:** Separate interface definition (api), implementation (plugins), and loading (registry) into distinct crates.
**When to use:** Always -- this is the foundational pattern for abi_stable plugin systems.
**Why:** The interface crate is compiled into both the host and plugin, ensuring type layout agreement. The plugin crate depends only on the interface. The host crate loads plugins and verifies compatibility at load time.

```rust
// slicecore-plugin-api/src/traits.rs
use abi_stable::prelude::*;
use abi_stable::sabi_trait;
use abi_stable::std_types::{RVec, RStr, RString, RResult};

/// FFI-safe infill generation request
#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct InfillRequest {
    /// Flattened polygon points: [x0, y0, x1, y1, ...]
    pub boundary_points: RVec<i64>,
    /// Number of points per polygon boundary
    pub boundary_lengths: RVec<u32>,
    /// Fill density (0.0 to 1.0)
    pub density: f64,
    /// Layer index
    pub layer_index: u64,
    /// Layer Z height in mm
    pub layer_z: f64,
    /// Extrusion line width in mm
    pub line_width: f64,
}

/// FFI-safe infill line segment
#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct FfiInfillLine {
    pub start_x: i64,
    pub start_y: i64,
    pub end_x: i64,
    pub end_y: i64,
}

/// FFI-safe infill generation result
#[repr(C)]
#[derive(StableAbi)]
pub struct InfillResult {
    pub lines: RVec<FfiInfillLine>,
}

/// FFI-safe plugin trait for infill pattern generation
#[sabi_trait]
pub trait InfillPatternPlugin: Send + Sync + Debug {
    /// Returns the name of this infill pattern (e.g., "zigzag")
    fn name(&self) -> RString;

    /// Returns a description of this infill pattern
    fn description(&self) -> RString;

    /// Generate infill lines for the given request
    #[sabi(last_prefix_field)]
    fn generate(&self, request: &InfillRequest) -> RResult<InfillResult, RString>;
}
```

### Pattern 2: RootModule for Native Plugin Entry Point
**What:** Each native plugin exports a RootModule struct that serves as the entry point for the host to discover and instantiate the plugin.
**When to use:** Every native (.so/.dll/.dylib) plugin.

```rust
// In the plugin crate (e.g., native-zigzag-infill/src/lib.rs)
use abi_stable::prelude::*;
use abi_stable::export_root_module;
use abi_stable::sabi_extern_fn;
use slicecore_plugin_api::{InfillPatternPlugin_TO, InfillPatternPluginBox};

/// The root module exported by this plugin
#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix))]
pub struct InfillPluginMod {
    /// Create a new instance of the infill pattern plugin
    #[sabi(last_prefix_field)]
    pub new: extern "C" fn() -> InfillPatternPluginBox,
}

impl RootModule for InfillPluginMod_Ref {
    const BASE_NAME: &'static str = "zigzag_infill";
    const NAME: &'static str = "zigzag_infill";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
    declare_root_module_statics! { InfillPluginMod_Ref }
}

#[export_root_module]
fn instantiate_root_module() -> InfillPluginMod_Ref {
    InfillPluginMod {
        new: new_plugin,
    }.leak_into_prefix()
}

#[sabi_extern_fn]
fn new_plugin() -> InfillPatternPluginBox {
    let plugin = ZigzagInfillPlugin::new();
    InfillPatternPlugin_TO::from_value(plugin, TD_Opaque)
}
```

### Pattern 3: WIT Interface for WASM Plugins
**What:** Define the plugin interface in WIT (WebAssembly Interface Types) and use bindgen! on both host and guest sides.
**When to use:** All WASM plugins.

```wit
// slicecore-plugin/wit/slicecore-plugin.wit
package slicecore:plugin@0.1.0;

/// Types shared between host and plugin
interface types {
    /// A 2D point in integer coordinate space
    record point2 {
        x: s64,
        y: s64,
    }

    /// An infill line segment
    record infill-line {
        start: point2,
        end: point2,
    }

    /// Parameters for infill generation
    record infill-request {
        /// Flattened boundary points
        boundary-points: list<point2>,
        /// Number of points per polygon
        boundary-lengths: list<u32>,
        /// Fill density 0.0 to 1.0
        density: f64,
        /// Current layer index
        layer-index: u64,
        /// Layer Z height in mm
        layer-z: f64,
        /// Line width in mm
        line-width: f64,
    }

    /// Result of infill generation
    record infill-result {
        lines: list<infill-line>,
    }
}

/// The world that infill plugins must implement
world infill-plugin {
    use types.{infill-request, infill-result};

    /// Return the plugin name
    export name: func() -> string;

    /// Return a description of this infill pattern
    export description: func() -> string;

    /// Generate infill for the given request
    export generate: func(request: infill-request) -> result<infill-result, string>;
}
```

### Pattern 4: Unified PluginRegistry
**What:** A single registry that manages both native and WASM plugins behind a common interface.
**When to use:** In the engine when looking up available infill patterns.

```rust
// slicecore-plugin/src/registry.rs
pub struct PluginRegistry {
    infill_plugins: HashMap<String, Box<dyn InfillPluginAdapter>>,
    plugin_manifests: Vec<PluginManifest>,
}

/// Adapter trait that unifies native and WASM plugin interfaces
trait InfillPluginAdapter: Send + Sync {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn generate(&self, request: InfillRequest) -> Result<Vec<InfillLine>, PluginError>;
    fn plugin_type(&self) -> PluginType;
}

enum PluginType {
    Native,
    Wasm,
    Builtin,
}

impl PluginRegistry {
    pub fn new() -> Self { /* ... */ }

    /// Scan a directory for plugins (.so/.dll/.dylib and .wasm files)
    pub fn discover(&mut self, plugin_dir: &Path) -> Result<Vec<PluginManifest>, PluginError> {
        // 1. Scan for plugin.toml manifests
        // 2. For each manifest, load the corresponding plugin
        // 3. Validate version compatibility
        // 4. Register in the appropriate map
    }

    /// Get an infill plugin by name
    pub fn get_infill_plugin(&self, name: &str) -> Option<&dyn InfillPluginAdapter> {
        self.infill_plugins.get(name).map(|p| p.as_ref())
    }

    /// List all registered infill plugins
    pub fn list_infill_plugins(&self) -> Vec<PluginInfo> { /* ... */ }
}
```

### Pattern 5: Engine Integration via Custom InfillPattern Variant
**What:** Extend the existing `InfillPattern` enum with a `Plugin(String)` variant that delegates to the registry.
**When to use:** When integrating plugin-provided infill into the existing engine pipeline.

```rust
// Modified InfillPattern enum
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfillPattern {
    Rectilinear,
    Grid,
    Honeycomb,
    Gyroid,
    // ... existing variants ...

    /// A plugin-provided infill pattern, identified by name
    #[serde(rename = "plugin")]
    Plugin(String),
}

// Modified generate_infill to dispatch to plugins
pub fn generate_infill(
    pattern: InfillPattern,
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    layer_z: f64,
    line_width: f64,
    lightning_context: Option<&lightning::LightningContext>,
    plugin_registry: Option<&PluginRegistry>,  // NEW parameter
) -> Vec<InfillLine> {
    match pattern {
        InfillPattern::Plugin(ref name) => {
            let registry = plugin_registry
                .expect("PluginRegistry required for plugin infill patterns");
            let plugin = registry.get_infill_plugin(name)
                .unwrap_or_else(|| panic!("Unknown plugin infill pattern: {}", name));
            let request = InfillRequest::from_regions(infill_region, density, layer_index, layer_z, line_width);
            plugin.generate(request).expect("Plugin infill generation failed")
        }
        // ... existing match arms unchanged ...
    }
}
```

### Anti-Patterns to Avoid
- **Sharing Rust-native types across FFI:** Never pass `Vec<T>`, `String`, `Box<T>`, or trait objects (`dyn Trait`) across the plugin boundary. Use `RVec<T>`, `RString`, `RBox<T>`, and `sabi_trait`-generated types instead.
- **Exposing complex internal types to plugins:** Don't expose `ValidPolygon` directly to plugins. Create simplified FFI-safe equivalents (flat arrays of coordinates) that plugins can consume without depending on internal crate types.
- **Single-crate plugin design:** Don't put the plugin interface and the plugin loader in the same crate. The three-crate split ensures plugins don't transitively depend on the host's internal types.
- **Ignoring panic safety:** Panics in plugins (especially native ones) will abort the host process if they unwind across FFI boundaries. `abi_stable` handles this with `AbortBomb`, but WASM plugins are naturally sandboxed. Still, always wrap native plugin calls in proper error handling.
- **Passing large geometry buffers to WASM:** The Component Model copies data across the sandbox boundary. Don't pass the entire mesh or all layer contours at once to a WASM plugin. Pass only the specific infill region for the current layer.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Stable ABI for Rust types | Custom `#[repr(C)]` wrappers with manual version checking | `abi_stable` with `StableAbi` derive and `sabi_trait` | Type layout verification, panic safety, version compatibility checking are all deceptively complex; abi_stable handles recursive layout validation |
| WASM sandbox with typed interfaces | Raw `wasmtime::Module` with manual function imports/exports | `wasmtime::component::bindgen!` with WIT interfaces | Component Model provides typed function signatures, automatic serialization/deserialization of complex types, proper error propagation |
| Plugin version compatibility | Manual string parsing and comparison | `semver` crate with `VersionReq` | Semver ranges, pre-release handling, build metadata are subtle; the semver crate handles all edge cases |
| FFI-safe trait objects | Manual vtable construction with `extern "C" fn` pointers | `#[sabi_trait]` attribute macro | Generates correct vtables with version-extensible prefix types, downcasting support, and panic safety |
| Plugin discovery | Manual directory scanning with glob patterns | Structured manifest (plugin.toml) + directory convention | Manifest-based discovery enables version checking and capability declaration before loading |

**Key insight:** The combination of `abi_stable` for native plugins and `wasmtime` Component Model for WASM plugins covers the two hardest problems in Rust plugin systems: (1) Rust's lack of stable ABI, and (2) safe sandboxing of untrusted code. Both libraries are battle-tested and handle edge cases that custom solutions would miss.

## Common Pitfalls

### Pitfall 1: Type Layout Mismatch Between Host and Plugin
**What goes wrong:** Plugin compiled with different Rust version or different version of shared types causes undefined behavior or crashes.
**Why it happens:** Rust has no stable ABI; struct layout, vtable layout, and enum representation can change between compiler versions.
**How to avoid:** Use `abi_stable`'s `StableAbi` derive on all types crossing the FFI boundary. The library performs automatic layout verification at load time and returns a clear error instead of UB.
**Warning signs:** Crashes when loading plugins compiled with a different rustc version; mysterious data corruption; segfaults in plugin functions.

### Pitfall 2: Panics Across FFI Boundaries
**What goes wrong:** A panic in a native plugin unwinds across the FFI boundary, causing undefined behavior (double-free, stack corruption, or immediate abort).
**Why it happens:** Rust panics use unwinding by default, which is UB across `extern "C"` function boundaries.
**How to avoid:** `abi_stable`'s `#[sabi_extern_fn]` and `sabi_trait` wrap all calls with `AbortBomb` to catch panics. For manual FFI, use `std::panic::catch_unwind`. WASM plugins are naturally isolated -- a trap in WASM becomes a `Result::Err` on the host side.
**Warning signs:** Process aborts instead of error returns; "caught unwind" messages in logs.

### Pitfall 3: WASM Data Copy Overhead
**What goes wrong:** Passing large polygon data to WASM plugins causes significant performance degradation due to serialization/copy overhead across the sandbox boundary.
**Why it happens:** WASM Component Model does not share memory between host and guest. All data is serialized, copied into guest linear memory, and deserialized. For infill regions with thousands of polygon vertices, this copy can be expensive.
**How to avoid:** Keep the plugin interface focused: pass only the infill region for a single layer (not the entire model). Pre-compute bounding boxes and spacing on the host side. For very complex patterns, consider making them native plugins instead of WASM.
**Warning signs:** WASM plugin infill generation is orders of magnitude slower than native; profiling shows most time in serialization rather than computation.

### Pitfall 4: Version Skew in Plugin API
**What goes wrong:** Plugin compiled against v0.1.0 of the API crate is loaded by a host using v0.2.0, and newly added trait methods are called on the old plugin.
**Why it happens:** Plugin developers don't update as fast as core developers. ABI compatibility is forward-only (newer host, older plugin should work if methods were added at the end).
**How to avoid:** Use `abi_stable`'s prefix types with `#[sabi(last_prefix_field)]` on the last stable method. New methods must be added at the end and should have default implementations. Use `semver` for version checking in plugin manifests. For WASM, WIT world versioning handles this.
**Warning signs:** Missing method panics when calling newer API on older plugin; plugin loads but fails at runtime.

### Pitfall 5: Blocking the Engine Pipeline with Slow Plugins
**What goes wrong:** A misbehaving plugin (infinite loop, excessive allocation) hangs the entire slicing pipeline.
**Why it happens:** Native plugins run in the same process with full access. WASM plugins can exhaust their memory allocation or CPU budget.
**How to avoid:** For WASM: configure memory limits (e.g., 64 MiB) and fuel-based CPU limits in the wasmtime `Store`. For native: run plugin infill generation behind a timeout (though this is harder to enforce in-process). Document performance expectations in the plugin API.
**Warning signs:** Slicing hangs on certain layers; memory usage spikes when specific plugins are loaded.

### Pitfall 6: Forgetting to Feature-Gate wasmtime
**What goes wrong:** Adding wasmtime as a hard dependency makes the core library fail to compile for wasm32-unknown-unknown target (since wasmtime itself is a native-only runtime).
**Why it happens:** wasmtime requires native OS features (mmap, signals, JIT) that don't exist in WASM environments.
**How to avoid:** Gate all wasmtime usage behind a `plugins` or `native-plugins` feature flag. The `slicecore-plugin` crate should NOT be compiled when targeting WASM. Use `cfg` attributes to exclude the WASM plugin loader on WASM targets.
**Warning signs:** Compilation errors mentioning `mmap`, `signal`, or `libc` when building for wasm32.

## Code Examples

Verified patterns from official sources:

### Loading a Native Plugin with abi_stable
```rust
// Source: abi_stable docs + NullDeref plugin tutorial
use abi_stable::library::RootModule;
use slicecore_plugin_api::InfillPluginMod_Ref;

fn load_native_plugin(path: &Path) -> Result<InfillPluginMod_Ref, PluginError> {
    // abi_stable handles:
    // 1. dlopen/LoadLibrary
    // 2. Symbol lookup for the root module
    // 3. Type layout verification (recursive)
    // 4. Version compatibility check
    let module = InfillPluginMod_Ref::load_from_directory(path)
        .map_err(|e| PluginError::LoadFailed {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

    Ok(module)
}

fn use_native_plugin(module: &InfillPluginMod_Ref) {
    let new_fn = module.new();
    let plugin = new_fn();

    println!("Loaded plugin: {}", plugin.name());

    let request = InfillRequest { /* ... */ };
    match plugin.generate(&request) {
        ROk(result) => {
            println!("Generated {} infill lines", result.lines.len());
        }
        RErr(msg) => {
            eprintln!("Plugin error: {}", msg);
        }
    }
}
```

### Loading a WASM Plugin with wasmtime Component Model
```rust
// Source: wasmtime docs (wasip2-plugins example)
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store, Config};
use wasmtime_wasi::WasiCtxBuilder;

// Generate bindings from WIT
wasmtime::component::bindgen!({
    world: "infill-plugin",
    path: "wit/slicecore-plugin.wit",
});

struct PluginState {
    wasi_ctx: wasmtime_wasi::WasiCtx,
    table: wasmtime::component::ResourceTable,
}

impl wasmtime_wasi::WasiView for PluginState {
    fn ctx(&mut self) -> &mut wasmtime_wasi::WasiCtx { &mut self.wasi_ctx }
    fn table(&mut self) -> &mut wasmtime::component::ResourceTable { &mut self.table }
}

fn load_wasm_plugin(wasm_path: &Path, engine: &Engine) -> Result<(), PluginError> {
    let component = Component::from_file(engine, wasm_path)?;

    let mut linker = Linker::<PluginState>::new(engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;

    let wasi_ctx = WasiCtxBuilder::new().build();
    let state = PluginState {
        wasi_ctx,
        table: wasmtime::component::ResourceTable::new(),
    };
    let mut store = Store::new(engine, state);

    // Set resource limits for sandboxing
    store.set_fuel(1_000_000)?;  // CPU limit
    // Memory limit set via engine config

    let (plugin, _instance) = InfillPlugin::instantiate(&mut store, &component, &linker)?;

    // Use the plugin
    let name = plugin.call_name(&mut store)?;
    println!("Loaded WASM plugin: {}", name);

    Ok(())
}
```

### Plugin Manifest Format
```toml
# plugin.toml - placed alongside the .so/.dll/.dylib/.wasm file
[plugin]
name = "zigzag-infill"
version = "1.0.0"
description = "Zigzag infill pattern with configurable angle"
author = "Example Developer"
license = "MIT"
min_api_version = "0.1.0"
max_api_version = "0.2.0"

[plugin.type]
kind = "native"  # or "wasm"
library = "libzigzag_infill.so"  # or "zigzag_infill.wasm"

[capabilities]
provides = ["infill_pattern"]

[resources]
# Only applicable for WASM plugins
max_memory_mb = 64
max_cpu_fuel = 1000000
```

### Converting Between FFI-Safe and Internal Types
```rust
// slicecore-plugin/src/convert.rs
use slicecore_plugin_api::{FfiInfillLine, InfillRequest};
use slicecore_engine::infill::InfillLine;
use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::IPoint2;

/// Convert internal ValidPolygon regions to FFI-safe InfillRequest
pub fn regions_to_request(
    regions: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    layer_z: f64,
    line_width: f64,
) -> InfillRequest {
    let mut boundary_points = RVec::new();
    let mut boundary_lengths = RVec::new();

    for poly in regions {
        let points = poly.points();
        boundary_lengths.push(points.len() as u32);
        for pt in points {
            boundary_points.push(pt.x);
            boundary_points.push(pt.y);
        }
    }

    InfillRequest {
        boundary_points,
        boundary_lengths,
        density,
        layer_index: layer_index as u64,
        layer_z,
        line_width,
    }
}

/// Convert FFI-safe infill lines back to internal type
pub fn ffi_lines_to_internal(ffi_lines: &[FfiInfillLine]) -> Vec<InfillLine> {
    ffi_lines.iter().map(|line| InfillLine {
        start: IPoint2::new(line.start_x, line.start_y),
        end: IPoint2::new(line.end_x, line.end_y),
    }).collect()
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Raw `libloading` + `extern "C"` functions | `abi_stable` with `sabi_trait` + `StableAbi` derive | abi_stable 0.9+ (2021+) | Eliminates manual type checking, version verification, panic safety |
| WASM core modules with manual import/export | WASM Component Model with WIT interfaces | wasmtime 8+ (2023+), maturing in 2024-2025 | Typed interfaces replace raw i32/i64 function signatures; rich types (strings, records, lists) |
| `wasm32-wasi` (preview 1) target | `wasm32-wasip2` target | Rust 1.82 (2024) | Component Model is now a Rust upstream target; `cargo component` or plain `cargo` can build components |
| Manual WIT binding generation | `wasmtime::component::bindgen!` macro | wasmtime 8+ | Compile-time code generation from WIT files; type-safe host-guest interaction |

**Deprecated/outdated:**
- **WASI Preview 1 for new plugins:** Use WASI Preview 2 / Component Model instead. Preview 1 uses core WASM modules without component typing.
- **`dlopen2` for Rust-to-Rust FFI:** Use `abi_stable` which wraps `libloading` with type safety.
- **Manual vtable construction for plugin traits:** Use `#[sabi_trait]` which generates correct, version-extensible vtables automatically.

## Open Questions

1. **Should native plugins use `cdylib` or `dylib` crate type?**
   - What we know: `abi_stable` expects `cdylib` crate type for dynamic libraries. This means the plugin's Cargo.toml must specify `[lib] crate-type = ["cdylib"]`.
   - What's unclear: Whether there are limitations with `cdylib` that affect plugin developer experience (e.g., inability to use certain Rust features).
   - Recommendation: Use `cdylib` as recommended by abi_stable. This is well-tested and the standard approach.

2. **How should the Engine handle plugin failures gracefully?**
   - What we know: WASM plugins trap cleanly (Result::Err). Native plugins can panic (caught by abi_stable) or segfault (not recoverable).
   - What's unclear: Whether the engine should fall back to a built-in pattern when a plugin fails, or propagate the error to the caller.
   - Recommendation: Propagate the error as `EngineError::Plugin { plugin, message }` (this error variant is already designed in the architecture docs). Let the caller decide whether to retry with a fallback pattern.

3. **Should G-code post-processing be a plugin extension point in v1?**
   - What we know: The design docs list 8 extension points (FileFormat, InfillPattern, SupportStrategy, GcodeDialect, PostProcessor, Analyzer, Optimizer, SeamStrategy). The requirements (PLUGIN-01) say "extension points for infill, supports, etc."
   - What's unclear: How many extension points to implement in Phase 7 vs defer to later.
   - Recommendation: Focus Phase 7 on **InfillPattern only** as the primary extension point (matching success criteria SC1 and SC2). Add GcodePostProcessor as a simpler secondary extension point if time permits. Defer other extension points (SupportStrategy, FileFormat, etc.) to future phases.

4. **Performance characteristics of WASM plugins for infill generation**
   - What we know: WASM has 1.5-3x performance overhead vs native. Infill generation involves moderate computation (scan-line intersection, point-in-polygon). Data transfer overhead for polygon boundaries.
   - What's unclear: Whether WASM infill generation is fast enough for production use or only suitable for experimentation/prototyping.
   - Recommendation: Implement and benchmark. If WASM infill is too slow for production, document it as a prototyping tool and recommend native plugins for performance-critical patterns.

5. **Feature gating strategy for the plugin crate**
   - What we know: wasmtime cannot compile to wasm32. abi_stable uses libloading which requires OS dynamic linking. The existing project uses feature flags (native, wasm).
   - What's unclear: Exact feature flag structure for the new plugin crates.
   - Recommendation: `slicecore-plugin` should have features: `native-plugins` (abi_stable, default on), `wasm-plugins` (wasmtime, default on for native builds). `slicecore-plugin-api` should have no feature gates (it defines types only). Exclude `slicecore-plugin` from WASM workspace members entirely.

## Sources

### Primary (HIGH confidence)
- [abi_stable docs](https://docs.rs/abi_stable/latest/abi_stable/) - API reference, sabi_trait macro, StableAbi derive, RootModule pattern
- [abi_stable sabi_trait docs](https://docs.rs/abi_stable/latest/abi_stable/attr.sabi_trait.html) - Full sabi_trait documentation including _TO types, prefix fields, version extensibility
- [wasmtime component model API](https://docs.wasmtime.dev/api/wasmtime/component/index.html) - Engine, Store, Component, Linker, bindgen! macro
- [wasmtime plugin example](https://docs.wasmtime.dev/wasip2-plugins.html) - Official plugin architecture example with WIT
- [WIT Reference](https://component-model.bytecodealliance.org/design/wit.html) - WIT interface definition language specification

### Secondary (MEDIUM confidence)
- [NullDeref: Plugins with abi_stable](https://nullderef.com/blog/plugin-abi-stable/) - Three-crate pattern, RootModule implementation, state management approaches
- [Arroyo: How to build a plugin system in Rust](https://www.arroyo.dev/blog/rust-plugin-systems/) - Architecture comparison (native vs WASM vs RPC), performance tradeoffs, FFI safety rules
- [Sy Brand: Building Native Plugin Systems with WebAssembly Components](https://tartanllama.xyz/posts/wasm-plugins/) - Component Model architecture, WIT patterns, security model, limitations
- [Ben Wishovich: Plugins with Rust and WASI Preview 2](https://benw.is/posts/plugins-with-rust-and-wasi) - cargo-component workflow, guest plugin implementation, host loading pattern
- [abi_stable GitHub](https://github.com/rodrimati1992/abi_stable_crates) - Source code, examples, README

### Tertiary (LOW confidence)
- wasmtime version tracking: Search results suggest v41.x is latest as of 2026-02-17, but docs.rs showed v40.0.1 as "latest". Recommend pinning to whichever version compiles successfully.
- Performance claims for WASM (1.5-3x overhead) come from the Arroyo blog post; actual overhead will depend on the specific workload and data transfer patterns.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - abi_stable and wasmtime are well-documented, widely used, and verified against official docs
- Architecture: MEDIUM-HIGH - Three-crate pattern is well-established; WIT interface design is straightforward but not yet validated against actual codebase types
- Pitfalls: HIGH - Documented across multiple sources with consistent warnings about ABI stability, panic safety, and WASM performance
- Code examples: MEDIUM - Patterns verified against official docs but not yet compiled against this specific codebase; type names and module structure may need adjustment

**Research date:** 2026-02-17
**Valid until:** 2026-03-17 (30 days - both abi_stable and wasmtime are mature and stable)
