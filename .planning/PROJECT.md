# libslic3r-rs

## What This Is

A modular, extensible Rust-based 3D printer slicing core that replaces the 15-year-old C++ libslic3r ecosystem. Designed for native desktop apps, headless CLI automation, cloud SaaS deployments, and WebAssembly browser-based slicing. Built from scratch with plugin architecture and AI/LLM integration as first-class features, not bolt-ons.

## Core Value

**The plugin architecture and AI integration must work from day one.** Modularity and intelligence are not features to add later — they define the architecture. If we can't write a custom infill plugin without touching core code, or call an LLM for profile suggestions, the design has failed.

## Requirements

### Validated

(None yet — this is a greenfield rewrite. Ship to validate.)

### Active

**v1 Milestone — Proof of Concept:**

- [ ] **Core slicing pipeline works**: Takes real STL/3MF files (from Thingiverse/Printables) and produces valid G-code that prints correctly
- [ ] **Feature parity with existing slicers**: Implements P0 features from design docs — mesh repair, slicing, perimeters, infill (standard patterns), supports, G-code generation
- [ ] **Plugin system validated**: Can write and load a custom infill pattern plugin without modifying core code
- [ ] **AI integration validated**: Can call local or cloud LLM for profile suggestions and get reasonable results
- [ ] **Passes validation suite**: Automated tests comparing output quality/correctness to PrusaSlicer/OrcaSlicer reference outputs
- [ ] **Multi-platform support**: Builds and runs on macOS, Linux, Windows (ARM + x86_64)
- [ ] **WASM target works**: Core compiles to WebAssembly for browser-based slicing
- [ ] **Performance target**: Matches or beats C++ libslic3r slice time (≥1.0x, targeting ≥1.5x)
- [ ] **Memory target**: Uses ≤80% memory compared to C++ libslic3r
- [ ] **Test coverage**: >80% line coverage on core algorithms
- [ ] **API-first design**: Well-documented Rust API, CLI interface, structured JSON/MessagePack output

### Out of Scope

- **GUI application** — Separate project that consumes the library API
- **Printer communication** — OctoPrint/Moonraker integration lives elsewhere
- **Resin/SLA/DLP slicing** — FDM only for v1
- **FFI bindings to C/C++/Python/Go** — Pure Rust ecosystem; build missing crates instead
- **Material science database** — External service, not embedded
- **Advanced AI features** — Full failure prediction, topology-aware infill deferred to v2+

## Context

**Problem:** The C++ libslic3r ecosystem (Slic3r → PrusaSlicer → BambuStudio → OrcaSlicer → CrealityPrint) carries 15 years of technical debt. Monolithic architecture, no plugin system, impossible AI integration, fragmented forks, no headless/API mode. Each fork diverges; bug fixes don't propagate; innovation stalls.

**Opportunity:** Rust enables memory safety, fearless concurrency, and WASM targets. Modern architecture (plugin system, API-first, modular) enables use cases impossible in C++: browser-based slicing, cloud SaaS, AI-driven optimization, print farm automation.

**Research completed:**
- Extensive C++ codebase analysis: `~/slicer-analysis/analysis/`
- Design documents: `designDocs/01-PRD.md` through `08-GLOSSARY.md`
- Codebase mapping: `.planning/codebase/`
- Feature matrix across 4 major forks (Prusa, Bambu, Orca, Creality)
- Performance hotspots identified: perimeter generation, polygon offsetting (Clipper), support structures

**Prior art:**
- Published `lib3mf-core` crate (pure Rust 3MF parser) — proof that pure-Rust ecosystem approach works

## Constraints

- **Pure Rust ecosystem**: No FFI to C/C++/Python/Go. Build missing crates when gaps identified (precedent: `lib3mf-core`)
- **Multi-platform**: macOS (ARM/x86), Linux (ARM/x86), Windows (ARM/x86)
- **WASM target**: Must compile to wasm32-wasi and wasm32-unknown-unknown
- **Performance**: Match or beat C++ libslic3r (targeting ≥1.5x faster)
- **Memory efficiency**: Use ≤80% memory vs C++ libslic3r
- **Solo developer**: Designed for incremental progress with AI coding assistants
- **Balanced timeline**: Steady progress with quality gates; ship when features work correctly, not by date

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Pure Rust (no FFI) | Avoid C++ technical debt; build Rust ecosystem instead. Proven with `lib3mf-core`. | — Pending validation at scale |
| Plugin architecture from day one | Core differentiator. Must be architectural, not bolted on later. Enables extensibility without forking. | — Pending implementation |
| AI integration from day one | Core differentiator. LLM calls for profile optimization must work in v1, even if minimal. | — Pending implementation |
| API-first design | Enables headless CLI, cloud SaaS, WASM, third-party tools. GUI consumes same API as external users. | — Pending implementation |
| Progressive disclosure settings (5 tiers) | Addresses UX problem: beginners overwhelmed, experts frustrated. Deferred to GUI layer but informs API design. | — Accepted, UI-layer concern |
| Feature parity before innovation | Must match existing slicers before adding novel features. Users won't adopt if basic features missing. | — Accepted |
| Modular crate structure | Core + sub-crates for geometry, G-code, AI, plugins. Enables targeted testing and reuse. | — Pending architecture |

---
*Last updated: 2026-02-14 after initialization*
