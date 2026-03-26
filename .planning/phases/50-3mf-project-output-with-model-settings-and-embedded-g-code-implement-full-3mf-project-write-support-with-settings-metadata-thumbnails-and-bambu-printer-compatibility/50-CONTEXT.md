# Phase 50: 3MF Project Output with Model Settings and Embedded G-code - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Enable saving complete slice sessions as 3MF project files containing model geometry, print settings metadata (both Bambu-compatible XML and SliceCore TOML), per-plate thumbnails, embedded G-code, plate layout JSON, and AMS filament mapping. Targets direct-to-printer compatibility with all current Bambu Lab printers and OrcaSlicer. Includes auto-detection of project output from .3mf extension on the slice command, per-plate G-code organization, per-plate thumbnails, embedded print statistics, project metadata header, and dual output (project .3mf + standalone .gcode).

Does NOT include: 3MF project import/reading, PrusaSlicer project format, send-to-printer communication, project validation command (Claude's discretion).

</domain>

<decisions>
## Implementation Decisions

### G-code embedding
- Store G-code inside the 3MF archive at `Metadata/plate_1.gcode` (Bambu convention)
- Multi-plate: one G-code file per plate (`Metadata/plate_1.gcode`, `Metadata/plate_2.gcode`, etc.)
- Complete G-code with all headers, thumbnail comments, config summary — identical to standalone output
- G-code is embedded automatically when slice output has .3mf extension (auto-detect from extension, no --project flag needed)

### Settings metadata — dual format
- Write Bambu/OrcaSlicer-compatible XML config files:
  - `Metadata/process_settings.config` — print/process profile settings
  - `Metadata/filament_settings.config` — filament profile settings
  - `Metadata/machine_settings.config` — printer/machine profile settings
- Also write SliceCore native config: `Metadata/slicecore_config.toml` — full PrintConfig snapshot for round-tripping
- Per-object overrides remain in `Metadata/model_settings.config` (existing, separate concern)
- Full provenance metadata: printer model name, filament type/brand/color, nozzle diameter, profile names used

### Plate layout
- Write `Metadata/plate_N.json` per plate with object positions, plate bounds, arrangement info
- Enables Bambu/Orca tools to display object placement without re-arranging

### Per-plate thumbnails
- Generate separate thumbnail per plate in `Metadata/plate_N.png`
- Each thumbnail shows only that plate's objects (not the full model set)
- Matches Bambu Studio format — displays correctly on printer LCD preview

### Embedded print statistics
- Filament usage (length, weight, cost), estimated print time, layer count per plate
- Stored in each `plate_N.json` metadata
- Bambu printers display this on LCD before printing starts

### Project metadata header
- `Metadata/project_settings.config` or dedicated metadata section with:
  - SliceCore version, creation timestamp
  - Source file hashes (SHA256 of input models)
  - Reproduce command (full CLI invocation)
- Similar to Phase 46 manifest but inside the 3MF archive

### Bambu compatibility
- Target: direct-to-printer compatible — Bambu printers can accept and print without re-slicing
- All current Bambu models: X1C, X1E, P1S, P1P, A1, A1 Mini
- Also OrcaSlicer compatible (same 3MF project structure with minor extensions)
- AMS filament mapping included — map filaments to AMS slots in project metadata
- Correct plate JSON schema, G-code naming, printer model tags

### CLI integration
- Auto-detect from extension: `slicecore slice model.stl -o output.3mf` produces full project
- Default output (no -o flag) remains .gcode — backward compatible
- Always write both: .3mf project + standalone .gcode alongside it
- `slicecore plate ... --format 3mf` produces full project too (not mesh-only)
- Project file goes in job dir when --job-dir is used (Phase 46 integration)

### Claude's Discretion
- Project validation command — whether to include a `slicecore project validate` subcommand in this phase or defer
- Exact Bambu plate JSON schema fields and structure
- AMS slot mapping algorithm and metadata format
- How to handle printers without AMS (A1 Mini, older models)
- XML config key mappings beyond what's already in `map_to_slicer_field()`
- Compression settings for G-code inside the ZIP archive
- How to generate per-plate thumbnails (render each plate's objects separately vs crop from full render)
- Error handling when some project components fail (e.g., thumbnail rendering fails — still write project without thumbnail?)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing 3MF export (primary extension point)
- `crates/slicecore-fileio/src/export.rs` — Current export_plate_to_3mf(), save_mesh_with_thumbnail(), triangle_mesh_to_model(), build_model_settings_config(), map_to_slicer_field()
- `crates/slicecore-fileio/src/threemf.rs` — ThreeMfObjectConfig struct, parse_with_config() for import side, field mapping between slicecore and slicer keys

### Thumbnail rendering
- `crates/slicecore-render/src/lib.rs` — ThumbnailConfig, Thumbnail struct, render_mesh(), ImageFormat enum
- `crates/slicecore-render/src/gcode_embed.rs` — format_gcode_thumbnail_block() for G-code thumbnail embedding

### G-code generation
- `crates/slicecore-gcode-io/src/writer.rs` — GcodeWriter, G-code output pipeline
- `crates/slicecore-gcode-io/src/bambu.rs` — Bambu dialect start/end G-code, AMS comments
- `crates/slicecore-gcode-io/src/thumbnail.rs` — write_thumbnail_comments() for G-code thumbnail embedding

### CLI
- `crates/slicecore-cli/src/main.rs` — cmd_slice(), cmd_slice_plate(), output path handling, --thumbnails flag
- `crates/slicecore-cli/src/plate_cmd.rs` — Plate command with --format flag

### Prior phase context
- `.planning/phases/22-migrate-from-lib3mf-to-lib3mf-core-ecosystem/22-CONTEXT.md` — lib3mf-core ecosystem decisions
- `.planning/phases/24-mesh-export-stl-3mf-write/24-CONTEXT.md` — Mesh export API design, delegation to lib3mf-core
- `.planning/phases/26-thumbnail-preview-rasterization/26-CONTEXT.md` — Thumbnail rendering decisions, camera angles, resolutions, 3MF embedding
- `.planning/phases/39-jpeg-thumbnail-export-add-jpeg-encoding-option-to-render-crate-alongside-existing-png-with-cli-flag-quality-control-and-3mf-g-code-thumbnail-embedding-support/39-CONTEXT.md` — JPEG support, 3MF always PNG, image crate
- `.planning/phases/46-job-output-directories-for-isolated-slice-execution/46-CONTEXT.md` — Job dir structure, manifest format, --job-dir interaction

### Requirements
- `.planning/REQUIREMENTS.md` — [MESH-03] requirement mapped to this phase

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `export_plate_to_3mf()` (export.rs:335): Multi-mesh 3MF export with per-object settings — extend to add G-code, configs, plate JSON
- `triangle_mesh_to_model_with_thumbnail()` (export.rs:113): Thumbnail embedding via model.attachments — same pattern for G-code and config attachments
- `build_model_settings_config()` (export.rs:264): XML config generation for per-object overrides — pattern for process/filament/machine configs
- `map_to_slicer_field()` (export.rs:220): SliceCore-to-PrusaSlicer field mapping — needs expansion for full profile export
- `render_mesh()` (slicecore-render): Returns Vec<Thumbnail> — call per-plate for per-plate thumbnails
- `GcodeWriter` (slicecore-gcode-io): Existing G-code output — capture output bytes for embedding
- Bambu dialect (bambu.rs): AMS M620/M621 commands already present in start G-code

### Established Patterns
- `model.attachments.insert(path, data)` for adding files to 3MF archive (used for thumbnail and model_settings.config)
- Dual-namespace metadata: `slicecore:` prefix for native keys + PrusaSlicer-compatible keys
- XML config format matching OrcaSlicer/Bambu Studio conventions
- lib3mf-core `Model::write()` handles ZIP archive creation

### Integration Points
- `export_plate_to_3mf()` — Primary function to extend with G-code, settings, plate JSON, thumbnails
- `cmd_slice()` in CLI — Add .3mf auto-detection and dual output (project + standalone gcode)
- `cmd_slice_plate()` in CLI — Extend plate --format 3mf to produce full project
- `slicecore-render` — Per-plate thumbnail rendering (filter objects per plate)
- `slicecore-engine` — Print statistics (filament, time, layers) for plate JSON metadata

</code_context>

<specifics>
## Specific Ideas

- Auto-detect from extension is key: `slicecore slice model.stl -o output.3mf` should Just Work as a full project — no extra flags
- Always dual output: when producing .3mf, also write standalone .gcode alongside it
- G-code inside 3MF is complete — exact same bytes as the standalone file
- Per-plate thumbnails show only that plate's objects, matching Bambu LCD preview
- Bambu printers display print stats from plate JSON before printing — filament/time/layers must be accurate
- AMS filament mapping enables multi-material without manual slot selection on printer
- lib3mf-core's `model.attachments` is the mechanism for embedding arbitrary files in the 3MF archive

</specifics>

<deferred>
## Deferred Ideas

- **3MF project import (round-trip)** — Read Bambu/Orca project files back, extract G-code, settings, plate layout. Enables opening someone else's project and re-slicing. Future phase.
- **PrusaSlicer project compatibility** — PrusaSlicer has its own 3MF project format (different from Bambu). Future phase.
- **Send-to-printer integration** — Upload project directly to printer via LAN/MQTT. Out of scope per PROJECT.md (printer communication is separate).
- **Project diff command** — `slicecore project diff a.3mf b.3mf` to compare settings, geometry, G-code between two project files.
- **Project template mode** — Save project without G-code as reusable template, re-slice later with `slicecore slice --from-project template.3mf`.
- **Multi-color paint data** — Per-face/per-region color assignments in 3MF for Bambu/Orca multi-color workflow.
- **Streaming project write** — Incremental 3MF write during slicing for large prints without buffering entire G-code in memory.

</deferred>

---

*Phase: 50-3mf-project-output*
*Context gathered: 2026-03-26*
