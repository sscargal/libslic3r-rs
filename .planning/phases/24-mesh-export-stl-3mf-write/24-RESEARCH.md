# Phase 24: Mesh Export (STL/3MF Write) - Research

**Researched:** 2026-03-10
**Domain:** Mesh file export via lib3mf-core and lib3mf-converters
**Confidence:** HIGH

## Summary

Phase 24 adds mesh export capabilities to `slicecore-fileio`, complementing the existing import-only functionality. The implementation delegates all format-specific writing to the lib3mf ecosystem: `lib3mf-core` 0.4 for 3MF write and `lib3mf-converters` 0.4 for Binary STL and OBJ export. Both crates are published on crates.io at version 0.4.0 and are owned by the project maintainer.

The core technical challenge is the `TriangleMesh -> lib3mf_core::Model` conversion, which is the inverse of the existing `threemf.rs` parse path. The conversion is straightforward: iterate `TriangleMesh.vertices()` (f64 Point3) to `lib3mf_core::model::Mesh` vertices (f32), map `TriangleMesh.indices()` to `lib3mf_core::model::Triangle` structs, wrap in an `Object` + `BuildItem`, and hand the `Model` to the exporter. The existing test code in `threemf.rs` already demonstrates this construction pattern.

**Primary recommendation:** Upgrade lib3mf-core from 0.3 to 0.4, add lib3mf-converters 0.4, implement `save_mesh`/`save_mesh_to_writer` mirroring the load API, and add a CLI `convert` subcommand.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Delegate all export logic to lib3mf-core (3MF write) and lib3mf-converters (STL/OBJ export)
- Do NOT implement native STL/OBJ writers in slicecore-fileio -- avoid duplicating work already done in lib3mf-converters
- lib3mf-converters exporters operate on lib3mf-core's `Model` type, so a `TriangleMesh -> Model` conversion is needed
- Upgrade lib3mf-core from 0.3 to 0.4 in this phase
- Fix any API changes in threemf.rs as part of the upgrade
- Add lib3mf-converters 0.4 as a new dependency
- Both from crates.io (not path dependencies)
- 3MF via lib3mf-core `Model::write()`
- Binary STL via lib3mf-converters `BinaryStlExporter::write()`
- OBJ via lib3mf-converters `ObjExporter::write()`
- ASCII STL not included (can add later if needed)
- Unified `save_mesh(&TriangleMesh, path, ...)` function that auto-detects format from file extension -- mirrors existing `load_mesh(path)`
- Also provide `save_mesh_to_writer(&TriangleMesh, writer, MeshFormat)` for programmatic use -- mirrors `load_mesh_from_reader()`
- `MeshFormat` enum selects output format (Stl, ThreeMf, Obj)
- Internal `TriangleMesh -> lib3mf_core::Model` conversion keeps the public API clean -- callers just pass TriangleMesh
- Add `slicecore convert input.stl output.3mf` subcommand
- Auto-detects input format (existing load_mesh) and output format from extension
- Reuses the new save_mesh API

### Claude's Discretion
- Exact TriangleMesh -> Model conversion details (unit metadata, build item transforms, etc.)
- Error type additions to FileIOError for write failures
- Whether MeshFormat is inferred from extension or explicit in the path-based API
- Test strategy (round-trip tests, fixture comparisons, etc.)
- How to handle 3MF metadata defaults (model name, units) when converting from TriangleMesh

### Deferred Ideas (OUT OF SCOPE)
- ASCII STL export -- can add later if human-readable mesh output is needed
- Extended 3MF metadata (materials, colors, textures) in exports -- future phase if needed
- lib3mf-async integration for async export pipelines -- noted in Phase 22 deferred
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| lib3mf-core | 0.4.0 | 3MF Model type + write API | Project's own crate; already used for 3MF import; `Model::write()` produces valid 3MF ZIP archives |
| lib3mf-converters | 0.4.0 | STL/OBJ export from Model | Project's own crate; `BinaryStlExporter::write()` and `ObjExporter::write()` are production-ready |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| glam | 0.31 | `Mat4::IDENTITY` for BuildItem transforms | Transitive dep of lib3mf-core; needed for `BuildItem.transform` field |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| lib3mf-converters | Hand-rolled STL/OBJ writers | More code, more bugs, user explicitly forbids this |

**Installation:**
```bash
# In crates/slicecore-fileio/Cargo.toml:
# Change: lib3mf-core = { version = "0.3", default-features = false }
# To:     lib3mf-core = { version = "0.4", default-features = false }
# Add:    lib3mf-converters = { version = "0.4", default-features = false }
```

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-fileio/src/
    lib.rs          # Add save_mesh(), save_mesh_to_writer(), re-export MeshFormat variants
    detect.rs       # MeshFormat enum -- add Stl variant for output (current has StlBinary/StlAscii)
    error.rs        # Add WriteError variant to FileIOError
    export.rs       # NEW: TriangleMesh -> Model conversion + format dispatch
    threemf.rs      # Fix any 0.3->0.4 API changes (likely none)
    ...existing...

crates/slicecore-cli/src/
    main.rs         # Add Convert subcommand
```

### Pattern 1: Mirror Import API for Export
**What:** The export API mirrors the existing import API structure exactly.
**When to use:** Always -- this is the locked decision.
**Example:**
```rust
// Import (existing):
pub fn load_mesh(data: &[u8]) -> Result<TriangleMesh, FileIOError>
pub fn load_mesh_from_reader<R: Read>(reader: &mut R) -> Result<TriangleMesh, FileIOError>

// Export (new, mirrors import):
pub fn save_mesh(mesh: &TriangleMesh, path: &Path) -> Result<(), FileIOError>
pub fn save_mesh_to_writer<W: Write>(mesh: &TriangleMesh, writer: W, format: MeshFormat) -> Result<(), FileIOError>
```

### Pattern 2: TriangleMesh -> lib3mf_core::Model Conversion
**What:** Internal conversion function that creates a minimal valid Model from a TriangleMesh.
**When to use:** Every export operation needs this because all three exporters operate on `Model`.
**Example:**
```rust
// Source: existing test code in threemf.rs lines 87-110, adapted
fn triangle_mesh_to_model(mesh: &TriangleMesh) -> Result<Model, FileIOError> {
    let mut lib3mf_mesh = lib3mf_core::model::Mesh::new();

    // Convert Point3 (f64) -> Vertex (f32). f64->f32 is lossy but acceptable for mesh geometry.
    for v in mesh.vertices() {
        lib3mf_mesh.add_vertex(v.x as f32, v.y as f32, v.z as f32);
    }

    // Convert [u32; 3] indices -> Triangle
    for tri in mesh.indices() {
        lib3mf_mesh.add_triangle(tri[0], tri[1], tri[2]);
    }

    let mut model = Model::default();
    let object = Object {
        id: ResourceId(1),
        object_type: ObjectType::Model,
        name: None,
        part_number: None,
        uuid: None,
        pid: None,
        pindex: None,
        thumbnail: None,
        geometry: Geometry::Mesh(lib3mf_mesh),
    };
    model.resources.add_object(object)
        .map_err(|e| FileIOError::WriteError(e.to_string()))?;
    model.build.items.push(BuildItem {
        object_id: ResourceId(1),
        uuid: None,
        path: None,
        part_number: None,
        transform: glam::Mat4::IDENTITY,
        printable: None,
    });

    Ok(model)
}
```

### Pattern 3: Format Detection from Extension for Output
**What:** Determine output format from file extension (`.stl` -> Stl, `.3mf` -> ThreeMf, `.obj` -> Obj).
**When to use:** In the path-based `save_mesh()` function.
**Example:**
```rust
fn format_from_extension(path: &Path) -> Result<MeshFormat, FileIOError> {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("stl") => Ok(MeshFormat::Stl),
        Some("3mf") => Ok(MeshFormat::ThreeMf),
        Some("obj") => Ok(MeshFormat::Obj),
        _ => Err(FileIOError::WriteError("unsupported output format".into())),
    }
}
```

### Anti-Patterns to Avoid
- **Exposing lib3mf_core::Model in the public API:** Callers should only deal with `TriangleMesh`. The Model conversion is internal.
- **Re-implementing STL/OBJ binary writing:** User explicitly forbids this. Delegate to lib3mf-converters.
- **Adding glam as a direct dependency:** It comes transitively from lib3mf-core. Only add as dev-dependency if needed for tests (already present).

## MeshFormat Enum Considerations

The existing `MeshFormat` enum has `StlBinary` and `StlAscii` variants for import detection. For export, the user decided only Binary STL is supported. Options:

**Recommended approach:** Add an `ExportFormat` enum (`Stl`, `ThreeMf`, `Obj`) separate from the import `MeshFormat`. This avoids confusion between input detection (which distinguishes STL variants) and output selection (which only has binary STL). The path-based API infers `ExportFormat` from extension.

**Alternative:** Reuse `MeshFormat` but document that `StlBinary` is the export format and `StlAscii` returns an error for export. Less clean but fewer types.

**Recommendation:** Use a simple `ExportFormat` enum to keep the API clean. The existing `MeshFormat` stays for import detection.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Binary STL writing | Custom byte-level STL writer | `BinaryStlExporter::write(&model, writer)` | Handles normal computation, byte ordering, header correctly |
| OBJ writing | Custom text-based OBJ writer | `ObjExporter::write(&model, writer)` | Handles vertex indexing, group naming, transforms |
| 3MF writing | Custom ZIP+XML construction | `Model::write(&mut writer)` | Handles XML serialization, ZIP packaging, content types |
| STL format detection for input | N/A | Already have `detect.rs` | Keep existing detection for import |

**Key insight:** All three export formats have existing, tested implementations in lib3mf-core/lib3mf-converters. The only new code needed is the `TriangleMesh -> Model` conversion bridge and the thin API surface.

## Common Pitfalls

### Pitfall 1: f64 -> f32 Precision Loss in Vertex Conversion
**What goes wrong:** TriangleMesh stores vertices as f64 Point3, but lib3mf-core Mesh stores f32 Vertex. The conversion `v.x as f32` can lose precision.
**Why it happens:** Mesh geometry in 3D printing is typically well within f32 range, but extremely precise coordinates or large coordinates could lose bits.
**How to avoid:** This is acceptable and expected. STL is inherently f32, OBJ typically f32, and 3MF spec uses float (f32). The import path already does the reverse (f32 -> f64 lossless). Document this as expected behavior.
**Warning signs:** Round-trip test (export then import) should produce vertices within f32 epsilon of originals, not exact f64 match.

### Pitfall 2: Empty Mesh Export
**What goes wrong:** Attempting to export an empty TriangleMesh (0 vertices/triangles) would create an invalid Model.
**Why it happens:** TriangleMesh::new() rejects empty meshes, but a caller might have a mesh reference from somewhere unexpected.
**How to avoid:** The save functions should validate mesh is non-empty. TriangleMesh invariant already guarantees this if constructed via `new()`, but belt-and-suspenders validation is good.
**Warning signs:** N/A -- TriangleMesh construction prevents this.

### Pitfall 3: lib3mf-core 0.3 -> 0.4 Breaking Changes
**What goes wrong:** API changes between versions could break existing threemf.rs code.
**Why it happens:** Semver allows breaking changes before 1.0.
**How to avoid:** Verified via `git diff v0.3.0..HEAD` on the lib3mf-rs repo: changes between 0.3 and 0.4 are almost entirely doc comments and a new `ResolvedMesh` export. The Object, Mesh, Model, BuildItem, ResourceId types are unchanged. The upgrade should be a version bump only.
**Warning signs:** Compilation errors after version bump -- fix immediately.

### Pitfall 4: MeshFormat Stl Export Ambiguity
**What goes wrong:** Existing MeshFormat has StlBinary/StlAscii variants. Using it for export creates confusion about which STL format to write.
**Why it happens:** The enum was designed for import detection, not output selection.
**How to avoid:** Use a separate ExportFormat enum for output, or document that only StlBinary is used for export.

### Pitfall 5: File I/O in save_mesh vs save_mesh_to_writer
**What goes wrong:** The path-based `save_mesh` needs to open a file, but the writer-based API takes a pre-opened writer. The 3MF writer needs `Write + Seek` (for ZIP), but STL/OBJ only need `Write`.
**Why it happens:** `Model::write()` requires `Write + Seek` because ZIP archives need seeking. `BinaryStlExporter::write()` and `ObjExporter::write()` only need `Write`.
**How to avoid:** The path-based API opens a `File` (which implements both `Write` and `Seek`). The writer-based API may need `W: Write + Seek` for 3MF, or accept `W: Write` and buffer internally for non-seekable writers. Simplest approach: require `W: Write + Seek` for the 3MF path, or use `Cursor<Vec<u8>>` as an intermediate buffer.

## Code Examples

### TriangleMesh -> Model Conversion (verified from existing test code)
```rust
// Source: crates/slicecore-fileio/src/lib.rs test load_mesh_dispatches_3mf (lines 176-212)
// This pattern is already proven in tests -- just extract to a function.
use lib3mf_core::model::{Geometry, Mesh, Object, ObjectType, ResourceId, BuildItem};
use lib3mf_core::Model;

fn triangle_mesh_to_model(mesh: &TriangleMesh) -> Result<Model, FileIOError> {
    let mut lib3mf_mesh = Mesh::new();
    for v in mesh.vertices() {
        lib3mf_mesh.add_vertex(v.x as f32, v.y as f32, v.z as f32);
    }
    for tri in mesh.indices() {
        lib3mf_mesh.add_triangle(tri[0], tri[1], tri[2]);
    }

    let mut model = Model::default();
    let object = Object {
        id: ResourceId(1),
        object_type: ObjectType::Model,
        name: None,
        part_number: None,
        uuid: None,
        pid: None,
        pindex: None,
        thumbnail: None,
        geometry: Geometry::Mesh(lib3mf_mesh),
    };
    model.resources.add_object(object)
        .map_err(|e| FileIOError::WriteError(e.to_string()))?;
    model.build.items.push(BuildItem {
        object_id: ResourceId(1),
        uuid: None,
        path: None,
        part_number: None,
        transform: glam::Mat4::IDENTITY,
        printable: None,
    });
    Ok(model)
}
```

### BinaryStlExporter Usage
```rust
// Source: lib3mf-converters/src/stl.rs doc example
use lib3mf_converters::stl::BinaryStlExporter;
BinaryStlExporter::write(&model, writer)
    .map_err(|e| FileIOError::WriteError(e.to_string()))?;
```

### ObjExporter Usage
```rust
// Source: lib3mf-converters/src/obj.rs doc example
use lib3mf_converters::obj::ObjExporter;
ObjExporter::write(&model, writer)
    .map_err(|e| FileIOError::WriteError(e.to_string()))?;
```

### 3MF Write Usage
```rust
// Source: lib3mf-core Model::write, used in existing tests
model.write(&mut writer)
    .map_err(|e| FileIOError::WriteError(e.to_string()))?;
```

### CLI Convert Subcommand Pattern
```rust
// Source: existing CLI subcommand pattern in main.rs
/// Convert a mesh file between formats
Convert {
    /// Input mesh file path
    input: PathBuf,
    /// Output mesh file path (format detected from extension)
    output: PathBuf,
},
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| lib3mf 0.1.3 (third-party) | lib3mf-core 0.3 (own crate) | Phase 22 | Already migrated; this phase bumps to 0.4 |
| Import-only file I/O | Bidirectional import+export | Phase 24 (this) | Enables mesh format conversion workflows |

**Deprecated/outdated:**
- lib3mf (third-party crate): Replaced in Phase 22 with lib3mf-core

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + cargo test |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p slicecore-fileio` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| N/A-01 | save_mesh writes valid 3MF | unit | `cargo test -p slicecore-fileio save_mesh_3mf` | Wave 0 |
| N/A-02 | save_mesh writes valid Binary STL | unit | `cargo test -p slicecore-fileio save_mesh_stl` | Wave 0 |
| N/A-03 | save_mesh writes valid OBJ | unit | `cargo test -p slicecore-fileio save_mesh_obj` | Wave 0 |
| N/A-04 | Round-trip: export then import preserves geometry | integration | `cargo test -p slicecore-fileio round_trip` | Wave 0 |
| N/A-05 | CLI convert subcommand works | integration | `cargo test -p slicecore-cli convert` | Wave 0 |
| N/A-06 | lib3mf-core 0.4 upgrade compiles | build | `cargo check -p slicecore-fileio` | Existing |
| N/A-07 | Format detection from extension | unit | `cargo test -p slicecore-fileio format_from_ext` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-fileio`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-fileio/src/export.rs` -- new module with conversion + export logic
- [ ] Round-trip tests in export.rs or lib.rs tests
- [ ] CLI convert subcommand test

## Open Questions

1. **Write + Seek for save_mesh_to_writer with 3MF**
   - What we know: `Model::write()` requires `W: Write + Seek` because ZIP needs seeking. STL and OBJ only need `W: Write`.
   - What's unclear: Should `save_mesh_to_writer` require `W: Write + Seek` universally (penalizing STL/OBJ callers), or should there be separate handling?
   - Recommendation: Use `W: Write + Seek` for the generic API. File and `Cursor<Vec<u8>>` both implement it. For callers with non-seekable writers wanting STL/OBJ, they can wrap in a Cursor. This matches the simplest correct approach.

2. **3MF Default Metadata (units, model name)**
   - What we know: `Model::default()` creates a model with no unit specification. The 3MF spec defaults to millimeters.
   - What's unclear: Should the conversion set explicit unit metadata?
   - Recommendation: Use `Model::default()` which gives millimeter units implicitly. TriangleMesh has no unit information, so the export cannot set it meaningfully. This matches the minimal-metadata approach.

## Sources

### Primary (HIGH confidence)
- Local codebase: `crates/slicecore-fileio/src/` -- all existing import code verified
- Local codebase: `~/lib3mf-rs/crates/lib3mf-converters/src/stl.rs` -- BinaryStlExporter::write API verified
- Local codebase: `~/lib3mf-rs/crates/lib3mf-converters/src/obj.rs` -- ObjExporter::write API verified
- Local codebase: `~/lib3mf-rs/crates/lib3mf-core/` -- Model::write API verified in existing test code
- crates.io: lib3mf-core 0.4.0 and lib3mf-converters 0.4.0 confirmed published
- git diff v0.3.0..HEAD on lib3mf-rs: API changes between 0.3 and 0.4 are doc-only (no breaking changes)

### Secondary (MEDIUM confidence)
- None needed -- all sources are primary (owned codebase)

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - both crates are owned by the user, verified on crates.io at 0.4.0, API inspected locally
- Architecture: HIGH - follows existing import pattern exactly, test code already demonstrates the conversion
- Pitfalls: HIGH - all identified from direct code inspection of both codebases

**Research date:** 2026-03-10
**Valid until:** 2026-04-10 (stable -- own crates, APIs verified)
