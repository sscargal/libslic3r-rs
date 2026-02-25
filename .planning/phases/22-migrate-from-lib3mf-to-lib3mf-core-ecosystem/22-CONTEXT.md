# Phase 22: Migrate from lib3mf to lib3mf-core ecosystem - Context

**Gathered:** 2026-02-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace the third-party `lib3mf` crate dependency in `slicecore-fileio` with the `lib3mf-core` ecosystem crates. The current `lib3mf` depends on `zip` → `zstd-sys` (C library), which blocks WASM compilation. The `lib3mf-core` ecosystem is pure Rust, aligning with the project's no-C/C++ FFI philosophy and enabling 3MF support on all targets including WASM.

</domain>

<decisions>
## Implementation Decisions

### Crate mapping
- Use only the minimum necessary crates from the lib3mf-core ecosystem on crates.io (latest stable version)
- Research what each ecosystem crate provides (lib3mf-core, lib3mf-converters, lib3mf-cli, lib3mf-async) and pull in only what's needed for current functionality
- lib3mf-async deferred to a future phase
- Any identified gaps between current usage and lib3mf-core capabilities should be tracked as TODOs or future phase items
- **Exclusion:** `threemf` (v0.7.0) and `threemf2` (v0.1.2) crates are NOT ours and must NOT be used

### WASM strategy
- Add a WASM 3MF parsing test to CI to prove the migration unlocked WASM support

### API surface
- Direct adoption of lib3mf-core types and idioms — no thin adapter layer
- Full migration of both production code and test code (no references to old lib3mf should remain)

### Feature scope
- Track any gaps between lib3mf-core and old lib3mf as TODO items or deferred items for future work
- Behavioral equivalence is sufficient for test assertions — same mesh geometry and triangle count, minor ordering differences acceptable

### Claude's Discretion
- Whether to enable 3MF on WASM (remove cfg gate) based on lib3mf-core's actual WASM compatibility
- Whether to remove the WASM error fallback path or keep it, based on how cleanly lib3mf-core compiles for WASM
- Which WASM targets to test (wasm32-unknown-unknown, wasm32-wasip2, or both)
- Error handling approach — same ThreeMfError pattern with new source, or richer errors if lib3mf-core provides them
- Whether public API of slicecore-fileio changes — prefer stability unless compelling reason
- Whether to do minimal swap or also adopt easy quick wins from lib3mf-core
- Whether write support is needed beyond tests (check codebase)

</decisions>

<specifics>
## Specific Ideas

- The only current lib3mf dependency is in `crates/slicecore-fileio/Cargo.toml` (`lib3mf = { version = "0.1", default-features = false }`)
- Usage is in two files: `src/threemf.rs` (main parsing) and `src/lib.rs` (test + cfg gating)
- Current usage: `Model::from_reader`, `Model::new`, `Mesh::new`, `Vertex::new`, `Triangle::new`, `Object::new`, `BuildItem::new`
- The cfg gate is `#[cfg(not(target_arch = "wasm32"))]` — removing it would make 3MF available everywhere
- Research should examine lib3mf-core's API to determine mapping from old types to new types

</specifics>

<deferred>
## Deferred Ideas

- lib3mf-async integration for async slicing pipelines — future phase
- Extended 3MF features beyond current read/write (e.g., materials, colors, metadata) — future phase if lib3mf-core supports them

</deferred>

---

*Phase: 22-migrate-from-lib3mf-to-lib3mf-core-ecosystem*
*Context gathered: 2026-02-25*
