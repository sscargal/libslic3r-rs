---
created: 2026-03-16T19:40:00.000Z
title: Sustainability — carbon footprint and material waste reduction
area: engine
files:
  - crates/slicecore-engine/src/cost_model.rs
  - crates/slicecore-engine/src/estimation.rs
  - crates/slicecore-engine/src/statistics.rs
  - crates/slicecore-cli/src/analysis_display.rs
---

## Problem

3D printing has a significant environmental footprint that users and organizations increasingly care about:

- **34% material waste rate** from failed prints (industry estimate)
- **Energy consumption**: A typical FDM printer uses 50-200W for hours; a farm of 20 printers runs 24/7
- **Material sourcing**: Petrochemical-based filaments (ABS, Nylon) vs. bio-based (PLA) have vastly different environmental impacts
- **No visibility**: Users have no data on the environmental cost of their printing — no carbon footprint estimate, no waste tracking, no energy monitoring

Regulatory pressure is growing: EU sustainability reporting, corporate ESG requirements, and institutional procurement rules increasingly require environmental impact data for manufacturing processes.

## Solution

### Feature 1: Per-Print Carbon Footprint Estimation

Extend the existing cost model (Phase 31) with environmental metrics:

```bash
slicecore analyze-gcode model.gcode --sustainability
# ┌─────────────────────────────────────┐
# │ Sustainability Report               │
# ├─────────────────────────────────────┤
# │ Material: PLA (bio-based)           │
# │ Weight: 47.2g                       │
# │                                     │
# │ Carbon Footprint:                   │
# │   Material production: 0.14 kg CO₂e │
# │   Electricity (printing): 0.08 kg   │
# │   Total: 0.22 kg CO₂e              │
# │                                     │
# │ Energy:                             │
# │   Print time: 2h 14m               │
# │   Estimated energy: 0.28 kWh       │
# │   Grid carbon intensity: 0.29 kg/kWh│
# │                                     │
# │ Material efficiency:                │
# │   Model volume: 12.3 cm³           │
# │   Support waste: 3.1g (6.6%)       │
# │   Purge waste: 0g (single material)│
# │   Infill ratio: 20%                │
# │   Solid equivalent: 63.2g          │
# │   Lightweighting: 25% saved        │
# └─────────────────────────────────────┘
```

**Data sources for carbon factors:**

| Material | Production CO₂e (kg/kg) | Source | End-of-life |
|----------|------------------------|--------|-------------|
| PLA | 3.0 | Bio-based (corn starch) | Compostable (industrial) |
| PETG | 4.5 | Petrochemical | Recyclable (#1 PET family) |
| ABS | 5.5 | Petrochemical | Recyclable (limited) |
| ASA | 5.8 | Petrochemical | Not widely recyclable |
| Nylon/PA | 8.5 | Petrochemical | Recyclable (limited) |
| TPU | 6.0 | Petrochemical | Not recyclable |
| Recycled PLA | 1.5 | Recycled feedstock | Compostable |
| Recycled PETG | 2.0 | Recycled feedstock | Recyclable |

**Electricity carbon intensity**: Configurable per region (e.g., 0.05 kg/kWh in Norway, 0.50 kg/kWh in Poland). Default from user's country setting or manual input.

### Feature 2: AI-Driven Waste Reduction

Leverage AI to reduce the 34% failure waste rate:

1. **Pre-print failure prediction** (ties to AI todos): Identify prints likely to fail before they start → don't waste material
2. **Optimal orientation for minimal support**: Auto-orient to minimize support material (Phase 27 already does this)
3. **Infill optimization**: AI suggests minimum infill density that meets strength requirements (ties to generative design todo)
4. **Batch printing efficiency**: Group multiple small parts on one plate to reduce per-part energy overhead

### Feature 3: Recycled Filament Profiles

Support profiles for recycled filaments with adjusted parameters:

```toml
[material]
type = "PLA"
source = "recycled"         # "virgin" | "recycled" | "bio-based" | "composite"
recycled_content_pct = 100  # Percentage of recycled content
carbon_factor_kg_co2e = 1.5 # Lifecycle carbon factor

[material.adjustments]
# Recycled filaments often need different settings
temperature_offset = +5     # Slightly higher temp for recycled
flow_multiplier = 1.02      # Slightly more flow (diameter variation)
max_speed_reduction = 0.9   # 10% slower for consistency
notes = "Recycled PLA may have slight color variation and reduced bridging performance"
```

### Feature 4: Sustainability Comparison

```bash
# Compare environmental impact of different approaches
slicecore analyze-gcode model.gcode --sustainability --compare-material PLA PETG "Recycled-PLA"
# Material Comparison:
#                  PLA      PETG     Recycled-PLA
# Weight (g):      47.2     47.2     47.2
# CO₂e (kg):       0.22     0.29     0.15
# Recyclable:      ◆        ✓        ◆
# Compostable:     ✓        ✗        ✓
# Cost ($):        0.94     1.42     0.85
#
# → Recycled-PLA saves 32% CO₂e and 10% cost vs. virgin PLA
```

### Feature 5: Sustainability Reporting for Organizations

For print farms and enterprises that need ESG reporting:

```bash
# Generate sustainability report for a time period
slicecore report sustainability --from 2026-01-01 --to 2026-03-31
# Q1 2026 Sustainability Report
# ─────────────────────────────
# Total prints: 1,247
# Material consumed: 23.4 kg
#   PLA: 15.2 kg (65%)
#   PETG: 6.8 kg (29%)
#   Recycled PLA: 1.4 kg (6%)
# Total CO₂e: 87.3 kg
# Energy consumed: 312 kWh
# Failed prints: 89 (7.1%) — 2.8 kg wasted
# Support waste: 1.9 kg (8.1%)
#
# vs. Previous quarter:
#   CO₂e: -12% (improved failure rate)
#   Waste: -18% (better support optimization)
```

This requires the job directory system (todo) to track historical print data.

## Implementation phases

1. **Phase A: Carbon footprint per print** — extend cost_model.rs with CO₂e calculation using material carbon factors + electricity estimate. Add `--sustainability` flag to `analyze-gcode`.
2. **Phase B: Material sustainability metadata** — add `source`, `recycled_content_pct`, `carbon_factor` to filament profiles. Create recycled filament profiles.
3. **Phase C: Sustainability comparison** — compare materials on environmental metrics alongside cost.
4. **Phase D: Waste tracking** — integrate with job directories to track cumulative material usage, waste, and failure rate.
5. **Phase E: Organizational reporting** — aggregate sustainability data for ESG/compliance reporting.

## Dependencies

- **Phase 31 (Cost model)**: ✓ Foundation for adding environmental metrics
- **Phase 19 (Statistics)**: ✓ Per-feature breakdown for material efficiency analysis
- **Job output directories** (todo): Needed for historical waste tracking
- **Material database** (todo): Carbon factors per material
- **AI failure prediction** (todo): Key driver for waste reduction
