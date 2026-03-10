# Phase 25: Parallel Slicing Pipeline (rayon) - Research

**Researched:** 2026-03-10
**Domain:** Rayon-based data parallelism for per-layer slicing pipeline
**Confidence:** HIGH

## Summary

This phase adds rayon-based parallelism to the per-layer processing loop in `slicecore-engine`. The codebase is well-structured for this: each layer's processing (perimeters, surface classification, infill, toolpath assembly) is largely independent, with only `previous_seam` (cross-layer seam alignment) and lightning infill (cross-layer tree state) as cross-layer dependencies. The user has already specified a two-pass approach for seam alignment and sequential fallback for lightning infill, which are the correct solutions.

Rayon 1.11.0 is already in the Cargo.lock as a transitive dependency (via criterion and wasmtime). The `par_iter()` on slices produces an `IndexedParallelIterator` whose `collect()` preserves index order -- this is the key guarantee enabling bit-identical output. The existing `CancellationToken` (Arc<AtomicBool>) and `EventBus` (Send+Sync) are already thread-safe.

**Primary recommendation:** Use `par_iter().enumerate().map(...).collect::<Vec<_>>()` on the layers slice for parallel processing, with a `maybe_par_iter!` macro that dispatches to `par_iter()` or `iter()` based on the `parallel` feature flag. Apply a two-pass approach: first pass processes all layers in parallel (without seam alignment), second pass applies sequential seam adjustment. Lightning infill layers fall back to sequential processing.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Parallelize per-layer processing only (the `for (layer_idx, layer) in layers.iter().enumerate()` loop)
- Do NOT parallelize mesh slicing or G-code generation in this phase
- Apply parallelism to ALL slice methods: `slice()`, `slice_with_events()`, `slice_with_preview()`, `slice_with_modifiers()`
- Thread count: auto-detect via rayon defaults, with optional `thread_count` field in PrintConfig for user override
- Bit-identical output required: parallel G-code must be byte-for-byte identical to sequential G-code
- Two-pass seam alignment: first pass processes layers in parallel WITHOUT seam alignment, second pass applies sequential seam adjustment
- Lightning infill excluded from parallelism -- process lightning layers sequentially
- Add `parallel_slicing: bool` (default true) to PrintConfig for sequential fallback mode
- `parallel` Cargo feature flag on slicecore-engine crate only (default-enabled on native, disabled for WASM)
- When `parallel` feature is disabled, rayon is not compiled
- Create a `maybe_par_iter()` helper macro/function that returns `par_iter()` or `iter()` based on the feature flag
- No wasm-rayon integration in this phase
- Atomic counter (AtomicUsize) for progress -- increments when any layer completes
- During parallel processing, only fire aggregate progress events (% complete) -- suppress per-layer detail events
- Per-layer events still work in sequential mode (when `parallel_slicing: false`)
- Cancellation strategy: Claude's discretion on whether to check between batches or per-layer
- Add criterion benchmarks comparing sequential vs parallel on a standard test mesh

### Claude's Discretion
- Exact rayon pool configuration and batch sizing
- Cancellation check strategy (between batches vs per-layer)
- How to structure the two-pass seam alignment implementation
- Whether to refactor duplicated slice methods before parallelizing or parallelize inline
- Criterion benchmark mesh selection and measurement methodology
- Thread pool initialization (global vs per-engine instance)

### Deferred Ideas (OUT OF SCOPE)
- Full pipeline parallelism (mesh slicing + G-code generation) -- future optimization phase
- wasm-rayon integration for browser multi-threading -- future WASM phase
- Inner parallelism within infill generation per layer -- future optimization
- Parallel BVH construction -- noted for future mesh performance work
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rayon | 1.11.0 | Data-parallel iterators for per-layer processing | De facto standard for CPU parallelism in Rust; already in Cargo.lock as transitive dep; par_iter on slices produces IndexedParallelIterator with ordered collect |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::sync::atomic | stable | AtomicUsize for progress counter, AtomicBool for cancellation | Already used for CancellationToken |
| criterion | 0.5 | Benchmark sequential vs parallel | Already a dev-dependency |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| maybe_par_iter macro | `maybe_parallel_iterator` crate | External dep for a ~10-line macro; not worth the dependency for a single crate |
| Global thread pool | Per-engine ThreadPool | Per-engine pools risk oversubscription when multiple engines exist; global pool with build_global is simpler and standard |

**Installation:**
```bash
# In crates/slicecore-engine/Cargo.toml:
# rayon = { version = "1.11", optional = true }
# [features]
# parallel = ["dep:rayon"]
# No cargo install needed -- rayon is already in Cargo.lock
```

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-engine/src/
├── engine.rs            # Slice methods using maybe_par_iter! in layer loops
├── parallel.rs          # NEW: maybe_par_iter! macro, thread pool init, AtomicProgress helper
├── config.rs            # Add parallel_slicing: bool, thread_count: Option<usize>
├── event.rs             # No changes needed (EventBus already Send+Sync)
└── Cargo.toml           # Add rayon optional dep, parallel feature
```

### Pattern 1: Conditional Parallelism via Macro
**What:** A `maybe_par_iter!` macro that compiles to `par_iter()` when the `parallel` feature is enabled, or `iter()` when disabled. This keeps the engine logic clean with zero `#[cfg]` attributes in the main processing code.
**When to use:** Every layer processing loop in engine.rs.
**Example:**
```rust
// crates/slicecore-engine/src/parallel.rs

/// When `parallel` feature is enabled, returns a parallel iterator.
/// When disabled, returns a sequential iterator.
#[cfg(feature = "parallel")]
macro_rules! maybe_par_iter {
    ($slice:expr) => {
        $slice.par_iter()
    };
}

#[cfg(not(feature = "parallel"))]
macro_rules! maybe_par_iter {
    ($slice:expr) => {
        $slice.iter()
    };
}

pub(crate) use maybe_par_iter;
```

### Pattern 2: Two-Pass Seam Alignment
**What:** Process all layers in parallel without seam info (pass `None` for `previous_seam`), then sequentially adjust seam positions in a second pass over the collected toolpaths.
**When to use:** When `parallel_slicing` is true and seam position is `Aligned`.
**Example:**
```rust
// Pass 1: Parallel layer processing (no seam alignment)
let layer_results: Vec<(LayerToolpath, Option<IPoint2>)> = maybe_par_iter!(layers)
    .enumerate()
    .map(|(layer_idx, layer)| {
        // ... perimeters, surfaces, infill, toolpath assembly
        // Pass None for previous_seam in parallel mode
        assemble_layer_toolpath(layer_idx, layer.z, layer.layer_height,
            &perimeters, &gap_fills, &infill, &self.config, None)
    })
    .collect();

// Pass 2: Sequential seam adjustment (only when using Aligned seam)
let mut previous_seam: Option<IPoint2> = None;
let mut layer_toolpaths: Vec<LayerToolpath> = Vec::with_capacity(layer_results.len());
for (mut toolpath, layer_seam) in layer_results {
    if self.config.seam_position == SeamPosition::Aligned {
        // Adjust seam position based on previous layer's seam
        adjust_seam_position(&mut toolpath, previous_seam);
    }
    if layer_seam.is_some() {
        previous_seam = layer_seam;
    }
    layer_toolpaths.push(toolpath);
}
```

### Pattern 3: Atomic Progress Counter for Parallel Mode
**What:** Use `AtomicUsize` to track completed layers. Emit aggregate progress events periodically rather than per-layer events (which would arrive out-of-order).
**When to use:** In `slice_with_events()` when parallel mode is active.
**Example:**
```rust
use std::sync::atomic::{AtomicUsize, Ordering};

let completed = Arc::new(AtomicUsize::new(0));
let total = layers.len();

// Inside parallel map closure:
let result = process_layer(layer_idx, layer);
let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
// Progress event emitted after collect, not inside closure
// (EventBus is not available inside rayon closure easily)

// After collect, emit progress:
if let Some(bus) = event_bus {
    bus.emit(&SliceEvent::Progress { /* ... */ });
}
```

### Pattern 4: Thread Pool Initialization
**What:** Use `rayon::ThreadPoolBuilder::new().build_global()` once at engine construction when `thread_count` is specified. Otherwise use rayon defaults (auto-detect CPU count).
**When to use:** When user sets `thread_count` in PrintConfig.
**Example:**
```rust
#[cfg(feature = "parallel")]
pub fn init_thread_pool(thread_count: Option<usize>) {
    if let Some(count) = thread_count {
        // build_global returns Err if already initialized -- that's fine
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(count)
            .build_global();
    }
}
```

### Anti-Patterns to Avoid
- **Nested par_iter:** Do NOT parallelize within a single layer (e.g., parallelizing infill lines). Per-layer parallelism is the right granularity -- each layer has enough work (~1-10ms) to amortize rayon overhead.
- **Sharing EventBus across threads:** The EventBus itself is Send+Sync, but emitting per-layer events from parallel closures produces out-of-order events. Use atomic counters and post-process progress reporting.
- **Multiple thread pools:** Do NOT create a new ThreadPool per Engine instance. Use the global pool. build_global() is idempotent (returns Err on second call, which is fine to ignore).
- **Locking inside parallel closures:** PrintConfig is read-only during slicing. Pass `&self.config` into closures. No Mutex needed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Parallel iterator with ordered collect | Manual thread spawning + index tracking | `rayon::par_iter().enumerate().map().collect()` | IndexedParallelIterator guarantees order preservation on collect |
| Work-stealing thread pool | Custom thread pool | `rayon::ThreadPoolBuilder::build_global()` | Rayon's work-stealing handles load imbalance automatically across heterogeneous layers |
| Conditional parallelism | Runtime if/else on every loop | `maybe_par_iter!` macro with feature flag | Zero-cost abstraction; no runtime overhead when parallel is disabled |
| Atomic progress tracking | Mutex<usize> counter | `AtomicUsize::fetch_add(1, Relaxed)` | Lock-free, no contention, perfect for a simple counter |

**Key insight:** Rayon's `par_iter()` on a slice produces an `IndexedParallelIterator`. When you call `.collect::<Vec<_>>()`, the results are placed in the correct index order. This is the fundamental guarantee that enables bit-identical output without manual index sorting.

## Common Pitfalls

### Pitfall 1: Rayon Overhead on Small Models
**What goes wrong:** Models with few layers (e.g., 5-10 layers for a flat part) may be slower with parallel processing than sequential due to rayon's work-stealing overhead.
**Why it happens:** Rayon's binary-tree splitting has fixed overhead per split. For <20 layers, this overhead exceeds the parallelism benefit.
**How to avoid:** The `parallel_slicing: bool` config flag provides a manual override. Consider a runtime heuristic: if `layers.len() < 20`, fall back to sequential even when parallel is enabled. This is Claude's discretion.
**Warning signs:** Benchmark showing parallel is slower for the 20mm calibration cube (100 layers at 0.2mm).

### Pitfall 2: Lightning Infill Cross-Layer State
**What goes wrong:** Lightning infill builds a tree structure across layers (lower layers must be processed before upper layers). Parallel processing breaks this dependency.
**Why it happens:** Lightning columns from upper layers merge into lower layers, requiring sequential bottom-up processing.
**How to avoid:** Already decided: exclude lightning infill from parallelism. When `infill_pattern == Lightning`, process all layers sequentially (or process non-lightning aspects in parallel and lightning sequentially).
**Warning signs:** Different lightning infill output between parallel and sequential modes.

### Pitfall 3: Seam Alignment Divergence
**What goes wrong:** When using `SeamPosition::Aligned`, each layer's seam position depends on the previous layer's seam. Parallel processing breaks this chain, producing different seam placement than sequential mode.
**Why it happens:** The `previous_seam` parameter to `assemble_layer_toolpath` creates a sequential dependency.
**How to avoid:** Two-pass approach (decided): first pass processes layers in parallel with `previous_seam = None`, second pass sequentially adjusts seam positions. The seam adjustment function must produce identical output to the original single-pass approach.
**Warning signs:** Byte-diff between parallel and sequential G-code output, particularly in seam-adjacent moves.

### Pitfall 4: Non-Deterministic Floating-Point Accumulation
**What goes wrong:** If any parallel reduction uses floating-point addition (which is not associative), different splitting points produce different results.
**Why it happens:** Rayon splits work non-deterministically based on load balancing.
**How to avoid:** The parallel loop processes layers independently and collects results into a Vec. There are no cross-layer reductions in the parallel section. The collect preserves order. No floating-point accumulation crosses thread boundaries.
**Warning signs:** Intermittent tiny differences in G-code E-values between runs.

### Pitfall 5: EventBus Usage in Parallel Context
**What goes wrong:** Emitting per-layer `LayerComplete` events from inside rayon closures produces events in non-deterministic order, confusing progress UIs.
**Why it happens:** Rayon processes layers in work-stealing order, not sequential order.
**How to avoid:** Already decided: suppress per-layer detail events in parallel mode. Use atomic counter for aggregate progress. Emit progress events after the parallel section completes (or at fixed intervals from the main thread).
**Warning signs:** Progress events arriving with layer indices out of order.

## Code Examples

### Layer Processing Loop Conversion
```rust
// Source: engine.rs current pattern (line 869)
// BEFORE (sequential):
for (layer_idx, layer) in layers.iter().enumerate() {
    // ... process layer ...
    let (toolpath, layer_seam) = assemble_layer_toolpath(..., previous_seam);
    previous_seam = layer_seam;
    layer_toolpaths.push(toolpath);
}

// AFTER (parallel with two-pass seam):
use crate::parallel::maybe_par_iter;

// Pass 1: Parallel processing (no seam alignment)
let layer_results: Vec<_> = maybe_par_iter!(layers)
    .enumerate()
    .map(|(layer_idx, layer)| {
        if cancel_ref.as_ref().map_or(false, |t| t.is_cancelled()) {
            return Err(EngineError::Cancelled);
        }
        if layer.contours.is_empty() {
            return Ok((LayerToolpath {
                layer_index: layer_idx,
                z: layer.z,
                layer_height: layer.layer_height,
                segments: Vec::new(),
            }, None));
        }
        // ... perimeters, surfaces, infill ...
        let (toolpath, seam) = assemble_layer_toolpath(
            layer_idx, layer.z, layer.layer_height,
            &perimeters, &gap_fills, &infill, &self.config,
            None, // No previous_seam in parallel mode
        );
        Ok((toolpath, seam))
    })
    .collect::<Result<Vec<_>, EngineError>>()?;

// Pass 2: Sequential seam adjustment
let mut previous_seam = None;
let layer_toolpaths: Vec<LayerToolpath> = layer_results.into_iter()
    .map(|(mut tp, seam)| {
        // Adjust seam if Aligned strategy
        if let Some(ref prev) = previous_seam {
            adjust_seam(&mut tp, *prev, &self.config);
        }
        if seam.is_some() { previous_seam = seam; }
        tp
    })
    .collect();
```

### Cargo.toml Feature Configuration
```toml
# Source: Rayon conditional dependency pattern
[features]
default = ["parallel"]
parallel = ["dep:rayon"]

[dependencies]
rayon = { version = "1.11", optional = true }

[target.'cfg(target_arch = "wasm32")'.dependencies]
# No rayon for WASM -- parallel feature is disabled
```

Note: The `default = ["parallel"]` enables parallelism on native targets. For WASM builds, the feature must be explicitly disabled: `cargo build --target wasm32-unknown-unknown --no-default-features`.

### Cancellation in Parallel Context
```rust
// CancellationToken is already Arc<AtomicBool> -- thread-safe
// Check per-layer inside the parallel closure:
.map(|(layer_idx, layer)| {
    if let Some(ref token) = cancel {
        if token.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
    }
    // ... process layer ...
    Ok(result)
})
.collect::<Result<Vec<_>, EngineError>>()?
// Note: collect will short-circuit on first Err, but rayon may have
// already started processing other layers. This is acceptable --
// cancellation is cooperative, not instant.
```

### PrintConfig Additions
```rust
// In config.rs:
/// Whether to use parallel (rayon) processing for per-layer operations.
/// When false, layers are processed sequentially (useful for debugging
/// or determinism verification). Default: true.
pub parallel_slicing: bool,

/// Number of threads for parallel processing. None = auto-detect
/// (rayon default: number of logical CPUs). Only effective when
/// parallel_slicing is true.
pub thread_count: Option<usize>,
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual thread spawning + channels | rayon par_iter with work-stealing | rayon 1.0 (2018) | Eliminates manual thread management |
| TBB parallel_for (C++ PrusaSlicer) | rayon par_iter (Rust) | N/A (different ecosystem) | Same concept, Rust borrow checker prevents data races at compile time |
| Global mutable state in parallel loops | Functional map/collect pattern | N/A | Rayon closures must be Fn (not FnMut), enforcing immutable captures |

**Current best practice:**
- rayon 1.11.0 is the current stable release (as seen in Cargo.lock)
- `par_iter().enumerate().map().collect()` is the idiomatic pattern for ordered parallel processing
- Feature-gated conditional parallelism via macro is the standard Rust pattern for optional rayon

## Open Questions

1. **Refactor Before Parallelize?**
   - What we know: There are 4 slice methods (`slice`, `slice_with_events`, `slice_with_preview`, `slice_with_modifiers`) with ~900 lines each of similar loop logic. The context mentions this as "opportunity to DRY but not required."
   - What's unclear: Whether extracting a shared `process_layer()` function first would simplify the parallelization or create unnecessary churn.
   - Recommendation: Extract a shared `process_single_layer()` function that all 4 methods call. This reduces the parallelization change from 4 sites to 1 helper function. The function takes layer index, layer ref, config ref, and returns `(LayerToolpath, Option<IPoint2>)`. This is Claude's discretion and strongly recommended.

2. **Minimum Layer Threshold for Parallelism**
   - What we know: Rayon has overhead per par_iter invocation. Very small models (5-10 layers) may not benefit.
   - What's unclear: The exact crossover point for this codebase.
   - Recommendation: Start with no threshold (always parallel when enabled). The benchmark will reveal if a threshold is needed. If so, add it as a follow-up.

3. **Progress Events During Parallel Processing**
   - What we know: User decided aggregate-only progress in parallel mode. EventBus is Send+Sync.
   - What's unclear: Whether to emit progress during or after the parallel section.
   - Recommendation: After the parallel collect completes, emit a single progress event. For long-running models, consider wrapping the parallel section with a progress polling thread, but this adds complexity. Start simple: emit progress before and after the parallel section only.

## Sources

### Primary (HIGH confidence)
- [rayon docs - IndexedParallelIterator](https://docs.rs/rayon/latest/rayon/iter/trait.IndexedParallelIterator.html) - Confirmed collect preserves index order
- [rayon docs - ParallelIterator](https://docs.rs/rayon/latest/rayon/iter/trait.ParallelIterator.html) - Confirmed collect behavior
- [rayon docs - ThreadPoolBuilder](https://docs.rs/rayon/latest/rayon/struct.ThreadPoolBuilder.html) - build_global configuration
- Codebase analysis of engine.rs (3792 lines), event.rs, config.rs, toolpath.rs - Direct code inspection

### Secondary (MEDIUM confidence)
- [maybe_parallel_iterator crate](https://github.com/finnbear/maybe_parallel_iterator) - Validated feature-flag pattern for conditional parallelism
- [rayon GitHub](https://github.com/rayon-rs/rayon) - Version confirmation

### Tertiary (LOW confidence)
- [Rayon overhead analysis](https://gendignoux.com/blog/2024/11/18/rust-rayon-optimized.html) - Performance pitfalls (from project's own PITFALLS.md research)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - rayon 1.11.0 already in lock file, well-documented API
- Architecture: HIGH - codebase inspection confirms per-layer independence, only 2 cross-layer deps (seam, lightning)
- Pitfalls: HIGH - project's own research/PITFALLS.md covers rayon overhead extensively
- Two-pass seam approach: MEDIUM - correct in principle, implementation details for `adjust_seam` need to be worked out during planning

**Research date:** 2026-03-10
**Valid until:** 2026-04-10 (stable ecosystem, rayon API unlikely to change)
