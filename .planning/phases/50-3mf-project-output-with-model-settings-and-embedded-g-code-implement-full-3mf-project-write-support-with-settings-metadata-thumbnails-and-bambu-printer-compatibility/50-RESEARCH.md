# Phase 50: 3MF Project Output with Model Settings and Embedded G-code - Research

**Researched:** 2026-03-26
**Domain:** 3MF project file format, Bambu/OrcaSlicer compatibility, ZIP archive construction
**Confidence:** HIGH

## Summary

This phase extends the existing 3MF mesh export (`export_plate_to_3mf()`) into a full project write pipeline that embeds G-code, print settings, thumbnails, plate metadata, and print statistics -- producing files directly printable on Bambu Lab printers and loadable in OrcaSlicer/Bambu Studio.

The codebase already has all the foundational pieces: `lib3mf_core::Model::attachments` for embedding arbitrary files in the 3MF ZIP archive, `render_mesh()` for thumbnail generation, `GcodeWriter` for G-code output, `build_model_settings_config()` for XML config generation, and `map_to_slicer_field()` for SliceCore-to-slicer key translation. The work is primarily composition and integration -- wiring these existing capabilities into a unified project output function and extending the CLI to auto-detect `.3mf` output extension.

The Bambu/OrcaSlicer 3MF project format is well-understood from prior phase research and from the existing import/export code. The archive structure uses `Metadata/plate_N.gcode` for embedded G-code, `Metadata/plate_N.png` for thumbnails, `Metadata/plate_N.json` for plate statistics, and `Metadata/*.config` files for serialized settings (process, filament, machine profiles in XML key=value format).

**Primary recommendation:** Extend `export_plate_to_3mf()` with a new `ProjectExportOptions` struct that bundles G-code bytes, settings configs, thumbnails, plate metadata, and statistics -- all inserted via `model.attachments.insert()` before calling `model.write()`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- G-code embedded at `Metadata/plate_1.gcode` (Bambu convention), one per plate
- G-code inside 3MF is complete -- identical to standalone output
- Auto-detect from `.3mf` extension on `-o` flag -- no `--project` flag needed
- Always dual output: .3mf project + standalone .gcode alongside it
- Dual settings format: Bambu/OrcaSlicer-compatible XML configs + SliceCore native TOML
- XML config files: `Metadata/process_settings.config`, `Metadata/filament_settings.config`, `Metadata/machine_settings.config`
- SliceCore native config: `Metadata/slicecore_config.toml`
- Per-plate thumbnails at `Metadata/plate_N.png`
- Per-plate layout JSON at `Metadata/plate_N.json` with object positions, plate bounds, arrangement info
- Embedded print statistics (filament usage, time, layers) in plate JSON
- Project metadata header in `Metadata/project_settings.config`
- Bambu compatibility: direct-to-printer for X1C, X1E, P1S, P1P, A1, A1 Mini
- OrcaSlicer compatibility
- AMS filament mapping included in project metadata
- CLI: `slicecore slice model.stl -o output.3mf` produces full project
- CLI: `slicecore plate ... --format 3mf` produces full project too
- Default output (no -o) remains .gcode -- backward compatible
- Project file goes in job dir when `--job-dir` is used

### Claude's Discretion
- Project validation command (`slicecore project validate`) -- whether to include now or defer
- Exact Bambu plate JSON schema fields and structure
- AMS slot mapping algorithm and metadata format
- How to handle printers without AMS (A1 Mini, older models)
- XML config key mappings beyond existing `map_to_slicer_field()`
- Compression settings for G-code inside the ZIP archive
- How to generate per-plate thumbnails (render each plate's objects separately vs crop)
- Error handling when project components fail (e.g., thumbnail fails -- still write project?)

### Deferred Ideas (OUT OF SCOPE)
- 3MF project import (round-trip reading)
- PrusaSlicer project compatibility
- Send-to-printer integration
- Project diff command
- Project template mode
- Multi-color paint data
- Streaming project write
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MESH-03 | **NOTE: Requirement mismatch** -- MESH-03 in REQUIREMENTS.md is "Import OBJ files" mapped to Phase 2. This phase (50) addresses 3MF project *output*, which does not have a dedicated requirement ID. The planner should treat the phase goal (3MF project write support) as the governing requirement, not MESH-03. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| lib3mf-core | (workspace) | 3MF model construction, ZIP archive writing via `Model::write()` | Already used for all 3MF I/O in this project |
| toml | (workspace) | Serialize PrintConfig to TOML for `slicecore_config.toml` | Already used throughout for config serialization |
| serde_json | (workspace) | Serialize plate metadata JSON | Already used in CLI and job_dir |
| sha2 | (workspace) | SHA-256 checksums for model provenance | Already used in job_dir manifest |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| slicecore-render | (workspace) | Per-plate thumbnail generation | Render each plate's objects separately |
| slicecore-gcode-io | (workspace) | G-code generation, Bambu dialect | Capture G-code bytes for embedding |
| chrono | (workspace) | Timestamps for project metadata | Already a dependency in slicecore-cli |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| lib3mf-core attachments | zip crate directly | lib3mf-core already manages the ZIP; no reason to bypass it |
| XML string building | quick-xml crate | Existing pattern uses string formatting; configs are simple key=value XML, not complex nested structures |

**Installation:** No new dependencies needed. All required crates are already in the workspace.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-fileio/src/
├── export.rs              # Extended: ProjectExportOptions, export_project_to_3mf()
├── project_config.rs      # NEW: XML config builders for process/filament/machine settings
├── plate_metadata.rs      # NEW: PlateMetadata struct, JSON serialization
├── threemf.rs             # Existing: import-side (unchanged)
└── lib.rs                 # Re-export new public types

crates/slicecore-cli/src/
├── main.rs                # Extended: auto-detect .3mf in cmd_slice output path
├── plate_cmd.rs           # Extended: --format 3mf produces full project
└── job_dir.rs             # Extended: project_path() helper for job dir integration
```

### Pattern 1: ProjectExportOptions Builder
**What:** A struct that collects all project components before writing
**When to use:** When constructing a 3MF project with optional components
**Example:**
```rust
/// Options for writing a full 3MF project file.
pub struct ProjectExportOptions {
    /// G-code bytes per plate (complete, identical to standalone output).
    pub gcode_per_plate: Vec<Vec<u8>>,
    /// PNG thumbnail data per plate.
    pub thumbnails_per_plate: Vec<Option<Vec<u8>>>,
    /// Plate metadata (positions, stats, filament usage).
    pub plate_metadata: Vec<PlateMetadata>,
    /// Full PrintConfig snapshot for slicecore_config.toml.
    pub config_toml: String,
    /// Process settings XML (Bambu/Orca compatible).
    pub process_settings_xml: String,
    /// Filament settings XML.
    pub filament_settings_xml: String,
    /// Machine settings XML.
    pub machine_settings_xml: String,
    /// Project-level metadata (version, timestamp, provenance).
    pub project_metadata: ProjectMetadata,
    /// AMS filament slot mapping (slot index -> filament info).
    pub ams_mapping: Option<AmsMapping>,
}
```

### Pattern 2: Attachment-Based Embedding
**What:** All project files are added via `model.attachments.insert(path, bytes)` before `model.write()`
**When to use:** Always -- this is the established pattern from thumbnail embedding
**Example:**
```rust
// Existing pattern already used for thumbnails and model_settings.config:
model.attachments.insert("Metadata/plate_1.gcode".to_string(), gcode_bytes);
model.attachments.insert("Metadata/plate_1.png".to_string(), thumbnail_png);
model.attachments.insert("Metadata/plate_1.json".to_string(), plate_json_bytes);
model.attachments.insert("Metadata/process_settings.config".to_string(), xml_bytes);
model.attachments.insert("Metadata/slicecore_config.toml".to_string(), toml_bytes);
```

### Pattern 3: XML Config Generation (Bambu/Orca Compatible)
**What:** Key=value XML format matching OrcaSlicer/Bambu Studio convention
**When to use:** For process_settings.config, filament_settings.config, machine_settings.config
**Example:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<config>
  <plate>
    <metadata key="layer_height" value="0.2"/>
    <metadata key="fill_density" value="15%"/>
    <metadata key="perimeters" value="3"/>
    <metadata key="support_material" value="0"/>
    <!-- ... more key=value pairs ... -->
  </plate>
</config>
```

### Pattern 4: Auto-Detect Output Format in CLI
**What:** Check output path extension to determine whether to produce a 3MF project
**When to use:** In `cmd_slice()` output path handling
**Example:**
```rust
let output_path = output.unwrap_or_else(|| input.with_extension("gcode"));
let is_project_output = output_path.extension()
    .and_then(|e| e.to_str())
    .is_some_and(|e| e.eq_ignore_ascii_case("3mf"));

if is_project_output {
    // Write both: project .3mf AND standalone .gcode alongside
    let gcode_path = output_path.with_extension("gcode");
    write_gcode(&gcode_path, &gcode_bytes)?;
    write_project(&output_path, &meshes, &gcode_bytes, &config, ...)?;
} else {
    write_gcode(&output_path, &gcode_bytes)?;
}
```

### Anti-Patterns to Avoid
- **Separate ZIP construction:** Do NOT bypass `lib3mf_core::Model::write()` and construct the ZIP manually. The model already handles 3MF-compliant archive structure, `[Content_Types].xml`, relationships, etc.
- **Lazy config mapping:** Do NOT just dump raw TOML into XML configs. Bambu/Orca expect specific key names (`fill_density` not `infill_density`, `perimeters` not `wall_count`). Use the existing `map_to_slicer_field()` pattern.
- **G-code re-generation:** Do NOT re-slice to produce embedded G-code. Capture the same bytes written to the standalone file and embed those.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| ZIP archive creation | Custom ZIP writer | `lib3mf_core::Model::write()` | Handles OPC compliance, content types, relationships automatically |
| PNG encoding | Manual PNG writer | `slicecore-render` encode module | Already handles PNG/JPEG with the `image` crate |
| JSON serialization | Manual JSON string building | `serde_json::to_string_pretty()` | Plate metadata is a struct; derive Serialize |
| XML escaping | Regex-based escaping | Existing `xml_escape()` in export.rs | Already handles &, ", <, > |
| SHA-256 hashing | Custom hasher | `JobDir::file_checksum()` pattern using sha2 | Already used in job_dir |
| Timestamp formatting | Manual epoch math | chrono (already a dependency) | The `epoch_days_to_date` function in slice_workflow.rs is fragile; use chrono |

**Key insight:** This phase is 90% integration of existing components. The novel work is: (1) XML config builders for process/filament/machine settings, (2) plate metadata JSON struct, (3) AMS mapping struct, and (4) CLI output path detection. Everything else is wiring.

## Common Pitfalls

### Pitfall 1: Bambu Printer Rejects 3MF
**What goes wrong:** Printer shows error or re-slices instead of printing embedded G-code
**Why it happens:** Missing or incorrectly named metadata files; Bambu firmware expects specific file paths and G-code naming
**How to avoid:** Use exact paths: `Metadata/plate_1.gcode` (1-indexed), `Metadata/plate_1.png`. Include `Metadata/project_settings.config` with printer model identification. Include G-code MD5 checksums (`Metadata/plate_1.gcode.md5`).
**Warning signs:** Printer shows "preparing" instead of starting immediately; thumbnail doesn't appear on LCD

### Pitfall 2: G-code Bytes Mismatch
**What goes wrong:** Embedded G-code differs from standalone file due to encoding issues
**Why it happens:** Converting String to bytes with wrong encoding, or modifying G-code after capture
**How to avoid:** Capture G-code as `Vec<u8>` once, write same bytes to both standalone file and archive. Use `GcodeWriter::into_inner()` to get the bytes, then clone for embedding.
**Warning signs:** Print fails mid-way, thermal runaway from wrong temperatures

### Pitfall 3: XML Config Key Mismatches
**What goes wrong:** OrcaSlicer/Bambu Studio ignores settings or shows wrong values
**Why it happens:** Using SliceCore key names instead of slicer-compatible names in XML configs
**How to avoid:** Always run through `map_to_slicer_field()` for the Bambu/Orca XML. Write both namespaces: `slicecore:` prefix for native keys AND mapped slicer keys.
**Warning signs:** Settings panel shows defaults instead of project values when opening in Bambu Studio

### Pitfall 4: Large G-code Memory Pressure
**What goes wrong:** OOM when embedding G-code for large prints (100+ MB G-code files)
**Why it happens:** Holding entire G-code in memory as `Vec<u8>` plus the ZIP buffer
**How to avoid:** Write G-code to a temp file first, then read back for embedding. Or write standalone file first, then read for embedding. Streaming write is deferred to a future phase.
**Warning signs:** Process killed on low-memory systems for large complex prints

### Pitfall 5: Missing Content Types in 3MF
**What goes wrong:** Some tools fail to parse the 3MF because `.gcode` or `.json` files lack MIME type entries
**Why it happens:** `lib3mf_core` auto-handles standard 3MF content types but may not know about `.gcode` or `.json` extensions
**How to avoid:** Verify the generated `[Content_Types].xml` includes entries for `.gcode`, `.json`, `.toml`, and `.config` extensions. May need to add these via model metadata or post-process.
**Warning signs:** Other slicers show "invalid 3MF" or "corrupt archive" errors

### Pitfall 6: Thumbnail Resolution for Bambu LCD
**What goes wrong:** Thumbnail appears blurry or doesn't display on printer LCD
**Why it happens:** Wrong resolution or format for the target device
**How to avoid:** Bambu printers expect specific thumbnail sizes. Standard is 256x256 PNG for plate thumbnails. Also include a smaller `bbl_thumbnail.png` (not required but improves compatibility).
**Warning signs:** Printer LCD shows generic icon instead of model preview

## Code Examples

### Plate Metadata JSON Structure (Bambu/OrcaSlicer Compatible)
```rust
use serde::Serialize;

/// Per-plate metadata embedded in the 3MF project.
#[derive(Debug, Serialize)]
pub struct PlateMetadata {
    /// 1-indexed plate number.
    pub plate_index: u32,
    /// Object placements on this plate.
    pub objects: Vec<PlateObject>,
    /// Build plate dimensions [x, y] in mm.
    pub plate_size: [f64; 2],
    /// Print statistics for this plate.
    pub statistics: PlateStatistics,
    /// Filament mapping (index -> AMS slot or external spool).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filament_mapping: Option<Vec<FilamentSlot>>,
}

#[derive(Debug, Serialize)]
pub struct PlateObject {
    pub name: String,
    pub position: [f64; 3],
    pub bounding_box: [f64; 6],
    pub triangle_count: usize,
}

#[derive(Debug, Serialize)]
pub struct PlateStatistics {
    pub filament_length_mm: f64,
    pub filament_weight_g: f64,
    pub filament_cost: f64,
    pub estimated_time_seconds: f64,
    pub layer_count: usize,
}

#[derive(Debug, Serialize)]
pub struct FilamentSlot {
    /// AMS tray index (0-3 for AMS, or "external" for spool holder).
    pub slot: String,
    /// Filament type (e.g., "PLA", "PETG", "ABS").
    pub filament_type: String,
    /// Filament color hex (e.g., "#FFFFFF").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}
```

### XML Config Builder Pattern
```rust
/// Builds a Bambu/Orca-compatible settings config XML.
///
/// Each settings category (process, filament, machine) uses the same format:
/// `<config><plate><metadata key="..." value="..."/></plate></config>`
fn build_settings_config(settings: &[(String, String)]) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n");
    xml.push_str("  <plate>\n");
    for (key, value) in settings {
        xml.push_str(&format!(
            "    <metadata key=\"{}\" value=\"{}\"/>\n",
            xml_escape(key),
            xml_escape(value),
        ));
    }
    xml.push_str("  </plate>\n");
    xml.push_str("</config>\n");
    xml
}
```

### Project Export Integration Point
```rust
/// Exports a complete 3MF project file with all embedded metadata.
///
/// Extends the existing `export_plate_to_3mf()` by adding G-code,
/// settings configs, thumbnails, plate metadata, and provenance info.
pub fn export_project_to_3mf<W: Write + Seek>(
    meshes: &[&TriangleMesh],
    object_configs: &[ThreeMfObjectConfig],
    project_options: &ProjectExportOptions,
    writer: W,
) -> Result<(), FileIOError> {
    // 1. Build the base model with meshes and per-object settings
    //    (reuse existing export_plate_to_3mf logic)
    // 2. Insert G-code per plate
    // 3. Insert thumbnails per plate
    // 4. Insert plate metadata JSON per plate
    // 5. Insert settings configs (process, filament, machine)
    // 6. Insert SliceCore native TOML config
    // 7. Insert project metadata header
    // 8. Write model to archive
}
```

### AMS Mapping for Printers Without AMS
```rust
/// For printers without AMS (A1 Mini, older P1P models), the filament
/// mapping contains a single "external" slot.
fn default_ams_mapping_no_ams(filament_type: &str) -> Vec<FilamentSlot> {
    vec![FilamentSlot {
        slot: "external".to_string(),
        filament_type: filament_type.to_string(),
        color: None,
    }]
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Mesh-only 3MF export | Full project 3MF with G-code + settings | Phase 50 | Enables direct-to-printer workflow |
| Manual G-code transfer | G-code embedded in 3MF archive | Phase 50 | Single file contains everything |
| Settings lost after slicing | Settings preserved in project file | Phase 50 | Reproducible prints |

**Deprecated/outdated:**
- None -- this is new functionality building on existing patterns

## Bambu 3MF Project File Structure (Reference)

Based on analysis of actual Bambu Studio / OrcaSlicer output files:

```
output.3mf (ZIP archive)
├── [Content_Types].xml
├── _rels/
│   └── .rels
├── 3D/
│   ├── 3dmodel.model              # Core 3MF XML with mesh geometry
│   └── _rels/
│       └── 3dmodel.model.rels
└── Metadata/
    ├── model_settings.config       # Per-object overrides (existing)
    ├── process_settings.config     # Print/process profile settings (NEW)
    ├── filament_settings.config    # Filament profile settings (NEW)
    ├── machine_settings.config     # Printer/machine profile settings (NEW)
    ├── project_settings.config     # Project-level metadata (NEW)
    ├── slicecore_config.toml       # SliceCore native full config (NEW)
    ├── plate_1.gcode               # Complete G-code for plate 1 (NEW)
    ├── plate_1.gcode.md5           # MD5 checksum of G-code (NEW)
    ├── plate_1.png                 # Plate 1 thumbnail (NEW)
    ├── plate_1.json                # Plate 1 metadata + stats (NEW)
    ├── plate_2.gcode               # (multi-plate only)
    ├── plate_2.png                 # (multi-plate only)
    ├── plate_2.json                # (multi-plate only)
    └── thumbnail.png               # Main project thumbnail (existing)
```

## Discretion Recommendations

### Project Validation Command: DEFER
Defer `slicecore project validate` to a future phase. Validation is not needed for the write path -- the focus here is producing correct output. A validation command adds scope without immediate user value.

### Compression Settings: DEFLATE default
Use the default ZIP compression (DEFLATE) from lib3mf-core for all files including G-code. G-code compresses well (60-80% reduction). No need for special handling.

### Per-Plate Thumbnail Strategy: Render Separately
Render each plate's objects separately using `render_mesh()` with a filtered mesh containing only that plate's objects. This is simpler and more correct than cropping from a full render.

### Error Handling Strategy: Graceful Degradation
If a non-critical component fails (e.g., thumbnail rendering), still write the project file without that component and emit a warning. Only fail the entire operation if mesh geometry or G-code embedding fails.

### Printers Without AMS
For printers without AMS (A1 Mini, some P1P models), include a single "external" filament slot in the mapping. The G-code already works without AMS commands; the mapping metadata simply reflects the actual filament source.

### G-code MD5 Checksums
Include `Metadata/plate_N.gcode.md5` files containing the MD5 hex digest of the corresponding G-code file. Bambu firmware uses these to verify file integrity.

## Open Questions

1. **Exact Bambu plate JSON schema**
   - What we know: Contains filament usage, print time, layer count, object positions
   - What's unclear: Exact field names and nesting expected by Bambu firmware
   - Recommendation: Use a reasonable schema based on OrcaSlicer conventions; test with actual printer

2. **Content-Type registration for non-standard extensions**
   - What we know: lib3mf-core handles standard 3MF content types
   - What's unclear: Whether `.gcode`, `.json`, `.toml`, `.md5` need explicit content-type registration
   - Recommendation: Test the generated archive; if tools reject it, add content-type overrides

3. **BambuStudio 3mfVersion metadata**
   - What we know: BambuStudio uses `BambuStudio:3mfVersion` metadata key for version compatibility
   - What's unclear: What version value to use for maximum compatibility
   - Recommendation: Set `BambuStudio:3mfVersion` to "1" (the current standard version)

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml per crate |
| Quick run command | `cargo test -p slicecore-fileio --lib` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| P50-01 | export_project_to_3mf writes valid archive | unit | `cargo test -p slicecore-fileio export_project -- --nocapture` | Wave 0 |
| P50-02 | G-code embedded at correct path | unit | `cargo test -p slicecore-fileio gcode_embedding -- --nocapture` | Wave 0 |
| P50-03 | Settings XML configs written correctly | unit | `cargo test -p slicecore-fileio settings_config -- --nocapture` | Wave 0 |
| P50-04 | Per-plate thumbnails embedded | unit | `cargo test -p slicecore-fileio plate_thumbnails -- --nocapture` | Wave 0 |
| P50-05 | Plate metadata JSON valid | unit | `cargo test -p slicecore-fileio plate_metadata_json -- --nocapture` | Wave 0 |
| P50-06 | CLI auto-detects .3mf extension | integration | `cargo test -p slicecore-cli project_output -- --nocapture` | Wave 0 |
| P50-07 | Dual output produces both .3mf and .gcode | integration | `cargo test -p slicecore-cli dual_output -- --nocapture` | Wave 0 |
| P50-08 | Archive can be re-read by lib3mf-core | unit | `cargo test -p slicecore-fileio project_roundtrip -- --nocapture` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-fileio --lib`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-fileio/src/project_config.rs` -- XML config builder tests
- [ ] `crates/slicecore-fileio/src/plate_metadata.rs` -- PlateMetadata serialization tests
- [ ] Tests in `export.rs` for `export_project_to_3mf()` function
- [ ] Integration tests in `slicecore-cli` for `.3mf` auto-detection

## Sources

### Primary (HIGH confidence)
- Existing codebase: `crates/slicecore-fileio/src/export.rs` -- current 3MF export API, attachment pattern
- Existing codebase: `crates/slicecore-fileio/src/threemf.rs` -- 3MF import with config extraction
- Existing codebase: `crates/slicecore-render/src/lib.rs` -- ThumbnailConfig, render_mesh() API
- Existing codebase: `crates/slicecore-cli/src/job_dir.rs` -- Manifest, PrintStats, checksum patterns
- Phase 26 research: `lib3mf_core::Model::attachments` is `HashMap<String, Vec<u8>>`

### Secondary (MEDIUM confidence)
- [DeepWiki: BambuStudio 3MF Project File Handling](https://deepwiki.com/bambulab/BambuStudio/2.3-3mf-project-file-handling) -- Archive structure, plate system, versioning
- [DeepWiki: OrcaSlicer 3MF Format](https://deepwiki.com/SoftFever/OrcaSlicer/7.1-3mf-format) -- Metadata directory structure, file naming conventions
- [Bambu Lab Forum: 3MF Internals](https://forum.bambulab.com/t/whats-inside-the-3mf-files-we-love-so-much/118218) -- User-verified archive contents
- [3MF File Format Notes](https://radagast.ca/linux/3mf-file-format.html) -- Archive exploration, Bambu-specific extensions

### Tertiary (LOW confidence)
- Exact Bambu plate JSON schema -- reconstructed from multiple sources, needs validation against actual printer
- AMS slot mapping format -- inferred from G-code comments in bambu.rs, not verified against firmware

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace, patterns established
- Architecture: HIGH -- direct extension of existing export.rs patterns
- Bambu compatibility: MEDIUM -- archive structure known, exact firmware expectations need testing
- Pitfalls: MEDIUM -- based on general ZIP/3MF knowledge and codebase analysis

**Research date:** 2026-03-26
**Valid until:** 2026-04-26 (stable domain, Bambu format rarely changes)
