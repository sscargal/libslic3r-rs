# Phase 22: Migrate from lib3mf to lib3mf-core ecosystem - Research

**Researched:** 2026-02-25
**Domain:** Rust 3MF parsing library migration (lib3mf 0.1.3 -> lib3mf-core 0.2.0)
**Confidence:** HIGH

## Summary

This phase replaces the `lib3mf` 0.1.3 crate dependency in `slicecore-fileio` with `lib3mf-core` 0.2.0 from the same author (sscargal). Both crates are pure Rust 3MF parsers, but lib3mf 0.1.3 uses `zip = "7.2"` with default features which pulls in `zstd-sys` (a C library), blocking WASM compilation. lib3mf-core 0.2.0 uses `zip = { version = "2.2.0", default-features = false, features = ["deflate"] }` which is entirely pure Rust and WASM-compatible.

The migration surface is small: two files (`threemf.rs` and `lib.rs`) in the `slicecore-fileio` crate. The API changes are well-defined -- primarily type-level differences (f64->f32 coordinates, usize->u32 indices, Vec->HashMap resources) and a different reading/writing workflow. The reading path changes from `Model::from_reader(cursor)` to either `Model::from_file(path)` or the lower-level `ZipArchiver::new(cursor)` + `find_model_path()` + `parse_model()` pipeline. For in-memory byte parsing (our use case), the archive-based pipeline is required.

**Primary recommendation:** Replace `lib3mf` with `lib3mf-core` using `default-features = false` (no crypto, parallel, or png-validation needed). Remove the `cfg(not(target_arch = "wasm32"))` gate on the `threemf` module and its dispatch function. Add a WASM CI test that proves 3MF parsing works on wasm32 targets.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Use only the minimum necessary crates from the lib3mf-core ecosystem on crates.io (latest stable version)
- Research what each ecosystem crate provides (lib3mf-core, lib3mf-converters, lib3mf-cli, lib3mf-async) and pull in only what's needed for current functionality
- lib3mf-async deferred to a future phase
- Any identified gaps between current usage and lib3mf-core capabilities should be tracked as TODOs or future phase items
- **Exclusion:** `threemf` (v0.7.0) and `threemf2` (v0.1.2) crates are NOT ours and must NOT be used
- Add a WASM 3MF parsing test to CI to prove the migration unlocked WASM support
- Direct adoption of lib3mf-core types and idioms -- no thin adapter layer
- Full migration of both production code and test code (no references to old lib3mf should remain)
- Track any gaps between lib3mf-core and old lib3mf as TODO items or deferred items for future work
- Behavioral equivalence is sufficient for test assertions -- same mesh geometry and triangle count, minor ordering differences acceptable

### Claude's Discretion
- Whether to enable 3MF on WASM (remove cfg gate) based on lib3mf-core's actual WASM compatibility
- Whether to remove the WASM error fallback path or keep it, based on how cleanly lib3mf-core compiles for WASM
- Which WASM targets to test (wasm32-unknown-unknown, wasm32-wasip2, or both)
- Error handling approach -- same ThreeMfError pattern with new source, or richer errors if lib3mf-core provides them
- Whether public API of slicecore-fileio changes -- prefer stability unless compelling reason
- Whether to do minimal swap or also adopt easy quick wins from lib3mf-core
- Whether write support is needed beyond tests (check codebase)

### Deferred Ideas (OUT OF SCOPE)
- lib3mf-async integration for async slicing pipelines -- future phase
- Extended 3MF features beyond current read/write (e.g., materials, colors, metadata) -- future phase if lib3mf-core supports them
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| lib3mf-core | 0.2.0 | 3MF parsing, validation, writing | Pure Rust, WASM-compatible, deflate-only ZIP, same author as lib3mf, richer API |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| lib3mf-converters | latest | STL/OBJ <-> 3MF conversion | NOT needed -- slicecore-fileio already has its own STL/OBJ parsers |
| lib3mf-cli | latest | CLI 3MF inspection tool | NOT needed -- this is an end-user tool, not a library |
| lib3mf-async | latest | Async I/O for 3MF | NOT needed -- deferred per user decision |
| lib3mf-wasm | latest | WASM bindings | NOT needed -- lib3mf-core itself compiles to WASM; this is for standalone WASM usage |

### Ecosystem Crate Analysis (per user requirement)

| Crate | What It Provides | Needed? | Reason |
|-------|-----------------|---------|--------|
| **lib3mf-core** | Core 3MF parsing, model types, validation, writing, archive handling | YES | Direct replacement for lib3mf -- provides Model, Mesh, Vertex, Triangle, Object, Build, BuildItem, plus ZipArchiver for reading from bytes |
| **lib3mf-converters** | STL importer/exporter, OBJ importer/exporter | NO | slicecore-fileio has its own STL (stl_binary, stl_ascii) and OBJ parsers; would create redundant functionality |
| **lib3mf-cli** | Command-line tool for inspecting/converting 3MF files | NO | End-user tool, not a library dependency |
| **lib3mf-async** | Async wrappers around lib3mf-core using tokio | NO | Deferred to future phase per user decision |
| **lib3mf-wasm** | Pre-built WASM bindings for browser use | NO | lib3mf-core compiles natively to WASM; this crate is for standalone WASM module distribution |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| lib3mf-core | threemf (v0.7.0) | EXCLUDED by user -- not our crate, different author |
| lib3mf-core | threemf2 (v0.1.2) | EXCLUDED by user -- not our crate, different author |
| lib3mf-core | Manual XML+ZIP parsing | Far more work, reinventing the wheel |

### Installation

In `crates/slicecore-fileio/Cargo.toml`:
```toml
# Remove:
# [target.'cfg(not(target_arch = "wasm32"))'.dependencies]
# lib3mf = { version = "0.1", default-features = false }

# Add (unconditional -- works on all targets including WASM):
[dependencies]
lib3mf-core = { version = "0.2", default-features = false }
```

## Architecture Patterns

### Current Architecture (Before Migration)

```
crates/slicecore-fileio/
  src/
    lib.rs           # cfg-gates threemf module, WASM fallback dispatch
    threemf.rs       # Uses lib3mf::Model::from_reader, iterates resources.objects
    error.rs         # ThreeMfError(String) variant
    detect.rs        # Format detection (3MF = ZIP magic bytes)
    stl_binary.rs    # Binary STL parser
    stl_ascii.rs     # ASCII STL parser
    obj.rs           # OBJ parser
    stl.rs           # STL dispatch
  Cargo.toml         # lib3mf cfg-gated behind not(wasm32)
```

### Target Architecture (After Migration)

```
crates/slicecore-fileio/
  src/
    lib.rs           # NO cfg gate -- threemf always available, remove WASM fallback
    threemf.rs       # Uses lib3mf_core archive pipeline, iterates resources
    error.rs         # ThreeMfError(String) unchanged
    detect.rs        # Unchanged
    stl_binary.rs    # Unchanged
    stl_ascii.rs     # Unchanged
    obj.rs           # Unchanged
    stl.rs           # Unchanged
  Cargo.toml         # lib3mf-core unconditional dependency
```

### Pattern 1: Reading 3MF from Bytes (Current -- lib3mf 0.1.3)

```rust
// Source: crates/slicecore-fileio/src/threemf.rs (current code)
use std::io::Cursor;

pub fn parse(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let cursor = Cursor::new(data);
    let model = lib3mf::Model::from_reader(cursor)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;

    for object in &model.resources.objects {
        if let Some(mesh) = &object.mesh {
            for v in &mesh.vertices {  // v.x, v.y, v.z are f64
                all_vertices.push(Point3::new(v.x, v.y, v.z));
            }
            for tri in &mesh.triangles {  // tri.v1, tri.v2, tri.v3 are usize
                all_indices.push([tri.v1 as u32 + offset, ...]);
            }
        }
    }
    // ...
}
```

### Pattern 2: Reading 3MF from Bytes (Target -- lib3mf-core 0.2.0)

```rust
// Source: lib3mf-core README + docs.rs API analysis
use std::io::Cursor;
use lib3mf_core::archive::{ZipArchiver, find_model_path};
use lib3mf_core::parser::parse_model;

pub fn parse(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let cursor = Cursor::new(data);
    let mut archiver = ZipArchiver::new(cursor)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model_path = find_model_path(&mut archiver)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model_data = archiver.read_entry(&model_path)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model = parse_model(Cursor::new(model_data))
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;

    for (_id, object) in model.resources.iter_objects() {
        if let Some(mesh) = &object.mesh {
            for v in &mesh.vertices {  // v.x, v.y, v.z are f32
                all_vertices.push(Point3::new(v.x as f64, v.y as f64, v.z as f64));
            }
            for tri in &mesh.triangles {  // tri.v1, tri.v2, tri.v3 are u32
                all_indices.push([tri.v1 + offset, tri.v2 + offset, tri.v3 + offset]);
            }
        }
    }
    // ...
}
```

### Pattern 3: Writing 3MF for Tests (Current -- lib3mf 0.1.3)

```rust
// Source: crates/slicecore-fileio/src/threemf.rs tests (current code)
let mut model = lib3mf::Model::new();
let mut mesh = lib3mf::Mesh::new();
mesh.vertices.push(lib3mf::Vertex::new(0.0, 0.0, 0.0));  // f64
mesh.triangles.push(lib3mf::Triangle::new(0, 1, 2));       // usize
let mut object = lib3mf::Object::new(1);                    // usize id
object.mesh = Some(mesh);
model.resources.objects.push(object);
model.build.items.push(lib3mf::BuildItem::new(1));

let mut buffer = Cursor::new(Vec::new());
model.to_writer(&mut buffer).expect("write 3MF");
```

### Pattern 4: Writing 3MF for Tests (Target -- lib3mf-core 0.2.0)

```rust
// Source: lib3mf-core docs + API analysis
use lib3mf_core::{Model, model::*};

let mut model = Model::default();
let mut mesh = Mesh::new();
mesh.add_vertex(0.0, 0.0, 0.0);      // f32, returns u32 index
mesh.add_triangle(0, 1, 2);           // u32 indices
// Object creation -- needs ResourceId, add to ResourceCollection
let mut object = Object { mesh: Some(mesh), ..Default::default() };
model.resources.add_object(ResourceId(1), object)?;
model.build.items.push(BuildItem { object_id: ResourceId(1), ..Default::default() });

let mut buffer = Cursor::new(Vec::new());
model.write(&mut buffer)?;
```

### Anti-Patterns to Avoid
- **Wrapping lib3mf-core in an adapter layer**: User explicitly decided "direct adoption of lib3mf-core types and idioms -- no thin adapter layer"
- **Keeping cfg gates for WASM**: lib3mf-core compiles cleanly to WASM; keeping the gate defeats the purpose of the migration
- **Using lib3mf-converters for STL/OBJ**: slicecore-fileio already has its own parsers; adding converters creates redundancy
- **Importing more lib3mf-core features than needed**: Use `default-features = false` -- no crypto, parallel, or png-validation

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 3MF ZIP parsing | Custom ZIP+XML pipeline | lib3mf-core ZipArchiver + parse_model | 3MF is a complex OPC format with relationships, extensions, and validation requirements |
| 3MF model serialization | Custom XML writer | lib3mf-core Model::write() | Correct OPC packaging requires proper relationship files, content types, etc. |
| 3MF validation | Custom validation | lib3mf-core validate(ValidationLevel) | 4 validation levels covering core spec + extensions |

**Key insight:** 3MF is not just "XML in a ZIP." It follows the Open Packaging Conventions (OPC) standard with relationship files, content types, and a specific directory structure. Hand-rolling any of this is error-prone.

## Common Pitfalls

### Pitfall 1: Vertex Coordinate Precision Change (f64 -> f32)
**What goes wrong:** lib3mf-core Vertex uses f32 coordinates while the old lib3mf used f64. If the conversion is not handled, precision loss could affect downstream geometry.
**Why it happens:** The 3MF specification actually defines vertex coordinates as ST_Number (decimal), and f32 provides sufficient precision for 3D printing (sub-micrometer accuracy). lib3mf-core chose f32 for performance.
**How to avoid:** Cast f32 to f64 explicitly when converting to Point3 (`v.x as f64`). This is lossless (f32 -> f64 is always exact). The precision change is actually correct for 3MF since the spec uses floating-point.
**Warning signs:** Test assertions that compare exact floating-point coordinates may need epsilon comparisons.

### Pitfall 2: Resource Collection API Difference (Vec -> HashMap)
**What goes wrong:** Old code iterates `model.resources.objects` (a `Vec<Object>`). New code must use `model.resources.iter_objects()` which yields `(&ResourceId, &Object)` tuples from a `HashMap`.
**Why it happens:** lib3mf-core uses a global resource ID namespace with HashMap for O(1) lookup, while lib3mf used a simpler Vec.
**How to avoid:** Replace `for object in &model.resources.objects` with `for (_id, object) in model.resources.iter_objects()`. The mesh access pattern (`object.mesh`) remains the same.
**Warning signs:** Compilation errors about `resources.objects` not being a field.

### Pitfall 3: Object Iteration Order May Differ
**What goes wrong:** HashMap iteration order is nondeterministic, so multi-object 3MF files may yield objects in a different order than the old Vec-based approach.
**Why it happens:** HashMap does not preserve insertion order.
**How to avoid:** User decision says "behavioral equivalence is sufficient -- same mesh geometry and triangle count, minor ordering differences acceptable." Tests should assert on total counts, not object ordering.
**Warning signs:** Tests that check specific vertex positions in multi-object files may fail if they assumed a particular object order.

### Pitfall 4: Reading Pipeline Change (Single Call -> Multi-Step)
**What goes wrong:** Old code used a single `Model::from_reader(cursor)` call. lib3mf-core requires: `ZipArchiver::new()` -> `find_model_path()` -> `read_entry()` -> `parse_model()`. Missing any step causes confusing errors.
**Why it happens:** lib3mf-core separates archive handling from XML parsing for flexibility (streaming, extension-specific parsing, etc.).
**How to avoid:** Wrap the multi-step pipeline in the existing `parse()` function. Each step maps to `FileIOError::ThreeMfError(e.to_string())`. Alternatively, check if `Model::from_file()` or `Model::from_reader()` convenience methods are available -- the README shows `Model::from_file()` exists, and web search suggests `Model::from_reader()` may also exist.
**Warning signs:** "entry not found" errors when skipping `find_model_path()`.

### Pitfall 5: Triangle Index Type Change (usize -> u32)
**What goes wrong:** Old code cast `tri.v1 as u32`. New code already has `tri.v1` as `u32`, so the cast is unnecessary but harmless.
**Why it happens:** lib3mf-core uses u32 natively for vertex indices (matching the 3MF spec's 32-bit unsigned integers).
**How to avoid:** Remove unnecessary `as u32` casts on triangle indices. The vertex offset addition (`tri.v1 + vertex_offset`) now works directly with u32 arithmetic.
**Warning signs:** Clippy warnings about unnecessary casts.

### Pitfall 6: Test Write API Changes
**What goes wrong:** Test helper functions that create 3MF data in-memory use lib3mf's write API extensively. Every `lib3mf::Vertex::new()`, `lib3mf::Triangle::new()`, `lib3mf::Object::new()`, `lib3mf::BuildItem::new()`, and `model.to_writer()` call must be updated.
**Why it happens:** lib3mf-core has different constructors: `Mesh::add_vertex()` instead of `Vertex::new()` + push, `Mesh::add_triangle()` instead of `Triangle::new()` + push, `ResourceCollection::add_object()` instead of Vec push, `model.write()` instead of `model.to_writer()`.
**How to avoid:** Systematically update all test code. The tests are self-contained in `threemf.rs` and `lib.rs` -- about 100 lines of test code total.
**Warning signs:** Compilation errors throughout the test module.

## Code Examples

### Example 1: Complete Parse Function (Production Code)
```rust
// Target implementation for crates/slicecore-fileio/src/threemf.rs
use std::io::Cursor;
use lib3mf_core::archive::{ZipArchiver, find_model_path, ArchiveReader};
use lib3mf_core::parser::parse_model;
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;
use crate::error::FileIOError;

pub fn parse(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let cursor = Cursor::new(data);
    let mut archiver = ZipArchiver::new(cursor)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model_path = find_model_path(&mut archiver)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model_data = archiver.read_entry(&model_path)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;
    let model = parse_model(Cursor::new(model_data))
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;

    let mut all_vertices: Vec<Point3> = Vec::new();
    let mut all_indices: Vec<[u32; 3]> = Vec::new();

    for (_id, object) in model.resources.iter_objects() {
        if let Some(mesh) = &object.mesh {
            let vertex_offset = all_vertices.len() as u32;
            for v in &mesh.vertices {
                // f32 -> f64 conversion is lossless
                all_vertices.push(Point3::new(v.x as f64, v.y as f64, v.z as f64));
            }
            for tri in &mesh.triangles {
                all_indices.push([
                    tri.v1 + vertex_offset,
                    tri.v2 + vertex_offset,
                    tri.v3 + vertex_offset,
                ]);
            }
        }
    }

    if all_vertices.is_empty() || all_indices.is_empty() {
        return Err(FileIOError::EmptyModel);
    }

    let mesh = TriangleMesh::new(all_vertices, all_indices)?;
    Ok(mesh)
}
```

### Example 2: Test Helper for Creating 3MF Data
```rust
// Target test helper for creating in-memory 3MF files
use std::io::Cursor;
use lib3mf_core::{Model, model::*};

fn create_test_3mf_with_mesh(vertices: &[(f32, f32, f32)], triangles: &[(u32, u32, u32)]) -> Vec<u8> {
    let mut model = Model::default();
    let mut mesh = Mesh::new();
    for &(x, y, z) in vertices {
        mesh.add_vertex(x, y, z);
    }
    for &(v1, v2, v3) in triangles {
        mesh.add_triangle(v1, v2, v3);
    }
    // Create object and add to resources
    // NOTE: Exact API for Object creation and ResourceCollection::add_object
    // needs verification during implementation -- may need Object::default()
    // or a constructor that takes ResourceId
    let object = Object { mesh: Some(mesh), ..Default::default() };
    model.resources.add_object(ResourceId(1), object).expect("add object");
    model.build.items.push(BuildItem {
        object_id: ResourceId(1),
        ..Default::default()
    });

    let mut buffer = Cursor::new(Vec::new());
    model.write(&mut buffer).expect("write 3MF");
    buffer.into_inner()
}
```

### Example 3: Removing the WASM cfg Gate
```rust
// BEFORE (lib.rs):
#[cfg(not(target_arch = "wasm32"))]
pub mod threemf;

#[cfg(not(target_arch = "wasm32"))]
fn parse_threemf_dispatch(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    threemf::parse(data)
}

#[cfg(target_arch = "wasm32")]
fn parse_threemf_dispatch(_data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    Err(FileIOError::ThreeMfError(
        "3MF parsing is not available on WASM targets".to_string(),
    ))
}

// AFTER (lib.rs):
pub mod threemf;  // Always available -- lib3mf-core is pure Rust

fn parse_threemf_dispatch(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    threemf::parse(data)
}
// WASM fallback removed entirely
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| lib3mf 0.1.3 with zip 7.2 (zstd-sys) | lib3mf-core 0.2.0 with zip 2.2.0 (deflate only) | 2026-02 | Eliminates C dependency, enables WASM |
| cfg-gated 3MF behind not(wasm32) | Unconditional 3MF on all targets | This phase | 3MF available everywhere including browser |
| Model::from_reader() single call | ZipArchiver + parse_model pipeline (or Model::from_reader if available) | 2026-02 | More explicit, supports streaming |
| Vec\<Object\> resource storage | HashMap\<ResourceId, Object\> with iter_objects() | 2026-02 | Better ID management, O(1) lookup |
| f64 vertex coordinates | f32 vertex coordinates | 2026-02 | Matches 3MF spec more closely, sufficient for 3D printing |

**Deprecated/outdated:**
- `lib3mf` 0.1.x: Still on crates.io but superseded by lib3mf-core for WASM-compatible use cases

## Type Migration Map

| lib3mf 0.1.3 | lib3mf-core 0.2.0 | Notes |
|---------------|-------------------|-------|
| `lib3mf::Model` | `lib3mf_core::Model` | Same concept, different API methods |
| `lib3mf::Model::new()` | `lib3mf_core::Model::default()` | Default trait instead of new() |
| `lib3mf::Model::from_reader(r)` | `ZipArchiver::new(r)` + `find_model_path()` + `parse_model()` | Multi-step pipeline; `Model::from_reader()` or `Model::from_file()` may also exist as convenience |
| `model.to_writer(w)` | `model.write(w)` | Method name change |
| `model.resources.objects` (Vec) | `model.resources.iter_objects()` (HashMap iter) | Yields `(&ResourceId, &Object)` |
| `lib3mf::Object::new(id: usize)` | Object with ResourceId | ID type changes from usize to ResourceId |
| `object.mesh: Option<Mesh>` | `object.mesh: Option<Mesh>` | Same pattern |
| `lib3mf::Mesh::new()` | `lib3mf_core::Mesh::new()` | Same |
| `mesh.vertices.push(Vertex::new(x,y,z))` | `mesh.add_vertex(x, y, z)` | Helper method; returns u32 index |
| `mesh.triangles.push(Triangle::new(v1,v2,v3))` | `mesh.add_triangle(v1, v2, v3)` | Helper method |
| `lib3mf::Vertex { x: f64, y: f64, z: f64 }` | `lib3mf_core::Vertex { x: f32, y: f32, z: f32 }` | Precision change |
| `lib3mf::Triangle { v1: usize, v2: usize, v3: usize, ... }` | `lib3mf_core::Triangle { v1: u32, v2: u32, v3: u32, ... }` | Index type change |
| `lib3mf::BuildItem::new(id: usize)` | `BuildItem { object_id: ResourceId(id), ..Default::default() }` | Struct literal instead of constructor |
| `model.build.items.push(item)` | `model.build.items.push(item)` | Same |
| `model.resources.objects.push(obj)` | `model.resources.add_object(id, obj)?` | Returns Result (validates uniqueness) |

## Files Requiring Changes

| File | Changes Needed | Complexity |
|------|---------------|------------|
| `crates/slicecore-fileio/Cargo.toml` | Replace lib3mf with lib3mf-core, remove cfg gate | LOW |
| `crates/slicecore-fileio/src/threemf.rs` | Update parse() function + all test code | MEDIUM |
| `crates/slicecore-fileio/src/lib.rs` | Remove cfg gates, remove WASM fallback, update 3MF test | LOW |
| `.github/workflows/ci.yml` | Potentially add WASM test step (beyond just build) | LOW |

### Write Support Analysis (per discretion item)

Checked codebase for 3MF write usage beyond tests:
- Production code: Only `parse()` function (read-only). No production write code exists.
- Test code: `create_test_3mf()`, `tetrahedron_3mf()`, `empty_3mf_returns_empty_model()`, `load_mesh_dispatches_3mf()` -- all use write API to create test fixtures.
- Conclusion: Write support is needed only for tests. No production write API is exposed.

## Open Questions

1. **Model::from_reader() availability in lib3mf-core**
   - What we know: README shows `Model::from_file()` exists. Web search suggests `Model::from_reader()` may also exist. The multi-step ZipArchiver pipeline definitely works.
   - What's unclear: Whether `Model::from_reader()` exists as a convenience method in lib3mf-core 0.2.0, or only `from_file()` (path-based).
   - Recommendation: During implementation, first try `Model::from_reader(Cursor::new(data))`. If it exists, use it (simplest migration). If not, use the multi-step ZipArchiver pipeline. Either approach works; the multi-step pipeline is documented and verified. **Confidence: MEDIUM**

2. **Exact Object construction API in lib3mf-core**
   - What we know: ResourceCollection has `add_object(ResourceId, Object)` method. Object has a `mesh` field. Mesh has `add_vertex()` and `add_triangle()`.
   - What's unclear: Whether Object has `Default` derive, or needs a specific constructor. Whether ResourceId wraps u32 or usize.
   - Recommendation: Verify during implementation by checking the compiler errors. The pattern `Object { mesh: Some(mesh), ..Default::default() }` should work if Object derives Default. **Confidence: MEDIUM**

3. **ArchiveReader trait import requirement**
   - What we know: `read_entry()` is defined on the `ArchiveReader` trait. ZipArchiver implements it.
   - What's unclear: Whether `use lib3mf_core::archive::ArchiveReader` is needed in scope to call `archiver.read_entry()`.
   - Recommendation: Import the trait if needed. Rust requires traits to be in scope for method calls. **Confidence: HIGH**

## Discretion Recommendations

Based on research findings, here are recommendations for the Claude's Discretion items:

1. **Enable 3MF on WASM (remove cfg gate)?** YES -- lib3mf-core explicitly advertises WASM support and uses deflate-only ZIP (no C dependencies). Remove the `#[cfg(not(target_arch = "wasm32"))]` gate entirely.

2. **Remove WASM error fallback path?** YES -- with lib3mf-core, the WASM fallback is dead code. Remove both the `#[cfg(target_arch = "wasm32")]` fallback function and the error message. This simplifies the code.

3. **Which WASM targets to test?** BOTH `wasm32-unknown-unknown` and `wasm32-wasip2` -- the existing CI already builds both targets. Add a compilation test or basic smoke test for 3MF on both.

4. **Error handling approach?** Keep `ThreeMfError(String)` pattern. lib3mf-core uses `Lib3mfError` which implements Display. Map it with `.map_err(|e| FileIOError::ThreeMfError(e.to_string()))` -- same pattern as current code. No need for richer errors since the string captures the detail.

5. **Public API changes?** NO changes needed. The `parse()` function signature (`fn parse(data: &[u8]) -> Result<TriangleMesh, FileIOError>`) is unchanged. The `load_mesh()` dispatcher is unchanged. The `FileIOError` enum is unchanged.

6. **Minimal swap or adopt quick wins?** Minimal swap. The goal is to replace the dependency, not to adopt new features. Quick wins from lib3mf-core (validation, streaming) can be explored in future phases.

7. **Write support beyond tests?** NO -- only test code uses write API. No production write path exists.

## Sources

### Primary (HIGH confidence)
- [docs.rs/lib3mf-core](https://docs.rs/lib3mf-core/latest/lib3mf_core/) - API surface, module structure, version 0.2.0
- [docs.rs/lib3mf/0.1.5](https://docs.rs/lib3mf/latest/lib3mf/) - Current lib3mf API for migration mapping
- [GitHub sscargal/lib3mf-rs](https://github.com/sscargal/lib3mf-rs) - Repository structure, README, workspace layout
- [GitHub raw source: mesh.rs](https://raw.githubusercontent.com/sscargal/lib3mf-rs/refs/heads/main/crates/lib3mf-core/src/model/mesh.rs) - Vertex (f32), Triangle (u32), Mesh struct definitions verified from source
- [GitHub raw source: core.rs](https://raw.githubusercontent.com/sscargal/lib3mf-rs/refs/heads/main/crates/lib3mf-core/src/model/core.rs) - Model struct with ResourceCollection, Build fields verified
- [GitHub raw source: build.rs](https://raw.githubusercontent.com/sscargal/lib3mf-rs/refs/heads/main/crates/lib3mf-core/src/model/build.rs) - Build and BuildItem struct definitions verified
- [GitHub raw source: resources.rs](https://raw.githubusercontent.com/sscargal/lib3mf-rs/refs/heads/main/crates/lib3mf-core/src/model/resources.rs) - ResourceCollection with HashMap<ResourceId, Object> and iter_objects() verified
- [GitHub raw source: Cargo.toml](https://raw.githubusercontent.com/sscargal/lib3mf-rs/refs/heads/main/crates/lib3mf-core/Cargo.toml) - zip dependency: `version = "2.2.0", default-features = false, features = ["deflate"]` verified
- Codebase: `/home/steve/libslic3r-rs/crates/slicecore-fileio/` - Current implementation verified from source
- Codebase: `/home/steve/libslic3r-rs/Cargo.lock` - lib3mf 0.1.3 depends on zip (default features) confirmed

### Secondary (MEDIUM confidence)
- [lib3mf-core README](https://raw.githubusercontent.com/sscargal/lib3mf-rs/refs/heads/main/crates/lib3mf-core/README.md) - Model::from_file() example, write_model() example
- [lib3mf-rs README](https://raw.githubusercontent.com/sscargal/lib3mf-rs/refs/heads/main/README.md) - ZipArchiver + find_model_path + parse_model workflow example

### Tertiary (LOW confidence)
- Web search suggesting `Model::from_reader()` exists in lib3mf-core -- needs validation during implementation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - lib3mf-core is the only option that matches all constraints (pure Rust, WASM, same author)
- Architecture: HIGH - Migration surface is small (2 files), API mapping is well-understood from docs and source
- Pitfalls: HIGH - All type differences verified from actual source code (f32 vs f64, u32 vs usize, HashMap vs Vec)
- Code examples: MEDIUM - Reading pipeline verified; writing/Object construction API needs implementation-time verification

**Research date:** 2026-02-25
**Valid until:** 2026-03-25 (stable library, unlikely to change significantly)
