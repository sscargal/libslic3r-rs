---
created: 2026-03-16T19:32:48.250Z
title: Image2Gcode diffusion model for sketch-to-print workflow
area: ai
files:
  - crates/slicecore-ai/src/lib.rs
  - crates/slicecore-ai/src/types.rs
---

## Problem

The traditional 3D printing workflow requires CAD expertise: design in CAD → export STL → configure slicer → print. This multi-step pipeline is a major barrier for non-expert users who have a visual idea of what they want but lack CAD skills.

Recent research (ArXiv) demonstrates diffusion-transformer models that can translate 2D sketches or photographs directly into executable G-code, bypassing the entire CAD→STL→Slicer pipeline. This could dramatically lower the barrier to entry for 3D printing.

## Solution

### Concept

Integrate an Image2Gcode pipeline as an alternative input path alongside traditional STL/3MF:

```
Traditional:  CAD → STL → Slicer → G-code → Print
New:          Sketch/Photo → AI Model → G-code → Print
```

### Two-stage approach

**Stage 1: Image → Mesh (more practical near-term)**
- Use existing image-to-3D models (Point-E, Shap-E, TripoSR, InstantMesh) to generate a mesh from a sketch/photo
- Feed the generated mesh into slicecore's normal slicing pipeline
- User gets to review and modify the mesh before slicing
- Lower risk: slicecore validates the mesh like any other input

```bash
slicecore generate --from-image sketch.png --style "functional bracket"
# → Generates mesh → user reviews → slicecore slices normally
```

**Stage 2: Image → G-code directly (research frontier)**
- Diffusion-transformer model generates G-code directly from visual input
- Bypasses mesh representation entirely — the model learns print-aware geometry
- Potentially captures printing constraints (overhangs, support needs) implicitly
- Higher risk: generated G-code needs extensive validation before sending to printer

```bash
slicecore generate --from-image sketch.png --direct-gcode --printer X1C
# → Generates G-code directly → validation → print
```

### Safety considerations

Direct G-code generation from AI is inherently risky:
- Generated G-code must pass through slicecore's G-code validator (Phase 21)
- Temperature commands must be within safe ranges for the target printer
- Motion commands must stay within printer build volume
- A "sandbox preview" mode should show the virtual toolpath before execution
- Never send AI-generated G-code to a printer without human review

### Integration with slicecore

- **New CLI command**: `slicecore generate` for AI-based model generation
- **Validation pipeline**: Generated G-code runs through existing G-code analysis
- **Preview**: Use slicecore-render to visualize generated toolpaths
- **Provider-agnostic**: Support multiple image-to-3D backends via the existing AI provider system
- **Local-first**: Prioritize models that can run locally (e.g., via Ollama or ONNX runtime) for privacy

### Research references

- Diffusion-transformer models for G-code generation (ArXiv)
- TripoSR / InstantMesh for single-image 3D reconstruction
- Point-E / Shap-E (OpenAI) for text/image to 3D

## Dependencies

- **Phase 8 (AI integration)**: ✓ Provider infrastructure for model inference
- **Phase 21 (G-code analysis)**: ✓ Validation of generated G-code
- **Phase 26 (Thumbnail/preview)**: ✓ Visualizing generated toolpaths
- **Image-to-3D model runtime**: Need ONNX or similar inference runtime in Rust

## Phased implementation

1. **Phase A**: Image → Mesh via external API (TripoSR/InstantMesh) → normal slicing pipeline
2. **Phase B**: Local image-to-mesh inference (ONNX runtime)
3. **Phase C**: Research direct Image → G-code models, prototype with validation
4. **Phase D**: Production Image → G-code with full safety pipeline
