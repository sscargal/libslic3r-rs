# Phase 28: G-code Post-Processing Plugin Point - Research

**Researched:** 2026-03-12
**Domain:** Plugin system extension, G-code transformation pipeline
**Confidence:** HIGH

## Summary

Phase 28 extends the existing plugin system to support G-code post-processing -- plugins that modify, filter, inject, or analyze G-code commands after generation but before final output. The implementation follows well-established patterns from the infill plugin system (Phase 7): `#[sabi_trait]` FFI-safe traits in `slicecore-plugin-api`, host-side adapter traits in `slicecore-plugin`, registry extension, and engine pipeline integration.

The codebase already has all the infrastructure needed. The `InfillPatternPlugin` trait + `InfillPluginMod` + `InfillPluginAdapter` pattern provides a proven template. The `GcodeCommand` enum in `slicecore-gcode-io` is the internal type that needs an FFI-safe counterpart (`FfiGcodeCommand`). The engine pipeline has a clear insertion point between purge tower (step 4c) and time estimation (step 5), at approximately line 1745 in `engine.rs`.

**Primary recommendation:** Replicate the infill plugin pattern exactly for post-processing: define `FfiGcodeCommand` + `PostProcessRequest`/`PostProcessResult` in `slicecore-plugin-api`, define `GcodePostProcessorPlugin` as `#[sabi_trait]`, extend `PluginRegistry` with `postprocessor_plugins` HashMap, and insert the post-processing hook in the engine pipeline.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Both processing modes: `process_all(commands) -> commands` for full-stream transforms AND `process_layer(layer_commands, layer_index) -> commands` for per-layer logic
- Plugins receive G-code commands + read-only PrintConfig snapshot (nozzle diameter, layer height, speeds, etc.)
- Return type: modified command vec (new `Vec<FfiGcodeCommand>`)
- FFI boundary uses `StableAbi`-derived wrapper types (`FfiGcodeCommand`) in `slicecore-plugin-api`
- Support both typed `FfiGcodeCommand` variants AND a `RawGcode(RString)` variant
- Post-processor plugins run after all built-in processing (arc fitting + purge tower), before time estimation and statistics
- Time estimation and filament usage always recomputed after post-processing
- Multiple plugins ordered by explicit priority
- Integrates with Phase 23 progress/cancellation API
- Four capabilities: command modification, injection, removal/filtering, read-only analysis
- Four built-in post-processors: pause at layer, timelapse camera, fan speed override, custom G-code injection
- Auto-discover: all discovered post-processor plugins available automatically
- Enable/disable via config file, CLI flags, and API
- Standalone CLI command: `slicecore post-process`
- Per-plugin configuration via plugin-defined schema in `plugin.toml`
- Self-skipping: plugins return input unchanged if nothing applies

### Claude's Discretion
- Exact `FfiGcodeCommand` variant definitions and field layouts
- Internal conversion code between `GcodeCommand` and `FfiGcodeCommand`
- `plugin.toml` schema extensions for post-processor type and config parameters
- Built-in post-processor implementation details
- Plugin priority default values and ordering algorithm
- How `process_layer` vs `process_all` selection works in the registry
- Standalone `post-process` CLI subcommand argument design

### Deferred Ideas (OUT OF SCOPE)
- Plugin dependency graph / run-before / run-after relationships
- GUI-level plugin management UI
- Plugin marketplace / remote plugin installation
- Streaming post-processing (process commands as generated, not batch)
- Plugin-provided diagnostics/warnings returned alongside modified commands
- `should_run()` pre-check method for fast skip
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| abi_stable | 0.11 | FFI-safe trait objects for native plugins | Already used by infill plugin system |
| serde | 1.x | Config serialization, plugin parameter schemas | Already workspace dependency |
| toml | workspace | Plugin manifest parsing, config files | Already used for plugin.toml |
| thiserror | workspace | Error types in slicecore-plugin | Already used in PluginSystemError |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| clap | workspace | CLI `post-process` subcommand | Already used in slicecore-cli |
| semver | workspace | Plugin version compatibility | Already used in discovery.rs |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `#[sabi_trait]` | Manual vtable | More boilerplate, no layout verification at load time |
| `RVec<FfiGcodeCommand>` | Flattened encoding | Flattened encoding used for infill points is simpler but GcodeCommand variants are too heterogeneous |

**Installation:**
No new dependencies needed. All libraries are already workspace dependencies.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-plugin-api/src/
├── lib.rs                    # Add re-exports for new types
├── types.rs                  # Existing infill types
├── postprocess_types.rs      # NEW: FfiGcodeCommand, PostProcessRequest, PostProcessResult, FfiPrintConfigSnapshot
├── traits.rs                 # Existing InfillPatternPlugin
├── postprocess_traits.rs     # NEW: GcodePostProcessorPlugin #[sabi_trait], PostProcessorPluginMod
├── metadata.rs               # Extend PluginCapability, PluginType enums
└── error.rs                  # Existing (reusable)

crates/slicecore-plugin/src/
├── registry.rs               # Extend PluginRegistry with postprocessor_plugins HashMap
├── convert.rs                # Existing infill conversions
├── postprocess_convert.rs    # NEW: GcodeCommand <-> FfiGcodeCommand conversion
├── discovery.rs              # Existing (handles new capability variant automatically)
├── native.rs                 # Extend to load PostProcessorPluginMod
└── postprocess.rs            # NEW: PostProcessorPluginAdapter trait + pipeline runner

crates/slicecore-engine/src/
├── engine.rs                 # Insert post-processing hook in pipeline
├── config.rs                 # Add PostProcessConfig section to PrintConfig
├── postprocess_builtin.rs    # NEW: 4 built-in post-processors
└── lib.rs                    # Re-exports

crates/slicecore-cli/src/
└── main.rs                   # Add PostProcess subcommand
```

### Pattern 1: FFI-Safe Post-Processor Trait (replicating InfillPatternPlugin)
**What:** Define `GcodePostProcessorPlugin` with `#[sabi_trait]` for FFI safety
**When to use:** All post-processor plugins (native, WASM, built-in)
**Example:**
```rust
// In slicecore-plugin-api/src/postprocess_traits.rs
#[sabi_trait]
pub trait GcodePostProcessorPlugin: Send + Sync + Debug {
    fn name(&self) -> RString;
    fn description(&self) -> RString;

    /// Process entire G-code stream. Default returns input unchanged.
    fn process_all(
        &self,
        request: &PostProcessRequest,
    ) -> RResult<PostProcessResult, RString>;

    /// Process a single layer's commands. Default returns input unchanged.
    fn process_layer(
        &self,
        request: &LayerPostProcessRequest,
    ) -> RResult<PostProcessResult, RString>;

    /// Which processing mode this plugin supports.
    #[sabi(last_prefix_field)]
    fn processing_mode(&self) -> ProcessingMode;
}
```

### Pattern 2: FfiGcodeCommand Enum (mirroring GcodeCommand)
**What:** FFI-safe counterpart to `GcodeCommand` with `StableAbi` derive
**When to use:** All data crossing plugin FFI boundary
**Example:**
```rust
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum FfiGcodeCommand {
    Comment(RString),
    LinearMove { x: ROption<f64>, y: ROption<f64>, z: ROption<f64>, e: ROption<f64>, f: ROption<f64> },
    RapidMove { x: ROption<f64>, y: ROption<f64>, z: ROption<f64>, f: ROption<f64> },
    SetFanSpeed(u8),
    SetExtruderTemp { temp: f64, wait: bool },
    SetBedTemp { temp: f64, wait: bool },
    Retract { distance: f64, feedrate: f64 },
    Unretract { distance: f64, feedrate: f64 },
    Dwell { ms: u32 },
    RawGcode(RString),  // Arbitrary printer-specific codes
    // ... all other GcodeCommand variants
}
```

### Pattern 3: Host-Side Adapter Trait (mirroring InfillPluginAdapter)
**What:** Internal trait wrapping native/WASM/built-in post-processors uniformly
**When to use:** Registry lookup and pipeline execution
**Example:**
```rust
pub trait PostProcessorPluginAdapter: Send + Sync {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn priority(&self) -> i32;
    fn process(
        &self,
        commands: &[GcodeCommand],
        config: &PrintConfigSnapshot,
    ) -> Result<Vec<GcodeCommand>, PluginSystemError>;
    fn processing_mode(&self) -> ProcessingMode;
    fn plugin_type(&self) -> PluginKind;
}
```

### Pattern 4: Pipeline Runner (ordered execution)
**What:** Execute post-processor plugins in priority order with cancellation checks
**When to use:** Engine pipeline step 4d
**Example:**
```rust
pub fn run_post_processors(
    commands: Vec<GcodeCommand>,
    plugins: &[&dyn PostProcessorPluginAdapter],
    config: &PrintConfigSnapshot,
    event_bus: Option<&EventBus>,
    cancel: Option<&CancellationToken>,
) -> Result<Vec<GcodeCommand>, EngineError> {
    let mut result = commands;
    let total = plugins.len();
    for (i, plugin) in plugins.iter().enumerate() {
        if let Some(token) = cancel {
            if token.is_cancelled() {
                return Err(EngineError::Cancelled);
            }
        }
        if let Some(bus) = event_bus {
            let progress = 0.91 + 0.04 * (i as f32 / total as f32);
            bus.emit(&SliceEvent::StageChanged {
                stage: format!("post_processing:{}", plugin.name()),
                progress,
            });
        }
        result = plugin.process(&result, config)?;
    }
    Ok(result)
}
```

### Anti-Patterns to Avoid
- **Passing full PrintConfig across FFI:** PrintConfig is complex and not StableAbi. Create a `FfiPrintConfigSnapshot` with only the fields post-processors need (nozzle_diameter, layer_height, speeds, temperatures, bed size).
- **Mutating commands in place:** The functional pattern (input -> output) is cleaner and matches the infill plugin pattern. Let plugins return new Vec, don't pass &mut Vec.
- **Ordering plugins by registration order:** Explicit priority is more predictable. Default priority 100, lower runs first.
- **Skipping time estimation after post-processing:** The user decisions explicitly require recomputing time estimation and filament usage after post-processing.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| FFI-safe trait objects | Manual vtable + transmute | `#[sabi_trait]` from abi_stable | Layout verification at load time prevents UB |
| Plugin discovery | Custom directory walker | Existing `discover_plugins()` in discovery.rs | Already handles manifests, version checking |
| G-code parsing (for CLI) | Custom parser | Existing `GcodeParser` in slicecore-gcode-io | Already handles all dialects and state tracking |
| Config serialization | Manual TOML builder | serde derive on config structs | Consistent with PrintConfig pattern |

**Key insight:** The infill plugin system already solved every hard problem (FFI safety, discovery, native/WASM loading, registry management). This phase is predominantly pattern replication with G-code-specific types.

## Common Pitfalls

### Pitfall 1: StableAbi Enum Size
**What goes wrong:** `FfiGcodeCommand` with many variants and `RString` fields becomes large per-element, causing performance issues with large G-code streams (100K+ commands).
**Why it happens:** Each enum instance is sized to the largest variant.
**How to avoid:** Keep variants lean. Use ROption<f64> not Option<f64> (ROption is StableAbi). Consider grouping rarely-used fields. Profile with realistic G-code sizes.
**Warning signs:** Allocation spikes during conversion, slow post-processing on large prints.

### Pitfall 2: GcodeCommand Lacks Clone-Friendliness
**What goes wrong:** GcodeCommand derives Clone but contains String fields -- cloning large command streams is expensive.
**Why it happens:** The functional transform pattern requires producing new Vec<GcodeCommand>.
**How to avoid:** For read-only analysis plugins, detect no-change and return the original vec. For modifications, clone only the changed commands.
**Warning signs:** Memory doubling during post-processing.

### Pitfall 3: Non-Local Definitions Lint
**What goes wrong:** `#[sabi_trait]` macro expansion triggers `non_local_definitions` lint on newer Rust compilers.
**Why it happens:** The abi_stable macro generates impl blocks outside the defining crate.
**How to avoid:** Add `#![allow(non_local_definitions)]` at crate level in slicecore-plugin-api, matching the existing pattern.
**Warning signs:** CI clippy failures after adding new sabi_trait.

### Pitfall 4: process_all vs process_layer Confusion
**What goes wrong:** A plugin implements both modes, and the host calls the wrong one.
**Why it happens:** Unclear dispatch logic.
**How to avoid:** Plugin declares `ProcessingMode::All`, `ProcessingMode::PerLayer`, or `ProcessingMode::Both` via the `processing_mode()` method. Host checks mode and dispatches accordingly. For `PerLayer` mode, host splits commands by layer boundaries (using Comment TYPE annotations or Z-change detection).
**Warning signs:** Plugins receiving empty command lists or wrong layer boundaries.

### Pitfall 5: Plugin Priority Collisions
**What goes wrong:** Two plugins with same priority have non-deterministic ordering.
**Why it happens:** HashMap iteration order or unstable sort.
**How to avoid:** Use stable sort on priority. Break ties by plugin name (alphabetical). Document this behavior.
**Warning signs:** Non-deterministic G-code output when multiple plugins enabled.

### Pitfall 6: WASM Target Compatibility
**What goes wrong:** New types or dependencies break the WASM CI gate.
**Why it happens:** wasmtime is cfg-gated for non-WASM targets, but slicecore-plugin-api must compile on all targets.
**How to avoid:** Keep slicecore-plugin-api dependency-light (only abi_stable, serde). Don't add wasmtime-dependent code to the API crate. Follow existing cfg-gate patterns.
**Warning signs:** WASM CI build failures.

## Code Examples

### FfiGcodeCommand Definition
```rust
// Must mirror every GcodeCommand variant with FFI-safe types
use abi_stable::std_types::{ROption, RString};
use abi_stable::StableAbi;

#[repr(u8)]  // Explicit discriminant for ABI stability
#[derive(StableAbi, Clone, Debug)]
pub enum FfiGcodeCommand {
    Comment(RString),
    LinearMove {
        x: ROption<f64>,
        y: ROption<f64>,
        z: ROption<f64>,
        e: ROption<f64>,
        f: ROption<f64>,
    },
    RapidMove {
        x: ROption<f64>,
        y: ROption<f64>,
        z: ROption<f64>,
        f: ROption<f64>,
    },
    Home { x: bool, y: bool, z: bool },
    SetAbsolutePositioning,
    SetRelativePositioning,
    SetAbsoluteExtrusion,
    SetRelativeExtrusion,
    SetExtruderTemp { temp: f64, wait: bool },
    SetBedTemp { temp: f64, wait: bool },
    SetFanSpeed(u8),
    FanOff,
    ResetExtruder,
    Dwell { ms: u32 },
    Retract { distance: f64, feedrate: f64 },
    Unretract { distance: f64, feedrate: f64 },
    ArcMoveCW {
        x: ROption<f64>,
        y: ROption<f64>,
        i: f64,
        j: f64,
        e: ROption<f64>,
        f: ROption<f64>,
    },
    ArcMoveCCW {
        x: ROption<f64>,
        y: ROption<f64>,
        i: f64,
        j: f64,
        e: ROption<f64>,
        f: ROption<f64>,
    },
    SetAcceleration { print_accel: f64, travel_accel: f64 },
    SetJerk { x: f64, y: f64, z: f64 },
    SetPressureAdvance { value: f64 },
    ToolChange(u8),
    RawGcode(RString),
}
```

### GcodeCommand <-> FfiGcodeCommand Conversion
```rust
// In slicecore-plugin/src/postprocess_convert.rs
pub fn gcode_to_ffi(cmd: &GcodeCommand) -> FfiGcodeCommand {
    match cmd {
        GcodeCommand::Comment(s) => FfiGcodeCommand::Comment(RString::from(s.as_str())),
        GcodeCommand::LinearMove { x, y, z, e, f } => FfiGcodeCommand::LinearMove {
            x: option_to_roption(*x),
            y: option_to_roption(*y),
            z: option_to_roption(*z),
            e: option_to_roption(*e),
            f: option_to_roption(*f),
        },
        GcodeCommand::Raw(s) => FfiGcodeCommand::RawGcode(RString::from(s.as_str())),
        // ... all variants
    }
}

pub fn ffi_to_gcode(cmd: &FfiGcodeCommand) -> GcodeCommand {
    match cmd {
        FfiGcodeCommand::Comment(s) => GcodeCommand::Comment(s.to_string()),
        FfiGcodeCommand::LinearMove { x, y, z, e, f } => GcodeCommand::LinearMove {
            x: roption_to_option(*x),
            y: roption_to_option(*y),
            z: roption_to_option(*z),
            e: roption_to_option(*e),
            f: roption_to_option(*f),
        },
        FfiGcodeCommand::RawGcode(s) => GcodeCommand::Raw(s.to_string()),
        // ... all variants
    }
}
```

### PrintConfigSnapshot for FFI
```rust
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct FfiPrintConfigSnapshot {
    pub nozzle_diameter: f64,
    pub layer_height: f64,
    pub first_layer_height: f64,
    pub bed_x: f64,
    pub bed_y: f64,
    pub print_speed: f64,
    pub travel_speed: f64,
    pub retract_length: f64,
    pub retract_speed: f64,
    pub nozzle_temp: f64,
    pub bed_temp: f64,
    pub fan_speed: u8,
    pub total_layers: u64,
}
```

### Built-in Post-Processor Example (Pause at Layer)
```rust
pub struct PauseAtLayerPlugin {
    layers: Vec<usize>,  // Layer indices to pause at
}

impl PostProcessorPluginAdapter for PauseAtLayerPlugin {
    fn name(&self) -> String { "pause-at-layer".to_string() }
    fn priority(&self) -> i32 { 100 }
    fn processing_mode(&self) -> ProcessingMode { ProcessingMode::All }

    fn process(
        &self,
        commands: &[GcodeCommand],
        _config: &PrintConfigSnapshot,
    ) -> Result<Vec<GcodeCommand>, PluginSystemError> {
        if self.layers.is_empty() {
            return Ok(commands.to_vec());
        }
        let mut result = Vec::with_capacity(commands.len() + self.layers.len() * 3);
        let mut current_layer = 0usize;
        for cmd in commands {
            // Detect layer change via TYPE comment
            if let GcodeCommand::Comment(text) = cmd {
                if text.starts_with("LAYER_CHANGE") {
                    current_layer += 1;
                }
            }
            result.push(cmd.clone());
            if self.layers.contains(&current_layer) {
                // Check immediately after LAYER_CHANGE comment
                if let GcodeCommand::Comment(text) = cmd {
                    if text.starts_with("LAYER_CHANGE") {
                        result.push(GcodeCommand::Comment(format!(
                            "Pause at layer {} inserted by pause-at-layer plugin",
                            current_layer
                        )));
                        result.push(GcodeCommand::Raw("M0".to_string()));
                    }
                }
            }
        }
        Ok(result)
    }
}
```

### Engine Pipeline Integration Point
```rust
// In engine.rs, after step 4c (purge tower), before step 5 (time estimation):

// 4d. Post-processing plugins (optional).
let gcode_commands = if !self.post_processors.is_empty() {
    if let Some(bus) = event_bus {
        bus.emit(&SliceEvent::StageChanged {
            stage: "post_processing".to_string(),
            progress: 0.91,
        });
    }
    run_post_processors(
        gcode_commands,
        &self.post_processors,
        &config_snapshot,
        event_bus,
        cancel_token,
    )?
} else {
    gcode_commands
};

// 5. Compute estimated time (always recomputed after post-processing).
```

### plugin.toml Extension for Post-Processors
```toml
[plugin]
name = "pause-at-layer"
version = "0.1.0"
description = "Insert pause (M0/M600) at specified layer numbers"
author = "slicecore"
license = "MIT OR Apache-2.0"
min_api_version = "0.1.0"
max_api_version = "0.2.0"

[plugin.type]
kind = "builtin"

[capabilities]
provides = ["gcode_post_processor"]

[config]
# Plugin-defined configuration schema
[config.parameters]
layers = { type = "array", item_type = "integer", default = [], description = "Layer numbers to pause at" }
pause_command = { type = "string", default = "M0", description = "Pause command (M0 or M600)" }
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| String-based G-code post-processing scripts (PrusaSlicer) | Typed command-level post-processing | This phase | Type safety, no regex parsing errors |
| External Python/bash scripts | In-engine plugin system | This phase | No external dependencies, sandboxed execution |
| Post-processing only at print time | `slicecore post-process` CLI | This phase | Re-process without re-slicing, process G-code from other slicers |

**Deprecated/outdated:**
- External post-processing script approach (PrusaSlicer `--post-process` flag): Error-prone, security risk, requires external runtime

## Open Questions

1. **Layer Boundary Detection for PerLayer Mode**
   - What we know: G-code comments contain `LAYER_CHANGE` markers and `TYPE:` annotations
   - What's unclear: Exact split strategy -- split on LAYER_CHANGE comment, include the comment in which group?
   - Recommendation: Split before each LAYER_CHANGE comment. The LAYER_CHANGE comment is the first command of each layer group. Layer 0 is everything before the first LAYER_CHANGE.

2. **Per-Plugin Config Parameter Types**
   - What we know: plugin.toml will define a config schema
   - What's unclear: How to represent typed config values across FFI (RVec<(RString, RString)> key-value pairs? Or a more structured approach?)
   - Recommendation: Use `RVec<FfiConfigParam>` where `FfiConfigParam` is an enum with `StringVal(RString)`, `IntVal(i64)`, `FloatVal(f64)`, `BoolVal(bool)`, `StringListVal(RVec<RString>)`. Simple, extensible, StableAbi-safe.

3. **Built-in Post-Processors in Engine vs Plugin Crate**
   - What we know: Built-ins need access to PrintConfig for configuration
   - What's unclear: Should built-ins live in `slicecore-engine` (access to full config) or `slicecore-plugin` (alongside plugin infra)?
   - Recommendation: Built-ins in `slicecore-engine` (like built-in infill patterns), registered as `PluginKind::Builtin`. They implement `PostProcessorPluginAdapter` directly without FFI overhead.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml workspace test settings |
| Quick run command | `cargo test -p slicecore-plugin-api -p slicecore-plugin -p slicecore-engine --lib` |
| Full suite command | `cargo test --all-features --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ADV-04 | Custom G-code injection (per-layer, per-feature hooks) | integration | `cargo test -p slicecore-engine --test post_process_integration` | Wave 0 |
| PLUGIN-01 | Plugin trait API (post-processor extension) | unit | `cargo test -p slicecore-plugin-api postprocess` | Wave 0 |
| PLUGIN-02 | PluginRegistry (post-processor registration) | unit | `cargo test -p slicecore-plugin registry::tests` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-plugin-api -p slicecore-plugin -p slicecore-engine --lib`
- **Per wave merge:** `cargo test --all-features --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-plugin-api/src/postprocess_types.rs` -- FfiGcodeCommand, PostProcessRequest/Result types
- [ ] `crates/slicecore-plugin-api/src/postprocess_traits.rs` -- GcodePostProcessorPlugin trait
- [ ] `crates/slicecore-plugin/src/postprocess_convert.rs` -- GcodeCommand <-> FfiGcodeCommand conversion
- [ ] `crates/slicecore-plugin/src/postprocess.rs` -- PostProcessorPluginAdapter, pipeline runner
- [ ] `crates/slicecore-engine/src/postprocess_builtin.rs` -- 4 built-in post-processors
- [ ] `crates/slicecore-engine/tests/post_process_integration.rs` -- integration tests

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `slicecore-plugin-api/src/traits.rs` -- InfillPatternPlugin #[sabi_trait] pattern
- Codebase inspection: `slicecore-plugin-api/src/types.rs` -- FfiInfillLine, InfillRequest, InfillResult FFI types
- Codebase inspection: `slicecore-plugin/src/registry.rs` -- PluginRegistry structure, InfillPluginAdapter trait
- Codebase inspection: `slicecore-gcode-io/src/commands.rs` -- GcodeCommand enum (22 variants)
- Codebase inspection: `slicecore-engine/src/engine.rs:1704-1745` -- Pipeline insertion point
- Codebase inspection: `slicecore-engine/src/event.rs` -- SliceEvent, EventBus, StageChanged pattern
- Codebase inspection: `slicecore-plugin/src/discovery.rs` -- Plugin manifest discovery pattern
- Codebase inspection: `slicecore-plugin-api/src/metadata.rs` -- PluginCapability, PluginManifest structs
- Codebase inspection: `slicecore-plugin/src/error.rs` -- PluginSystemError enum
- Codebase inspection: `slicecore-engine/src/custom_gcode.rs` -- Existing custom G-code hooks (complementary, not replaced)

### Secondary (MEDIUM confidence)
- abi_stable 0.11 documentation -- ROption, RString, RVec, StableAbi derive for enums

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All libraries already in use, no new dependencies
- Architecture: HIGH -- Direct replication of proven infill plugin pattern with domain-specific types
- Pitfalls: HIGH -- Identified from existing codebase patterns and prior phase decisions (e.g., non_local_definitions lint)

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable -- internal architecture, no external API dependencies)
