# Phase 28: G-code Post-Processing Plugin Point - Context

**Gathered:** 2026-03-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Extend the plugin system to support G-code post-processing — plugins that can modify, filter, inject, or analyze G-code commands after generation but before final output. Includes FFI-safe plugin trait, pipeline integration, standalone CLI command, config-file/CLI/API activation, and built-in post-processor plugins. Does NOT include new G-code command types, new slicing algorithms, or GUI-level plugin management.

</domain>

<decisions>
## Implementation Decisions

### Plugin Interface Design
- Both processing modes available: `process_all(commands) -> commands` for full-stream transforms AND `process_layer(layer_commands, layer_index) -> commands` for per-layer logic. Registry calls whichever the plugin implements
- Plugins receive G-code commands + read-only PrintConfig snapshot (nozzle diameter, layer height, speeds, etc.) — enables config-aware transformations without exposing engine internals
- Return type: modified command vec (new `Vec<FfiGcodeCommand>`). Clean functional style matching infill plugin's request/result pattern
- FFI boundary uses `StableAbi`-derived wrapper types (`FfiGcodeCommand`) in `slicecore-plugin-api`, with conversion to/from internal `GcodeCommand`. Consistent with existing `FfiInfillLine` pattern
- Support both typed `FfiGcodeCommand` variants AND a `RawGcode(RString)` variant for arbitrary printer-specific codes (M600, Klipper macros, custom G-code). Best of both worlds for printer-specific customization

### Pipeline Insertion Point
- Post-processor plugins run after all built-in processing (arc fitting + purge tower), before time estimation and statistics
- Time estimation and filament usage always recomputed after post-processing to ensure stats reflect actual output
- Multiple plugins ordered by explicit priority: each plugin declares priority in manifest or config, plugins run in priority order, user can override order in config file
- Integrates with Phase 23 progress/cancellation API: emit `StageChanged("post_processing", progress)` events, cancellation token checked between plugin invocations

### Use Case Scope — All Four Capabilities
- **Command modification**: modify existing commands (override fan speed, adjust feedrate, change temperature)
- **Command injection**: insert new commands at specific points (pause at layer, filament change, camera park for timelapse, nozzle wipe)
- **Command removal/filtering**: remove or skip commands (strip comments, remove redundant moves)
- **Read-only analysis**: inspect without modifying (validate constraints, count retractions, check for issues) — return unmodified commands

### Built-in Post-Processors (v1)
- **Pause at layer**: insert M0/M600 at specified layer number(s) for filament color changes or inspection
- **Timelapse camera**: move print head to park position at layer changes for camera capture; configurable park position, dwell time, retraction
- **Fan speed override**: override fan speed rules per layer range (e.g., 100% after layer 5, gradual ramp)
- **Custom G-code injection**: generic plugin — inject user-specified G-code string at configurable triggers (every N layers, at specific layers, before/after retraction). Covers nozzle wipe, camera, and other custom needs

### CLI & Config Integration
- Auto-discover: all discovered post-processor plugins are available automatically
- Enable/disable via config file, CLI flags, and API — config file is primary, CLI and API can override
- Standalone CLI command: `slicecore post-process` reads existing G-code file, runs specified plugins, writes output — enables re-processing without re-slicing and processing G-code from other slicers
- Per-plugin configuration via plugin-defined schema in `plugin.toml` (parameter name, type, default). Config file and CLI pass values as key-value pairs. Plugin receives params as FFI-safe map
- Self-skipping: plugins return input unchanged if nothing applies. No special API — engine can detect no-change case

### Claude's Discretion
- Exact `FfiGcodeCommand` variant definitions and field layouts
- Internal conversion code between `GcodeCommand` and `FfiGcodeCommand`
- `plugin.toml` schema extensions for post-processor type and config parameters
- Built-in post-processor implementation details
- Plugin priority default values and ordering algorithm
- How `process_layer` vs `process_all` selection works in the registry
- Standalone `post-process` CLI subcommand argument design

</decisions>

<specifics>
## Specific Ideas

- Users want to wipe the nozzle at a certain layer frequency — custom G-code injection plugin handles this
- Users want to move print head to home so camera can take a photo (timelapse), then resume — timelapse camera built-in handles this
- Custom G-code is especially helpful for different printers and features — the `RawGcode(RString)` variant enables printer-specific codes without needing new typed variants
- Plugins should be intelligent enough to skip themselves if not required — return input unchanged pattern
- Users should be able to run post-processing after slicing as a separate step — standalone `slicecore post-process` CLI command
- Configuration file is the primary mechanism for specifying and overriding default plugin values, in addition to CLI options and API methods

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `slicecore-plugin-api` crate: `InfillPatternPlugin` trait with `#[sabi_trait]`, `FfiInfillLine`/`InfillRequest`/`InfillResult` FFI-safe types — pattern to replicate for `GcodePostProcessorPlugin`
- `slicecore-plugin` crate: `PluginRegistry` with `discover_and_load()`, `register_*()`, `get_*()` — extend with `register_postprocessor()`, `get_postprocessor()`, etc.
- `InfillPluginMod` / `InfillPluginMod_Ref`: `abi_stable` `RootModule` pattern for native plugin entry points — replicate as `PostProcessorPluginMod`
- `plugin.toml` manifest with `PluginManifest` struct — extend with post-processor type and config schema
- `SandboxConfig` for WASM plugin resource limits — reusable for post-processor WASM plugins

### Established Patterns
- Plugin trait uses `#[sabi_trait]` for FFI-safe trait objects
- Plugin types: `PluginKind::Native`, `PluginKind::Wasm`, `PluginKind::Builtin`
- Host-side adapter trait (`InfillPluginAdapter`) wraps FFI trait objects with standard Rust error handling
- Existing plugin examples in `plugins/examples/` for both native and WASM
- G-code pipeline in `engine.rs`: generate → arc fitting → purge tower → [POST-PROCESS HOOK HERE] → time estimate → stats → write
- `GcodeWriter` in `slicecore-gcode-io` handles serialization to text

### Integration Points
- `engine.rs` lines 1705-1745: insert post-processing call after purge tower, before time estimation (line 1747)
- `PluginRegistry`: add `postprocessor_plugins` HashMap alongside `infill_plugins`
- `plugin.toml` / `PluginManifest`: add `PostProcessor` variant to `PluginType` enum
- `slicecore-cli/src/main.rs`: add `post-process` subcommand
- `PrintConfig`: add post-processor enable/disable and per-plugin config sections
- Event system: add `StageChanged("post_processing", ...)` emission

</code_context>

<deferred>
## Deferred Ideas

- Plugin dependency graph / run-before / run-after relationships — overkill for v1, consider if plugin ecosystem grows
- GUI-level plugin management UI — separate phase
- Plugin marketplace / remote plugin installation — future feature
- Streaming post-processing (process commands as they're generated, not batch) — optimization for large prints
- Plugin-provided diagnostics/warnings returned alongside modified commands — v2 enhancement
- `should_run()` pre-check method for fast skip without passing full command stream — optimization if needed

</deferred>

---

*Phase: 28-g-code-post-processing-plugin-point*
*Context gathered: 2026-03-12*
