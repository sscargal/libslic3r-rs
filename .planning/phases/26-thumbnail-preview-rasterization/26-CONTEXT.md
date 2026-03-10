# Phase 26: Thumbnail/Preview Rasterization - Context

**Gathered:** 2026-03-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Rasterize 3D model meshes into PNG thumbnail images for embedding in 3MF files, G-code comments, and standalone output. Renders shaded 3D views of the input mesh from multiple camera angles using a custom CPU-based software renderer. No GPU acceleration. No 2D toolpath rendering (existing SlicePreview handles visualization data).

</domain>

<decisions>
## Implementation Decisions

### Rendering approach
- Custom CPU-based software triangle rasterizer — no external rendering dependencies (no tiny-skia, no GPU)
- Gouraud shading with vertex normal interpolation for smooth curved surfaces
- Single directional light from upper-right with ambient term (~20%) — no shadow casting
- Smooth shaded surfaces only — no wireframe edges, no silhouette outlines
- Model only, no build plate or reference plane
- Projection math (Matrix4, view/projection transforms) kept internal to the render crate — not added to slicecore-math
- New `slicecore-render` crate at Layer 1, depends on `slicecore-mesh` (Layer 0) for `TriangleMesh` input directly

### Camera angles
- 6 standard angles: front, left, right, back, top, and isometric (45° elevation, 45° rotation)
- All 6 angles rendered at the same resolution per invocation
- Default view for 3MF embedding: isometric (matching Bambu Studio convention)
- Caller can select which angles to render

### Thumbnail resolutions & output targets
- Supported preset resolutions: 220x124 (Bambu Lab), 300x300 (PrusaSlicer), 640x480 (high-res)
- Custom arbitrary resolutions supported via API parameter
- Resolution priority: printer profile `thumbnail_resolution` config field > user override parameter > sensible default (300x300)
- Add `thumbnail_resolution` field to PrintConfig/printer profile schema

### Output formats & embedding
- PNG encoding via the `png` crate (pure Rust, WASM-compatible)
- Three output targets:
  1. **3MF**: Embed at `/Metadata/thumbnail.png` — default is single isometric view
  2. **G-code**: Base64-encoded PNG in header comments — support multiple firmware comment formats (PrusaSlicer '; thumbnail begin', Creality '; png begin', etc.), selected based on target firmware from printer profile
  3. **Standalone**: Save as individual PNG files on disk
- Renderer exposes both raw RGBA pixel buffer API and convenience `render_to_png()` wrapper

### Background & transparency
- Configurable background: transparent (alpha=0) or caller-specified solid color
- Default: transparent background

### Model coloring
- Use filament color from print profile as primary model color
- Multi-material models: each mesh region rendered in its assigned filament color
- Fallback color when no filament color specified: light gray (#C8C8C8)

### Pipeline integration
- Thumbnail generation is pre-slice — renders from input TriangleMesh, not from toolpath data
- Separate step from Engine::slice() — standalone `render_thumbnails()` function/method, not coupled into the slicing pipeline
- No automatic generation during slice — caller explicitly invokes rendering

### CLI integration
- Standalone `slicecore thumbnail` subcommand for generating thumbnails without slicing (flags: --angles, --resolution, --output)
- `slicecore slice --thumbnails` flag to embed thumbnails in output 3MF/G-code during slicing
- CLI makes default (single isometric) and full-set (all 6 angles) both easy to invoke

### Claude's Discretion
- Exact z-buffer implementation details
- Vertex normal computation strategy (face-weighted vs area-weighted)
- Optimal ambient lighting ratio
- PNG compression level
- Auto-fit camera distance calculation (model bounding sphere)
- G-code firmware format detection heuristics

</decisions>

<specifics>
## Specific Ideas

- "Bambu Lab 3MF files typically include one primary thumbnail image at /Metadata/thumbnail.png — a 3D shaded isometric/perspective view, not 2D toolpaths. This should be the default."
- "Custom renderer removes external dependencies — focus only on rendering features needed for this project, don't clone tiny-skia"
- "Make it simple for a CLI to easily create the default (single isometric), and all 6 rendered views"
- "Exclude GPU acceleration — introduces too many dependencies with GPU drivers, SDKs, vendor/model/version matrix"

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SlicePreview`/`LayerPreview` (`slicecore-engine/src/preview.rs`): Existing toolpath visualization data — not used for thumbnails but establishes the preview pattern
- `TriangleMesh` (`slicecore-mesh`): Input mesh with vertices, faces, and normals — direct input to the renderer
- 3MF export (`slicecore-fileio/src/export.rs`): Has `thumbnail: None` field on objects, ready to accept thumbnail data
- `png` crate: Already available in the Rust ecosystem, pure Rust, WASM-compatible

### Established Patterns
- Layer-based crate organization: `slicecore-render` fits at Layer 1 (depends on Layer 0 mesh)
- Serde serialization for data exchange between crates
- `#[cfg(target_arch = "wasm32")]` gating for WASM-specific behavior

### Integration Points
- `slicecore-fileio/src/export.rs`: Fill the `thumbnail: None` field with rendered PNG data
- `slicecore-gcode-io`: Add thumbnail comment block to G-code header output
- `slicecore-config`: Add `thumbnail_resolution` field to printer profile schema
- `bins/slicecore-cli`: Add `thumbnail` subcommand and `--thumbnails` flag on `slice` command
- `slicecore-engine`: Optional `render_thumbnails()` convenience method (delegates to render crate)

</code_context>

<deferred>
## Deferred Ideas

- 2D toolpath rasterization (rendering layer previews as images) — could be a future enhancement
- GPU-accelerated rendering via wgpu — rejected for v1 due to dependency complexity
- Build plate/bed visualization in thumbnails — future enhancement
- Animated GIF/video previews — separate capability entirely

</deferred>

---

*Phase: 26-thumbnail-preview-rasterization*
*Context gathered: 2026-03-10*
