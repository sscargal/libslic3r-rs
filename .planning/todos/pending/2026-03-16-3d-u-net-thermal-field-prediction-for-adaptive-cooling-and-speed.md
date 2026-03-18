---
created: 2026-03-16T19:35:00.000Z
title: 3D U-Net thermal field prediction for adaptive cooling and speed
area: engine
files:
  - crates/slicecore-engine/src/engine.rs
  - crates/slicecore-engine/src/gcode_gen.rs
  - crates/slicecore-engine/src/config.rs
---

## Problem

Thermal distortion (warping, curling, poor layer adhesion, stringing) is the #1 cause of FDM print failures. The root cause is uneven temperature distribution during printing:

- **Hot spots**: Small features, overhangs, and thin walls where heat accumulates because there isn't enough mass or time for cooling between layers
- **Cold spots**: Large flat areas where the part cools too much between layer deposits, weakening interlayer adhesion
- **Thermal gradients**: Rapid temperature changes across a part cause differential shrinkage → warping

Traditional Finite Difference Method (FDM/FEM) thermal simulation takes minutes to hours — impractical for an interactive slicer. A 3D U-Net surrogate model can predict the thermal field in milliseconds with ~99.8% less compute than full simulation.

## Solution: ML-Based Thermal Prediction

### 3D U-Net Surrogate Model

**What it is**: A convolutional neural network trained on thermal simulation data. Input: voxelized model geometry + print parameters. Output: predicted temperature at every voxel at every time step (or key time steps).

**Architecture**:
```
Input: [X × Y × Z × C] voxel grid
  C channels: geometry (occupied/empty), material properties,
              print order (which voxel prints when),
              local feature thickness

Encoder: 3D conv blocks with max pooling (capture multi-scale features)
  64 → 128 → 256 → 512 feature maps

Bottleneck: 512 feature maps at lowest resolution

Decoder: 3D conv blocks with upsampling + skip connections
  512 → 256 → 128 → 64 feature maps

Output: [X × Y × Z × 1] temperature field (°C at key time points)
```

**Training data generation**:
- Voxelize 1000+ diverse 3D models at print resolution
- Run full thermal simulation (Finite Difference Method) for each
- Simulation parameters: material thermal conductivity, specific heat, density, ambient temp, bed temp, nozzle temp, print speed, fan speed
- Generate [input, ground_truth_temperature] pairs
- Augment with rotations, scaling, material variations

### Integration into slicing pipeline

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Slice model │────▶│  Voxelize &  │────▶│  3D U-Net    │
│  (layers)    │     │  predict     │     │  inference    │
└──────────────┘     └──────────────┘     └──────┬───────┘
                                                  │
                                          thermal field
                                                  │
                     ┌──────────────┐     ┌───────▼───────┐
                     │  Modified    │◀────│  Adaptive     │
                     │  G-code      │     │  speed/fan    │
                     └──────────────┘     └───────────────┘
```

1. **Post-slice voxelization**: Convert sliced geometry to voxel grid at ~0.5mm resolution
2. **Thermal prediction**: Run U-Net inference → temperature field per layer
3. **Hot spot detection**: Identify voxels where T > threshold (too hot → curling risk)
4. **Cold spot detection**: Identify voxels where T < threshold (too cold → adhesion risk)
5. **Adaptive adjustment**: Modify G-code per region:
   - Hot spots: increase fan speed, decrease print speed, add dwell time
   - Cold spots: decrease fan speed, increase speed (less cooling time), increase temp
6. **Output**: Modified G-code with per-region thermal optimization + thermal map visualization

### Adaptive adjustments from thermal field

| Thermal condition | G-code modification | Effect |
|-------------------|-------------------|--------|
| Hot spot (small feature, insufficient cooling time) | Insert `M106 S255` (max fan), reduce `F` speed by 20-40% | More cooling time per layer, prevents curling |
| Hot spot (overhang with heat buildup) | Reduce speed + increase fan + add short dwell | Allows solidification before next layer |
| Cold spot (large flat area, too cool) | Reduce fan, increase speed slightly | Maintains interlayer adhesion temp |
| Thermal gradient (one side hot, other cold) | Adjust print order within layer to equalize | Print hot side last (more cooling time) |
| Bed-level thermal variation | Adjust first-layer speed spatially | Compensate for cold corners / hot center |

### Inference runtime options

**Option A: ONNX Runtime (recommended for MVP)**
- Export trained PyTorch model to ONNX
- Use `ort` crate (Rust ONNX Runtime bindings) for inference
- CPU inference: ~50-200ms for typical part
- GPU inference: ~5-20ms (if available)
- WASM compatible via ONNX.js (browser slicing with thermal prediction)

**Option B: Burn (pure Rust ML framework)**
- Load model weights into `burn` framework
- Pure Rust, no external runtime dependency
- Slower than ONNX Runtime but fully self-contained
- Better for distribution (single binary)

**Option C: External service**
- Run inference via HTTP to a Python/PyTorch server
- Best for SaaS (GPU inference on server)
- Not suitable for CLI/offline use

### Simpler alternatives (no ML)

If ML is too heavy for v1, heuristic thermal estimation can still help:

1. **Minimum layer time enforcement**: Already common in slicers — if a layer completes too fast, slow down to allow cooling. But this is global, not spatial.

2. **Feature-aware cooling**: Detect thin features (from perimeter width or island size) and increase cooling / decrease speed locally. No thermal simulation needed.

3. **Analytical thermal model**: Simplified 1D heat equation along the Z-axis per region. Much faster than FDM simulation, captures the main effects (small features cool slowly due to low thermal mass).

## Implementation phases

1. **Phase A: Heuristic thermal awareness** (no ML)
   - Detect thin features and small islands per layer
   - Apply conservative speed/fan adjustments
   - Effort: Medium. Immediate quality improvement.

2. **Phase B: Analytical thermal model**
   - 1D heat equation per region for approximate temperature history
   - Generate thermal maps (visualization) alongside G-code
   - Effort: Medium-high. Good accuracy for simple geometries.

3. **Phase C: 3D U-Net integration**
   - Train model on simulation data (separate Python project)
   - Integrate via ONNX Runtime for inference
   - Full per-voxel thermal field → spatially adaptive G-code
   - Effort: Very high (training data generation + model training + integration)

4. **Phase D: Thermal visualization**
   - Render thermal map overlaid on model (heat map colors)
   - `slicecore thermal model.stl --config profile.toml --visualize`
   - Export as colored 3MF or standalone HTML viewer
   - Effort: Medium (extends render crate)

## Dependencies

- **Phase 26 (Render)**: ✓ Visualization of thermal maps
- **ONNX Runtime**: `ort` crate for ML inference
- **Training pipeline**: Separate Python/PyTorch project for model training
- **Per-region speed control**: Engine must support spatially varying speed/fan within a layer (currently layer-level only)

## Research references

- Yavari et al., "Thermal Modeling in Metal AM Using 3D CNNs" (2021) — U-Net for thermal field prediction
- Balta et al., "Machine Learning Thermal History Prediction in L-PBF" (2023) — surrogate model approach
- Li et al., "Real-time Thermal Simulation for FDM" (2020) — adaptive cooling from thermal prediction
