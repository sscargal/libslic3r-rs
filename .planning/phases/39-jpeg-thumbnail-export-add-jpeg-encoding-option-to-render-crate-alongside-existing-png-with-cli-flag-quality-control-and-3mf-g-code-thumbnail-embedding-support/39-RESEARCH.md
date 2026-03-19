# Phase 39: JPEG Thumbnail Export - Research

**Researched:** 2026-03-19
**Domain:** Image encoding (PNG/JPEG), Rust `image` crate, CLI flag patterns, G-code/3MF embedding
**Confidence:** HIGH

## Summary

This phase adds JPEG encoding to the existing thumbnail pipeline. The changes span five crates: `slicecore-render` (core encoding + struct changes), `slicecore-cli` (format/quality flags), `slicecore-gcode-io` (parameter rename), `slicecore-fileio` (parameter rename), and the render crate's `gcode_embed` module (field rename). The primary dependency change is replacing the `png` crate with the `image` crate, which provides both PNG and JPEG encoding through a unified API while eliminating the existing `unsafe` block.

The existing code is well-structured for this change. `render_mesh()` already separates RGBA rendering from encoding (line 100-101 of `lib.rs`), making it straightforward to dispatch to PNG or JPEG encoding based on a new `output_format` field. The field renames (`png_data` to `encoded_data`) are mechanical but touch multiple crates and tests.

**Primary recommendation:** Use `image` crate 0.25.x with `default-features = false, features = ["png", "jpeg"]`. Encode PNG via `RgbaImage::write_to()` and JPEG via `write_with_encoder(JpegEncoder::new_with_quality())` after RGBA-to-RGB conversion with alpha compositing.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- PNG remains the default format everywhere (CLI `thumbnail` command, `slice --thumbnails`, 3MF embedding)
- JPEG is opt-in via `--format jpeg` flag on both `thumbnail` and `slice` commands
- CLI flag is `--format jpeg/png` (not `--jpeg` shorthand) -- matches existing `--format` patterns on schema/stats commands
- Format is CLI-only -- no `PrintConfig` setting. Thumbnail format is an output concern, not a print profile setting
- Auto-detect format from output file extension: `thumbnail input.stl -o thumb.jpg` selects JPEG without `--format`
- `ThumbnailConfig` gets an `output_format: ImageFormat` field (enum: `Png`, `Jpeg`) and `quality: Option<u8>` field
- `ImageFormat` enum is simple two-variant (`Png`, `Jpeg`) -- not `#[non_exhaustive]`. Add more later if needed
- `Thumbnail` struct: rename `png_data` to `encoded_data`, add `format: ImageFormat` field
- One format per `render_mesh()` call -- if both formats needed, call twice (RGBA rendering is the expensive part, encoding is cheap)
- Render pipeline stays RGBA throughout, RGB conversion only at JPEG encode time
- Replace `png` crate with `image` crate (pure Rust, WASM-compatible)
- `image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }` -- minimal features only
- Eliminate the `unsafe` block in `png_encode.rs` by using `image::RgbaImage::from_raw()` safe API
- `image` crate is always included (not feature-gated) -- JPEG support is the point of this phase
- Default quality: 85 (good balance for thumbnail resolution)
- CLI flag: `--quality 1-100` (numeric only, no named presets)
- `--quality` with PNG format: warn on stderr and ignore (non-fatal)
- Quality validation: error on out-of-range values (< 1 or > 100)
- Quality stored in `ThumbnailConfig` as `quality: Option<u8>` (None = format default)
- `--format` and `--quality` available on both `thumbnail` and `slice --thumbnails` commands
- Same resolution regardless of format -- resolution is about detail level, not compression
- JPEG doesn't support transparency -- when format is JPEG, auto-set white [255,255,255] background
- Warn if user explicitly set transparent background with JPEG format
- RGBA pipeline internally, convert to RGB only at JPEG encode step
- Same `; thumbnail begin WxH SIZE` / `; thumbnail end` framing for both PNG and JPEG data
- Base64-encode JPEG bytes the same way as PNG -- firmware detects format from data, not tags
- Creality format keeps `; png begin` tag even for JPEG data (changing it would break firmware compatibility)
- `write_thumbnail_comments()` in gcode-io: add format awareness (rename `png_data` param, accept format info for future use)
- `thumbnail_format_for_dialect()` unchanged -- returns comment style only, not image format recommendation
- 3MF always uses PNG regardless of `--format` flag (3MF spec requires PNG at `Metadata/thumbnail.png`)
- When `slice --thumbnails --format jpeg` targets 3MF output: warn on stderr "JPEG not supported for 3MF thumbnails, using PNG" and embed PNG
- `save_mesh_with_thumbnail()` in fileio: rename `thumbnail_png` parameter to `thumbnail_data` (docstring clarifies PNG-only for 3MF)
- JPEG files use `.jpg` extension (not `.jpeg`)
- Multi-angle output: `input_front.jpg` for JPEG, `input_front.png` for PNG -- extension matches format

### Claude's Discretion
- Internal encode module structure (rename `png_encode.rs` to `encode.rs` or keep separate modules)
- Exact `image` crate API usage for PNG and JPEG encoding
- How to handle the alpha-to-white compositing before JPEG encoding
- Error messages for format/quality validation
- Test strategy and which existing tests need updating for the API rename
- Whether `gcode_embed.rs` in render crate needs format-aware changes alongside gcode-io changes

### Deferred Ideas (OUT OF SCOPE)
- WebP/AVIF format support -- future image formats for thumbnails
- Auto-quality size targeting -- `--max-size` flag to binary search quality for firmware size limits
- Firmware format recommendations -- `thumbnail_format_for_dialect()` returning recommended ImageFormat per firmware
- Named quality presets -- `--quality high/medium/low` instead of numeric
- Feature-gated JPEG -- optional JPEG support behind Cargo feature flag for minimal WASM builds
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `image` | 0.25.10 | PNG + JPEG encoding from RGBA buffers | Pure Rust, WASM-compatible, replaces `png` crate, provides both formats through unified API |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `base64` | 0.22 | Already a dependency | G-code thumbnail embedding (unchanged) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `image` | `png` + `jpeg-encoder` | Two deps instead of one; `image` gives unified API |
| `image` | `turbojpeg` | Faster but requires C library (violates pure Rust constraint) |

**Dependency change in Cargo.toml:**
```toml
# Remove:
png = "0.17"

# Add:
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
```

## Architecture Patterns

### Recommended Module Structure

Rename `png_encode.rs` to `encode.rs` since it now handles both PNG and JPEG:

```
crates/slicecore-render/src/
    lib.rs          # ThumbnailConfig, Thumbnail, ImageFormat, render_mesh()
    encode.rs       # encode_png(), encode_jpeg(), encode() dispatcher
    gcode_embed.rs  # format_gcode_thumbnail_block() - field rename png_data -> encoded_data
    camera.rs       # unchanged
    pipeline.rs     # unchanged
    ...
```

### Pattern 1: ImageFormat Enum

```rust
/// Image encoding format for thumbnails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// PNG format (lossless, supports transparency).
    Png,
    /// JPEG format (lossy, no transparency).
    Jpeg,
}

impl ImageFormat {
    /// File extension for this format (without dot).
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }
}
```

### Pattern 2: Encode Dispatcher Using `image` Crate

```rust
// Source: docs.rs/image/0.25.10/
use image::{RgbaImage, ImageBuffer, codecs::jpeg::JpegEncoder};

pub(crate) fn encode(
    width: u32,
    height: u32,
    pixels: &[[u8; 4]],
    format: ImageFormat,
    quality: Option<u8>,
) -> Vec<u8> {
    match format {
        ImageFormat::Png => encode_png(width, height, pixels),
        ImageFormat::Jpeg => encode_jpeg(width, height, pixels, quality.unwrap_or(85)),
    }
}

fn encode_png(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    // Flatten &[[u8; 4]] to Vec<u8> safely (no unsafe!)
    let flat: Vec<u8> = pixels.iter().flat_map(|px| px.iter().copied()).collect();
    let img = RgbaImage::from_raw(width, height, flat)
        .expect("pixel buffer size matches dimensions");

    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("PNG encoding failed");
    buf.into_inner()
}

fn encode_jpeg(width: u32, height: u32, pixels: &[[u8; 4]], quality: u8) -> Vec<u8> {
    // Alpha composite onto white background, then convert to RGB
    let rgb: Vec<u8> = pixels.iter().flat_map(|px| {
        let a = px[3] as f32 / 255.0;
        let r = (px[0] as f32 * a + 255.0 * (1.0 - a)) as u8;
        let g = (px[1] as f32 * a + 255.0 * (1.0 - a)) as u8;
        let b = (px[2] as f32 * a + 255.0 * (1.0 - a)) as u8;
        [r, g, b]
    }).collect();

    let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgb)
            .expect("pixel buffer size matches dimensions");

    let mut buf = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    img.write_with_encoder(encoder)
        .expect("JPEG encoding failed");
    buf
}
```

### Pattern 3: CLI Format Auto-Detection from Extension

```rust
fn detect_image_format(output: Option<&str>, explicit_format: Option<&str>) -> ImageFormat {
    // Explicit --format takes priority
    if let Some(fmt) = explicit_format {
        return match fmt {
            "jpeg" | "jpg" => ImageFormat::Jpeg,
            _ => ImageFormat::Png,
        };
    }
    // Auto-detect from output extension
    if let Some(out) = output {
        let path = Path::new(out);
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            return match ext.to_ascii_lowercase().as_str() {
                "jpg" | "jpeg" => ImageFormat::Jpeg,
                _ => ImageFormat::Png,
            };
        }
    }
    ImageFormat::Png // default
}
```

### Anti-Patterns to Avoid
- **DynamicImage for RGBA-to-RGB conversion:** Using `DynamicImage::from(rgba_img).to_rgb8()` drops the alpha channel without compositing onto white. Must do manual alpha compositing first.
- **Using `image::ImageFormat` as the public enum:** The `image` crate's `ImageFormat` has many variants we don't need. Define our own two-variant enum.
- **Encoding before background compositing:** JPEG cannot store transparency. Alpha must be composited onto white before RGB conversion, not just dropped.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| PNG encoding | Manual `png` crate calls with unsafe | `image::RgbaImage::write_to()` | Safe API, no unsafe block needed |
| JPEG encoding | Raw JPEG bitstream writing | `image::codecs::jpeg::JpegEncoder` | Complex format with DCT, huffman tables |
| RGBA-to-flat-bytes | `unsafe std::slice::from_raw_parts` | `pixels.iter().flat_map()` | Safe, same performance after optimization |
| Alpha compositing | Complex blending library | Simple per-pixel formula | Only need `over` operator with solid white |

**Key insight:** The `image` crate's `from_raw()` + `write_to()` / `write_with_encoder()` API eliminates the unsafe block and provides both PNG and JPEG through the same pattern.

## Common Pitfalls

### Pitfall 1: Alpha Channel Dropped Without Compositing
**What goes wrong:** JPEG has no alpha channel. Naive RGBA-to-RGB conversion (just dropping the alpha byte) makes transparent pixels black instead of white.
**Why it happens:** The default transparent background `[0, 0, 0, 0]` becomes `[0, 0, 0]` (black) when alpha is discarded.
**How to avoid:** Always composite RGBA onto white `[255, 255, 255]` before converting to RGB for JPEG.
**Warning signs:** Dark/black halos around rendered objects in JPEG thumbnails.

### Pitfall 2: `write_to` Requires `Write + Seek`
**What goes wrong:** `ImageBuffer::write_to()` requires a writer implementing both `Write` and `Seek`.
**Why it happens:** Some image formats need seeking. Using `Vec<u8>` directly doesn't work.
**How to avoid:** Use `std::io::Cursor::new(Vec::new())` as the writer, then call `.into_inner()`.
**Warning signs:** Compilation error "the trait bound `Vec<u8>: Seek` is not satisfied."

### Pitfall 3: `write_with_encoder` Avoids Seek Requirement
**What goes wrong:** Using `write_to` for JPEG when `write_with_encoder` would be simpler.
**Why it happens:** JPEG doesn't actually need seeking, but `write_to` enforces it for all formats.
**How to avoid:** For JPEG, use `write_with_encoder(JpegEncoder::new_with_quality(&mut buf, quality))` which only needs `Write`.
**Warning signs:** Unnecessary `Cursor` wrapping for JPEG output.

### Pitfall 4: Test Breakage from Field Rename
**What goes wrong:** Renaming `png_data` to `encoded_data` on `Thumbnail` breaks all downstream code.
**Why it happens:** Struct field is public and used in tests, CLI, gcode_embed, and gcode-io.
**How to avoid:** Do the rename in a single plan that touches all call sites. Compile-check after.
**Warning signs:** Compiler errors across multiple crates after partial rename.

### Pitfall 5: Confusing Image Format Enums
**What goes wrong:** Our `ImageFormat` enum collides with `image::ImageFormat` from the crate.
**Why it happens:** Same name, different types. Import confusion.
**How to avoid:** Keep our enum as the public API. Use `image::ImageFormat::Png` only internally in encode.rs with a qualified path or alias.
**Warning signs:** "Ambiguous type" or "wrong number of variants" errors.

### Pitfall 6: 3MF Always Needs PNG Data
**What goes wrong:** When `--format jpeg` is used with 3MF output, the system tries to embed JPEG in 3MF.
**Why it happens:** Format flag applies globally, but 3MF spec requires PNG.
**How to avoid:** In `cmd_slice`, when output is 3MF and format is JPEG, override to PNG for 3MF embedding and warn on stderr.
**Warning signs:** Invalid 3MF archives or firmware rejection.

## Code Examples

### Current Code That Must Change

**`lib.rs` - ThumbnailConfig (add fields):**
```rust
// Current
pub struct ThumbnailConfig {
    pub width: u32,
    pub height: u32,
    pub angles: Vec<CameraAngle>,
    pub background: [u8; 4],
    pub model_color: [u8; 3],
}

// New
pub struct ThumbnailConfig {
    pub width: u32,
    pub height: u32,
    pub angles: Vec<CameraAngle>,
    pub background: [u8; 4],
    pub model_color: [u8; 3],
    pub output_format: ImageFormat,
    pub quality: Option<u8>,
}
```

**`lib.rs` - Thumbnail struct (rename field + add format):**
```rust
// Current
pub struct Thumbnail {
    pub angle: CameraAngle,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<[u8; 4]>,
    pub png_data: Vec<u8>,
}

// New
pub struct Thumbnail {
    pub angle: CameraAngle,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<[u8; 4]>,
    pub encoded_data: Vec<u8>,
    pub format: ImageFormat,
}
```

**`gcode_embed.rs` - field access rename:**
```rust
// Current
let png_size = thumbnail.png_data.len();
let b64 = base64::engine::general_purpose::STANDARD.encode(&thumbnail.png_data);

// New
let data_size = thumbnail.encoded_data.len();
let b64 = base64::engine::general_purpose::STANDARD.encode(&thumbnail.encoded_data);
```

**`cli/main.rs` - cmd_thumbnail output path (extension from format):**
```rust
// Current
input.with_extension("png")

// New
input.with_extension(format.extension())
```

### Files That Need Changes (Complete List)

| File | Change Type | Scope |
|------|------------|-------|
| `slicecore-render/Cargo.toml` | Replace `png` with `image` | 1 line |
| `slicecore-render/src/lib.rs` | Add `ImageFormat` enum, update `ThumbnailConfig`, `Thumbnail`, `render_mesh()` | ~30 lines |
| `slicecore-render/src/png_encode.rs` | Rename to `encode.rs`, rewrite with `image` crate, add JPEG support | Full rewrite ~60 lines |
| `slicecore-render/src/gcode_embed.rs` | Rename `png_data` -> `encoded_data` in function body and tests | ~5 lines |
| `slicecore-cli/src/main.rs` | Add `--format`/`--quality` to Thumbnail and Slice commands, update `cmd_thumbnail()` and `cmd_slice()` | ~50 lines |
| `slicecore-gcode-io/src/thumbnail.rs` | Rename `png_data` param to `encoded_data` in signature and docstring | ~5 lines |
| `slicecore-fileio/src/export.rs` | Rename `thumbnail_png` param to `thumbnail_data` in signature and docstring | ~5 lines |
| `slicecore-cli/tests/cli_thumbnail.rs` | Add JPEG tests, update existing references | ~40 lines |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `png` crate with unsafe flatten | `image` crate with safe `from_raw()` | This phase | Eliminates 1 unsafe block |
| PNG-only thumbnails | PNG + JPEG option | This phase | Smaller thumbnails for firmware with size limits |
| Hardcoded `.png` extensions | Format-aware extensions | This phase | `.jpg` for JPEG output |

## Open Questions

1. **Performance of `flat_map` vs unsafe flatten**
   - What we know: The safe `flat_map` approach allocates a new Vec; the old unsafe approach was zero-copy.
   - What's unclear: Whether the allocation matters at thumbnail resolutions (300x300 = 360KB).
   - Recommendation: Use safe code. At thumbnail sizes (< 1MB) the allocation is negligible. Can optimize later if profiling shows a hot spot.

2. **Whether `gcode_embed.rs` should gain format awareness beyond the field rename**
   - What we know: The Creality format must keep `; png begin` tags even for JPEG data (firmware compat). The PrusaSlicer format uses `; thumbnail begin` which is already format-agnostic.
   - What's unclear: Whether any firmware validates that the base64 data matches the tag's implied format.
   - Recommendation: Keep `format_gcode_thumbnail_block()` unchanged beyond the field rename. The function already works with any binary data -- it base64-encodes whatever is in `encoded_data`.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Workspace Cargo.toml |
| Quick run command | `cargo test -p slicecore-render -p slicecore-cli -p slicecore-gcode-io -p slicecore-fileio` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| N/A | PNG encoding still works after migration | unit | `cargo test -p slicecore-render -- encode` | Needs update (Wave 0) |
| N/A | JPEG encoding produces valid JFIF data | unit | `cargo test -p slicecore-render -- encode_jpeg` | New (Wave 0) |
| N/A | JPEG alpha compositing produces white bg | unit | `cargo test -p slicecore-render -- jpeg_white_background` | New (Wave 0) |
| N/A | CLI --format jpeg produces .jpg file | integration | `cargo test -p slicecore-cli --test cli_thumbnail` | Needs extension |
| N/A | CLI auto-detect from .jpg extension | integration | `cargo test -p slicecore-cli --test cli_thumbnail` | New |
| N/A | Quality validation rejects out-of-range | unit | `cargo test -p slicecore-cli` | New |
| N/A | gcode_embed uses encoded_data field | unit | `cargo test -p slicecore-render -- gcode_embed` | Needs update |
| N/A | 3MF always embeds PNG regardless of format | integration | `cargo test -p slicecore-fileio` | Existing covers |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-render -p slicecore-cli`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Update `png_encode.rs` tests for new module name and API
- [ ] Add JPEG encoding unit tests (magic bytes `FF D8 FF`, valid output)
- [ ] Add JPEG alpha compositing test
- [ ] Add CLI integration tests for `--format jpeg` and `--quality`
- [ ] Update `gcode_embed.rs` tests for field rename

## Sources

### Primary (HIGH confidence)
- [image crate docs](https://docs.rs/image/0.25.10/) - `RgbaImage::from_raw()`, `write_to()`, `write_with_encoder()`, `JpegEncoder::new_with_quality()`
- [crates.io/image](https://crates.io/crates/image) - Version 0.25.10 confirmed current
- Existing codebase files (read directly): `lib.rs`, `png_encode.rs`, `gcode_embed.rs`, `thumbnail.rs`, `export.rs`, `main.rs`, `cli_thumbnail.rs`

### Secondary (MEDIUM confidence)
- [image-rs/image GitHub](https://github.com/image-rs/image) - Feature flags, WASM compatibility

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - `image` crate is the de facto Rust image library, version confirmed
- Architecture: HIGH - Existing code structure clearly shows where changes go
- Pitfalls: HIGH - Alpha compositing and Write+Seek issues are well-documented

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable domain, `image` crate mature)
