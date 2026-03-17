# Codebase Concerns

**Analysis Date:** 2026-02-13

## Project Status

This is a **design-phase project**, not yet implemented. The design documents in `/home/steve/libslic3r-rs/designDocs/` outline an ambitious clean-room Rust rewrite of LibSlic3r. No source code has been written yet. These concerns identify **architectural and implementation risks** that must be addressed before and during development.

---

## Architectural Risks

### Scope Creep: Feature Parity Timeline

**Issue:** The design ambitiously lists 20+ crates across 5 layers plus AI, plugins, WASM, and multiple API interfaces. A solo developer faces substantial risk of feature parity taking longer than estimated.

**Impact:**
- Cloud SaaS cannot launch until core slicing is feature-complete
- Incomplete feature set makes project unsuitable for production replacement of C++ LibSlic3r
- Development velocity may stall if intermediate milestones lack visible progress

**Files:** `designDocs/04-IMPLEMENTATION-GUIDE.md` (Section 2.1 - Roadmap shows 20+ weeks to feature parity; Phase 0-4 sequential dependency)

**Fix approach:**
- Explicitly ship Phase 0-1 (minimal slice CLI) before committing to Phase 2-4
- Use 80/20 rule: identify which 20% of features serve 80% of users; ship those first
- Defer advanced features (AI, tree supports, multi-material) to post-MVP milestone
- Each Phase should have a shippable CLI/API artifact

---

### Floating-Point Robustness: Coordinate System Design Debt

**Issue:** The design acknowledges that floating-point imprecision is the #1 source of bugs in computational geometry (section 1.2). The proposal is to use integer coordinates internally (scaled by 1,000,000 for nanometer precision), but this is a major implementation burden not yet proven.

**Impact:**
- Polygon clipping operations (boolean union, difference, intersection) are extremely fragile if not designed carefully
- Edge cases in slicing (touching perimeters, thin walls) may produce invalid G-code
- Bugs may only appear with specific models/parameters, making regression testing difficult
- Porting algorithms from C++ reference requires careful validation of numeric boundaries

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 1.2), `designDocs/02-ARCHITECTURE.md` (Section 4.1)

**Fix approach:**
- Before implementing `slicecore-geo`, establish a test suite using known problematic models from the C++ codebase (thin walls, touching perimeters, extremely small features)
- Implement coordinate type as described (`Coord = i64`, `SCALE = 1_000_000.0`)
- Fuzz test all polygon operations with random inputs and edge cases
- Document conversion boundaries between user-facing f64 and internal i64 representations
- Add property-based tests (proptest) to verify invariants (e.g., "output area ≤ input area" for intersection)

---

### Numerical Validation & Mesh Repair: Under-Specified

**Issue:** The design describes a TriangleMesh structure with optional BVH spatial indexing, but doesn't define the mesh repair pipeline. C++ LibSlic3r has hundreds of heuristics for detecting and fixing non-manifold edges, self-intersections, degenerate triangles, and disconnected components.

**Impact:**
- User-provided STL files are often malformed (3D printer models rarely perfect)
- Without robust repair, slicing will fail on real-world inputs
- Repair algorithms have many configuration parameters (threshold for "degenerate", how aggressively to heal, etc.) that aren't documented

**Files:** `designDocs/02-ARCHITECTURE.md` (Section 4.2 - MeshError enum defined but repair strategy not detailed)

**Fix approach:**
- Document mesh repair strategy in `slicecore-mesh` crate before implementation
- Establish test set of known problematic models (self-intersecting, non-manifold)
- Implement minimal repair first (remove degenerate triangles); defer aggressive healing to Phase 2
- Make repair parameters tunable in PrintConfig
- Add warnings/suggestions to MeshError (as shown in design, Section 10.2)

---

### Plugin System Safety: WASM Sandboxing Complexity

**Issue:** The design proposes WASM plugins via Wasmtime with strict resource limits (64 MiB memory, 30s timeout). This is ambitious and introduces new attack surfaces not present in monolithic design.

**Impact:**
- WASM plugin marketplace requires security review/vetting process (not specified)
- Resource limits may be too strict for some legitimate use cases or too loose for others
- WASM FFI (host functions exposed to plugins) must be carefully designed to prevent capability leaks
- Debugging WASM plugins in production is difficult

**Files:** `designDocs/02-ARCHITECTURE.md` (Section 5.3), `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 4.4)

**Fix approach:**
- Defer WASM plugin support to Phase 4; ship with built-in and dylib plugins only initially
- Document plugin capability model explicitly (e.g., "infill plugins can only read layer geometry and write extrusion paths; they cannot access file system or network")
- Establish plugin review/signing process before marketplace launch
- Implement plugin isolation testing (confirm WASM plugin cannot exceed memory limit, cannot access host filesystem)

---

### Concurrency Model: Rayon Thread Pool Coordination

**Issue:** The design proposes data parallelism using Rayon for per-layer operations (slicing, region classification, toolpath generation) with a sequential G-code emission stage. The interaction between parallel stages and sequential output is not fully specified.

**Impact:**
- Memory usage during parallel stages could spike (all layer threads may allocate simultaneously)
- Progress reporting across thread pool needs careful synchronization
- Cancellation tokens must be checked frequently to enable responsive cancellation
- Layer ordering for output requires buffering or careful scheduling

**Files:** `designDocs/02-ARCHITECTURE.md` (Section 8 - Concurrency Model, Section 8.2 - Thread Pool Design)

**Fix approach:**
- Implement cancellation token checking in all loop bodies before multi-threaded code ships
- Add memory budget tracking (design already proposes this in Section 1.10)
- Test concurrent slicing with memory constraints (simulated low-memory device)
- Implement layer ordering via atomic counter or epoch-based buffering
- Add per-stage progress reporting with thread-safe counters

---

## Implementation Risks

### Missing Configuration Schema Specification

**Issue:** The config system is described as "declarative" with schema driving validation, UI generation, and AI prompts (Section 6.2), but the schema file format and versioning strategy are not specified.

**Impact:**
- Without a clear schema definition language, implementing `slicecore-config` is undefined
- Backward compatibility with PrusaSlicer/OrcaSlicer profiles requires precise mapping (not documented)
- AI prompt generation from schema requires structured metadata (what fields are required?)

**Files:** `designDocs/02-ARCHITECTURE.md` (Section 6 - Configuration System)

**Fix approach:**
- Define config schema format (e.g., "TOML with JSON schema validators" or "custom Rust struct with derive macros")
- Document schema version bumping strategy (semantic versioning for schema)
- Create migration layer for old profile formats → new schema
- Specify which schema fields auto-generate AI prompts

---

### Data Format Compliance: 3MF Output Requirements Vague

**Issue:** The design states LibSlic3r-RS must produce valid 3MF files with thumbnails and Bambu Lab compatibility, but the exact structure (namespace, thumbnail resolutions, AMS mappings) is not defined. Bambu Lab printers have specific 3MF requirements.

**Impact:**
- Without exact 3MF structure, files may not open in Bambu Lab slicers
- Thumbnail generation at correct resolutions (if wrong size, preview won't display)
- AMS slot mappings (filament type → machine slot) require external data

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 3.1)

**Fix approach:**
- Before Phase 1 fileio work, research Bambu Lab 3MF structure via reverse-engineering `.3mf` files from BambuStudio
- Document required thumbnail resolutions and namespace URIs
- Add integration test: generate 3MF file, verify with `lib3mf` validation
- Defer AMS mapping to Phase 2 (not required for MVP)

---

### Licensing Risk: Polygon Clipping IP

**Issue:** The design acknowledges that C++ LibSlic3r relies on Angus Johnson's Clipper library (Boost license) and that a clean-room rewrite is required. However, there's no documented proof of independent development for the clipping algorithm.

**Impact:**
- If clipping algorithm is derived from Clipper, it may inherit GPL/LGPL obligations (legal risk)
- Competitors may challenge the "clean-room" claim during commercialization
- Using an existing Rust polygon clipping crate (`i-overlay`, etc.) may bring license baggage

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 1.1 - Licensing & Legal Strategy)

**Fix approach:**
- Evaluate existing Rust polygon clipping crates and their licenses upfront
- If using external crate, run through `cargo deny` license checker
- If implementing from scratch, reference only academic papers (not Clipper source)
- Document algorithm origin in code comments
- Conduct prior art search for polynomial clipping patents before filing patents on derived work

---

### API Design: Streaming G-code Interface Under-Specified

**Issue:** The design mentions "streaming G-code" as a novel feature (start printing before slicing finishes), but the API boundary between progressive layer generation and G-code output is not defined.

**Impact:**
- Unclear how to implement incremental G-code emission without buffering all layers
- Progress callback API needs clear threading semantics (can callbacks block?)
- Network serialization format for streaming (if used over API) is not specified

**Files:** `designDocs/02-ARCHITECTURE.md` (Section 4.2 - Data Flow mentions streaming layers), `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 1.5)

**Fix approach:**
- Defer streaming G-code to Phase 2
- For MVP, require complete slicing before G-code output
- Define callback trait in Phase 1 to unblock architecture (even if not streaming yet)
- Document serialization format (JSON Lines? MessagePack?) for streaming protocol

---

## Security Concerns

### Input Validation: Limits Not Enforced in Code

**Issue:** The design proposes limits (500 MiB file size, 10 million triangles, 100,000 layers) but these are not yet implemented. If attackers provide malformed files, memory exhaustion or CPU DoS is possible.

**Impact:**
- Zip bombs in 3MF files could extract to gigabytes
- STL files with claimed billions of triangles could cause OOM
- Deeply nested assemblies in STEP files could cause stack overflow

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 4.1 - Input Validation)

**Fix approach:**
- Implement input size limits in `slicecore-fileio` before any file parsing
- Check file size before opening; reject if > configured limit (default 500 MiB)
- During parsing, count triangles/vertices incrementally; abort if exceeds limit
- Test with fuzz-generated pathological files (via cargo-fuzz)

---

### API Key Exposure: Secrets Management

**Issue:** The design mentions using the `secrecy` crate for AI provider API keys, but doesn't specify key injection method, secret rotation, or audit logging for the cloud SaaS.

**Impact:**
- If API keys are logged (even in debug logs), they could leak via crash reports
- Key rotation policy not defined (how often? zero-downtime?)
- No audit trail of which slices used which API keys/models

**Files:** `designDocs/02-ARCHITECTURE.md` (Section 7.2 - AiConfig with SecretString), `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 4.3 - API Security)

**Fix approach:**
- Use `secrecy` crate for all API keys; never serialize or log
- Implement key injection via environment variables (never config files)
- Add audit logging for AI provider calls (model used, tokens consumed, timestamp) without logging the key itself
- Document key rotation procedure for cloud SaaS

---

### G-code Injection: Whitelist Approach Not Implemented

**Issue:** The design proposes sanitizing user-provided custom G-code and whitelisting allowed commands per firmware, but no whitelist data structures exist.

**Impact:**
- Cloud SaaS users could inject dangerous commands (M502 factory reset, firmware flashing)
- Whitelist must be maintained per firmware type (Marlin, Klipper, RepRapFirmware, Bambu)
- Injection attack surface includes start G-code, end G-code, and custom layer changes

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 4.2 - G-code Injection Prevention)

**Fix approach:**
- Build whitelist of safe G-code commands in `slicecore-gcode-gen` crate
- Document dangerous commands per firmware (M502, M504, M509, etc.)
- Implement validator that parses user G-code and rejects unsafe commands
- Add config flag to disable validation in trusted environments (local CLI)

---

## Testing & Quality Gaps

### Golden File Testing: Determinism Not Yet Proven

**Issue:** The design heavily relies on golden-file testing (comparing SHA256 of output to baseline) to detect regressions. But floating-point operations are inherently non-deterministic; the claim that LibSlic3r-RS will have deterministic output is not yet validated.

**Impact:**
- If floating-point rounding differs across platforms (x86 vs ARM, SSE vs AVX, Debug vs Release), golden files fail
- Cannot verify determinism without actual implementation
- May need to switch to looser similarity metrics (e.g., "within 1% of baseline") instead of exact matching

**Files:** `designDocs/02-ARCHITECTURE.md` (Section 11.2 - Golden File Testing), `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 1.2 - Determinism claim)

**Fix approach:**
- Implement Phase 0 primitives (vectors, matrices) and test for determinism across platforms (x86, ARM, WASM)
- Run same slicing operation on multiple platforms; verify binary-identical output
- If determinism fails, document relaxed acceptance criteria (% similarity threshold)
- Add CI step to run tests on ARM target (via QEMU or cross-compilation)

---

### Mesh Repair Validation: No Test Suite

**Issue:** Mesh repair is critical for real-world input handling, but no test suite of known problematic models is mentioned.

**Impact:**
- Repair algorithms untested until first user encounters broken model
- Cannot establish regression tests
- Performance of repair unknown (may be slow for large/damaged meshes)

**Files:** `designDocs/02-ARCHITECTURE.md` (Section 4.2 - Mesh repair hinted at but not detailed)

**Fix approach:**
- Curate test set of 10-20 real-world problematic STL files (non-manifold, self-intersecting, etc.)
- Store in repo as `tests/fixtures/mesh_repair/`
- For each model, document the issues and expected repair strategy
- Add integration tests that load→repair→slice each model

---

### Cross-Slicer Comparison: Metric Thresholds Not Specified

**Issue:** The design proposes comparing LibSlic3r-RS output against PrusaSlicer to validate feature parity, but acceptance criteria ("within 10%" for time, "within 5%" for filament) are arbitrary guesses.

**Impact:**
- Without defined thresholds, it's unclear when validation passes
- Different settings profiles may have different acceptable variance
- Layer count discrepancies need investigation (algorithm differences?)

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 5.3 - Cross-Slicer Comparison Testing)

**Fix approach:**
- For Phase 1 MVP, defer cross-slicer comparison testing
- For Phase 2, establish validation matrix with specific models × firmware × settings
- Document why variance occurs (rounding, algorithm differences)
- Automate comparison in CI (generate G-code with both slicers, extract metrics, compare)

---

### Stress Testing: No Resource Limits Tested

**Issue:** The design mentions stress testing (1M+ triangle models, extreme aspect ratios) but resource limits haven't been validated.

**Impact:**
- "Low memory mode" (streaming to disk) not yet implemented
- Unknown at what model size the engine runs out of memory
- Progress estimation for huge models unreliable

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 5.4 - Stress Testing), `designDocs/02-ARCHITECTURE.md` (Section 8.2 - thread pool uses configurable thread count but no memory budgeting yet)

**Fix approach:**
- After Phase 1 MVP, add stress testing CI job with artificially constrained memory (e.g., `ulimit -v`)
- Implement memory monitoring in `SliceEngine` (track peak usage per stage)
- Establish baseline memory usage per layer (for estimation purposes)
- Document memory requirements and when streaming mode activates

---

## Technical Debt & Deferred Work

### Phase 2-4 Blocking: Feature Parity Uncertain

**Issue:** Phases 2-4 (feature parity, intelligence, integration) are sketched but not detailed. Many features are listed without implementation strategies.

**Impact:**
- Advanced features (tree supports, Arachne, adaptive infill) have research risk
- AI integration (Section 7) depends on external models/APIs not yet integrated
- Plugin system (Phase 4) depends on Phase 1-3 core being stable

**Files:** `designDocs/04-IMPLEMENTATION-GUIDE.md` (Section 2.1 - Roadmap shows Phases 2-4 as future work)

**Fix approach:**
- Ship Phase 0-1 before committing to Phase 2-4 timelines
- For Phase 2 features, research existing Rust implementations (e.g., `polyclipping` for arachne-style perimeters)
- Establish feature gates (Cargo features) for experimental algorithms
- Document Phase 2+ feature implementation strategy as code is written

---

### Internationalization: Deferred but Complex

**Issue:** The design proposes i18n-ready error messages and setting descriptions, but implementation is deferred and locale switching not yet designed.

**Impact:**
- If localization is added later, refactoring hard-coded strings is tedious
- Cloud SaaS may need locale per user; unclear how to propagate

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 1.3 - Internationalization)

**Fix approach:**
- For Phase 0-1, use string keys instead of hard-coded English from day one (e.g., `"error.mesh.non_manifold"` instead of `"Non-manifold edge at..."`)
- Defer translation files to Phase 3
- Use `fluent` or `unic-locale` for i18n infrastructure

---

### Performance Profiling: No Baseline Established

**Issue:** The design emphasizes deterministic execution and reproducible testing, but no performance baselines or profiling infrastructure is set up.

**Impact:**
- Cannot detect performance regressions until they're severe
- Unknown which crate/algorithm consumes most CPU/memory
- Optimization efforts are ad-hoc

**Files:** `designDocs/04-IMPLEMENTATION-GUIDE.md` (Section 1.2 - Lists `cargo-flamegraph` but not integrated into CI)

**Fix approach:**
- Set up benchmark suite in `benches/` before Phase 1 completes
- Establish baseline performance for standard models (calibration cube, benchy)
- Run benchmarks in CI (via criterion); track regressions
- Profile hot paths with `cargo flamegraph` monthly

---

### Reproducible Builds: Lockfile Strategy Undefined

**Issue:** The design claims "reproducible builds" as an advantage, but the lockfile strategy (Cargo.lock in repo? dependencies pinned?) is not specified.

**Impact:**
- Builds may differ if Cargo.lock is not committed or if features are enabled conditionally
- WASM target may have different resolved dependencies than native
- Users cannot verify that claimed binary matches source

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 4.5 - Supply Chain Security mentions Cargo.lock but not fully specified)

**Fix approach:**
- Commit `Cargo.lock` to repo
- Document build process (OS, Rust version, feature flags) in CONTRIBUTING.md
- Add CI step to verify binary reproducibility (rebuild and compare hashes)
- Use `cargo-sbom` to generate software bill of materials

---

## Scaling & Production Risks

### Cloud SaaS Infrastructure: Not Designed

**Issue:** The design describes a Cloud SaaS product but doesn't address deployment, scaling, cost modeling, or SLA requirements.

**Impact:**
- How many concurrent slices can the service handle?
- What's the cost per slice (GPU/AI inference)? Who pays?
- How are long-running slices (1M+ triangle models) scheduled?
- Failure scenarios (AI provider down, OOM) not documented

**Files:** `designDocs/02-ARCHITECTURE.md` (Section 2.1 mentions "Cloud SaaS" but no implementation)

**Fix approach:**
- Defer Cloud SaaS to post-MVP (Phase 4+)
- For Phase 1, provide CLI and local Python API only
- When designing cloud service, define SLA, concurrency limits, and cost model first
- Use containerization (Docker/Podman) for repeatable deployment

---

### Print Farm Integration: Fleet Management Vague

**Issue:** The design claims suitability for print farms ("no headless/CLI mode suitable for automation") but doesn't specify fleet management APIs (multiple printers, per-printer profiles, batch slicing).

**Impact:**
- What's the API for submitting jobs to farm?
- How do profiles vary per printer? (bed size, nozzle diameter, etc.)
- How is progress/status reported?

**Files:** `designDocs/01-PRODUCT_REQUIREMENTS.md` (Section 2.2 lists print farm as target user)

**Fix approach:**
- For Phase 1 MVP, focus on single-model slicing via CLI
- Document farm use case requirements before Phase 3
- Implement batch slicing API in Phase 2
- Add job queue/scheduling in Phase 4 (defer to cloud SaaS)

---

### Dependency Supply Chain: Minimal Dependencies Claimed

**Issue:** The design emphasizes minimal dependencies to reduce attack surface, but the actual dependency list (once crates are implemented) is unknown. Some foundational decisions (use `i-overlay` for polygon clipping? use `lib3mf-core` for 3MF?) are deferred.

**Impact:**
- If a critical dependency (e.g., polygon clipping library) has a vulnerability, update urgency is high
- WASM target may not support all dependencies (e.g., some require std or libc)
- Bloat-time test may show unexpected large dependencies

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 4.5 - Supply Chain Security), `designDocs/02-ARCHITECTURE.md` (Section 1.1 mentions wrapping vs. clean rewrite; FFI to C++ libraries rejected)

**Fix approach:**
- Document candidate dependencies for each crate before implementation
- For core crates (math, geo, mesh), prefer pure Rust implementations
- For file I/O, evaluate `lib3mf-core` and alternatives (license, WASM support)
- Run `cargo-deny` checks in every PR
- Maintain SBOM (software bill of materials) for security audits

---

## Algorithm & Feature Risks

### Arachne Perimeters: Research Implementation Risk

**Issue:** Arachne is a complex wall generation algorithm from PrusaSlicer that produces higher-quality perimeters. The design lists it as Phase 2 work but it's algorithmically non-trivial.

**Impact:**
- Implementation may take longer than estimated (research phase needed)
- May require numerical stability improvements (see floating-point risk above)
- User experience depends on quality of Arachne implementation

**Files:** `designDocs/04-IMPLEMENTATION-GUIDE.md` (Section 2.1 shows Arachne as "Arachne perimeters" :p2f scheduled after p1d, 4w)

**Fix approach:**
- Phase 1 uses simple perimeter generation (fixed offset)
- For Phase 2 Arachne research, allocate 1 week to study PrusaSlicer's implementation
- Document algorithm in design document before implementation
- Use academic papers (if available) as primary reference, not just source code

---

### Adaptive Layer Heights: Not Mentioned

**Issue:** Adaptive layer height (varying height based on geometry) is listed in telemetry example but not in feature roadmap. It's valuable for print time optimization.

**Impact:**
- User expectation gap: if design doc shows telemetry for "layer height", users expect it exists
- Missing from Phase 1-2 roadmap

**Files:** `designDocs/07-MISSING-CONSIDERATIONS.md` (Section 1.4 shows `layer_height` in telemetry, but no implementation plan)

**Fix approach:**
- Either add adaptive layer heights to Phase 2 roadmap or remove from examples
- If adding, design profile impact (config schema for layer height rules)
- Consider impact on support generation (height transitions at support boundaries)

---

### AI Integration: Provider Dependency Risk

**Issue:** The design proposes AI integration (Section 7) with multiple provider abstractions (OpenAI, Anthropic, Google, local Ollama), but the actual "LLM for slicing optimization" use cases are vague.

**Impact:**
- LLM provider APIs change; design must be flexible
- Cost per slice unknown (API calls are metered)
- Offline-first design (Ollama support) complicates deployment
- Quality of AI recommendations depends on model quality and training data

**Files:** `designDocs/02-ARCHITECTURE.md` (Section 7 - AI Integration Architecture)

**Fix approach:**
- Defer Phase 3 (AI) to after Phase 2 is complete
- For Phase 3, start with single provider (OpenAI or Anthropic) as proof of concept
- Design provider abstraction to be testable (mock provider for CI)
- Document example use cases (profile suggestion, support placement) with expected accuracy
- Implement cost estimation before cloud SaaS launch

---

## Documentation & Maintenance

### Glossary: 3D Printing Terminology

**Issue:** The design includes a glossary (doc 08-GLOSSARY.md) but it wasn't provided in the designDocs folder.

**Impact:**
- Without glossary, new contributors misunderstand terms ("infill", "perimeter", "bridge", "overhang angle")
- Code comments may use inconsistent terminology

**Files:** `designDocs/08-GLOSSARY.md` (referenced but not included in exploration)

**Fix approach:**
- Create comprehensive glossary before Phase 1 code starts
- Include both domain terms (3D printing) and crate-specific terms (SliceLayer, ExtrusionSegment, etc.)
- Link glossary in all crate README.md files
- Add glossary references in doc comments

---

### Developer Documentation: Assumptions About C++ Knowledge

**Issue:** The design assumes developers can read C++ LibSlic3r as reference, but modern Rust developers may not be fluent in C++.

**Impact:**
- Algorithm porting is slow if developer must debug C++ to understand intent
- Opportunities for misunderstanding (C++ quirks vs. algorithm intent)

**Files:** `designDocs/04-IMPLEMENTATION-GUIDE.md` (Section 1.2 - "Reference implementation guided" strategy; Section 6 - "Implementing an algorithm from C++ reference")

**Fix approach:**
- For Phase 1-2, prioritize algorithms with published academic papers
- For algorithms without papers, write pseudocode in design documents before implementation
- Use Claude Code with context: provide C++ code + pseudocode + unit test expectations

---

## Summary of High-Priority Fixes

1. **Floating-point robustness** (Section: Numerical Robustness) — Establish coordinate system and test before Phase 1 polygon operations
2. **Mesh repair strategy** (Section: Mesh Repair Validation) — Curate test suite of problematic models
3. **Plugin deferred** (Section: Plugin System Safety) — Defer WASM plugins to Phase 4
4. **Input limits enforced** (Section: Input Validation) — Add size checks before any file parsing
5. **Determinism proven** (Section: Golden File Testing) — Validate on ARM and WASM before claiming reproducible builds
6. **Phase 2-4 research** (Section: Phase 2-4 Blocking) — Document feature strategies as code written
7. **Performance baseline** (Section: Performance Profiling) — Add benchmark suite in Phase 1
8. **Cross-platform CI** (Section: Determinism Not Yet Proven) — Run tests on ARM target (via QEMU/cross-compilation)

---

*Concerns audit: 2026-02-13*
