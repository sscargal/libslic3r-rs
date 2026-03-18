---
created: 2026-03-18T19:57:15.106Z
title: Smart purging volumes for multi-material filament changes
area: engine
files:
  - crates/slicecore-engine/src/multimaterial.rs
  - crates/slicecore-engine/src/gcode_gen.rs
  - crates/slicecore-profiles/src/lib.rs
---

## Problem

When printing with two or more materials, the printer must purge residual filament from the nozzle during filament changes. Existing slicers (PrusaSlicer, OrcaSlicer) use a simple to/from matrix based on material type and color — e.g., switching from dark PLA to light PLA requires more purging than the reverse. This matrix is manually configured and often wastes material because the defaults are conservative.

Key gaps:
- No purging volume system exists yet in libslic3r-rs for multi-material tool changes
- The simple matrix approach is known to over-purge, wasting filament and time
- Users have no way to calibrate purging volumes for their specific filament combinations
- Color distance (dark→light vs light→dark) significantly affects required purge volume but current algorithms are crude

## Solution

Multi-layered approach:

1. **Baseline**: Implement the standard to/from purging volume matrix (material type × color pairs) matching PrusaSlicer behavior. Store in print profiles. Only activate when ≥2 materials are assigned.

2. **Calibration print**: Generate a calibration tower that prints graduated purge volumes for each filament pair. User visually inspects where color contamination disappears and inputs the result, automatically populating the matrix with optimized values.

3. **Advanced algorithms**:
   - Use CIE ΔE color distance (perceptual) instead of simple dark/light heuristics
   - Model pigment concentration decay as exponential washout curve
   - Factor in filament viscosity and temperature differences between materials

4. **AI/autoresearch optimization**:
   - Use the autoresearch/box method approach to efficiently search the purge volume space with minimal calibration prints
   - Bayesian optimization to converge on optimal volumes with few test prints
   - Train on community-shared calibration data for common filament combinations
   - Could predict optimal volumes from filament properties (color, type, brand) without calibration

5. **Waste reduction strategies**:
   - Purge-into-infill to reuse purge material as infill
   - Purge-into-support for functional waste reuse
   - Sparse purging when contamination tolerance is high (internal features)
