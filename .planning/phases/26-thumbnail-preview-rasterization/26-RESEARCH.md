# Phase 26: Thumbnail/Preview Rasterization - Research

**Researched:** 2026-03-10
**Domain:** CPU software rasterization, PNG encoding, 3MF/G-code thumbnail embedding
**Confidence:** HIGH

## Summary

This phase implements a custom CPU-based software triangle rasterizer in a new `slicecore-render` crate at Layer 1. The renderer takes `TriangleMesh` input, applies view/projection transforms, performs z-buffered triangle rasterization with Gouraud shading, and outputs RGBA pixel buffers that are encoded as PNG. Output targets include 3MF embedding (via lib3mf-core attachments), G-code header comments (base64-encoded PNG), and standalone PNG files.

The core rendering pipeline is well-understood computer graphics fundamentals: model-view-projection matrix transforms, scanline triangle rasterization with barycentric interpolation, z-buffering, and Gouraud shading. No exotic dependencies are needed -- the `png` crate (v0.18, pure Rust, WASM-compatible) handles PNG encoding, and a new `base64` dependency handles G-code thumbnail encoding. All projection math stays internal to the render crate per user decision.

**Primary recommendation:** Build the renderer bottom-up: framebuffer/z-buffer primitives first, then triangle rasterization with edge functions, then camera/projection math, then shading, then PNG encoding, then integration with 3MF export and G-code output.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Custom CPU-based software triangle rasterizer -- no external rendering dependencies (no tiny-skia, no GPU)
- Gouraud shading with vertex normal interpolation for smooth curved surfaces
- Single directional light from upper-right with ambient term (~20%) -- no shadow casting
- Smooth shaded surfaces only -- no wireframe edges, no silhouette outlines
- Model only, no build plate or reference plane
- Projection math (Matrix4, view/projection transforms) kept internal to the render crate -- not added to slicecore-math
- New `slicecore-render` crate at Layer 1, depends on `slicecore-mesh` (Layer 0) for `TriangleMesh` input directly
- 6 standard camera angles: front, left, right, back, top, and isometric (45 deg elevation, 45 deg rotation)
- All 6 angles rendered at same resolution per invocation; caller selects which angles
- Default 3MF view: isometric (Bambu Studio convention)
- Preset resolutions: 220x124 (Bambu Lab), 300x300 (PrusaSlicer), 640x480 (high-res); custom arbitrary supported
- Resolution priority: printer profile `thumbnail_resolution` > user override > 300x300 default
- PNG encoding via the `png` crate (pure Rust, WASM-compatible)
- Three output targets: 3MF (Metadata/thumbnail.png), G-code (base64 in header comments), standalone PNG files
- Renderer exposes both raw RGBA pixel buffer API and convenience `render_to_png()` wrapper
- Configurable background: transparent (alpha=0) or caller-specified solid color; default transparent
- Filament color from print profile as primary model color; multi-material per-region color; fallback #C8C8C8
- Pre-slice rendering from TriangleMesh, not from toolpath data
- Standalone `render_thumbnails()` function, not coupled into slicing pipeline
- Standalone `slicecore thumbnail` CLI subcommand; `slicecore slice --thumbnails` flag
- Add `thumbnail_resolution` field to PrintConfig/printer profile schema

### Claude's Discretion
- Exact z-buffer implementation details
- Vertex normal computation strategy (face-weighted vs area-weighted)
- Optimal ambient lighting ratio
- PNG compression level
- Auto-fit camera distance calculation (model bounding sphere)
- G-code firmware format detection heuristics

### Deferred Ideas (OUT OF SCOPE)
- 2D toolpath rasterization (rendering layer previews as images)
- GPU-accelerated rendering via wgpu
- Build plate/bed visualization in thumbnails
- Animated GIF/video previews
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `png` | 0.18 | PNG encoding from RGBA buffers | Pure Rust, WASM-compatible, no unsafe, battle-tested, fast fdeflate encoder |
| `base64` | 0.22 | Base64 encoding for G-code thumbnail comments | De facto standard Rust base64 crate, pure Rust, WASM-compatible |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `lib3mf-core` | 0.4.0 | 3MF thumbnail embedding via `Model.attachments` | Already in project; embed PNG at `Metadata/thumbnail.png` key |
| `slicecore-math` | workspace | Point3, Vec3, BBox3, Matrix4x4 types | Input mesh geometry types; NOT for render-internal projection math |
| `slicecore-mesh` | workspace | TriangleMesh input | Vertices, indices, normals, AABB access |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom rasterizer | tiny-skia | User explicitly rejected external rendering deps |
| Custom rasterizer | softbuffer/pixels | These are display libraries, not offscreen renderers |
| `png` | `image` | image crate is heavier; png is focused and lighter |
| `base64` | manual encoding | Not worth hand-rolling; base64 is tiny and correct |

**Installation:**
```bash
cargo add png@0.18 --package slicecore-render
cargo add base64@0.22 --package slicecore-gcode-io  # or slicecore-render
```

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-render/
  src/
    lib.rs           # Public API: render_thumbnails(), ThumbnailConfig, CameraAngle
    framebuffer.rs   # Framebuffer (RGBA + Z-buffer)
    rasterizer.rs    # Triangle rasterization (edge functions, scanline fill)
    camera.rs        # View/projection matrices, camera angles, auto-fit
    shading.rs       # Gouraud shading, lighting model
    pipeline.rs      # Full render pipeline: mesh -> framebuffer -> PNG
    png_encode.rs    # PNG encoding wrapper
    gcode_embed.rs   # G-code thumbnail comment formatting (base64)
    types.rs         # Internal math types (Mat4, Vec4) for projection -- NOT exported
  Cargo.toml
```

### Pattern 1: Framebuffer with Z-Buffer
**What:** A simple RGBA pixel buffer paired with a depth buffer for hidden surface removal.
**When to use:** Every render operation writes to this structure.
**Example:**
```rust
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,  // RGBA per pixel
    pub depth: Vec<f32>,        // Z-depth per pixel (f32 sufficient for thumbnails)
}

impl Framebuffer {
    pub fn new(width: u32, height: u32, background: [u8; 4]) -> Self {
        let pixel_count = (width * height) as usize;
        Self {
            width,
            height,
            pixels: vec![background; pixel_count],
            depth: vec![f32::INFINITY; pixel_count],
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, z: f32, color: [u8; 4]) {
        let idx = (y * self.width + x) as usize;
        if z < self.depth[idx] {
            self.depth[idx] = z;
            self.pixels[idx] = color;
        }
    }
}
```

### Pattern 2: Scanline Triangle Rasterization with Edge Functions
**What:** Rasterize projected triangles using edge function tests within bounding box.
**When to use:** Core rendering loop for every triangle.
**Example:**
```rust
// For each triangle:
// 1. Compute screen-space vertices (after MVP transform + viewport)
// 2. Compute bounding box, clamp to framebuffer
// 3. For each pixel in bbox, compute barycentric coordinates via edge functions
// 4. If all barycentric >= 0, pixel is inside triangle
// 5. Interpolate Z and vertex colors/normals using barycentric weights
// 6. Write to framebuffer with Z-test

fn edge_function(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (c[0] - a[0]) * (b[1] - a[1]) - (c[1] - a[1]) * (b[0] - a[0])
}
```

### Pattern 3: Camera Auto-Fit via Bounding Sphere
**What:** Automatically position camera so model fills the viewport.
**When to use:** Every render -- ensures consistent framing regardless of model size.
**Example:**
```rust
// 1. Compute AABB center from TriangleMesh.aabb()
// 2. Compute bounding sphere radius: distance from center to farthest corner
// 3. Camera distance = radius / sin(fov/2) for perspective, or radius for ortho
// 4. Camera target = AABB center
// 5. Camera position = target + direction * distance
```

**Recommendation for Claude's discretion:** Use orthographic projection for thumbnails (not perspective). Orthographic is simpler, avoids perspective distortion on small models, and matches how most slicer thumbnails look. Auto-fit distance is simply the bounding sphere radius plus a small margin.

### Pattern 4: Gouraud Shading with Vertex Normals
**What:** Compute lighting per-vertex, interpolate across triangle via barycentric coords.
**When to use:** Shading every visible triangle.
**Example:**
```rust
// Per vertex:
//   intensity = ambient + max(0, dot(normal, light_dir)) * (1.0 - ambient)
// Interpolate intensity across triangle using barycentric coordinates
// Final color = model_color * intensity (per channel)

const AMBIENT: f32 = 0.2;
const LIGHT_DIR: [f32; 3] = [0.577, 0.577, 0.577]; // normalized upper-right-front
```

### Pattern 5: Vertex Normal Computation
**What:** Compute smooth vertex normals by averaging face normals of adjacent triangles.
**When to use:** Once per mesh, before rendering.
**Recommendation for Claude's discretion:** Use area-weighted face normals. This produces better results than equal-weight averaging because large triangles contribute more to the perceived surface direction. Implementation: for each vertex, sum (face_normal * triangle_area) for all adjacent faces, then normalize.

### Anti-Patterns to Avoid
- **Perspective projection for thumbnails:** Perspective distortion makes small objects look odd in tiny thumbnails. Use orthographic.
- **f64 for framebuffer math:** The rasterizer inner loop should use f32 for performance. f64 precision is unnecessary for pixel-level rendering.
- **Allocating per-triangle:** Pre-allocate the vertex normal array and projected vertex array. The rasterizer inner loop must be allocation-free.
- **Exposing internal projection types:** The render crate's Mat4/Vec4 types must NOT be public. The public API accepts `&TriangleMesh` and config structs, returns PNG bytes or RGBA buffers.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| PNG encoding | Manual PNG chunk writing | `png` crate | Deflate compression, IDAT chunking, CRC correctness, APNG edge cases |
| Base64 encoding | Manual base64 impl | `base64` crate | Padding, line wrapping, RFC compliance |
| 3MF ZIP embedding | Manual ZIP writing | lib3mf-core `Model.attachments` | Already handles OPC relationships, ZIP structure, content types |

**Key insight:** The rendering math (transforms, rasterization, shading) IS hand-rolled by design -- the user explicitly chose custom CPU rendering. But encoding (PNG, base64, ZIP) should use established crates.

## Common Pitfalls

### Pitfall 1: Winding Order After Projection
**What goes wrong:** Triangles facing the camera appear back-face culled.
**Why it happens:** The model-view-projection transform can flip triangle winding in screen space depending on the camera orientation and coordinate system conventions.
**How to avoid:** After projecting to screen space, check if the triangle's screen-space winding is CCW. If CW, it's back-facing -- skip it. This also serves as back-face culling for performance.
**Warning signs:** Entire model renders black or invisible from certain angles.

### Pitfall 2: Viewport Y-Axis Inversion
**What goes wrong:** Image appears upside-down.
**Why it happens:** Screen coordinates typically have Y increasing downward, but NDC has Y increasing upward.
**How to avoid:** Flip Y during viewport transform: `screen_y = height - 1 - ndc_y_mapped`.
**Warning signs:** Thumbnails appear mirrored vertically.

### Pitfall 3: Z-Buffer Precision with Orthographic Projection
**What goes wrong:** Z-fighting on nearly coplanar surfaces.
**Why it happens:** Poor near/far plane selection wastes depth buffer precision.
**How to avoid:** Set near = -bounding_sphere_radius * 1.1, far = +bounding_sphere_radius * 1.1 (tight to model extent). f32 is sufficient for thumbnail rendering.
**Warning signs:** Flickering or striped artifacts on flat surfaces.

### Pitfall 4: G-code Thumbnail Line Length
**What goes wrong:** Firmware fails to decode thumbnail.
**Why it happens:** Some firmware limits comment line length (typically 76 or 78 chars of base64 per line).
**How to avoid:** Split base64 output into 76-character lines, each prefixed with `; `. PrusaSlicer uses 78-char lines.
**Warning signs:** Printer shows blank or corrupted thumbnail.

### Pitfall 5: Empty or Degenerate Mesh
**What goes wrong:** Division by zero in bounding sphere calculation.
**Why it happens:** Mesh with 0 triangles or all-coincident vertices has zero bounding sphere radius.
**How to avoid:** Check triangle count > 0 and bounding box has non-zero extent before rendering. Return empty/default thumbnail for degenerate input.
**Warning signs:** NaN in camera matrices, panic in normalize.

### Pitfall 6: PNG Encoder Row Ordering
**What goes wrong:** Image appears garbled.
**Why it happens:** PNG expects rows top-to-bottom, but framebuffer might store bottom-to-top.
**How to avoid:** Ensure framebuffer stores pixels in row-major, top-to-bottom order (matching PNG convention), OR reverse rows during encoding.
**Warning signs:** Diagonal striping or shifted image bands.

## Code Examples

### PNG Encoding from RGBA Buffer
```rust
// Using png crate 0.18
use png::Encoder;
use std::io::Cursor;

fn encode_png(width: u32, height: u32, rgba_pixels: &[[u8; 4]]) -> Vec<u8> {
    let mut output = Vec::new();
    {
        let mut encoder = Encoder::new(Cursor::new(&mut output), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast); // Good balance for thumbnails
        let mut writer = encoder.write_header().expect("PNG header");
        // Flatten [[u8; 4]] to &[u8]
        let flat: &[u8] = bytemuck_or_unsafe_cast(rgba_pixels); // or manual flatten
        writer.write_image_data(flat).expect("PNG data");
    }
    output
}
```

### 3MF Thumbnail Embedding
```rust
// lib3mf-core Model has: pub attachments: HashMap<String, Vec<u8>>
// The package writer auto-detects "Metadata/thumbnail.png" key and creates
// OPC relationship with type "http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail"
model.attachments.insert(
    "Metadata/thumbnail.png".to_string(),
    png_bytes,
);
```

### G-code Thumbnail Comment Block (PrusaSlicer Format)
```
; thumbnail begin 300x300 18444
; iVBORw0KGgoAAAANSUhEUgAAASwAAAEsCAYAAAB5fY51AAAA...
; ... (76-char base64 lines, each prefixed with "; ")
; thumbnail end
```

### G-code Thumbnail Comment Block (Creality Format)
```
; png begin 300x300 18444
; iVBORw0KGgoAAAANSUhEUgAAASwAAAEsCAYAAAB5fY51AAAA...
; png end
```

### Camera Angle Definitions
```rust
pub enum CameraAngle {
    Front,       // Looking along -Y axis
    Back,        // Looking along +Y axis
    Left,        // Looking along +X axis
    Right,       // Looking along -X axis
    Top,         // Looking along -Z axis
    Isometric,   // 45 deg elevation, 45 deg azimuth rotation
}

impl CameraAngle {
    /// Returns (eye_direction, up_vector) for the camera.
    pub fn direction_and_up(&self) -> ([f64; 3], [f64; 3]) {
        match self {
            Self::Front =>     ([0.0, -1.0,  0.0], [0.0, 0.0, 1.0]),
            Self::Back =>      ([0.0,  1.0,  0.0], [0.0, 0.0, 1.0]),
            Self::Left =>      ([1.0,  0.0,  0.0], [0.0, 0.0, 1.0]),
            Self::Right =>     ([-1.0, 0.0,  0.0], [0.0, 0.0, 1.0]),
            Self::Top =>       ([0.0,  0.0, -1.0], [0.0, 1.0, 0.0]),
            Self::Isometric => {
                // 45 deg elevation, 45 deg azimuth
                let s = std::f64::consts::FRAC_1_SQRT_2; // sin(45) = cos(45)
                let dir = [s * s, -s * s, -s]; // normalized
                (dir, [0.0, 0.0, 1.0])
            }
        }
    }
}
```

### Internal Look-At Matrix (render crate only, NOT exported)
```rust
/// Compute a look-at view matrix (right-handed).
fn look_at(eye: [f64; 3], target: [f64; 3], up: [f64; 3]) -> [[f64; 4]; 4] {
    let f = normalize(sub(target, eye));  // forward
    let r = normalize(cross(f, up));      // right
    let u = cross(r, f);                  // true up
    [
        [r[0], r[1], r[2], -dot(r, eye)],
        [u[0], u[1], u[2], -dot(u, eye)],
        [-f[0], -f[1], -f[2], dot(f, eye)],
        [0.0, 0.0, 0.0, 1.0],
    ]
}
```

### Orthographic Projection Matrix (render crate only)
```rust
fn ortho(left: f64, right: f64, bottom: f64, top: f64, near: f64, far: f64) -> [[f64; 4]; 4] {
    let w = right - left;
    let h = top - bottom;
    let d = far - near;
    [
        [2.0/w, 0.0,   0.0,    -(right+left)/w],
        [0.0,   2.0/h, 0.0,    -(top+bottom)/h],
        [0.0,   0.0,   -2.0/d, -(far+near)/d],
        [0.0,   0.0,   0.0,    1.0],
    ]
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| OpenGL-based thumbnail generation | CPU software rendering for embedded/WASM contexts | N/A (design choice) | No GPU dependency, WASM-compatible |
| Custom PNG writer | `png` crate with fdeflate | png 0.18 (2024) | Faster encoding, better compression |
| PrusaSlicer-only thumbnail format | Multiple firmware comment formats | OrcaSlicer 2.0+ | Must support both "; thumbnail begin" and "; png begin" |

**Current ecosystem:**
- Bambu Studio embeds thumbnails at `/Metadata/thumbnail.png` in 3MF (220x124 resolution)
- PrusaSlicer uses `; thumbnail begin WxH SIZE` format in G-code
- Creality firmware uses `; png begin WxH SIZE` variant
- Most slicers use isometric 3D shaded views (not 2D toolpath renders) for primary thumbnails

## Open Questions

1. **G-code thumbnail format selection heuristic**
   - What we know: PrusaSlicer uses `; thumbnail begin`, Creality uses `; png begin`, Bambu uses 3MF-only
   - What's unclear: Exact mapping from `GcodeDialect` to thumbnail comment format
   - Recommendation: Map Marlin/Klipper/RepRapFirmware -> `; thumbnail begin` (PrusaSlicer format), Bambu -> skip G-code thumbnails (use 3MF only). Add a `thumbnail_format` config field for explicit override.

2. **Aspect ratio handling for non-square resolutions**
   - What we know: 220x124 is 16:9-ish, 300x300 is square, 640x480 is 4:3
   - What's unclear: Should model be stretched or letterboxed?
   - Recommendation: Always preserve model aspect ratio with letterboxing (transparent/background color padding). This matches how all major slicers handle it.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | workspace Cargo.toml |
| Quick run command | `cargo test -p slicecore-render` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| RENDER-01 | Framebuffer creation and pixel write with z-test | unit | `cargo test -p slicecore-render framebuffer -x` | Wave 0 |
| RENDER-02 | Triangle rasterization produces correct pixels | unit | `cargo test -p slicecore-render rasterizer -x` | Wave 0 |
| RENDER-03 | Camera angles produce distinct views | unit | `cargo test -p slicecore-render camera -x` | Wave 0 |
| RENDER-04 | Gouraud shading varies with surface orientation | unit | `cargo test -p slicecore-render shading -x` | Wave 0 |
| RENDER-05 | PNG encoding produces valid PNG file | unit | `cargo test -p slicecore-render png -x` | Wave 0 |
| RENDER-06 | 3MF export includes thumbnail attachment | integration | `cargo test -p slicecore-fileio thumbnail -x` | Wave 0 |
| RENDER-07 | G-code output includes thumbnail comment block | integration | `cargo test -p slicecore-gcode-io thumbnail -x` | Wave 0 |
| RENDER-08 | CLI thumbnail subcommand produces PNG file | integration | `cargo test -p slicecore-cli thumbnail -x` | Wave 0 |
| RENDER-09 | WASM compilation succeeds | smoke | `cargo build -p slicecore-render --target wasm32-unknown-unknown` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-render`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-render/` -- entire crate is new
- [ ] `crates/slicecore-render/Cargo.toml` -- new crate manifest
- [ ] `crates/slicecore-render/src/lib.rs` -- public API entry point
- [ ] PNG and base64 dependencies in workspace Cargo.toml

## Sources

### Primary (HIGH confidence)
- lib3mf-core 0.4.0 source (local: `~/.cargo/registry/src/*/lib3mf-core-0.4.0/`) -- verified `Model.attachments` HashMap for thumbnail embedding, package writer auto-detects `Metadata/thumbnail.png` path
- slicecore-mesh source -- `TriangleMesh` API: `vertices()`, `indices()`, `normals()`, `aabb()`
- slicecore-math source -- `Matrix4x4` has multiply, transform_point3, rotation_x/y/z, translation, scaling, inverse; stored row-major
- slicecore-fileio export.rs -- Object `thumbnail: None` field ready for String path

### Secondary (MEDIUM confidence)
- [png crate docs](https://docs.rs/png/0.18.0/png/) -- Encoder API, ColorType::Rgba, Compression::Fast
- [PrusaSlicer thumbnail format](https://help.prusa3d.com/article/model-preview_648687) -- `; thumbnail begin WxH SIZE` format
- [Creality thumbnail format](https://forum.creality.com/t/gcode-thumbnails/2738) -- `; png begin` variant for some Creality firmware
- [Bambu Studio 3MF convention](https://crates.io/crates/lib3mf-core) -- thumbnail at `/Metadata/thumbnail.png`

### Tertiary (LOW confidence)
- G-code base64 line length (76 chars) -- based on PrusaSlicer source code convention; some firmware may accept longer lines

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - png crate is well-established, pure Rust, WASM-compatible; base64 is trivial
- Architecture: HIGH - software rasterization is textbook computer graphics, well-understood algorithms
- Pitfalls: HIGH - common rendering pitfalls are well-documented in graphics programming literature
- Integration points: HIGH - verified lib3mf-core attachments API and 3MF export code directly in source

**Research date:** 2026-03-10
**Valid until:** 2026-04-10 (stable domain, no fast-moving dependencies)
