# Phase 23: Progress/Cancellation API - Research

**Researched:** 2026-02-27
**Domain:** Progress reporting and cooperative cancellation for long-running slicing operations
**Confidence:** HIGH

## Summary

Phase 23 adds rich progress reporting and cooperative cancellation to the slicing engine by extending the existing `EventBus` system (built in Phase 9, Plan 09-04) with new `SliceEvent` variants for progress data, and introducing a `CancellationToken` type (a thin `Arc<AtomicBool>` wrapper). The implementation is straightforward: the engine already emits `StageChanged` and `LayerComplete` events at the right pipeline points -- these need enrichment with percentage, ETA, elapsed time, and throughput data. Cancellation requires checking a token once per layer at the start of per-layer processing and returning `Err(EngineError::Cancelled)` on trigger.

The user has made very specific decisions that tightly constrain the implementation. All public `slice*` methods gain an `Option<CancellationToken>` final parameter (not new method names). `CancellationToken` is a standalone type in `slicecore-engine` re-exported at crate root -- no external dependency (not tokio-util). ETA estimation uses a rolling average over the last N layers. The existing EventBus subscriber pattern is reused without modification.

**Primary recommendation:** Add 3 new `SliceEvent` variants (`Progress`, `StageProgress`, `Cancelled`), implement `CancellationToken` as a 30-line struct wrapping `Arc<AtomicBool>`, thread it through the 5 public slice methods as `Option<CancellationToken>`, and add a `EngineError::Cancelled` variant. This is a focused internal change touching primarily `event.rs`, `error.rs`, `engine.rs`, and `lib.rs`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Extend the existing EventBus (Vec<Box<dyn EventSubscriber>>) with new SliceEvent variants for progress data
- Do NOT create a separate progress API -- one system, not two
- CancellationToken is a separate type (Arc<AtomicBool> wrapper), not a return value from EventSubscriber
- CancellationToken lives in slicecore-engine, re-exported at crate root
- Own implementation -- no external dependency (no tokio-util). Simple Arc<AtomicBool> wrapper with .cancel() and .is_cancelled() methods
- Rich progress data: percentage, ETA, elapsed time, layers/second
- Both overall_percent (0-100% across entire slice) and stage_percent (0-100% within current stage) reported
- ETA estimation uses rolling average over last N layers (adapts to varying layer complexity)
- Per-layer progress events at minimum
- Cancellation returns Err(EngineError::Cancelled) -- clean error, no partial results
- Engine checks cancellation token once per layer at start of layer processing
- CancellationToken passed as Option<CancellationToken> parameter on existing slice methods (slice, slice_with_events, slice_with_preview, etc.)
- Callers that don't need cancellation pass None -- backwards compatible with a signature change
- One-line update for existing callers: add None as final parameter
- User specifically wanted Option<CancellationToken> on existing methods rather than new method names (slice_with_cancel) or required parameters
- Pattern: `engine.slice(&mesh, &config, Some(token))` or `engine.slice(&mesh, &config, None)`

### Claude's Discretion
- Whether sub-layer progress events are needed for expensive operations (large layers with complex infill)
- Cross-platform timing approach for ETA on WASM (Instant fallback vs always-optional timing)
- EventSubscriber Send+Sync trait bound decisions for WASM compatibility

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::sync::atomic::AtomicBool` | std | Cancellation signal | Zero-cost, lock-free, available on all targets including WASM |
| `std::sync::Arc` | std | Shared ownership of cancellation state | Standard pattern for thread-safe shared state |
| `serde` | 1.x (workspace) | Serialize new SliceEvent variants | Already used by existing SliceEvent |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `web-time` | 1.x | `Instant` replacement on wasm32-unknown-unknown | Only if WASM ETA timing is needed |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `Arc<AtomicBool>` | `tokio_util::sync::CancellationToken` | tokio-util adds async runtime dependency, conflicts with pure-sync engine and WASM; user explicitly rejected |
| `web-time` | `#[cfg(not(target_arch = "wasm32"))]` conditional timing | Simpler, no new dependency, ETA just absent on WASM; recommended approach (see Discretion section) |

**No new dependencies required.** All needed primitives are in `std`. The `web-time` crate is optional and likely unnecessary given the recommendation below.

## Architecture Patterns

### Recommended Changes by File

```
crates/slicecore-engine/src/
├── event.rs         # Add Progress, StageProgress SliceEvent variants
├── error.rs         # Add EngineError::Cancelled variant
├── engine.rs        # Add CancellationToken type, thread through 5 public methods + 1 internal
├── lib.rs           # Re-export CancellationToken at crate root
```

Also affected (signature changes only -- add `None` parameter):
```
crates/slicecore-cli/src/main.rs          # CLI calls engine.slice() -- add None
crates/slicecore-engine/tests/*.rs        # Integration/unit tests -- add None
crates/slicecore-engine/benches/*.rs      # Benchmarks -- add None
```

### Pattern 1: CancellationToken

**What:** A simple `Arc<AtomicBool>` wrapper with `Clone` for sharing across threads.
**When to use:** Passed into slice methods by callers who need cancellation capability.
**Example:**

```rust
// Source: project CONTEXT.md design decisions + std library patterns
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Thread-safe cancellation token for cooperative cancellation of slicing operations.
///
/// Create a token, pass it (or a clone) to a slice method, and call `.cancel()`
/// from any thread to request cancellation. The engine checks the token once
/// per layer and returns `Err(EngineError::Cancelled)` if triggered.
#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a new cancellation token in the non-cancelled state.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Requests cancellation. All clones observe this immediately.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Returns `true` if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}
```

### Pattern 2: Rich Progress Events via Extended SliceEvent

**What:** New `SliceEvent` variants carrying percentage, ETA, elapsed time, and throughput.
**When to use:** Emitted during the per-layer loop in `slice_to_writer_with_events`.
**Example:**

```rust
// New SliceEvent variants added to existing enum
pub enum SliceEvent {
    // ... existing variants ...

    /// Rich progress update emitted after each layer.
    Progress {
        /// Overall progress across entire slice operation (0.0 to 100.0).
        overall_percent: f32,
        /// Progress within current pipeline stage (0.0 to 100.0).
        stage_percent: f32,
        /// Current pipeline stage name.
        stage: String,
        /// Current layer index (zero-based).
        layer: usize,
        /// Total number of layers.
        total_layers: usize,
        /// Elapsed time since slice start in seconds.
        elapsed_seconds: f64,
        /// Estimated time remaining in seconds (None if insufficient data).
        eta_seconds: Option<f64>,
        /// Processing throughput in layers per second.
        layers_per_second: f64,
    },
}
```

### Pattern 3: Cancellation Check in Per-Layer Loop

**What:** Single check point at the start of each layer iteration.
**When to use:** Inside `slice_to_writer_with_events` at the top of the `for (layer_idx, layer) in layers.iter().enumerate()` loop.
**Example:**

```rust
for (layer_idx, layer) in layers.iter().enumerate() {
    // Check cancellation before processing this layer.
    if let Some(ref token) = cancel_token {
        if token.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
    }

    // ... existing per-layer processing ...
}
```

### Pattern 4: Rolling Average ETA Estimation

**What:** Track last N layer durations and compute ETA from average.
**When to use:** After each layer completes, update rolling window and compute estimate.
**Example:**

```rust
// Rolling average ETA estimator
const ETA_WINDOW_SIZE: usize = 20;
let mut layer_durations: Vec<f64> = Vec::with_capacity(total_layers);

// After each layer:
let layer_elapsed = layer_start.elapsed().as_secs_f64();
layer_durations.push(layer_elapsed);

let window = if layer_durations.len() > ETA_WINDOW_SIZE {
    &layer_durations[layer_durations.len() - ETA_WINDOW_SIZE..]
} else {
    &layer_durations
};
let avg_layer_time = window.iter().sum::<f64>() / window.len() as f64;
let remaining_layers = total_layers - (layer_idx + 1);
let eta_seconds = avg_layer_time * remaining_layers as f64;
```

### Pattern 5: Method Signature Changes (Backward Compatible)

**What:** Add `Option<CancellationToken>` as the last parameter to all public slice methods.
**When to use:** All 5 public entry points.
**Example:**

```rust
// Before:
pub fn slice(&self, mesh: &TriangleMesh) -> Result<SliceResult, EngineError>
pub fn slice_with_events(&self, mesh: &TriangleMesh, event_bus: &EventBus) -> Result<SliceResult, EngineError>
pub fn slice_to_writer<W: Write>(&self, mesh: &TriangleMesh, writer: W) -> Result<SliceResult, EngineError>
pub fn slice_with_preview(&self, mesh: &TriangleMesh) -> Result<SliceResult, EngineError>
pub fn slice_with_modifiers(&self, mesh: &TriangleMesh, modifiers: &[ModifierMesh]) -> Result<SliceResult, EngineError>

// After:
pub fn slice(&self, mesh: &TriangleMesh, cancel: Option<CancellationToken>) -> Result<SliceResult, EngineError>
pub fn slice_with_events(&self, mesh: &TriangleMesh, event_bus: &EventBus, cancel: Option<CancellationToken>) -> Result<SliceResult, EngineError>
pub fn slice_to_writer<W: Write>(&self, mesh: &TriangleMesh, writer: W, cancel: Option<CancellationToken>) -> Result<SliceResult, EngineError>
pub fn slice_with_preview(&self, mesh: &TriangleMesh, cancel: Option<CancellationToken>) -> Result<SliceResult, EngineError>
pub fn slice_with_modifiers(&self, mesh: &TriangleMesh, modifiers: &[ModifierMesh], cancel: Option<CancellationToken>) -> Result<SliceResult, EngineError>
```

### Anti-Patterns to Avoid
- **Creating a separate progress API parallel to EventBus:** User explicitly said "one system, not two." All progress goes through EventBus as SliceEvent variants.
- **Using `impl Into<Option<CancellationToken>>`:** Adds unnecessary complexity. Plain `Option<CancellationToken>` is clearer and the user said "one-line update for existing callers: add None."
- **Checking cancellation in tight inner loops (per-segment):** User specified "once per layer at start of layer processing." More frequent checks add overhead with minimal benefit since layers process in milliseconds.
- **Returning partial results on cancellation:** User specified "clean error, no partial results." Return `Err(EngineError::Cancelled)`.
- **Adding async/tokio dependency:** This is a synchronous cooperative cancellation API. The token is checked synchronously in the loop.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Thread-safe boolean flag | Custom mutex-based flag | `Arc<AtomicBool>` | Lock-free, zero-overhead, available everywhere including WASM |
| Serialization of new event variants | Manual JSON formatting | `serde::Serialize` derive | SliceEvent already uses serde with tag="type"; new variants auto-serialize |

**Key insight:** This phase requires almost no external dependencies because `std::sync::atomic` and `std::sync::Arc` provide everything needed for the cancellation token, and the EventBus infrastructure is already built.

## Common Pitfalls

### Pitfall 1: WASM `Instant::now()` Panics
**What goes wrong:** `std::time::Instant::now()` panics on `wasm32-unknown-unknown` because the platform has no system clock API by default.
**Why it happens:** The wasm32-unknown-unknown target provides stubs that panic for time operations. The project already has one call to `Instant::now()` in `slice_with_events` (line 514 of engine.rs) that would panic on WASM.
**How to avoid:** Use `#[cfg(not(target_arch = "wasm32"))]` conditional compilation to make ETA/elapsed timing optional. On WASM, emit `Progress` events with `elapsed_seconds: 0.0` and `eta_seconds: None`. Alternatively, use the `web-time` crate, but conditional compilation avoids a new dependency.
**Warning signs:** Any `Instant::now()` call without cfg-gate will panic in WASM tests.
**Recommendation:** Conditional timing is simpler and matches existing project patterns. Create a helper function like `fn now_if_available() -> Option<std::time::Instant>` that returns `Some(Instant::now())` on native and `None` on wasm32.

### Pitfall 2: Breaking 100+ Call Sites
**What goes wrong:** Adding a parameter to `engine.slice()` requires updating every single caller -- 100+ call sites across CLI, tests, and benchmarks.
**Why it happens:** The signature change is intentional (user decision), but the sheer number of call sites makes this error-prone.
**How to avoid:** Use a systematic approach: first change the signature, then fix compilation errors. The compiler will catch every missed call site. Consider doing the signature change and call-site updates in one plan, and the actual progress/cancellation logic in another.
**Warning signs:** Compilation errors with "expected N arguments, found N-1."

### Pitfall 3: Incorrect Atomic Ordering
**What goes wrong:** Using `Ordering::Relaxed` for both store and load can theoretically allow the cancellation signal to be delayed indefinitely on some architectures.
**Why it happens:** Relaxed ordering provides no cross-thread synchronization guarantees.
**How to avoid:** Use `Ordering::Release` for `store` (cancel) and `Ordering::Acquire` for `load` (is_cancelled). This ensures the cancel signal is visible to the checking thread promptly.
**Warning signs:** Tests passing but production cancellation being slow or unreliable (hard to reproduce).

### Pitfall 4: ETA Instability in Early Layers
**What goes wrong:** The first few layers (especially layer 0 with brim/skirt) take much longer than subsequent layers, causing wildly inaccurate initial ETA estimates.
**Why it happens:** Layer 0 has first-layer extras (skirt/brim generation) and different processing characteristics.
**How to avoid:** The rolling average window naturally handles this -- after ~5-10 layers, the average stabilizes. Consider not reporting ETA until at least 3 layers have completed (return `None` for `eta_seconds` until then).
**Warning signs:** ETA jumping from 200 seconds to 20 seconds after a few layers.

### Pitfall 5: Cancellation in `slice_with_preview` Double-Slice
**What goes wrong:** `slice_with_preview()` calls `self.slice(mesh)` internally (line 1379 of engine.rs), then runs a second pass for preview data. If cancellation is checked only in the first pass, the second pass runs to completion even after cancellation.
**Why it happens:** The preview method has its own layer loop that doesn't share the `slice_to_writer_with_events` code path.
**How to avoid:** Thread the cancellation token through both the internal `self.slice()` call and the preview layer loop. Check `is_cancelled()` at the start of each preview layer iteration too.
**Warning signs:** Cancelling during a `slice_with_preview` call takes much longer than expected to return.

## Code Examples

### CancellationToken Complete Implementation

```rust
// Source: std::sync::atomic documentation, project CONTEXT.md
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}
```

### EngineError::Cancelled Variant

```rust
// Added to existing EngineError enum in error.rs
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    // ... existing variants ...

    /// Operation was cancelled via CancellationToken.
    #[error("Slicing operation was cancelled")]
    Cancelled,
}
```

### WASM-Safe Timing Helper

```rust
// Conditional timing that avoids Instant::now() panic on WASM
#[cfg(not(target_arch = "wasm32"))]
fn elapsed_since(start: &Option<std::time::Instant>) -> f64 {
    start.map_or(0.0, |s| s.elapsed().as_secs_f64())
}

#[cfg(target_arch = "wasm32")]
fn elapsed_since(_start: &Option<()>) -> f64 {
    0.0
}

// Or simpler approach: just cfg-gate the Instant creation
#[cfg(not(target_arch = "wasm32"))]
fn start_timer() -> Option<std::time::Instant> {
    Some(std::time::Instant::now())
}

#[cfg(target_arch = "wasm32")]
fn start_timer() -> Option<std::time::Instant> {
    None // Instant exists on WASM but panics -- just skip timing
}
```

### Progress Emission in Per-Layer Loop

```rust
// Inside slice_to_writer_with_events, after each layer completes:
if let Some(bus) = event_bus {
    let overall_pct = ((layer_idx + 1) as f32 / total_layers as f32) * 80.0 + 10.0;
    // 10-90% for layer processing (0-10% mesh slicing, 90-100% gcode gen)

    let (elapsed, eta, lps) = if let Some(start) = start_time {
        let elapsed = start.elapsed().as_secs_f64();
        let layers_done = layer_idx + 1;
        let lps = layers_done as f64 / elapsed.max(0.001);

        // Rolling average ETA
        let window = &layer_times[layer_times.len().saturating_sub(ETA_WINDOW)..];
        let avg = window.iter().sum::<f64>() / window.len().max(1) as f64;
        let remaining = total_layers - layers_done;
        let eta = if layers_done >= 3 { Some(avg * remaining as f64) } else { None };

        (elapsed, eta, lps)
    } else {
        (0.0, None, 0.0)
    };

    bus.emit(&SliceEvent::Progress {
        overall_percent: overall_pct,
        stage_percent: ((layer_idx + 1) as f32 / total_layers as f32) * 100.0,
        stage: "layer_processing".to_string(),
        layer: layer_idx,
        total_layers,
        elapsed_seconds: elapsed,
        eta_seconds: eta,
        layers_per_second: lps,
    });
}
```

## Discretion Recommendations

### Sub-Layer Progress Events
**Recommendation: Not needed for Phase 23.** Individual layers process in single-digit milliseconds even for complex infill patterns. Sub-layer events would add overhead without meaningful user benefit. Per-layer granularity gives responsive progress bars (100-layer model updates 100 times). This can be added later if specific use cases demand it (e.g., extremely complex models with 10-second layers).

### Cross-Platform Timing (WASM)
**Recommendation: Conditional compilation (`#[cfg]`).** Use `std::time::Instant` on native platforms and skip timing on `wasm32`. On WASM, `Progress` events still fire with `elapsed_seconds: 0.0` and `eta_seconds: None`. This avoids adding a new dependency (`web-time`), matches the existing project pattern of cfg-gating WASM differences (see `getrandom` in Cargo.toml), and is simpler than the alternatives. The existing `Instant::now()` call on line 514 of `engine.rs` should also be wrapped in this pattern to fix the latent WASM panic.

### EventSubscriber Send+Sync Bounds
**Recommendation: Keep existing `Send + Sync` bounds.** The trait already requires `Send + Sync` (line 100 of event.rs). `AtomicBool` is `Send + Sync` on all platforms including WASM. No changes needed to trait bounds. The `CancellationToken` with `Arc<AtomicBool>` is automatically `Send + Sync`.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Callback-based progress (function pointer) | EventBus pub/sub (already implemented) | Phase 9 (2026-02-18) | Progress flows through same system as warnings/errors |
| tokio-util CancellationToken | `Arc<AtomicBool>` wrapper (no async runtime) | User decision for this phase | No async dependency, WASM compatible |
| Separate progress polling API | Extended SliceEvent variants | User decision for this phase | Single API surface, not two parallel systems |

**Deprecated/outdated:**
- The `instant` crate (for WASM `Instant`) is deprecated in favor of `web-time`. However, neither is needed given the cfg-gate approach.

## Open Questions

1. **Progress percentage allocation across stages**
   - What we know: The pipeline has ~5 stages: mesh slicing, support generation, per-layer processing, gcode generation, statistics. Currently StageChanged uses 0.0, 0.1, 0.9.
   - What's unclear: Exact percentage allocations. Per-layer processing dominates runtime (~80%), but mesh slicing and gcode gen can be significant for large models.
   - Recommendation: Use 0-10% for mesh slicing/setup, 10-90% for per-layer processing, 90-100% for gcode generation/statistics. These are approximate -- exact allocation is not critical since `stage_percent` gives per-stage precision.

2. **Rolling average window size (N)**
   - What we know: User said "last N layers." Need to pick N.
   - What's unclear: Optimal N depends on model complexity variation.
   - Recommendation: N=20 as default. Small enough to adapt quickly to changing layer complexity, large enough to smooth out noise. This is an implementation detail within Claude's discretion.

3. **Cancellation in `slice_with_preview` and `slice_with_modifiers`**
   - What we know: Both methods have their own layer loops. `slice_with_preview` internally calls `self.slice()`.
   - What's unclear: Whether the preview re-slice pass should also check cancellation.
   - Recommendation: Yes, check cancellation in all layer loops. The `slice_with_preview` method should pass the token through to its internal `self.slice()` call and also check in its own preview loop.

## Sources

### Primary (HIGH confidence)
- `/home/steve/libslic3r-rs/crates/slicecore-engine/src/event.rs` - Existing EventBus implementation (367 lines)
- `/home/steve/libslic3r-rs/crates/slicecore-engine/src/engine.rs` - Current slice pipeline (5 public methods, 1 internal)
- `/home/steve/libslic3r-rs/crates/slicecore-engine/src/error.rs` - Current EngineError enum (44 lines)
- `/home/steve/libslic3r-rs/.planning/phases/23-progress-cancellation-api/23-CONTEXT.md` - User decisions
- `/home/steve/libslic3r-rs/designDocs/03-API-DESIGN.md` - Original API design doc with CancellationToken pattern (lines 643-663)
- Rust std::sync::atomic documentation - AtomicBool, Ordering semantics

### Secondary (MEDIUM confidence)
- [wasm32-unknown-unknown rustc book](https://doc.rust-lang.org/beta/rustc/platform-support/wasm32-unknown-unknown.html) - Confirms Instant::now() limitations on this target
- [web-time crate](https://crates.io/crates/web-time) - Alternative WASM timing (not recommended for this phase)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All primitives are in std, no new dependencies needed
- Architecture: HIGH - Extending existing well-understood EventBus + straightforward Arc<AtomicBool> pattern
- Pitfalls: HIGH - WASM timing is well-documented, call-site changes are compiler-verified, atomic ordering is standard practice

**Research date:** 2026-02-27
**Valid until:** 2026-03-27 (stable domain, no moving targets)
