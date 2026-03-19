# Phase 39: JPEG Thumbnail Export - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Add JPEG encoding option to the render crate alongside existing PNG. Includes CLI flag for format selection, quality control, and updated G-code/3MF thumbnail embedding support. Does NOT include WebP/AVIF support, auto-quality size targeting, or firmware-specific format recommendations.

</domain>

<decisions>
## Implementation Decisions

### Image Format Selection
- PNG remains the default format everywhere (CLI `thumbnail` command, `slice --thumbnails`, 3MF embedding)
- JPEG is opt-in via `--format jpeg` flag on both `thumbnail` and `slice` commands
- CLI flag is `--format jpeg/png` (not `--jpeg` shorthand) — matches existing `--format` patterns on schema/stats commands
- Format is CLI-only — no `PrintConfig` setting. Thumbnail format is an output concern, not a print profile setting
- Auto-detect format from output file extension: `thumbnail input.stl -o thumb.jpg` selects JPEG without `--format`

### Render Crate API Changes
- `ThumbnailConfig` gets an `output_format: ImageFormat` field (enum: `Png`, `Jpeg`) and `quality: Option<u8>` field
- `ImageFormat` enum is simple two-variant (`Png`, `Jpeg`) — not `#[non_exhaustive]`. Add more later if needed
- `Thumbnail` struct: rename `png_data` to `encoded_data`, add `format: ImageFormat` field
- One format per `render_mesh()` call — if both formats needed, call twice (RGBA rendering is the expensive part, encoding is cheap)
- Render pipeline stays RGBA throughout, RGB conversion only at JPEG encode time

### Dependency Changes
- Replace `png` crate with `image` crate (pure Rust, WASM-compatible)
- `image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }` — minimal features only
- Eliminate the `unsafe` block in `png_encode.rs` by using `image::RgbaImage::from_raw()` safe API
- `image` crate is always included (not feature-gated) — JPEG support is the point of this phase

### JPEG Quality Control
- Default quality: 85 (good balance for thumbnail resolution)
- CLI flag: `--quality 1-100` (numeric only, no named presets)
- `--quality` with PNG format: warn on stderr and ignore (non-fatal)
- Quality validation: error on out-of-range values (< 1 or > 100)
- Quality stored in `ThumbnailConfig` as `quality: Option<u8>` (None = format default)
- `--format` and `--quality` available on both `thumbnail` and `slice --thumbnails` commands
- Same resolution regardless of format — resolution is about detail level, not compression

### Transparency Handling
- JPEG doesn't support transparency — when format is JPEG, auto-set white [255,255,255] background
- Warn if user explicitly set transparent background with JPEG format
- RGBA pipeline internally, convert to RGB only at JPEG encode step

### G-code Embedding
- Same `; thumbnail begin WxH SIZE` / `; thumbnail end` framing for both PNG and JPEG data
- Base64-encode JPEG bytes the same way as PNG — firmware detects format from data, not tags
- Creality format keeps `; png begin` tag even for JPEG data (changing it would break firmware compatibility)
- `write_thumbnail_comments()` in gcode-io: add format awareness (rename `png_data` param, accept format info for future use)
- `thumbnail_format_for_dialect()` unchanged — returns comment style only, not image format recommendation

### 3MF Embedding
- 3MF always uses PNG regardless of `--format` flag (3MF spec requires PNG at `Metadata/thumbnail.png`)
- When `slice --thumbnails --format jpeg` targets 3MF output: warn on stderr "JPEG not supported for 3MF thumbnails, using PNG" and embed PNG
- `save_mesh_with_thumbnail()` in fileio: rename `thumbnail_png` parameter to `thumbnail_data` (docstring clarifies PNG-only for 3MF)

### Output Naming
- JPEG files use `.jpg` extension (not `.jpeg`)
- Multi-angle output: `input_front.jpg` for JPEG, `input_front.png` for PNG — extension matches format

### Claude's Discretion
- Internal encode module structure (rename `png_encode.rs` to `encode.rs` or keep separate modules)
- Exact `image` crate API usage for PNG and JPEG encoding
- How to handle the alpha-to-white compositing before JPEG encoding
- Error messages for format/quality validation
- Test strategy and which existing tests need updating for the API rename
- Whether `gcode_embed.rs` in render crate needs format-aware changes alongside gcode-io changes

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Render crate (primary changes)
- `crates/slicecore-render/src/lib.rs` — ThumbnailConfig, Thumbnail struct, render_mesh() — all need format support
- `crates/slicecore-render/src/png_encode.rs` — Current PNG encoding with unsafe block — to be replaced with image crate
- `crates/slicecore-render/Cargo.toml` — Replace `png` dep with `image` (minimal features)

### G-code embedding
- `crates/slicecore-render/src/gcode_embed.rs` — format_gcode_thumbnail_block() uses thumbnail.png_data — needs field rename
- `crates/slicecore-gcode-io/src/thumbnail.rs` — write_thumbnail_comments() takes png_data param — needs format awareness

### 3MF embedding
- `crates/slicecore-fileio/src/export.rs` — save_mesh_with_thumbnail() takes thumbnail_png param — rename to thumbnail_data

### CLI integration
- `crates/slicecore-cli/src/main.rs` — Thumbnail command, cmd_thumbnail(), cmd_slice() with --thumbnails flag — add --format and --quality
- `crates/slicecore-cli/tests/cli_thumbnail.rs` — Existing CLI thumbnail tests — extend for JPEG

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `render_mesh()` — Returns `Vec<Thumbnail>` with RGBA pixels and encoded data. Pipeline stays the same, encoding step changes
- `pipeline::render_to_framebuffer()` — Produces raw RGBA framebuffer. Format-agnostic, no changes needed
- `format_gcode_thumbnail_block()` — Formats thumbnail as G-code comments with base64. Works with any image data
- `write_thumbnail_comments()` in gcode-io — Independent thumbnail writer, base64-encodes raw bytes
- `save_mesh_with_thumbnail()` — 3MF export with thumbnail embedding
- `parse_camera_angles()`, `parse_resolution()` — Existing CLI helpers

### Established Patterns
- `--format` flag pattern used on schema and stats commands (value_parser with string choices)
- `--json` flag pattern on multiple commands
- `ThumbnailConfig` builder pattern with Default impl
- Base64 encoding via `base64` crate (already a dependency)
- Feature-minimal Cargo.toml deps (workspace edition, version)

### Integration Points
- `crates/slicecore-render/` — Core changes: ThumbnailConfig, Thumbnail struct, encode module
- `crates/slicecore-cli/src/main.rs` — CLI flag additions on Thumbnail and Slice commands
- `crates/slicecore-gcode-io/src/thumbnail.rs` — Parameter rename + format awareness
- `crates/slicecore-fileio/src/export.rs` — Parameter rename
- `crates/slicecore-render/src/gcode_embed.rs` — Field rename (png_data -> encoded_data)

</code_context>

<specifics>
## Specific Ideas

- Extension auto-detection: `thumbnail input.stl -o thumb.jpg` should Just Work without --format
- 3MF override behavior should be a warning, not an error — user's workflow shouldn't break
- The unsafe elimination in png_encode.rs is a nice cleanup win that comes naturally with the image crate migration
- G-code firmware compatibility is paramount — don't change tag names, firmware parses tags not image formats

</specifics>

<deferred>
## Deferred Ideas

- **WebP/AVIF format support** — future image formats for thumbnails
- **Auto-quality size targeting** — `--max-size` flag to binary search quality for firmware size limits
- **Firmware format recommendations** — `thumbnail_format_for_dialect()` returning recommended ImageFormat per firmware
- **Named quality presets** — `--quality high/medium/low` instead of numeric
- **Feature-gated JPEG** — optional JPEG support behind Cargo feature flag for minimal WASM builds

</deferred>

---

*Phase: 39-jpeg-thumbnail-export-add-jpeg-encoding-option-to-render-crate-alongside-existing-png-with-cli-flag-quality-control-and-3mf-g-code-thumbnail-embedding-support*
*Context gathered: 2026-03-19*
