---
created: 2026-03-16T19:25:00.000Z
title: Feature-specific multi-filament selection per print feature
area: engine
files:
  - crates/slicecore-engine/src/multimaterial.rs
  - crates/slicecore-engine/src/extrusion.rs
  - crates/slicecore-engine/src/config.rs
  - crates/slicecore-engine/src/gcode_gen.rs
---

## Problem

Current multi-material in slicers is primarily color-based: different regions of a model use different extruders/filaments for visual effect. Users increasingly want **functional** multi-material, where the filament choice is driven by the print feature, not the geometry:

- **Outer walls**: Expensive CF-PETG for strength and surface finish
- **Inner walls**: Standard PETG for bulk structure (cheaper)
- **Infill**: Cheapest available PLA for filler (since infill is internal and not load-bearing)
- **Supports**: Soluble PVA or breakaway material for easy removal
- **Top/bottom surfaces**: High-quality material for visible surfaces
- **Brim/skirt**: Cheapest material (it's waste anyway)

This isn't just about cost savings — it's about material properties. CF-PETG outer walls resist abrasion while cheap PLA infill saves 60% on material cost. TPU inner walls provide vibration dampening while rigid PETG outer walls maintain dimensional accuracy.

## Solution

### Feature-to-Filament Mapping

```toml
[multi_material]
mode = "per_feature"  # "color" | "per_feature" | "both"

[multi_material.feature_map]
outer_wall = "T0"      # Extruder 0: CF-PETG
inner_wall = "T0"      # Same as outer for structural continuity
infill = "T1"          # Extruder 1: Cheap PLA
top_surface = "T0"     # CF-PETG for visible surface
bottom_surface = "T1"  # PLA (not visible)
support = "T2"         # Extruder 2: PVA soluble
support_interface = "T2"
brim = "T1"            # Cheapest material
ironing = "T0"         # Outer material for surface finish
gap_fill = "T0"        # Match wall material
```

### Purge and Transition Management

The core challenge: material changes within a single layer are expensive. Each transition requires purging the old material from the nozzle — wasting filament and time.

**Purge optimization strategies:**

1. **Minimize transitions per layer**: Group features by extruder. Print all T0 features first (outer walls, top surfaces), then all T1 features (infill, bottom surfaces), then T2 (support). This reduces transitions from potentially dozens per layer to 2-3.

2. **Purge-to-infill**: Instead of purging into a waste tower, purge directly into infill. Since infill is internal, color contamination doesn't matter. This saves both time and material.
   ```
   Instead of:  T0→purge_tower→T1→infill
   Do:          T0→T1→infill (first N mm of infill serves as purge)
   ```

3. **Transition tower optimization**: When purge-to-infill isn't enough, size the transition tower per material pair. Some transitions need more purge (dark→light) than others (light→dark).

4. **Smart transition ordering**: If layer needs T0, T1, T2:
   - T0→T1 requires 20mm purge (similar materials)
   - T1→T2 requires 40mm purge (PLA→PVA, very different)
   - T0→T2 requires 35mm purge
   - Optimal order: T0→T1→T2 (total 60mm) vs T0→T2→T1 (total 75mm)
   - This is a shortest-Hamiltonian-path problem over the purge-distance graph.

5. **Avoid unnecessary transitions**: If outer wall and gap fill both use T0, schedule them adjacent — no transition needed between them.

### Purge volume matrix

```toml
# Purge volume (mm³) required when transitioning between extruders
# Asymmetric: dark→light needs more purge than light→dark
[purge_matrix]
#        To T0   To T1   To T2
from_T0 = [  0,    45,    60 ]   # CF-PETG → PLA = 45mm³, → PVA = 60mm³
from_T1 = [ 30,     0,    50 ]   # PLA → CF-PETG = 30mm³ (light to dark, less purge)
from_T2 = [ 55,    40,     0 ]   # PVA → anything needs more purge (sticky)
```

### Cost analysis output

The CLI should show the cost breakdown per material:

```
Material Usage Breakdown:
  T0 (CF-PETG):  23.4g  ($1.87) — outer walls, top surfaces, gap fill
  T1 (PLA):      47.2g  ($0.94) — infill, bottom surfaces, brim
  T2 (PVA):       8.1g  ($0.97) — support, support interface
  Purge waste:   12.3g  ($0.62) — transition material
  ──────────────────────────────
  Total:         91.0g  ($4.40)

  vs. single-material CF-PETG: 78.7g ($6.30)
  Savings: $1.90 (30%) with per-feature multi-material
```

### Compatibility matrix

| Feature | Single extruder | Dual extruder | AMS/CFS (4+) | IDEX |
|---------|----------------|---------------|---------------|------|
| Per-feature filament | N/A | 2 materials | Full | Full |
| Purge-to-infill | N/A | Yes | Yes | Yes (no purge between heads) |
| Transition tower | N/A | Yes | Yes | Not needed (independent heads) |
| Material compatibility | N/A | Must check | Must check | Any combo |

**IDEX advantage**: Dual independent extruders don't need purging when switching — each head is always primed. Per-feature multi-material on IDEX is essentially free (no waste).

### Material compatibility checking

Not all material combinations are safe:
- PLA outer + ABS infill: different bed temps, will fail
- PETG + PLA: different temp ranges but can coexist with careful transitions
- TPU + PLA: TPU needs very different retraction settings

The slicer should warn or block incompatible combinations:
```
⚠ Warning: T0 (PETG, 240°C) and T1 (PLA, 210°C) have a 30°C temp gap.
  Outer wall adhesion to infill may be weak. Consider using PETG for both
  walls and infill, or enable "interface layers" between materials.
```

## Implementation phases

1. **Phase A**: Feature-to-extruder mapping in config + G-code gen assigns correct tool per feature
2. **Phase B**: Transition ordering optimization (minimize total purge volume per layer)
3. **Phase C**: Purge-to-infill (use infill extrusion as purge, eliminate waste tower where possible)
4. **Phase D**: Cost analysis with per-material breakdown
5. **Phase E**: Material compatibility checking and warnings

## Dependencies

- **Phase 6 (Multi-material)**: ✓ Basic multi-material toolpath generation exists
- **Spool memory** (todo): Per-spool calibration enables material-specific tuning
- **AMS filament mapping** (todo): Auto-resolve AMS slots to user profiles
- **Cost estimation** (Phase 31): ✓ Cost model exists — extend with per-material breakdown
