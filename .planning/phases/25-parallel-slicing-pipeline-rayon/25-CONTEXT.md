# Phase 25: Parallel Slicing Pipeline (rayon) - Context

**Gathered:** 2026-03-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Add rayon-based parallelism to the per-layer processing pipeline in slicecore-engine. Layers are processed in parallel for perimeters, surface classification, infill, and toolpath assembly. Mesh slicing and G-code generation remain sequential. Output must be bit-identical to sequential processing. WASM targets fall back to single-threaded via feature flag.

</domain>

<decisions>
## Implementation Decisions

### Parallelization scope
- Parallelize per-layer processing only (the `for (layer_idx, layer) in layers.iter().enumerate()` loop)
- Do NOT parallelize mesh slicing or G-code generation in this phase — defer full pipeline parallelism to future phase
- Apply parallelism to ALL slice methods: `slice()`, `slice_with_events()`, `slice_with_preview()`, `slice_with_modifiers()`
- Thread count: auto-detect via rayon defaults, with optional `thread_count` field in PrintConfig for user override

### Determinism
- Bit-identical output required: parallel G-code must be byte-for-byte identical to sequential G-code
- Two-pass seam alignment: first pass processes layers in parallel WITHOUT seam alignment, second pass applies sequential seam adjustment
- Lightning infill excluded from parallelism — process lightning layers sequentially (lightning builds cross-layer tree state)
- Add `parallel_slicing: bool` (default true) to PrintConfig for sequential fallback mode — enables diffing outputs for debugging

### WASM compatibility
- `parallel` Cargo feature flag on slicecore-engine crate only (default-enabled on native, disabled for WASM)
- When `parallel` feature is disabled, rayon is not compiled — engine runs single-threaded
- Create a `maybe_par_iter()` helper macro/function that returns `par_iter()` or `iter()` based on the feature flag — keeps engine.rs clean with zero `#[cfg]` in main logic
- No wasm-rayon integration in this phase — WASM targets run single-threaded

### Progress and cancellation
- Atomic counter (`AtomicUsize`) for progress — increments when any layer completes, progress = completed/total
- During parallel processing, only fire aggregate progress events (% complete) — suppress per-layer detail events (they'd arrive out of order)
- Per-layer events still work in sequential mode (when `parallel_slicing: false`)
- Cancellation strategy: Claude's discretion on whether to check between batches or per-layer

### Benchmarks
- Add criterion benchmarks comparing sequential vs parallel on a standard test mesh
- Measure wall time to validate parallelization provides actual speedup

### Claude's Discretion
- Exact rayon pool configuration and batch sizing
- Cancellation check strategy (between batches vs per-layer)
- How to structure the two-pass seam alignment implementation
- Whether to refactor duplicated slice methods before parallelizing or parallelize inline
- Criterion benchmark mesh selection and measurement methodology
- Thread pool initialization (global vs per-engine instance)

</decisions>

<specifics>
## Specific Ideas

- The main loop is at engine.rs:869 — `for (layer_idx, layer) in layers.iter().enumerate()`
- CancellationToken already uses `Arc<AtomicBool>` — thread-safe by design
- There are 4 slice methods with ~900 lines each of similar loop logic — opportunity to DRY but not required
- `previous_seam: Option<IPoint2>` is the main cross-layer state that prevents naive parallelization
- Lightning infill is already special-cased in the engine (separate code path)

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `CancellationToken` (engine.rs:72): Already `Arc<AtomicBool>` — works across threads without modification
- `EngineEvent` / event system (event.rs): Existing progress reporting infrastructure from Phase 23
- `PrintConfig` (config.rs): Where `parallel_slicing` and `thread_count` fields will be added

### Established Patterns
- Layer processing loop: perimeters → surface classification → infill → toolpath assembly (per layer, mostly independent)
- `EngineError::Cancelled` variant already exists for cancellation handling
- `SliceResult` struct collects all layer toolpaths into a Vec — parallel layers just need to be sorted by index

### Integration Points
- `slicecore-engine/Cargo.toml` — add rayon dependency with `parallel` feature flag
- `engine.rs` layer loops (4 instances) — convert `.iter()` to `maybe_par_iter()`
- `config.rs` — add `parallel_slicing: bool` and `thread_count: Option<usize>` to PrintConfig
- `event.rs` — add atomic progress counter for parallel mode

</code_context>

<deferred>
## Deferred Ideas

- Full pipeline parallelism (mesh slicing + G-code generation) — future optimization phase
- wasm-rayon integration for browser multi-threading — future WASM phase
- Inner parallelism within infill generation per layer — future optimization
- Parallel BVH construction — noted for future mesh performance work

</deferred>

---

*Phase: 25-parallel-slicing-pipeline-rayon*
*Context gathered: 2026-03-10*
