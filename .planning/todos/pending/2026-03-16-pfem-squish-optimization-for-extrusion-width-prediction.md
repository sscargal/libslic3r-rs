---
created: 2026-03-16T19:40:00.000Z
title: PFEM squish optimization for extrusion width prediction
area: engine
files:
  - crates/slicecore-engine/src/extrusion.rs
  - crates/slicecore-engine/src/arachne.rs
  - crates/slicecore-engine/src/config.rs
---

## Problem

Current slicers treat extruded lines as simple rectangles with nominal width = nozzle diameter (or configured line width). In reality, the extruded filament deforms under:

- **Squish**: Nozzle presses the molten filament against the previous layer, spreading it wider than the nozzle diameter
- **Gravity/weight**: Upper layers compress lower layers (significant for tall thin walls)
- **Viscosity**: Material flow behavior depends on temperature, speed, and material type
- **Flow velocity**: Higher speeds produce thinner, less squished lines

This mismatch between assumed and actual line width causes:
- Dimensional inaccuracy (parts are wider/narrower than modeled)
- Over/under-extrusion artifacts
- Poor surface finish from inconsistent line overlap
- Elephant's foot (first layers spread more due to bed squish)

The Particle Finite Element Method (PFEM) models free-surface fluid flow and can predict the actual cross-section shape of an extruded line under real conditions.

## Solution

### Level 1: Empirical squish model (no PFEM)

Simple analytical model of line width vs. parameters:

```
actual_width = f(nozzle_diameter, layer_height, flow_rate, speed, temperature, material_viscosity)
```

**Empirical formula** (validated against experimental data):
```
W_actual = W_nozzle + k * (layer_height / W_nozzle) * (flow_rate / speed)
```
Where `k` is a material-dependent constant (calibratable per filament).

**Implementation**:
1. Add `actual_line_width()` function to `extrusion.rs`
2. Use actual (not nominal) width for:
   - Perimeter offset calculations → better dimensional accuracy
   - Extrusion amount calculations → correct flow
   - Gap fill detection → accurate thin-wall handling
   - Arachne variable-width → more accurate medial axis widths
3. Calibratable via `slicecore calibrate flow` (Phase 31 already measures actual wall width)

### Level 2: Physics-based squish simulation

Model the extrusion cross-section using simplified 2D fluid dynamics:

**Inputs**: Nozzle geometry (diameter, taper), layer height, extrusion rate, material viscosity (from temperature), bed/previous-layer surface

**Simulation**: 2D cross-section of the extruded bead:
```
  Nozzle (pushing down)
  ══════════════════
       ╭────────╮        ← Actual bead shape
      ╱          ╲       (wider than nozzle due to squish)
  ───╱────────────╲───   ← Previous layer / bed
```

Solve Stokes flow (low Reynolds number) for the bead shape:
- Material exits nozzle at flow velocity
- Spreads laterally as nozzle squishes it against substrate
- Width, height, and contact angle depend on viscosity + speed + gap

**Output**: Lookup table of `(speed, temperature, layer_height) → (actual_width, actual_height, contact_angle)` for each material.

### Level 3: PFEM for full deformation prediction

**Particle Finite Element Method**:
- Mesh-free method that handles free-surface flows naturally
- Models the entire extrusion process: material exiting nozzle → depositing → cooling → deforming under load
- Predicts not just width but:
  - Cross-section shape (rounded vs. flat top)
  - Layer bonding interface area (affects adhesion strength)
  - Residual stress from cooling (predicts warping tendency)
  - Deformation under subsequent layer weight

**Integration approach**: Pre-compute PFEM results into lookup tables indexed by operating parameters. Runtime cost is then a table lookup, not a simulation.

**Training simulations**: Run PFEM for a grid of parameter combinations:
- 5 materials × 10 speeds × 10 temperatures × 5 layer heights × 3 nozzle sizes = 7,500 simulations
- Each simulation takes seconds (2D cross-section only)
- Store results as interpolatable lookup tables per material

### Applications in the slicer

| Feature | Without squish model | With squish model |
|---------|---------------------|-------------------|
| Dimensional accuracy | ±0.2mm typical | ±0.05mm achievable |
| Elephant's foot | Compensated by global offset | Compensated per-layer based on actual squish |
| Thin wall quality | Gap-fill guesses at widths | Accurate variable-width from real bead geometry |
| Over-extrusion detection | Flow rate mismatch | Predicted width vs. intended width comparison |
| First layer calibration | Trial-and-error Z offset | Calculated from squish model + material properties |
| Arachne accuracy | Nominal widths in medial axis | Actual widths → better gap fill |

### Industrial applications

For specialized printing (concrete 3D printing, bioprinting, high-viscosity materials), PFEM squish prediction is not optional — it's essential. These applications have:
- Much higher layer heights (1-10mm)
- Non-Newtonian materials (viscosity changes with shear rate)
- Significant deformation under self-weight
- Tight tolerance requirements

Supporting these use cases makes slicecore viable for industrial/research applications beyond hobby FDM.

## Implementation phases

1. **Phase A: Empirical squish model** — analytical formula with calibratable constant per material. Wire into extrusion width calculations. Immediate dimensional accuracy improvement.
2. **Phase B: First-layer squish compensation** — predict actual first-layer width from Z-offset + material properties → auto-adjust flow and offset. Eliminates elephant's foot without trial-and-error.
3. **Phase C: Lookup table generation** — run simplified 2D fluid sim for common materials → ship pre-computed tables with the slicer.
4. **Phase D: PFEM integration** — full cross-section prediction for custom/industrial materials. Either embedded solver or external service.

## Dependencies

- **Phase 31 (Calibration)**: ✓ Flow calibration generates real-world width data for model validation
- **Material database** (todo): Viscosity curves per material needed for physics-based models
- **Arachne** (existing): Variable-width perimeters benefit most from accurate width prediction
