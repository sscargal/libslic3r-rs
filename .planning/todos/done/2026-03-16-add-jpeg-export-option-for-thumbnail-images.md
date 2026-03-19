---
created: 2026-03-16T18:35:00.000Z
title: Add JPEG export option for thumbnail images
area: render
files:
  - crates/slicecore-render/src/png_encode.rs
  - crates/slicecore-render/src/lib.rs
  - crates/slicecore-gcode-io/src/thumbnail.rs
---

## Problem

Phase 26 implemented thumbnail rasterization with PNG-only output. Some use cases benefit from JPEG:

- **G-code thumbnail comments**: Some firmware (especially Bambu) embeds JPEG thumbnails in G-code headers for smaller file size
- **SaaS/web**: JPEG is often preferred for web display due to smaller size for photographic-style renders
- **3MF embedding**: The 3MF spec supports both PNG and JPEG thumbnails

Currently `slicecore-render` only has `png_encode.rs` — no JPEG path exists.

## Solution

1. Add a `jpeg` feature-gated dependency (e.g., `image` crate's JPEG encoder, or a lightweight JPEG encoder like `jpeg-encoder`)
2. Create `jpeg_encode.rs` alongside `png_encode.rs`
3. Add `ImageFormat` enum (`Png` / `Jpeg { quality: u8 }`) to the render API
4. Update CLI `--thumbnail-format png|jpeg` flag (default: `png`)
5. Update G-code thumbnail embedding to support both formats
6. Update 3MF thumbnail embedding to support both formats
7. Keep PNG as the default — lossless and universally supported
