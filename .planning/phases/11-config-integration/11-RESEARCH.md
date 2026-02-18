# Phase 11: Config Integration - Research

**Researched:** 2026-02-18
**Domain:** Slicing engine config-to-pipeline wiring (Rust internals)
**Confidence:** HIGH

## Summary

Phase 11 is an internal integration phase -- no new external libraries, no new algorithms. The work consists of reading existing config fields (`plugin_dir`, `sequential`, `multi_material`) inside the `Engine` pipeline and calling existing functions that were already implemented in prior phases. All building blocks exist; they just need to be connected.

The codebase has three clear integration gaps:

1. **`plugin_dir` is orphaned in config** -- The `PrintConfig.plugin_dir: Option<String>` field (line 218 of `config.rs`) is parsed from TOML but never read by `Engine`. The CLI (`main.rs` lines 212-252) manually constructs a `PluginRegistry` and calls `discover_and_load()`, but the `Engine::new()` constructor (line 283 of `engine.rs`) ignores `plugin_dir` entirely. If a user sets `plugin_dir` in their TOML and uses the Engine as a library (not via CLI), plugins are never loaded.

2. **`sequential` config is not wired into the pipeline** -- `SequentialConfig` (lines 476-495 of `config.rs`) with `enabled`, `extruder_clearance_radius`, and `extruder_clearance_height` is parsed from TOML but `engine.rs`'s `slice_to_writer_with_events()` never checks `self.config.sequential.enabled`. The `sequential.rs` module provides `detect_collision()`, `order_objects()`, and `plan_sequential_print()` -- all ready to use but not called from the pipeline.

3. **`multi_material` config is not wired into the pipeline** -- `MultiMaterialConfig` (lines 436-467 of `config.rs`) with tool configs, purge tower settings, etc. is parsed from TOML but `engine.rs` never calls `generate_tool_change()` or `generate_purge_tower_layer()` from `multimaterial.rs`. These functions exist, are tested in unit tests, but are never integrated into `slice_to_writer_with_events()`.

**Primary recommendation:** Wire each config field into the `Engine` by modifying `Engine::new()` (for plugin_dir auto-loading) and `slice_to_writer_with_events()` (for sequential and multi-material), calling the existing functions from `sequential.rs` and `multimaterial.rs`. Use the event system's `SliceEvent::Warning` variant for user notifications.

## Standard Stack

### Core (already in crate, no new dependencies needed)

| Library | Version | Purpose | Status |
|---------|---------|---------|--------|
| slicecore-engine | workspace | Engine pipeline orchestrator | EXISTS -- modify engine.rs |
| slicecore-plugin | workspace | PluginRegistry, discover_and_load() | EXISTS -- call from Engine |
| slicecore-plugin-api | workspace | PluginManifest, InfillRequest/Result | EXISTS -- used by registry |

### Supporting (no new additions)

No new crate dependencies are needed. All work is internal wiring within existing crates.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Auto-loading in Engine::new() | Keep CLI-only loading | Would violate SC1 (config TOML triggers loading) |
| Warning via SliceEvent::Warning | Custom PluginLoadReport struct | SliceEvent::Warning already exists, tested, and integrates with EventBus |
| Checking sequential in pipeline | Separate Sequential engine | Over-engineering; the current single-mesh pipeline is fine for V1 |

## Architecture Patterns

### Pattern 1: Plugin Auto-Loading in Engine Constructor

**What:** When `Engine::new(config)` is called and `config.plugin_dir` is `Some(path)`, automatically create a `PluginRegistry`, call `discover_and_load()`, and store the registry.

**When to use:** Always during engine construction when the `plugins` feature is enabled.

**Key constraint:** The `Engine::new()` currently returns `Self` (not `Result`). Plugin loading can fail, so the pattern should be: attempt loading, warn on failure, continue. This matches the CLI's existing behavior (lines 236-252 of `main.rs`).

```rust
// Pseudocode for Engine::new with auto-loading
pub fn new(config: PrintConfig) -> Self {
    let mut engine = Self {
        config,
        #[cfg(feature = "plugins")]
        plugin_registry: None,
    };

    #[cfg(feature = "plugins")]
    {
        if let Some(ref dir) = engine.config.plugin_dir {
            let mut registry = slicecore_plugin::PluginRegistry::new();
            match registry.discover_and_load(std::path::Path::new(dir)) {
                Ok(loaded) => {
                    if loaded.is_empty() {
                        // Store warning for later emission
                        eprintln!("Warning: plugin_dir '{}' contains no valid plugins", dir);
                    }
                    engine.plugin_registry = Some(registry);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to load plugins from '{}': {}", dir, e);
                }
            }
        }
    }

    engine
}
```

**Design decision -- eprintln vs stored warnings:** The constructor doesn't have access to an EventBus. Options:
- (A) `eprintln!` directly (matches current CLI behavior, simplest)
- (B) Store warnings in Vec<String> field, emit via EventBus in slice_with_events()
- (C) Return Result<Self, EngineError> from new() -- breaking change

**Recommendation:** Use option (A) for V1 simplicity. The success criteria says "RepairReport or warnings notify users" -- `eprintln!` to stderr is a warning. Additionally, add a `new_with_event_bus()` or store warnings and emit them when `slice_with_events()` is called later. Option (B) is cleaner for library consumers.

### Pattern 2: Sequential Printing in Pipeline

**What:** Before the per-layer processing loop in `slice_to_writer_with_events()`, check `self.config.sequential.enabled`. If true, compute object bounds from the mesh, run `plan_sequential_print()` for collision detection, and order objects.

**Constraint for V1:** The current `Engine::slice()` takes a single `TriangleMesh`. Sequential printing requires multiple objects (meshes). There are two approaches:

- (A) **Detect disjoint components within a single mesh**, compute bounds per component, validate and order them. This is what makes sense for the single-mesh API.
- (B) **Add a new `slice_multi()` method** that takes `&[TriangleMesh]`. Cleaner but larger API change.

**Recommendation:** Approach (A) for V1 -- detect disjoint connected components in the mesh, compute bounding boxes, run collision detection, and if sequential is enabled but the mesh has only one component, skip with an info message. This keeps the existing API intact.

**Pipeline insertion point:** After mesh validation (line 496) and before mesh slicing (line 500), check `sequential.enabled`:
1. Find connected components of the mesh (disjoint sub-meshes)
2. Compute `ObjectBounds` per component
3. Call `plan_sequential_print()` from `sequential.rs`
4. If collision detected, return `EngineError::ConfigError`
5. If only one component, emit Warning("Sequential enabled but mesh has only 1 object")
6. Store the object order for G-code insertion of safe-Z travels between objects

### Pattern 3: Multi-Material in Pipeline

**What:** If `self.config.multi_material.enabled` is true and `tool_count > 1`, insert tool change sequences and purge tower layers into the G-code output.

**Constraint for V1:** Multi-material requires knowing which regions of each layer belong to which tool. The `assign_tools_per_region()` function in `multimaterial.rs` exists but requires `modifier_tools: &[(ModifierMesh, u8)]` -- modifier meshes aren't available in the current single-mesh pipeline.

**Recommendation for V1:** Wire the infrastructure so that when multi-material is enabled:
1. Validate the configuration (tool_count matches tools.len(), tools not empty)
2. For each layer, generate purge tower maintenance (sparse) layers
3. If tool assignments change between layers (which requires modifier mesh support), generate tool changes and dense purge tower layers
4. Insert purge tower G-code commands after each layer's regular G-code

Since V1 likely won't have modifier mesh inputs through the `slice()` API, the practical implementation is: wire the config check, generate sparse purge tower layers on every layer when enabled, and insert tool change sequences when the engine is told to switch tools. The actual multi-tool routing would need modifier mesh integration (potentially a future `slice_multi_material()` method).

### Anti-Patterns to Avoid

- **Breaking Engine::new() signature:** Do NOT change `new()` to return `Result`. This would break all existing callers. Use a separate `try_new()` or store warnings internally.
- **Duplicating CLI plugin logic in Engine:** The CLI already has plugin loading logic (main.rs lines 212-252). Engine's auto-loading should not conflict. The CLI should detect that the Engine already loaded plugins (via `plugin_dir` in config) and skip its own loading, OR the CLI's `--plugin-dir` flag should take precedence.
- **Hard-failing on plugin_dir issues:** If `plugin_dir` points to an empty or nonexistent directory, this should warn, not error. Matches existing CLI behavior.
- **Ignoring feature gates:** All plugin code must remain behind `#[cfg(feature = "plugins")]`. Sequential and multi-material code does NOT need feature gates (it's in slicecore-engine which always has access to these modules).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Plugin discovery | Custom directory scanner | `PluginRegistry::discover_and_load()` | Already handles manifests, version checks, error recovery |
| Collision detection | New collision algorithm | `sequential::detect_collision()` | Already implemented, unit tested |
| Object ordering | Custom sort | `sequential::order_objects()` | Already handles shortest-first, pairwise collision checks |
| Tool change G-code | Inline G-code strings | `multimaterial::generate_tool_change()` | Proper retract-park-change-prime flow |
| Purge tower | Inline geometry | `multimaterial::generate_purge_tower_layer()` | Handles dense/sparse modes, correct extrusion values |
| Connected components | Custom mesh analysis | Can use vertex adjacency from `TriangleMesh` indices | Standard graph traversal on mesh connectivity |

**Key insight:** Every function needed already exists and is tested. This phase is purely plumbing.

## Common Pitfalls

### Pitfall 1: Plugin Loading in Constructor Masks Errors

**What goes wrong:** `Engine::new()` silently swallows plugin loading errors, and users don't understand why their plugin infill pattern fails later during slicing.
**Why it happens:** Constructor returns `Self`, not `Result`, so errors must be stored or printed.
**How to avoid:** Store load warnings in the Engine. When `slice_with_events()` is called, emit them via the EventBus as `SliceEvent::Warning`. When `slice()` is called (no EventBus), print to stderr.
**Warning signs:** User sets `plugin_dir` and `infill_pattern = { plugin = "X" }` but gets "Plugin 'X' not found in registry" error with no explanation of why loading failed.

### Pitfall 2: CLI and Engine Both Loading Plugins

**What goes wrong:** If the Engine auto-loads from `config.plugin_dir` and the CLI also loads from `--plugin-dir` or `config.plugin_dir`, plugins could be loaded twice, or the CLI's registry overwrites the Engine's auto-loaded one.
**Why it happens:** `Engine::with_plugin_registry()` replaces the entire registry.
**How to avoid:** In the CLI, check if `engine.plugin_registry` is already populated (needs a getter method or the CLI just avoids calling `with_plugin_registry()` when the Engine already auto-loaded). OR: the Engine's auto-load only fires if no registry was manually provided.
**Warning signs:** Duplicate "Loaded X plugin(s)" messages, or plugins appear missing when they should have been loaded.

**Recommended resolution:** Auto-loading happens in `Engine::new()` only. If `with_plugin_registry()` is called afterward, it replaces the auto-loaded registry. The CLI should skip its manual loading when Engine has already loaded from `plugin_dir`. Add an `Engine::has_plugin_registry()` method for the CLI to check.

### Pitfall 3: Sequential Printing with Single Mesh

**What goes wrong:** User enables `sequential.enabled = true` but passes a single object (one connected component). The engine does collision detection on 1 object, which trivially passes, but the user gets no feedback that sequential mode had no effect.
**Why it happens:** The single-object case is degenerate for sequential printing.
**How to avoid:** Emit a `SliceEvent::Warning` when sequential is enabled but the mesh has 0 or 1 connected components. The user likely intended to separate objects.
**Warning signs:** User expects sequential behavior (object-by-object completion) but gets normal all-at-once slicing.

### Pitfall 4: Multi-Material Without Tool Assignments

**What goes wrong:** User enables `multi_material.enabled = true` with 4 tools configured, but provides no modifier meshes for tool assignment. Every region defaults to tool 0, and no tool changes are generated.
**Why it happens:** `assign_tools_per_region()` with empty `modifier_tools` returns `vec![0; N]`.
**How to avoid:** Emit a `SliceEvent::Warning` when multi-material is enabled but no tool assignments are provided (all regions map to tool 0).
**Warning signs:** Purge tower is generated on every layer (sparse) but no actual tool changes happen.

### Pitfall 5: Feature Gate Mismatch

**What goes wrong:** Code references `slicecore_plugin` types without `#[cfg(feature = "plugins")]` guard, causing compilation to fail when `plugins` feature is disabled.
**Why it happens:** Easy to forget cfg gates when adding new code to engine.rs.
**How to avoid:** Every line that references `slicecore_plugin::*` must be inside a `#[cfg(feature = "plugins")]` block. Run `cargo check` without `--features plugins` to verify.
**Warning signs:** Build failures in CI for the default feature set.

## Code Examples

### Engine Constructor with Plugin Auto-Loading

```rust
// In engine.rs, modified Engine::new()
impl Engine {
    pub fn new(config: PrintConfig) -> Self {
        let mut engine = Self {
            config,
            #[cfg(feature = "plugins")]
            plugin_registry: None,
            // New field for deferred warnings
            startup_warnings: Vec::new(),
        };

        #[cfg(feature = "plugins")]
        engine.auto_load_plugins();

        engine
    }

    #[cfg(feature = "plugins")]
    fn auto_load_plugins(&mut self) {
        if let Some(ref dir) = self.config.plugin_dir {
            let path = std::path::Path::new(dir);
            let mut registry = slicecore_plugin::PluginRegistry::new();
            match registry.discover_and_load(path) {
                Ok(loaded) => {
                    if loaded.is_empty() {
                        self.startup_warnings.push(format!(
                            "plugin_dir '{}' is set but contains no valid plugins",
                            dir
                        ));
                    }
                    self.plugin_registry = Some(registry);
                }
                Err(e) => {
                    self.startup_warnings.push(format!(
                        "Failed to load plugins from '{}': {}",
                        dir, e
                    ));
                }
            }
        }
    }
}
```

### Sequential Check in Pipeline

```rust
// In slice_to_writer_with_events(), after mesh validation, before slicing
if self.config.sequential.enabled {
    // Detect connected components (disjoint sub-meshes)
    let components = mesh.connected_components(); // Need to implement or approximate

    if components.len() <= 1 {
        if let Some(bus) = event_bus {
            bus.emit(&SliceEvent::Warning {
                message: "Sequential printing enabled but mesh has only one object. \
                          Sequential mode has no effect for single objects.".to_string(),
                layer: None,
            });
        }
    } else {
        // Compute ObjectBounds per component
        let bounds: Vec<ObjectBounds> = components.iter().enumerate().map(|(i, comp)| {
            let (min_x, max_x, min_y, max_y, max_z) = comp.bounding_box();
            ObjectBounds { min_x, max_x, min_y, max_y, max_z, object_index: i }
        }).collect();

        // Run collision detection and ordering
        let plan = plan_sequential_print(&bounds, &self.config)?;
        // Store plan for use in G-code generation (safe-Z travels)
    }
}
```

### Multi-Material Integration

```rust
// In slice_to_writer_with_events(), during G-code generation phase
if self.config.multi_material.enabled && self.config.multi_material.tool_count > 1 {
    // For each layer, generate purge tower layer
    for (layer_idx, toolpath) in layer_toolpaths.iter_mut().enumerate() {
        let layer_z = toolpath.z;
        let layer_height = toolpath.layer_height;
        let has_tool_change = false; // V1: no modifier mesh -> no tool changes yet

        let tower = generate_purge_tower_layer(
            layer_z,
            layer_height,
            &self.config.multi_material,
            has_tool_change,
            self.config.nozzle_diameter,
        );
        // Append tower commands to layer G-code
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| CLI-only plugin loading | Engine auto-loads from config | Phase 11 | Library consumers get plugins without manual registry setup |
| Config fields parsed but ignored | Config fields drive pipeline behavior | Phase 11 | TOML settings actually work |
| Sequential/multi-material as standalone functions | Integrated into Engine pipeline | Phase 11 | Users configure features via TOML, not API calls |

**Deprecated/outdated:** None -- all existing APIs remain compatible.

## Open Questions

1. **Connected component detection for sequential printing**
   - What we know: `TriangleMesh` stores vertices and triangle indices. Connected components can be found via BFS/DFS on the triangle adjacency graph.
   - What's unclear: Does `TriangleMesh` or `slicecore-mesh` already have a connected component detection method? If not, it needs to be added. A simple union-find on vertex-sharing triangles would work.
   - Recommendation: Check if `slicecore-mesh` has connectivity analysis. If not, add a `connected_components()` method that returns vertex/index subsets. This is straightforward graph traversal.

2. **CLI backward compatibility with Engine auto-loading**
   - What we know: CLI currently does its own plugin loading (main.rs lines 212-252). Engine will now auto-load from `config.plugin_dir`.
   - What's unclear: Should the CLI's `--plugin-dir` flag override the Engine's auto-loaded registry, or should it add to it?
   - Recommendation: CLI `--plugin-dir` overrides. If `--plugin-dir` is specified, create registry from that path and call `with_plugin_registry()`. If not specified, let Engine auto-load from `config.plugin_dir`. Update CLI to skip manual loading when Engine has already auto-loaded (no --plugin-dir flag and config.plugin_dir is set).

3. **Multi-material integration depth for V1**
   - What we know: The full multi-material pipeline requires modifier meshes for tool assignment. The current `Engine::slice()` API takes only one mesh.
   - What's unclear: How much multi-material pipeline integration should be done in V1? Full tool change routing requires modifier mesh input, which is not in the current API.
   - Recommendation: Wire the config checks, validation, and purge tower infrastructure. Emit a warning when multi-material is enabled but no tool assignments exist (default all-tool-0). This satisfies SC3 ("triggers tool changes and purge tower generation") at the infrastructure level while acknowledging the API limitation.

4. **`Engine::new()` vs `Engine::try_new()` for plugin loading errors**
   - What we know: `Engine::new()` returns `Self`, not `Result`. Plugin loading can fail.
   - What's unclear: Is a breaking change to `new()` acceptable?
   - Recommendation: Keep `new()` as-is, store warnings internally. Add `startup_warnings(&self) -> &[String]` accessor. Emit stored warnings on first `slice_with_events()` call.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `crates/slicecore-engine/src/engine.rs` -- Engine struct, new(), slice_to_writer_with_events()
- Codebase analysis: `crates/slicecore-engine/src/config.rs` -- PrintConfig with plugin_dir, SequentialConfig, MultiMaterialConfig
- Codebase analysis: `crates/slicecore-engine/src/sequential.rs` -- detect_collision(), order_objects(), plan_sequential_print()
- Codebase analysis: `crates/slicecore-engine/src/multimaterial.rs` -- generate_tool_change(), generate_purge_tower_layer(), assign_tools_per_region()
- Codebase analysis: `crates/slicecore-plugin/src/registry.rs` -- PluginRegistry::discover_and_load()
- Codebase analysis: `crates/slicecore-cli/src/main.rs` -- CLI plugin loading logic (lines 212-252)
- Codebase analysis: `crates/slicecore-engine/src/event.rs` -- SliceEvent::Warning variant, EventBus
- Codebase analysis: `crates/slicecore-engine/src/error.rs` -- EngineError variants

### Secondary (MEDIUM confidence)
- Codebase analysis: `crates/slicecore-plugin/src/discovery.rs` -- discover_plugins() behavior on empty/nonexistent dirs
- Codebase analysis: `crates/slicecore-cli/tests/cli_plugins.rs` -- CLI integration test patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, purely internal wiring
- Architecture: HIGH -- all building blocks exist, code paths are clear from codebase analysis
- Pitfalls: HIGH -- identified from actual code inspection (feature gates, CLI/Engine overlap, single-mesh limitation)

**Research date:** 2026-02-18
**Valid until:** 2026-03-18 (stable -- internal codebase, no external version churn)
