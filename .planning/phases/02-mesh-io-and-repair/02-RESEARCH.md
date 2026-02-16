# Phase 2: Mesh I/O and Repair - Research

**Researched:** 2026-02-16
**Domain:** 3D file format parsing (STL/3MF/OBJ), mesh repair algorithms (non-manifold, self-intersection, degenerate triangles), G-code emission, ValidPolygon enforcement
**Confidence:** HIGH

## Summary

Phase 2 builds two new crates on top of the Phase 1 foundation: `slicecore-fileio` (Layer 1: file format import) and `slicecore-gcode-io` (Layer 1: G-code output). It also extends `slicecore-mesh` with mesh repair algorithms and enhances the existing mesh transformation suite. The ValidPolygon type system from Phase 1 (slicecore-geo) is already in place and enforces the requirement that only cleaned/validated geometry enters downstream algorithms.

The file format landscape in Rust is well-served for this domain. STL parsing is simple enough to hand-roll (binary STL is 84 bytes header + 50 bytes/triangle; ASCII STL is a simple text grammar). The `lib3mf` crate (v0.1.3, pure Rust, published by the project author) handles 3MF reading with full mesh vertex/triangle extraction. OBJ parsing can be done with `tobj` (v4.0.3, mature, 66 reverse dependencies, WASM-aware) or hand-rolled for the minimal subset needed (vertices + faces only). G-code writing is straightforward text emission and should be hand-rolled -- no existing Rust crate handles multi-dialect G-code generation for 3D printing.

Mesh repair is the most algorithmically complex area. The PrusaSlicer/admesh approach uses a well-defined repair pipeline: (1) connect nearby edges within tolerance, (2) remove degenerate triangles, (3) fill holes by adding triangles, (4) fix normal directions for consistent winding, (5) fix normal values to be perpendicular unit vectors. Self-intersection detection and repair is significantly more complex, requiring spatial indexing (BVH already exists from Phase 1) and triangle-triangle intersection tests. For Phase 2, a practical approach is to implement detection with a "flag and report" strategy, deferring full topological self-intersection resolution to a future enhancement.

**Primary recommendation:** Hand-roll STL parser and G-code writer (simple formats, full control, no dependency baggage). Use `lib3mf` for 3MF (complex ZIP+XML format, already proven pure Rust). Use `tobj` for OBJ unless WASM concerns arise (then hand-roll the ~200-line subset parser). Implement mesh repair following the admesh pipeline order. Validate WASM compilation for all new dependencies at integration time.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| lib3mf | 0.1.3 | 3MF file reading (ZIP+XML container with mesh data) | Pure Rust, no unsafe code, handles OPC/XML parsing, mesh vertex/triangle extraction, published by project author. Complex format that should NOT be hand-rolled. |
| byteorder | 1.x | Binary STL reading (little-endian f32 values) | no_std compatible, WASM-safe, zero-cost abstractions for endian-aware reads. Widely used (3000+ dependents). |
| thiserror | 2.x | Error type derivation for new crates | Already in workspace. WASM-compatible. |
| serde + serde_derive | 1.x | Serialization for file format types | Already in workspace. Needed for G-code metadata, mesh stats serialization. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tobj | 4.0.3 | OBJ file parsing (Wavefront format) | If full OBJ support is needed (materials, normals, texture coords). Has WASM-aware async loading. If only vertices+faces needed, hand-rolling is simpler (~200 lines). |
| memmap2 | 0.9.x | Memory-mapped file I/O for large STL files | Performance optimization for very large binary STL files (>100MB). Not needed for MVP; can be added later behind a feature flag. Not WASM-compatible (requires OS file mapping). |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled STL parser | stl_io v0.10.0 | stl_io works but has limited API (read_stl returns IndexedMesh, not directly compatible with our TriangleMesh). Hand-rolling gives us: direct-to-TriangleMesh construction (zero intermediate allocation), exact error handling we want, control over degenerate triangle handling during parse, no extra dependency. Binary STL parser is ~100 lines; ASCII is ~150 lines. |
| Hand-rolled STL parser | nom_stl v0.2.2 | nom_stl depends on nom 5.x (older version). Small additional dependency for a simple format. nom's combinator style adds complexity for what is fundamentally a sequential binary read. |
| lib3mf | threemf | threemf only supports WRITING 3MF files, not reading. Not suitable. |
| lib3mf | Rust3MF (asmatic77) | GitHub project, not published to crates.io, unknown maintenance status. lib3mf is published, documented, tested. |
| tobj for OBJ | obj-rs | obj-rs is viable but tobj has more users (66 reverse deps vs ~20 for obj-rs) and explicit async/WASM support. Either works for simple vertex+face loading. |
| Hand-rolled G-code writer | gen_gcode | gen_gcode is a simple library but doesn't handle firmware dialects (Marlin/Klipper/RRF/Bambu differences). G-code writing is simple text formatting; the complexity is in the dialect-specific differences, which no library handles. |
| Hand-rolled G-code writer | gcode crate v0.6.1 | gcode is a PARSER (no_std, embedded-focused), not a writer. Does not generate G-code. |

**Installation:**
```toml
# Cargo.toml for slicecore-fileio
[dependencies]
slicecore-math = { path = "../slicecore-math" }
slicecore-mesh = { path = "../slicecore-mesh" }
byteorder = "1"
lib3mf = "0.1"
serde = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
tempfile = "3"

# Cargo.toml for slicecore-gcode-io
[dependencies]
slicecore-math = { path = "../slicecore-math" }
serde = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
```

## Architecture Patterns

### Recommended Project Structure
```
crates/
├── slicecore-fileio/
│   └── src/
│       ├── lib.rs            # Re-exports, format detection
│       ├── detect.rs         # Magic-byte format sniffing
│       ├── stl_binary.rs     # Binary STL reader
│       ├── stl_ascii.rs      # ASCII STL reader
│       ├── stl.rs            # Unified STL interface (auto-detect binary/ascii)
│       ├── threemf.rs        # 3MF reader via lib3mf
│       ├── obj.rs            # OBJ reader (tobj or hand-rolled)
│       └── error.rs          # FileIOError types
├── slicecore-gcode-io/
│   └── src/
│       ├── lib.rs            # Re-exports
│       ├── writer.rs         # GcodeWriter trait + implementations
│       ├── dialect.rs        # Firmware dialect definitions
│       ├── commands.rs       # Structured G-code command types
│       ├── marlin.rs         # Marlin-specific formatting
│       ├── klipper.rs        # Klipper-specific formatting
│       ├── reprap.rs         # RepRapFirmware-specific formatting
│       ├── bambu.rs          # Bambu-specific formatting
│       └── error.rs          # GcodeError types
└── slicecore-mesh/           # EXTENDED (existing crate)
    └── src/
        ├── repair.rs         # NEW: Mesh repair pipeline
        ├── repair/
        │   ├── degenerate.rs # Remove degenerate triangles
        │   ├── normals.rs    # Fix normal directions and values
        │   ├── stitch.rs     # Connect nearby unconnected edges
        │   ├── holes.rs      # Fill holes by adding triangles
        │   └── intersect.rs  # Detect self-intersections
        └── ... (existing files unchanged)
```

### Pattern 1: Format Detection via Magic Bytes
**What:** Detect file format from the first few bytes before dispatching to format-specific parser.
**When to use:** When loading files where the extension may be wrong or missing.
**Example:**
```rust
// Source: STL format specification + 3MF (ZIP) specification
pub enum MeshFormat {
    StlBinary,
    StlAscii,
    ThreeMf,
    Obj,
}

pub fn detect_format(data: &[u8]) -> Result<MeshFormat, FileIOError> {
    if data.len() < 4 {
        return Err(FileIOError::FileTooSmall);
    }
    // 3MF is a ZIP file: starts with PK\x03\x04
    if data.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        return Ok(MeshFormat::ThreeMf);
    }
    // ASCII STL starts with "solid " followed by a name
    // BUT: some binary STL files also start with "solid" in their header
    // Heuristic: check if "facet normal" appears in first ~1000 bytes
    if data.starts_with(b"solid") {
        let check_range = &data[..data.len().min(1000)];
        if check_range.windows(12).any(|w| w == b"facet normal") {
            return Ok(MeshFormat::StlAscii);
        }
        // Could still be binary STL with "solid" in header
    }
    // Binary STL: 80-byte header + 4-byte triangle count
    // Validate: file_size == 84 + num_triangles * 50
    if data.len() >= 84 {
        let num_triangles = u32::from_le_bytes([data[80], data[81], data[82], data[83]]);
        let expected_size = 84 + num_triangles as usize * 50;
        if data.len() == expected_size || data.len() == expected_size + 1 {
            return Ok(MeshFormat::StlBinary);
        }
    }
    // OBJ detection: look for "v " (vertex) lines
    if data.starts_with(b"v ") || data.starts_with(b"# ") {
        return Ok(MeshFormat::Obj);
    }
    Err(FileIOError::UnrecognizedFormat)
}
```

### Pattern 2: Load-to-TriangleMesh Pipeline
**What:** Every file format parser returns `Result<TriangleMesh, FileIOError>`. The caller never sees format-specific intermediate types.
**When to use:** All file loading operations.
**Example:**
```rust
/// Unified loading function. Detects format and dispatches.
pub fn load_mesh(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    match detect_format(data)? {
        MeshFormat::StlBinary => stl_binary::parse(data),
        MeshFormat::StlAscii => stl_ascii::parse(data),
        MeshFormat::ThreeMf => threemf::parse(data),
        MeshFormat::Obj => obj::parse(data),
    }
}

/// Load mesh from a reader (for streaming / large files).
pub fn load_mesh_from_reader<R: std::io::Read>(
    reader: &mut R,
) -> Result<TriangleMesh, FileIOError> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    load_mesh(&data)
}
```

### Pattern 3: Mesh Repair Pipeline (admesh-inspired)
**What:** Repair operations are applied in a specific order. Each step may modify the mesh. The pipeline returns a RepairReport documenting what was changed.
**When to use:** After loading any mesh from a file, before passing to slicing.
**Example:**
```rust
// Source: admesh repair pipeline + PrusaSlicer TriangleMesh::repair()
pub struct RepairReport {
    pub degenerate_removed: usize,
    pub edges_stitched: usize,
    pub holes_filled: usize,
    pub normals_fixed: usize,
    pub self_intersections_detected: usize,
    pub was_already_clean: bool,
}

/// Full repair pipeline. Applies all fixes in the correct order.
pub fn repair(mesh: &mut TriangleMesh) -> RepairReport {
    let mut report = RepairReport::default();

    // Step 1: Remove degenerate triangles (zero area, duplicate vertices)
    report.degenerate_removed = remove_degenerate_triangles(mesh);

    // Step 2: Stitch nearby unconnected edges (within tolerance)
    report.edges_stitched = stitch_edges(mesh, STITCH_TOLERANCE);

    // Step 3: Fill holes by adding triangles
    report.holes_filled = fill_holes(mesh);

    // Step 4: Fix normal directions (consistent outward winding)
    report.normals_fixed = fix_normal_directions(mesh);

    // Step 5: Recompute normal values
    recompute_normals(mesh);

    // Step 6: Detect self-intersections (report, don't fix yet)
    report.self_intersections_detected = detect_self_intersections(mesh);

    report
}
```

### Pattern 4: G-code Writer with Dialect Abstraction
**What:** A trait-based G-code writer where each firmware dialect implements formatting differences. Common structure is shared; only dialect-specific commands differ.
**When to use:** All G-code output.
**Example:**
```rust
pub enum GcodeDialect {
    Marlin,
    Klipper,
    RepRapFirmware,
    Bambu,
}

pub struct GcodeWriter<W: std::io::Write> {
    writer: W,
    dialect: GcodeDialect,
    current_position: Point3,
    current_feedrate: f64,
    current_extrusion: f64,
    line_number: u32,
    use_relative_extrusion: bool,
}

impl<W: std::io::Write> GcodeWriter<W> {
    /// Emit a linear move (G0 or G1) with appropriate parameters
    pub fn move_to(&mut self, x: f64, y: f64, z: f64, f: f64) -> Result<(), GcodeError> {
        // G0/G1 format is universal across all dialects
        write!(self.writer, "G1 X{:.3} Y{:.3} Z{:.3} F{:.0}\n", x, y, z, f * 60.0)?;
        self.current_position = Point3::new(x, y, z);
        self.current_feedrate = f;
        Ok(())
    }

    /// Set extruder temperature (dialect-specific behavior)
    pub fn set_temperature(&mut self, temp: f64, wait: bool) -> Result<(), GcodeError> {
        let cmd = if wait { "M109" } else { "M104" };
        write!(self.writer, "{} S{:.0}\n", cmd, temp)?;
        Ok(())
    }

    /// Emit dialect-specific start sequence
    pub fn write_start_gcode(&mut self, config: &StartConfig) -> Result<(), GcodeError> {
        match self.dialect {
            GcodeDialect::Marlin => self.write_marlin_start(config),
            GcodeDialect::Klipper => self.write_klipper_start(config),
            GcodeDialect::RepRapFirmware => self.write_reprap_start(config),
            GcodeDialect::Bambu => self.write_bambu_start(config),
        }
    }
}
```

### Anti-Patterns to Avoid
- **Parsing STL into intermediate types then converting to TriangleMesh:** Parse directly into `Vec<Point3>` and `Vec<[u32; 3]>` to avoid double allocation. Binary STL doesn't even have an index buffer (each triangle is standalone), so de-duplication must happen during parsing.
- **Using std::fs in core parsing functions:** Parse from `&[u8]` or `impl Read`, not from file paths. This enables WASM compatibility (where there is no filesystem) and makes testing trivial (pass byte slices directly).
- **Attempting full self-intersection repair in Phase 2:** Self-intersection resolution is a research-grade problem. PrusaSlicer uses CGAL for this. Detect and report, but don't attempt topological repair yet.
- **Hardcoding G-code as strings in one giant function:** Use structured types (`GcodeCommand` enum) that are formatted to strings. This makes testing possible (assert on structured commands, not string matching).
- **Modifying TriangleMesh in place for repair when OnceLock BVH exists:** The current TriangleMesh has an immutable API with lazy BVH. Repair needs mutable access. Solution: repair operates on raw vecs and constructs a new TriangleMesh, or repair takes ownership and returns a new mesh.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 3MF ZIP+XML parsing | Custom ZIP parser + XML parser | lib3mf v0.1.3 | 3MF is a ZIP container with OPC conventions, XML namespaces, and optional encryption. The specification is 1000+ pages. lib3mf handles all of this. |
| ZIP decompression | Custom deflate implementation | lib3mf (uses flate2/zip internally) | DEFLATE is a complex algorithm with many edge cases. lib3mf wraps this for 3MF files. |
| Vertex de-duplication hash | Naive O(n^2) comparison | HashMap with quantized vertex key | Binary STL files list every triangle independently with no shared vertices. De-duplicating vertices into an indexed mesh requires hashing. Use a HashMap with integer-quantized keys for exact matching. |

**Key insight:** STL (binary and ASCII) and OBJ are simple enough to hand-roll and gain full control. 3MF is complex enough to warrant a library. G-code writing is simple text emission but the dialect differences are the real complexity, and no library handles this -- hand-roll it.

## Common Pitfalls

### Pitfall 1: Binary STL Files That Start with "solid"
**What goes wrong:** Binary STL files sometimes have the word "solid" in their 80-byte header (because the header is free-form text and some tools write descriptive headers). A naive format detector checks for "solid" and misclassifies binary STL as ASCII STL, then the ASCII parser fails or produces garbage.
**Why it happens:** The STL format specification is ambiguous: ASCII STL starts with "solid name" but binary STL headers are arbitrary bytes.
**How to avoid:** Use a two-phase heuristic: (1) Check for "solid" AND "facet normal" in the first ~1000 bytes (ASCII STL always has "facet normal" early). (2) If only "solid" is found, check the binary size formula: `file_size == 84 + num_triangles * 50`. If both conditions fail, report an error.
**Warning signs:** STL files from Thingiverse/Printables that load as empty meshes or produce parser errors despite being valid in other slicers.

### Pitfall 2: STL Vertex Deduplication Precision
**What goes wrong:** Binary STL stores each triangle with its own 3 vertices (no shared vertex buffer). Two triangles sharing an edge have vertices that should be identical but differ by floating-point epsilon due to the original modeling tool. Naive exact-match deduplication misses these, producing a non-manifold mesh with double vertices at shared edges.
**Why it happens:** STL stores f32 values. Different triangles store the "same" vertex with slightly different float representations.
**How to avoid:** Quantize vertices to a grid before hashing. With our COORD_SCALE of 1e6, converting to integer coordinates (mm_to_coord on each component) and using the integer tuple as the hash key gives exact matching at nanometer precision. Alternatively, use a tolerance-based spatial hash (grid cells of ~1e-5 mm).
**Warning signs:** Meshes reporting "not watertight" or "non-manifold" despite looking correct in a viewer; extremely high vertex counts (3 * triangle_count instead of expected ~triangle_count/2).

### Pitfall 3: Mesh Repair Ordering Dependencies
**What goes wrong:** Running hole-filling before degenerate triangle removal can fill holes with more degenerate triangles. Running normal direction fix before edge stitching may flip normals incorrectly because adjacency information is incomplete.
**Why it happens:** Each repair step depends on the output of previous steps. The admesh pipeline was designed through trial and error over many years.
**How to avoid:** Follow the admesh order: (1) remove degenerates, (2) stitch edges, (3) fill holes, (4) fix normals, (5) recompute normal values. Test with a suite of known-broken meshes and verify repair produces manifold results.
**Warning signs:** Repair making meshes worse (volume becomes negative, watertight status regresses).

### Pitfall 4: G-code Dialect Differences in Extrusion Mode
**What goes wrong:** Marlin defaults to absolute extrusion (E parameter is cumulative distance). Klipper and RepRapFirmware often use relative extrusion (E parameter is per-move distance). If the wrong mode is used, the first layer works but subsequent layers extrude increasingly wrong amounts.
**Why it happens:** Different firmware interpret the E axis differently by default. The G-code must set the mode explicitly at the start (M82 for absolute, M83 for relative).
**How to avoid:** Always emit M82/M83 in the start G-code to set extrusion mode explicitly. Use relative extrusion (M83) as the default -- it's simpler (no cumulative tracking), works with all firmware, and avoids the "E-axis overflow" problem on long prints.
**Warning signs:** First layer prints correctly but subsequent layers have too much or too little filament.

### Pitfall 5: lib3mf WASM Compatibility Unknown
**What goes wrong:** lib3mf depends on `zip` and `flate2` crates for decompressing 3MF archives. These may have WASM compatibility issues depending on their backend (miniz_oxide is pure Rust and WASM-safe; zlib-ng is C and not WASM-safe).
**Why it happens:** Transitive dependencies may pull in C libraries for compression.
**How to avoid:** After adding lib3mf, immediately test `cargo build --target wasm32-unknown-unknown`. If it fails, check the dependency tree with `cargo tree` and identify the non-WASM-compatible dep. Potential fix: configure flate2 to use miniz_oxide backend (usually the default, but verify). If lib3mf is not WASM-compatible, it can be feature-gated behind `#[cfg(not(target_arch = "wasm32"))]` and 3MF loading can be unavailable in WASM initially.
**Warning signs:** Linker errors mentioning `__wasm_import_*` or `undefined symbol: inflate` when building for wasm32-unknown-unknown.

### Pitfall 6: OBJ Files with Non-Triangle Faces
**What goes wrong:** OBJ files can contain quads, pentagons, and arbitrary n-gons. A parser that only handles triangles will skip or crash on non-triangle faces.
**Why it happens:** OBJ format is not restricted to triangles. Many CAD tools export quads or mixed face types.
**How to avoid:** Triangulate non-triangle faces during parsing. For quads, split into 2 triangles. For n-gons (n > 4), use ear-clipping triangulation or fan triangulation (simple but only works for convex polygons). tobj handles triangulation automatically if configured with `triangulate: true` in `LoadOptions`.
**Warning signs:** Missing faces in loaded models; mesh has fewer triangles than expected.

## Code Examples

Verified patterns from format specifications and official sources:

### Binary STL Parser
```rust
// Source: STL format specification (80-byte header + 4-byte count + 50-byte facets)
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};
use std::collections::HashMap;

pub fn parse_binary_stl(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    if data.len() < 84 {
        return Err(FileIOError::FileTooSmall);
    }

    let mut cursor = Cursor::new(data);

    // Skip 80-byte header
    cursor.set_position(80);

    let num_triangles = cursor.read_u32::<LittleEndian>()? as usize;

    // Validate file size
    let expected = 84 + num_triangles * 50;
    if data.len() < expected {
        return Err(FileIOError::UnexpectedEof);
    }

    // Parse triangles and deduplicate vertices
    let mut vertices: Vec<Point3> = Vec::new();
    let mut indices: Vec<[u32; 3]> = Vec::with_capacity(num_triangles);
    let mut vertex_map: HashMap<[i64; 3], u32> = HashMap::new();

    for _ in 0..num_triangles {
        // Skip normal (12 bytes) -- we recompute from vertices
        cursor.set_position(cursor.position() + 12);

        let mut tri_indices = [0u32; 3];
        for j in 0..3 {
            let x = cursor.read_f32::<LittleEndian>()? as f64;
            let y = cursor.read_f32::<LittleEndian>()? as f64;
            let z = cursor.read_f32::<LittleEndian>()? as f64;

            // Quantize to integer key for deduplication
            let key = [
                (x * 1e5).round() as i64,
                (y * 1e5).round() as i64,
                (z * 1e5).round() as i64,
            ];

            let idx = match vertex_map.get(&key) {
                Some(&existing) => existing,
                None => {
                    let new_idx = vertices.len() as u32;
                    vertices.push(Point3::new(x, y, z));
                    vertex_map.insert(key, new_idx);
                    new_idx
                }
            };
            tri_indices[j] = idx;
        }

        // Skip 2-byte attribute byte count
        cursor.set_position(cursor.position() + 2);

        indices.push(tri_indices);
    }

    TriangleMesh::new(vertices, indices).map_err(FileIOError::from)
}
```

### ASCII STL Parser
```rust
// Source: STL format specification (text-based facet/normal/vertex grammar)
pub fn parse_ascii_stl(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let text = std::str::from_utf8(data)
        .map_err(|_| FileIOError::InvalidUtf8)?;

    let mut vertices: Vec<Point3> = Vec::new();
    let mut indices: Vec<[u32; 3]> = Vec::new();
    let mut vertex_map: HashMap<[i64; 3], u32> = HashMap::new();

    let mut in_facet = false;
    let mut tri_verts = Vec::with_capacity(3);

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("vertex") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                let x: f64 = parts[1].parse().map_err(|_| FileIOError::ParseError)?;
                let y: f64 = parts[2].parse().map_err(|_| FileIOError::ParseError)?;
                let z: f64 = parts[3].parse().map_err(|_| FileIOError::ParseError)?;

                let key = [
                    (x * 1e5).round() as i64,
                    (y * 1e5).round() as i64,
                    (z * 1e5).round() as i64,
                ];

                let idx = *vertex_map.entry(key).or_insert_with(|| {
                    let new_idx = vertices.len() as u32;
                    vertices.push(Point3::new(x, y, z));
                    new_idx
                });
                tri_verts.push(idx);
            }
        } else if trimmed.starts_with("endfacet") {
            if tri_verts.len() == 3 {
                indices.push([tri_verts[0], tri_verts[1], tri_verts[2]]);
            }
            tri_verts.clear();
        }
    }

    TriangleMesh::new(vertices, indices).map_err(FileIOError::from)
}
```

### 3MF Loading via lib3mf
```rust
// Source: lib3mf v0.1.3 documentation (docs.rs/lib3mf)
use lib3mf::Model;

pub fn parse_3mf(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let cursor = std::io::Cursor::new(data);
    let model = Model::from_reader(cursor)
        .map_err(|e| FileIOError::ThreeMfError(e.to_string()))?;

    let mut all_vertices: Vec<Point3> = Vec::new();
    let mut all_indices: Vec<[u32; 3]> = Vec::new();

    for object in &model.resources.objects {
        if let Some(mesh) = &object.mesh {
            let vertex_offset = all_vertices.len() as u32;

            for vertex in &mesh.vertices {
                all_vertices.push(Point3::new(
                    vertex.x as f64,
                    vertex.y as f64,
                    vertex.z as f64,
                ));
            }

            for triangle in &mesh.triangles {
                all_indices.push([
                    triangle.v1 as u32 + vertex_offset,
                    triangle.v2 as u32 + vertex_offset,
                    triangle.v3 as u32 + vertex_offset,
                ]);
            }
        }
    }

    if all_vertices.is_empty() {
        return Err(FileIOError::EmptyModel);
    }

    TriangleMesh::new(all_vertices, all_indices).map_err(FileIOError::from)
}
```

### G-code Marlin Dialect Start Sequence
```rust
// Source: Marlin firmware documentation + PrusaSlicer start G-code defaults
fn write_marlin_start(&mut self, config: &StartConfig) -> Result<(), GcodeError> {
    writeln!(self.writer, "; Generated by slicecore-rs")?;
    writeln!(self.writer, "M82 ; absolute extrusion mode")?;
    writeln!(self.writer, "M140 S{:.0} ; set bed temp", config.bed_temp)?;
    writeln!(self.writer, "M104 S{:.0} ; set extruder temp", config.nozzle_temp)?;
    writeln!(self.writer, "M190 S{:.0} ; wait for bed temp", config.bed_temp)?;
    writeln!(self.writer, "M109 S{:.0} ; wait for extruder temp", config.nozzle_temp)?;
    writeln!(self.writer, "G28 ; home all axes")?;
    writeln!(self.writer, "G92 E0 ; reset extruder")?;
    Ok(())
}
```

### Degenerate Triangle Removal
```rust
// Source: admesh algorithm + mesh repair literature
/// Remove triangles with zero area (duplicate vertices or collinear vertices).
/// Returns the number of triangles removed.
pub fn remove_degenerate_triangles(
    vertices: &[Point3],
    indices: &mut Vec<[u32; 3]>,
) -> usize {
    let original_count = indices.len();

    indices.retain(|tri| {
        // Check for duplicate vertex indices
        if tri[0] == tri[1] || tri[1] == tri[2] || tri[0] == tri[2] {
            return false;
        }

        // Check for zero-area triangle (collinear vertices)
        let v0 = vertices[tri[0] as usize];
        let v1 = vertices[tri[1] as usize];
        let v2 = vertices[tri[2] as usize];

        let edge1 = Vec3::from_points(v0, v1);
        let edge2 = Vec3::from_points(v0, v2);
        let cross = edge1.cross(edge2);

        // Area = 0.5 * |cross product|
        cross.length_squared() > 1e-20
    });

    original_count - indices.len()
}
```

### Normal Direction Fix (Consistent Winding)
```rust
// Source: admesh stl_fix_normal_directions algorithm
/// Fix normal directions so all faces have consistent outward-facing normals.
/// Uses a flood-fill approach: pick a seed triangle, propagate winding
/// direction to neighbors via shared edges.
pub fn fix_normal_directions(
    vertices: &[Point3],
    indices: &mut Vec<[u32; 3]>,
) -> usize {
    let n = indices.len();
    if n == 0 {
        return 0;
    }

    // Build edge-to-face adjacency map
    let mut edge_map: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, tri) in indices.iter().enumerate() {
        for j in 0..3 {
            let a = tri[j];
            let b = tri[(j + 1) % 3];
            let key = if a < b { (a, b) } else { (b, a) };
            edge_map.entry(key).or_default().push(i);
        }
    }

    // BFS flood-fill from triangle 0
    let mut visited = vec![false; n];
    let mut flipped = 0usize;
    let mut queue = std::collections::VecDeque::new();

    visited[0] = true;
    queue.push_back(0);

    while let Some(current) = queue.pop_front() {
        let tri = indices[current];
        for j in 0..3 {
            let a = tri[j];
            let b = tri[(j + 1) % 3];
            let key = if a < b { (a, b) } else { (b, a) };

            if let Some(neighbors) = edge_map.get(&key) {
                for &neighbor in neighbors {
                    if visited[neighbor] {
                        continue;
                    }
                    visited[neighbor] = true;

                    // Check if neighbor has consistent winding
                    // If current has edge (a,b), neighbor should have (b,a)
                    let ntri = indices[neighbor];
                    let same_direction = has_same_edge_direction(ntri, a, b);

                    if same_direction {
                        // Flip the neighbor triangle
                        indices[neighbor] = [ntri[0], ntri[2], ntri[1]];
                        flipped += 1;
                    }

                    queue.push_back(neighbor);
                }
            }
        }
    }

    flipped
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| admesh C library for STL repair | Custom Rust repair pipeline | Phase 2 (new) | admesh is GPL-licensed C code. Rust reimplementation avoids license issues and FFI. The algorithms are well-documented in Anthony Martin's Masters Thesis and admesh source. |
| stl_io / nom_stl for STL parsing | Hand-rolled binary+ASCII parser | Phase 2 (new) | Avoids external dependency for a trivially simple format. Direct-to-TriangleMesh construction eliminates intermediate allocations. |
| C++ lib3mf (3MF Consortium) for 3MF | Rust lib3mf crate (pure Rust) | 2025-2026 | Pure Rust, no FFI, WASM-potential. Published by project author. Handles the complex ZIP+XML+OPC format that should NOT be hand-rolled. |
| Printf-style G-code generation | Structured G-code types + dialect formatting | Phase 2 (new) | Enables testing at the command level, not just string output. Dialect abstraction makes adding new firmware trivial. |
| PrusaSlicer uses CGAL for self-intersection repair | Detection only (Phase 2), full repair deferred | Phase 2 (new) | CGAL is C++ with GPL license. Self-intersection REPAIR is a research problem. DETECTION with BVH-accelerated triangle-triangle tests is practical. Full repair can use a pure Rust approach later. |

**Deprecated/outdated:**
- **admesh for Rust projects:** admesh is ANSI C, GPL-licensed. Its ALGORITHMS are valuable reference material, but its code cannot be directly used (GPL license incompatible with MIT/Apache-2.0, and FFI violates project constraint). Reimplement the algorithm, not the code.
- **Clipper1 for STL repair:** Clipper2 (via clipper2-rust) is the current standard. Not directly relevant to mesh repair but important context.

## Open Questions

1. **lib3mf WASM Compatibility**
   - What we know: lib3mf v0.1.3 is pure Rust with no unsafe code. It depends on `zip` and `flate2` crates.
   - What's unclear: Whether `zip`/`flate2` compile to wasm32-unknown-unknown out of the box. The `flate2` crate with `miniz_oxide` backend (default) should be pure Rust and WASM-safe, but this needs verification.
   - Recommendation: Add lib3mf dependency early and test WASM build immediately. If it fails, feature-gate 3MF loading behind `#[cfg(not(target_arch = "wasm32"))]` and file a tracking issue.

2. **OBJ Parser: tobj vs Hand-Roll**
   - What we know: tobj v4.0.3 is mature (66 reverse deps), supports triangulation, has async/WASM-aware loading. Hand-rolling a minimal OBJ parser is ~200 lines (vertices + faces only).
   - What's unclear: Whether tobj's file I/O dependencies cause WASM issues. tobj's `load_obj_buf()` accepts a `BufRead` which should be WASM-safe.
   - Recommendation: Start with tobj and test WASM compilation. If dependencies cause issues, fall back to hand-rolled parser (the OBJ subset we need -- vertex positions and triangle faces -- is trivial).

3. **Mesh Repair: Mutable TriangleMesh API**
   - What we know: The current TriangleMesh is essentially immutable after construction (OnceLock BVH, private fields). Repair needs to modify vertices and indices.
   - What's unclear: Whether to add a mutable repair API to TriangleMesh or have repair functions take raw vecs and construct a new TriangleMesh.
   - Recommendation: Repair functions should operate on `(Vec<Point3>, Vec<[u32; 3]>)` and return a new `TriangleMesh`. This preserves the immutable-after-construction pattern and avoids complications with the OnceLock BVH. The caller flow is: parse file -> get raw vecs -> repair raw vecs -> construct TriangleMesh.

4. **Self-Intersection Detection Performance**
   - What we know: Naive all-pairs triangle intersection is O(n^2). BVH-accelerated detection is O(n log n) for well-distributed meshes. Phase 1's BVH supports bounding-box overlap queries.
   - What's unclear: How many real-world Thingiverse models have self-intersections, and whether detection is sufficient or repair is needed for Phase 2.
   - Recommendation: Implement BVH-accelerated detection using triangle-AABB overlap queries to find candidate pairs, then exact triangle-triangle intersection test (Moller 1997 algorithm). Report count in RepairReport. Do NOT attempt repair in Phase 2.

5. **G-code Validation for Phase 3 (Success Criterion 4)**
   - What we know: Phase 2 success criterion 4 requires "G-code writer can emit valid Marlin-dialect output, tested with G-code syntax validation."
   - What's unclear: What constitutes "G-code syntax validation" -- is it just format correctness (G1 X1.000 Y2.000 F3000) or also semantic correctness (valid temperature ranges, realistic feedrates)?
   - Recommendation: Implement a G-code validator that checks: (1) Every line is a valid G/M command or comment. (2) All coordinates are finite numbers. (3) Feedrate is positive. (4) Temperature is in valid range (0-400C). This is format+basic-semantic validation, not full print simulation.

6. **Test Data: Real-World Models from Thingiverse/Printables**
   - What we know: Success criterion 1 requires testing against "10+ real models from Thingiverse/Printables." These files need to be downloadable and redistributable for CI testing.
   - What's unclear: Licensing of Thingiverse/Printables models for inclusion in a test suite.
   - Recommendation: Create a `tests/fixtures/` directory with hand-crafted test models (unit cube, sphere, complex shape with known properties) for CI. Use real Thingiverse models for manual/local testing documented in a test plan, but do not commit third-party models to the repo. The CI test suite should verify the same code paths using synthetic models with known defects (non-manifold, degenerate, self-intersecting).

## Sources

### Primary (HIGH confidence)
- [STL Format Specification (Wikipedia)](https://en.wikipedia.org/wiki/STL_(file_format)) - Binary/ASCII format structure, 80-byte header, 50-byte facets
- [lib3mf docs.rs](https://docs.rs/lib3mf/latest/lib3mf/) - v0.1.3 API, pure Rust, Model::from_reader, mesh vertex/triangle access
- [tobj docs.rs](https://docs.rs/tobj/latest/tobj/) - v4.0.3 API, load_obj/load_obj_buf, async WASM support
- [stl_io docs.rs](https://docs.rs/stl_io/latest/stl_io/) - v0.10.0, binary+ASCII read, binary write, API structure
- [gcode crate docs.rs](https://docs.rs/gcode/latest/gcode/) - v0.6.1, no_std parser (not writer), confirmed parse-only
- [admesh GitHub](https://github.com/admesh/admesh) - Repair pipeline: degenerate removal, edge stitching, hole filling, normal fix
- [ADMesh docs](https://admesh.readthedocs.io/en/latest/cli.html) - Default repair operations and ordering
- [Klipper G-Codes](https://www.klipper3d.org/G-Codes.html) - Supported commands, M204 behavior, extended commands
- [RepRap G-code wiki](https://reprap.org/wiki/G-code) - Standard commands, Marlin vs RRF differences
- [PrusaSlicer TriangleMesh.cpp](https://github.com/slic3r/Slic3r/blob/master/xs/src/libslic3r/TriangleMesh.cpp) - repair() method, admesh integration, stl_repair parameter order
- Phase 1 codebase (local) - TriangleMesh, Point3, Vec3, BBox3, BVH, ValidPolygon types and APIs

### Secondary (MEDIUM confidence)
- [Bambu Lab G-Code Forum](https://forum.bambulab.com/t/bambu-lab-x1-specific-g-code/666) - Bambu-specific commands (M620/M621), differences from Marlin
- [byteorder GitHub](https://github.com/BurntSushi/byteorder) - no_std support, WASM-compatible via core library
- [flate2 WASM issue](https://github.com/rust-lang/flate2-rs/issues/161) - miniz_oxide backend is pure Rust, WASM-safe
- [RWTH Aachen Mesh Repair Tutorial](https://www.graphics.rwth-aachen.de/media/papers/eg2012_tutorial_meshrepair_021.pdf) - Non-manifold "cut and restitch" approach, hole filling algorithms

### Tertiary (LOW confidence)
- [nom_stl docs.rs](https://docs.rs/nom_stl/latest/nom_stl/) - v0.2.2, nom 5.x dependency, alternative STL parser (not recommended)
- [gen_gcode GitHub](https://github.com/codytrey/gen_gcode) - Simple G-code generator, no dialect support (not suitable)
- Bambu specific G-code commands (M991, M992) - Could not find official documentation; community-sourced only

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All recommended libraries verified via docs.rs documentation, API confirmed, dependencies understood. STL format specification is well-documented. Hand-roll recommendation supported by format simplicity analysis.
- Architecture: HIGH - Patterns derived from Phase 1 codebase (immutable TriangleMesh, arena+index, OnceLock BVH), admesh repair pipeline (well-documented in source + thesis), design doc Layer 1 architecture.
- Pitfalls: HIGH - Binary STL "solid" misdetection is well-documented. Vertex deduplication issues verified by examining STL format (no shared vertices). Repair ordering from admesh source. G-code dialect differences from firmware documentation. WASM compatibility concerns from Phase 1 experience + flate2 issue tracker.

**Research date:** 2026-02-16
**Valid until:** 2026-03-16 (30 days -- lib3mf is new and may have updates; G-code dialect details may change with firmware updates)
