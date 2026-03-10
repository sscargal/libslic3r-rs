# Phase 24: Mesh Export (STL/3MF Write) - Context

**Gathered:** 2026-03-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Add mesh write/export capabilities to `slicecore-fileio`. Supports writing 3MF, Binary STL, and OBJ formats by delegating to the lib3mf-* crate ecosystem (lib3mf-core for 3MF write, lib3mf-converters for STL/OBJ export). Includes upgrading lib3mf-core from 0.3 to 0.4, adding lib3mf-converters as a new dependency, and adding a CLI `convert` subcommand.

</domain>

<decisions>
## Implementation Decisions

### Delegation strategy
- Delegate all export logic to lib3mf-core (3MF write) and lib3mf-converters (STL/OBJ export)
- Do NOT implement native STL/OBJ writers in slicecore-fileio — avoid duplicating work already done in lib3mf-converters
- lib3mf-converters exporters operate on lib3mf-core's `Model` type, so a `TriangleMesh → Model` conversion is needed

### lib3mf-core version upgrade
- Upgrade lib3mf-core from 0.3 to 0.4 in this phase
- Fix any API changes in threemf.rs as part of the upgrade
- Add lib3mf-converters 0.4 as a new dependency
- Both from crates.io (not path dependencies)

### Export formats
- 3MF via lib3mf-core `Model::write()`
- Binary STL via lib3mf-converters `BinaryStlExporter::write()`
- OBJ via lib3mf-converters `ObjExporter::write()`
- ASCII STL not included (can add later if needed)

### API design
- Unified `save_mesh(&TriangleMesh, path, ...)` function that auto-detects format from file extension — mirrors existing `load_mesh(path)`
- Also provide `save_mesh_to_writer(&TriangleMesh, writer, MeshFormat)` for programmatic use — mirrors `load_mesh_from_reader()`
- `MeshFormat` enum selects output format (Stl, ThreeMf, Obj)
- Internal `TriangleMesh → lib3mf_core::Model` conversion keeps the public API clean — callers just pass TriangleMesh

### CLI convert command
- Add `slicecore convert input.stl output.3mf` subcommand
- Auto-detects input format (existing load_mesh) and output format from extension
- Reuses the new save_mesh API

### Claude's Discretion
- Exact TriangleMesh → Model conversion details (unit metadata, build item transforms, etc.)
- Error type additions to FileIOError for write failures
- Whether MeshFormat is inferred from extension or explicit in the path-based API
- Test strategy (round-trip tests, fixture comparisons, etc.)
- How to handle 3MF metadata defaults (model name, units) when converting from TriangleMesh

</decisions>

<specifics>
## Specific Ideas

- The user owns the entire lib3mf-* ecosystem at ~/lib3mf-rs — lib3mf-core and lib3mf-converters are their crates
- lib3mf-converters already has working `BinaryStlExporter`, `AsciiStlExporter`, and `ObjExporter`
- lib3mf-core's `Model::write()` already works and is exercised in existing test code in threemf.rs (lines 105-110, 174-212)
- The existing `load_mesh()` / `load_mesh_from_reader()` pattern should be mirrored for export

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `slicecore-fileio::load_mesh()` / `load_mesh_from_reader()` — pattern to mirror for export API
- `slicecore-fileio::detect.rs` — format detection from magic bytes/extension, reusable for output format selection
- `slicecore-fileio::threemf.rs` — already has lib3mf-core Model construction in test code (TriangleMesh → Model path exists in tests)
- `GcodeWriter<W: Write>` in slicecore-gcode-io — established writer pattern with generic Write trait

### Established Patterns
- File I/O crate (`slicecore-fileio`) owns all mesh format handling
- `FileIOError` enum with `thiserror` for error types
- Format detection via magic bytes + extension heuristics
- TriangleMesh is the universal mesh type: `Vec<Point3>` vertices + `Vec<[u32; 3]>` indices

### Integration Points
- `slicecore-fileio/src/lib.rs` — add `save_mesh`, `save_mesh_to_writer`, `MeshFormat` exports
- `slicecore-fileio/Cargo.toml` — add lib3mf-converters dependency, bump lib3mf-core to 0.4
- `slicecore-cli/src/main.rs` — add `convert` subcommand using clap

</code_context>

<deferred>
## Deferred Ideas

- ASCII STL export — can add later if human-readable mesh output is needed
- Extended 3MF metadata (materials, colors, textures) in exports — future phase if needed
- lib3mf-async integration for async export pipelines — noted in Phase 22 deferred

</deferred>

---

*Phase: 24-mesh-export-stl-3mf-write*
*Context gathered: 2026-03-10*
