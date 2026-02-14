# Pitfalls Research

**Domain:** Rust computational geometry / 3D printing slicer engine
**Researched:** 2026-02-14
**Confidence:** HIGH (multiple sources, verified against real Rust geometry projects)

## Critical Pitfalls

### Pitfall 1: Floating-Point Robustness in Geometric Predicates

**What goes wrong:**
Geometric algorithms silently produce incorrect results due to floating-point rounding errors. Orientation tests, intersection calculations, and point-in-polygon checks return wrong answers for near-degenerate configurations. This causes polygon boolean operations to produce self-intersecting output, missing faces in sliced layers, or infinite loops in sweep-line algorithms.

**Why it happens:**
Naive `f64` arithmetic accumulates rounding errors. An orientation test using a simple cross product can flip sign for nearly-collinear points. Developers assume `f64` has "enough precision" without understanding that geometric predicates require *exact* answers (left/right/on), not approximate ones. The problem is invisible in testing because it only manifests with specific geometric configurations that happen to hit precision boundaries.

**How to avoid:**
- Use Shewchuk's adaptive-precision predicates via the [`robust`](https://github.com/georust/robust) or [`geometry-predicates`](https://github.com/elrnv/geometry-predicates-rs) crates for orientation and incircle tests. These provide exact results using adaptive-precision floating-point arithmetic, falling back to expensive exact arithmetic only when needed (fast path remains fast).
- For polygon boolean operations, use integer-coordinate libraries like [`i_overlay`](https://github.com/iShape-Rust/iOverlay) (supports i32, f32, f64 APIs) or Clipper2 (i64 internally). Integer arithmetic eliminates rounding entirely for predicates.
- Never use `f64::EPSILON` as a universal tolerance. As [documented in rust-clippy#6816](https://github.com/rust-lang/rust-clippy/issues/6816), `f64::EPSILON` (~2.2e-16) is essentially useless as an error margin -- it is the smallest difference between 1.0 and the next representable f64, not a meaningful geometric tolerance. Use application-specific tolerances or ULP-based comparison via the [`float-cmp`](https://github.com/mikedilger/float-cmp) or [`approx`](https://lib.rs/crates/approx) crates.
- Establish a coordinate precision strategy in Phase 1: decide whether the internal representation is i64 (like Clipper/PrusaSlicer), f64 with robust predicates, or a hybrid. This decision cascades through every algorithm.

**Warning signs:**
- Tests pass on simple geometries but fail on real-world STL files
- Polygon operations produce self-intersecting output "sometimes"
- Algorithms hang or infinite-loop on specific models
- Different results on different platforms (x87 vs SSE floating-point)
- Comparison operators on float coordinates without tolerance

**Phase to address:**
Phase 1 (Foundation). The coordinate type and precision strategy must be the first architectural decision. Every algorithm depends on it. Changing later requires rewriting everything.

**Real-world examples:**
- The `geometry-predicates` crate documents that predicates do NOT handle exponent overflow: inputs with floats < 1e-142 or > 1e201 produce inaccurate results.
- PrusaSlicer uses Clipper with integer arithmetic internally (scaled from mm to nanometers) specifically because floating-point boolean operations were unreliable.
- The `geo` crate's `simplify_vw_preserve` function has a [known issue (#1049)](https://github.com/georust/geo/issues/1049) where simplification produces self-intersecting rings -- a direct consequence of floating-point coordinate arithmetic.

---

### Pitfall 2: Borrow Checker vs. Graph-Like Geometric Data Structures

**What goes wrong:**
Mesh data structures (half-edge, winged-edge, face-adjacency graphs) require cyclic references: edges point to vertices, vertices point to edges, faces point to edges and vice versa. Rust's ownership model forbids cyclic references. Developers either fight the borrow checker with `Rc<RefCell<T>>` everywhere (slow, verbose, runtime panics) or give up and use `unsafe` (defeats Rust's safety guarantees).

**Why it happens:**
Computational geometry data structures were designed for C/C++ where pointers are unrestricted. A half-edge mesh has a `HalfEdge` pointing to its twin, next, prev, origin vertex, and face -- all mutual references. Rust's hierarchical ownership cannot express "A owns B which references A."

**How to avoid:**
- **Use arena allocation with index-based references.** Store all vertices in one `Vec<Vertex>`, all edges in one `Vec<HalfEdge>`, all faces in one `Vec<Face>`. References between them are `usize` indices, not `&` references. This eliminates lifetime issues entirely because indices are just numbers. The [`typed-arena`](https://crates.io/crates/typed-arena) crate or simple `Vec`-based arenas work well.
- **Separate mutable state from immutable topology.** As demonstrated in the [geometric Rust adventure](https://eev.ee/blog/2018/03/30/a-geometric-rust-adventure/), keeping geometric data (coordinates) separate from algorithmic state (sweep-line status) lets you borrow each independently.
- **Avoid `Rc<RefCell<T>>` for hot paths.** The [half-edge-mesh-rs](https://github.com/mhintz/half-edge-mesh-rs) project uses `Option<Weak<RefCell<T>>>` which works but has overhead from reference counting and runtime borrow checking.
- Design the data layout for cache efficiency: Structure-of-Arrays (SoA) over Array-of-Structures (AoS) for vertex coordinates that will be iterated in tight loops.

**Warning signs:**
- Lifetime parameters propagating through 5+ levels of function signatures
- `RefCell` borrow panics at runtime instead of compile-time safety
- `'a` lifetime annotations on every struct in the geometry module
- Functions requiring `&mut self` when logically they only modify a subset of data

**Phase to address:**
Phase 1 (Foundation). The mesh/polygon data structure design determines API ergonomics for every subsequent phase. Choose arena + indices from the start.

**Real-world examples:**
- The [Rust forum discussion on half-edge data structures](https://users.rust-lang.org/t/how-to-implement-a-half-edge-data-structure-in-rust/8905) shows multiple developers struggling with the same problem, with arena allocation emerging as the consensus solution.
- [Plexus](https://plexus.rs/) mesh library uses index-based graph structures to avoid the lifetime problem entirely.
- The eev.ee blog post documents spending significant effort restructuring a C++ sweep-line algorithm to satisfy Rust's borrow checker, ultimately using `typed-arena` and separating mutable from immutable data.

---

### Pitfall 3: Rayon Over-Parallelization Overhead

**What goes wrong:**
Converting every loop to `par_iter()` (as PrusaSlicer does with TBB's `parallel_for` in 47+ sites) introduces overhead that exceeds the parallelism benefit. Small workloads run *slower* with Rayon than sequentially. The work-stealing thread pool constantly polls via `futex` syscalls even when idle, and splitting small collections into parallel jobs creates more synchronization overhead than computation.

**Why it happens:**
Rayon makes parallelism syntactically trivial (`iter()` to `par_iter()`), which creates a false sense that parallelism is always beneficial. Developers port C++ `parallel_for` sites 1:1 without measuring. Rayon's binary-tree splitting model has structural limitations: setting `with_max_len(1)` (one item per job) can make code [2x slower than sequential](https://gendignoux.com/blog/2024/11/18/rust-rayon-optimized.html) due to job-tree overhead. On an 8-core system, naive Rayon usage achieved only 2x speedup (not 8x) in a real benchmark.

**How to avoid:**
- **Measure before parallelizing.** Use `criterion` benchmarks comparing sequential vs. parallel for each call site. Only parallelize where the per-item work exceeds ~1-10 microseconds.
- **Use `with_min_len()`** to set minimum chunk sizes that amortize Rayon's overhead. For geometry operations on polygons, a minimum of 64-256 items per chunk is a reasonable starting point.
- **Pin threads to CPU cores** using `sched_setaffinity()` or the `core_affinity` crate. Rayon does not do CPU pinning by default, causing thread migrations that destroy L1/L2 cache locality. This alone can yield [10-20% speedup](https://gendignoux.com/blog/2024/11/18/rust-rayon-optimized.html) on cache-sensitive geometry workloads.
- **Avoid nested parallelism** unless the outer loop has few items with large inner work. Nested `par_iter()` inside `par_iter()` floods the thread pool.
- **Profile with `perf` for futex/sched_yield overhead.** If >40% of syscall time is futex operations, your parallel granularity is too fine.
- Establish parallel vs. sequential thresholds per algorithm family (e.g., "parallelize layer slicing across layers, but run per-polygon operations sequentially within a layer").

**Warning signs:**
- `par_iter()` is slower than `iter()` in benchmarks
- CPU utilization is high but throughput is low (spinning on synchronization)
- `perf stat` shows high context switches relative to useful instructions
- Adding more threads does not improve (or worsens) performance

**Phase to address:**
Phase 2-3 (Algorithm Implementation). Add parallelism incrementally, benchmarking each site. Do NOT parallelize in Phase 1 -- get correct sequential algorithms first, then measure and parallelize selectively.

**Real-world examples:**
- [NPB-Rust benchmarks](https://arxiv.org/html/2502.15536v1) (2025) showed Rust+Rayon consistently slower than C++ with OpenMP, with Rayon incurring 1.75% scheduling overhead even in favorable cases, and hitting scaling limits earlier for small-granularity computations.
- [Guillaume Endignoux's detailed analysis](https://gendignoux.com/blog/2024/11/18/rust-rayon-optimized.html) found 49.36% of syscall time spent on futex operations and 48.67% on sched_yield in a Rayon workload -- the thread pool was spending more time synchronizing than computing.

---

### Pitfall 4: WASM Threading and Memory Constraints

**What goes wrong:**
Code that works natively fails completely in WASM: threads panic, memory runs out at 4GB, `mutex.lock()` crashes on the main thread, and half of crate dependencies fail to compile. The "compile to WASM" story is not as simple as changing the target triple.

**Why it happens:**
WebAssembly has fundamental platform constraints that differ from native:
- **No threads by default.** WASM threads require `SharedArrayBuffer`, which needs COOP/COEP security headers and is not available in all browsers. Rust does not ship a precompiled standard library with threading support for WASM -- you must recompile with `-C target-feature=+atomics,+bulk-memory,+mutable-globals`.
- **4GB memory ceiling.** WASM32 has a 32-bit address space. Large meshes (millions of triangles) can exhaust this, especially when `wasm-bindgen` doubles memory by copying typed arrays across the JS/WASM boundary.
- **`i32.atomic.wait` panics on main thread.** Any `Mutex::lock()` or blocking operation on the browser main thread will crash. This is a [hard browser constraint](https://www.tweag.io/blog/2022-11-24-wasm-threads-and-messages/), not a Rust limitation.
- **No filesystem, no system libraries.** Any crate binding to a C system library (e.g., via `cc` or `cmake`) will fail to compile for WASM.

**How to avoid:**
- **Design for single-threaded WASM from the start.** Use feature flags: `#[cfg(target_arch = "wasm32")]` to conditionally disable Rayon/threading. Provide sequential fallbacks for all parallel algorithms.
- **Use `#![cfg_attr(not(feature = "std"), no_std)]`** at the crate level with `extern crate alloc` for heap allocation. This ensures the core geometry library compiles for WASM without pulling in `std`-only dependencies.
- **Streaming/chunked processing** for large meshes instead of loading everything into memory. Process one layer at a time rather than materializing the entire slice result.
- **Avoid `Mutex`, `RwLock`, and blocking operations** in code that may run on WASM main thread. Use message-passing patterns instead.
- **Test WASM compilation in CI** from day one: `cargo build --target wasm32-unknown-unknown` should be a CI gate. Add `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` for native-only dependencies like Rayon.
- Consider the [`talc`](https://crates.io/crates/talc) allocator for WASM -- it produces smaller binaries and better performance than the default `dlmalloc`.

**Warning signs:**
- Growing dependency tree with `cc` or `cmake` build scripts
- `std::fs`, `std::net`, or `std::thread` usage in core geometry code
- No WASM CI target
- Assuming `rayon::par_iter()` will "just work" everywhere

**Phase to address:**
Phase 1 (Foundation) for `no_std` compatibility and WASM CI gate. Phase 4+ for actual WASM integration, but the architecture must not preclude it from Phase 1.

**Real-world examples:**
- The [iShape-js project](https://github.com/iShape-Rust/iShape-js) successfully compiles iOverlay to WASM for polygon boolean operations in the browser, demonstrating that it is achievable with careful architecture.
- The [wasm-bindgen issue #2498](https://github.com/rustwasm/wasm-bindgen/issues/2498) documents users struggling to get >2GB memory working, with workarounds involving direct `WebAssembly.Memory` manipulation.
- The [wasm-bindgen issue #2241](https://github.com/wasm-bindgen/wasm-bindgen/issues/2241) reports unbounded memory growth when WASM modules are invoked repeatedly -- memory "always growing" with no way to shrink.

---

### Pitfall 5: Polygon Degeneracy and Self-Intersection Handling

**What goes wrong:**
Boolean operations (union, intersection, difference) crash or produce garbage when input polygons contain degeneracies: zero-area spikes, collinear vertices, duplicate points, self-intersections, or incorrect winding order. Real-world STL files from 3D modeling software are rife with these problems.

**Why it happens:**
Academic polygon clipping algorithms assume "general position" inputs -- no coincident vertices, no collinear edges, no self-intersections. Real-world input violates every assumption. Developers implement the textbook algorithm and are surprised when production data breaks it. The Martinez-Rueda algorithm implementation in [`rust-geo-booleanop`](https://github.com/21re/rust-geo-booleanop) is known to fail on degenerate inputs.

**How to avoid:**
- **Always validate and clean input polygons** before boolean operations. Remove duplicate vertices, merge collinear edges, fix winding order. The iOverlay library provides `simplify_shape` for this purpose.
- **Use libraries designed for degenerate input.** iOverlay explicitly handles self-intersections for "all polygon varieties." Clipper2 similarly handles degeneracies internally.
- **Define polygon validity invariants** as types: `ValidPolygon` (cleaned, correct winding, no self-intersection) vs. raw `Polygon` (unchecked input). Make boolean operations accept only `ValidPolygon`. This uses Rust's type system to enforce the precondition.
- **Polygon offsetting requires valid input.** As documented: "Offsetting a polygon works reliably only with valid polygons that have no self-intersections and proper boundary orientation." Always clean before offset.
- Build a comprehensive test suite of degenerate cases: zero-area triangles, bowtie polygons, kissing vertices, sliver triangles, polygons with holes that touch the outer boundary.

**Warning signs:**
- Boolean operations work on test rectangles but fail on real STL files
- Output polygons have zero-area spikes or self-intersections
- "Occasionally" produces empty results for valid-looking input
- Panics in sort comparators due to non-total ordering of intersection events

**Phase to address:**
Phase 2 (Polygon Operations). Build a validation/cleaning pipeline before implementing boolean operations. Every boolean operation function should accept only validated input.

**Real-world examples:**
- PrusaSlicer's `ClipperUtils.cpp` wraps every Clipper operation with input validation and output cleaning because raw Clipper output can still contain micro-artifacts.
- The `geo` crate's [issue #1049](https://github.com/georust/geo/issues/1049) demonstrates how simplification algorithms can *introduce* self-intersections.
- iOverlay's documentation explicitly recommends `simplify_shape` before `offset` operations.

---

### Pitfall 6: Global Mutable State (The Print Object Anti-Pattern)

**What goes wrong:**
Porting PrusaSlicer's architecture pattern of a global `Print` object that holds all state creates a monolithic, untestable, un-parallelizable system. In Rust, global mutable state requires `static mut` (unsafe), `Mutex<T>` (deadlock-prone), or `OnceCell`/`LazyLock` (inflexible). This pattern prevents concurrent slicing of multiple models and makes unit testing nearly impossible.

**Why it happens:**
C++ codebases accumulate global state because it is syntactically easy (`Print::instance()`). When porting to Rust, developers reach for the same pattern. Rust's ownership model actively fights this: a global `Mutex<PrintState>` requires every function to lock/unlock, creating contention and potential deadlocks. It also prevents running multiple slice jobs in parallel.

**How to avoid:**
- **Pass state explicitly through function parameters.** The [Rust community consensus](https://users.rust-lang.org/t/beginner-guidance-on-dependency-injections-and-globals/134207) is clear: "A pure safe way is using no global variables at all and passing everything through function arguments."
- **Use a context/session struct** that holds all per-job state: `SliceJob { model: &Model, config: &Config, results: Vec<Layer> }`. Create one per slice operation. This naturally enables parallel slicing of multiple models.
- **Dependency injection through constructors.** Services receive their dependencies at creation time rather than reaching for globals.
- **For truly global configuration** (thread pool size, logging level), use `OnceCell`/`LazyLock` for immutable-after-init values. Never for mutable state.

**Warning signs:**
- `static mut` or `lazy_static!` with `Mutex` in core library code
- Functions that take no arguments but access global state
- Tests that must run sequentially (`#[serial]`) because they share global state
- Cannot slice two models concurrently

**Phase to address:**
Phase 1 (Foundation). The state management architecture must be established before any algorithms are implemented. Every algorithm should receive its context as a parameter.

**Real-world examples:**
- PrusaSlicer's global `Print` object is the single biggest architectural problem for parallelism and testability in the C++ codebase. The Rust port must not replicate this.
- The Bevy game engine's ECS architecture demonstrates how Rust projects successfully manage complex state without globals: components are data, systems are functions that receive queries -- no global mutable state anywhere.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Using `f64` everywhere without precision strategy | Fast to implement, familiar | Subtle bugs in boolean ops, non-reproducible results across platforms | Never for polygon boolean ops. Acceptable for display/visualization coordinates. |
| `clone()` on polygon/mesh data in algorithms | Avoids lifetime complexity | 10-30x slower for large meshes (measured: [cloning 1MB Vec is 30x slower than copying 1MB str](https://github.com/rust-lang/rust/issues/13472)). Memory doubles. | Early prototyping only. Must be eliminated before benchmarking. |
| `Rc<RefCell<T>>` for mesh connectivity | Compiles quickly, familiar OOP pattern | Runtime borrow panics, ~2x overhead vs. index-based access, not `Send`/`Sync` (breaks Rayon) | Never in performance-critical paths. Acceptable for configuration/UI trees. |
| `unwrap()` on float comparisons | Concise code | Panic on NaN/Infinity input -- production crash | Never in library code. Use `total_cmp()` (stabilized in Rust 1.62) or handle `None` explicitly. |
| Blanket `par_iter()` on all loops | Appears to "use all cores" | Overhead exceeds benefit for small workloads. 2x slowdown measured with `with_max_len(1)`. | Only after benchmarking proves net benefit for that specific call site. |
| Monomorphizing generics over coordinate types | Zero-cost abstraction, type-safe | Compile-time explosion. Each `<f32>`, `<f64>`, `<i32>`, `<i64>` instantiation duplicates all code. glam avoids generics entirely for this reason. | Acceptable if limited to 1-2 coordinate types. Use trait objects (`dyn CoordFloat`) for rarely-called code paths. |
| Skipping input validation | Faster processing, simpler code | Crashes on real-world STL files with degenerate geometry | Never for external input. Acceptable between internal validated-to-validated transformations. |

## Integration Gotchas

Common mistakes when connecting to external libraries and systems.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Clipper2 via FFI | Assuming Clipper2's C++ types map cleanly to Rust. `std::string` return-by-value is not FFI-safe ([bindgen #2071](https://github.com/rust-lang/rust-bindgen/issues/2071)). Panic unwinding into C++ code is UB. | Use the [`cxx`](https://cxx.rs/) crate instead of raw `bindgen` for C++ interop. Wrap every callback in `catch_unwind`. Or prefer pure-Rust alternatives (iOverlay). |
| WASM via wasm-bindgen | Passing large `Vec<f64>` vertex arrays through wasm-bindgen, which copies the entire buffer across the JS/WASM boundary, doubling memory usage. | Allocate buffers in WASM memory, expose as `js_sys::Float64Array` views. Use `wasm-bindgen`'s `Clamped<&[u8]>` or manual pointer-based transfer. |
| STL/3MF file parsing | Using `std::fs::read` (pulls entire file into memory). For large models (100MB+ STL), this peaks at 2x file size in memory. | Use streaming parsers. Read triangle-by-triangle. Build spatial index incrementally. |
| Rayon thread pool | Creating multiple thread pools (one per library/module) that oversubscribe CPU cores. | Use a single global `rayon::ThreadPoolBuilder::new().build_global()` at application startup. All `par_iter()` calls share it. |
| External geometry crates | Assuming `geo::Polygon<f64>` and your internal `Polygon` type are interchangeable. Different crates have different winding order conventions, coordinate spaces, and validity assumptions. | Define explicit conversion traits (`From`/`Into`) with validation at boundaries. Document winding order convention once and enforce it. |

## Performance Traps

Patterns that work at small scale but fail as model complexity grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| O(n^2) point-in-polygon for all intersection tests | Works fast for test cubes | Use spatial indexing (BVH, R-tree, grid). `rstar` crate for R-trees. | >10K polygons per layer |
| Allocating `Vec<Point>` per polygon per layer | Invisible for simple models | Pre-allocate buffers, reuse across layers. Use arena allocators. | >1000 layers with >100 polygons each (~100K allocations) |
| Naive polygon clipping without spatial pre-filtering | All pairs tested for intersection | Bounding-box pre-filter eliminates >95% of non-intersecting pairs | >100 polygons per boolean operation |
| Storing full polygon copies at every processing stage | Easy debugging, immutable pipeline | Use copy-on-write (`Cow<'_, [Point]>`) or in-place mutation with undo capability | Models with >1M vertices (e.g., organic sculpted models) |
| String-based error types (`String` in `Result<T, String>`) | Easy to write, informative messages | Allocates on every error. Define enum-based errors with `thiserror`. | Hot loops that handle many validation errors |
| Computing bounding boxes repeatedly | Correct, simple code | Cache bounding boxes. Invalidate on mutation. | Algorithms that check bounds thousands of times per layer |

## Security Mistakes

Domain-specific safety issues for a geometry processing library.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Processing untrusted STL/3MF without size limits | Memory exhaustion DoS: a crafted STL with billions of declared triangles allocates unbounded memory | Enforce maximum triangle count, maximum file size, and maximum coordinate range on input. Reject files exceeding limits before allocation. |
| `unsafe` in FFI wrappers without validation | C/C++ library receives invalid pointers, causing memory corruption that Rust cannot detect or prevent ([documented in FFIChecker research](https://www.zhuohua.me/assets/ESORICS2022-FFIChecker.pdf)) | Minimize FFI surface. Validate all pointers before passing. Prefer pure-Rust implementations (iOverlay over clipper-sys). |
| Trusting coordinate values from input files | NaN/Infinity coordinates propagate through algorithms silently, producing garbage output or panics | Validate all input coordinates are finite: `f64::is_finite()`. Reject NaN/Infinity at the parsing boundary. |
| Plugin/extension code executing in the same address space | A buggy plugin can corrupt the slicer's memory via unsafe code | Use WASM sandboxing for plugins (Wasmtime/Wasmer). Plugins get a separate memory space. |

## UX Pitfalls

Common user-experience mistakes in geometry processing library APIs.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Geometry API requires lifetime annotations on all types | Users cannot store `Polygon<'a>` in their own structs without propagating `'a` everywhere. "Lifetime hell." | Use owned types (`Polygon`) by default. Provide borrowing variants (`PolygonRef<'a>`) as opt-in for advanced users. |
| Functions that silently produce invalid output | Users get corrupted geometry without any indication. Debug cycle: "why does my print fail?" | Return `Result<ValidPolygon, GeometryError>` with specific error variants. Fail loudly. |
| Inconsistent coordinate systems | Users pass coordinates in wrong units (mm vs. nanometers) or wrong winding order | Use newtype wrappers: `Millimeters(f64)`, `Nanometers(i64)`. Enforce winding order in constructors. |
| Error messages without geometric context | "intersection failed" with no indication of which polygons, at which coordinates, on which layer | Include geometric context in errors: layer index, polygon index, approximate coordinates of the failure. |
| Breaking API changes between versions | Users must rewrite code on every update | Stabilize core types (`Point`, `Polygon`, `Layer`) in Phase 1 and commit to their API. Changes to internals should not affect public types. |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Polygon boolean operations:** Often missing degenerate input handling -- verify with zero-area triangles, collinear points, self-intersecting polygons, and kissing vertices
- [ ] **Parallel algorithms:** Often missing benchmarks proving parallelism actually helps -- verify with `criterion` comparing `iter()` vs `par_iter()` on realistic workloads (not just large random data)
- [ ] **Floating-point tolerance:** Often missing consistency -- verify the SAME epsilon/tolerance is used throughout the entire pipeline, not ad-hoc values per function
- [ ] **WASM build:** Often missing runtime testing -- verify `wasm-pack test` actually runs geometry algorithms, not just that `cargo check --target wasm32-unknown-unknown` compiles
- [ ] **Mesh data structures:** Often missing edge cases in topology queries -- verify half-edge traversal works for boundary edges, isolated vertices, and non-manifold geometry
- [ ] **Input parsing:** Often missing malformed file handling -- verify with truncated files, NaN coordinates, zero-length normals, and files with declared-vs-actual triangle count mismatch
- [ ] **Memory usage:** Often missing peak measurement -- verify with `dhat` or `heaptrack` that peak RSS stays within WASM's 4GB limit for target model sizes
- [ ] **Thread safety:** Often missing `Send + Sync` bounds -- verify all public types are `Send + Sync` (or explicitly documented as `!Send`/`!Sync` with rationale)

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Wrong coordinate precision (f64 when needed i64) | HIGH | Must rewrite coordinate types, all algorithms that depend on exact predicates, and all tests. Estimated 2-4 weeks for a mature codebase. Mitigate by abstracting coordinate type behind a trait from Phase 1. |
| `Rc<RefCell<T>>` mesh structures in hot paths | MEDIUM | Replace with arena + index pattern. Mechanical refactoring but touches every file that accesses mesh topology. ~1 week if data structures are well-encapsulated. |
| Global mutable state throughout codebase | HIGH | Must thread context parameters through all function signatures. Every call site changes. Often requires redesigning module boundaries. Estimated 3-6 weeks. |
| Over-parallelized codebase (Rayon everywhere) | LOW | Replace `par_iter()` with `iter()` at underperforming sites. Mechanical, guided by benchmarks. ~1-2 days. |
| WASM incompatible dependencies deep in tree | MEDIUM | Identify offending deps with `cargo tree`. Replace with WASM-compatible alternatives or feature-gate behind `#[cfg(not(target_arch = "wasm32"))]`. 1-2 weeks depending on dependency depth. |
| Missing input validation causing production crashes | LOW-MEDIUM | Add validation layer at input boundary. Existing algorithms may need audit for NaN/Infinity propagation. ~1 week. |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Floating-point robustness | Phase 1: Foundation | Benchmark robust predicates vs. naive. Test suite with degenerate geometry configurations. All predicate tests must be exact (not approximate). |
| Borrow checker vs. graph structures | Phase 1: Foundation | No `Rc<RefCell<T>>` in core data structures. All mesh types are `Send + Sync`. Lifetime annotations do not appear in public API. |
| Rayon over-parallelization | Phase 2-3: Algorithm Implementation | Every `par_iter()` call site has a corresponding `criterion` benchmark proving >20% speedup over sequential. Minimum chunk sizes documented per algorithm. |
| WASM constraints | Phase 1: CI gate; Phase 4+: Integration | `cargo build --target wasm32-unknown-unknown` passes in CI from Phase 1. No `std::thread`, `std::fs` in core geometry crate. Feature-gated parallelism. |
| Polygon degeneracy handling | Phase 2: Polygon Operations | Test suite includes 20+ degenerate polygon configurations. All boolean operations accept only `ValidPolygon` type (enforced by type system). |
| Global mutable state | Phase 1: Foundation | No `static mut` or `lazy_static! { Mutex<_> }` in core library. All functions receive context as parameters. Tests run in parallel without `#[serial]`. |
| Monomorphization bloat | Phase 1: Foundation | Compile time tracked in CI. If adding a coordinate type doubles compile time, switch to trait objects for non-hot paths. |
| Clone overhead on large data | Phase 2-3: Algorithm Implementation | `clippy::clone_on_ref_ptr` and `clippy::redundant_clone` lints enabled. Benchmark memory usage with `dhat`. |
| NaN/Infinity propagation | Phase 1: Foundation | All input parsing validates `f64::is_finite()`. `debug_assert!` on intermediate results in geometry algorithms. |
| FFI memory safety | Phase 2+: If using C/C++ libraries | Minimize FFI surface area. Prefer pure-Rust alternatives. Wrap all FFI in safe abstractions with validation. Run under Miri for unsafe code. |

## Sources

- [Optimization adventures: making a parallel Rust workload 10x faster with (or without) Rayon](https://gendignoux.com/blog/2024/11/18/rust-rayon-optimized.html) -- detailed Rayon overhead analysis with benchmarks (HIGH confidence)
- [A geometric Rust adventure](https://eev.ee/blog/2018/03/30/a-geometric-rust-adventure/) -- practical experience porting C++ computational geometry to Rust (HIGH confidence)
- [NPB-Rust: NAS Parallel Benchmarks in Rust](https://arxiv.org/html/2502.15536v1) -- 2025 academic benchmark of Rayon vs OpenMP (HIGH confidence)
- [geometry-predicates-rs](https://github.com/elrnv/geometry-predicates-rs) -- Shewchuk's robust predicates in Rust (HIGH confidence)
- [robust crate](https://github.com/georust/robust) -- robust adaptive floating-point predicates (HIGH confidence)
- [iOverlay](https://github.com/iShape-Rust/iOverlay) -- polygon boolean operations with integer coordinate support (HIGH confidence)
- [iOverlay performance benchmarks](https://ishape-rust.github.io/iShape-js/overlay/performance/performance.html) -- iOverlay 6-22x faster than Clipper2 (MEDIUM confidence, vendor benchmarks)
- [float-cmp crate](https://github.com/mikedilger/float-cmp) -- ULP and epsilon-based float comparison (HIGH confidence)
- [EPSILON is a bad error margin - rust-clippy#6816](https://github.com/rust-lang/rust-clippy/issues/6816) -- why f64::EPSILON is wrong for tolerance (HIGH confidence)
- [half-edge-mesh-rs](https://github.com/mhintz/half-edge-mesh-rs) -- Rc<RefCell<T>> approach to half-edge mesh (MEDIUM confidence)
- [Rust forum: half-edge data structures](https://users.rust-lang.org/t/how-to-implement-a-half-edge-data-structure-in-rust/8905) -- community consensus on arena allocation (MEDIUM confidence)
- [Avoiding allocations in Rust to shrink WASM modules](https://nickb.dev/blog/avoiding-allocations-in-rust-to-shrink-wasm-modules/) -- WASM allocation strategies (MEDIUM confidence)
- [wasm-bindgen threading issues](https://github.com/rustwasm/wasm-bindgen/issues/2498) -- 4GB memory limit and SharedArrayBuffer constraints (HIGH confidence)
- [WASM thread restrictions](https://www.tweag.io/blog/2022-11-24-wasm-threads-and-messages/) -- main thread blocking panics (HIGH confidence)
- [geo crate issue #1049](https://github.com/georust/geo/issues/1049) -- simplification causing self-intersections (HIGH confidence)
- [Rust Singleton anti-patterns](https://users.rust-lang.org/t/beginner-guidance-on-dependency-injections-and-globals/134207) -- community guidance on avoiding global state (MEDIUM confidence)
- [Generics and compile time in Rust](https://www.pingcap.com/blog/generics-and-compile-time-in-rust/) -- monomorphization overhead analysis (MEDIUM confidence)
- [cache_padded crate](https://docs.rs/cache-padded) -- false sharing prevention (HIGH confidence)
- [False sharing measurement](https://alic.dev/blog/false-sharing) -- 25-49% performance impact from false sharing (MEDIUM confidence)
- [Nine rules for running Rust on the web and embedded](https://towardsdatascience.com/nine-rules-for-running-rust-on-the-web-and-on-embedded-94462ef249a2/) -- no_std and WASM compatibility guide (MEDIUM confidence)
- [Rust no_std Playbook](https://hackmd.io/@alxiong/rust-no-std) -- practical guide for no_std crate development (MEDIUM confidence)
- [FFIChecker: Detecting Cross-Language Memory Management Issues](https://www.zhuohua.me/assets/ESORICS2022-FFIChecker.pdf) -- academic research on FFI memory safety bugs (HIGH confidence)

---
*Pitfalls research for: Rust computational geometry / 3D printing slicer engine*
*Researched: 2026-02-14*
