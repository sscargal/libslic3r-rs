# Phase 23: Progress/Cancellation API - Context

**Gathered:** 2026-02-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Add progress reporting and cancellation support to the slicing engine so GUI, web service, and print farm applications can track slicing progress and cancel operations mid-flight. Extends the existing EventBus system rather than creating a parallel mechanism.

</domain>

<decisions>
## Implementation Decisions

### Callback Design
- Extend the existing EventBus (Vec<Box<dyn EventSubscriber>>) with new SliceEvent variants for progress data
- Do NOT create a separate progress API — one system, not two
- CancellationToken is a separate type (Arc<AtomicBool> wrapper), not a return value from EventSubscriber
- CancellationToken lives in slicecore-engine, re-exported at crate root
- Own implementation — no external dependency (no tokio-util). Simple Arc<AtomicBool> wrapper with .cancel() and .is_cancelled() methods

### Progress Granularity
- Rich progress data: percentage, ETA, elapsed time, layers/second
- Both overall_percent (0-100% across entire slice) and stage_percent (0-100% within current stage) reported
- ETA estimation uses rolling average over last N layers (adapts to varying layer complexity)
- Per-layer progress events at minimum

### Cancellation Semantics
- Cancellation returns Err(EngineError::Cancelled) — clean error, no partial results
- Engine checks cancellation token once per layer at start of layer processing
- CancellationToken passed as Option<CancellationToken> parameter on existing slice methods (slice, slice_with_events, slice_with_preview, etc.)
- Callers that don't need cancellation pass None — backwards compatible with a signature change
- One-line update for existing callers: add None as final parameter

### Claude's Discretion
- Whether sub-layer progress events are needed for expensive operations (large layers with complex infill)
- Cross-platform timing approach for ETA on WASM (Instant fallback vs always-optional timing)
- EventSubscriber Send+Sync trait bound decisions for WASM compatibility

</decisions>

<specifics>
## Specific Ideas

- User specifically wanted Option<CancellationToken> on existing methods rather than new method names (slice_with_cancel) or required parameters
- "Best of both worlds" — single API surface, optional cancellation, no method proliferation
- Pattern: `engine.slice(&mesh, &config, Some(token))` or `engine.slice(&mesh, &config, None)`

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 23-progress-cancellation-api*
*Context gathered: 2026-02-27*
