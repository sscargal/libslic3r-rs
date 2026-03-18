---
created: 2026-03-16T19:32:48.250Z
title: Multi-plate arrange by color for minimal filament swaps
area: engine
files:
  - crates/slicecore-engine/src/planner.rs
  - crates/slicecore-engine/src/sequential.rs
  - crates/slicecore-engine/src/multimaterial.rs
---

## Problem

When printing a multi-part project with different colored parts (e.g., a board game with red, white, and blue pieces), users must currently either:

1. **Print all colors on every plate** using an AMS/MMU: Frequent filament swaps per layer, slow purge towers, wasted filament on color transitions
2. **Manually sort parts by color**: Tedious, error-prone, and no slicer automates this

The optimal approach for print farms and single-material printers: group all same-colored parts onto the same build plate. Print all white parts → swap filament → print all red parts → swap → print all blue parts. This minimizes human intervention to one filament change per color instead of hundreds of in-print swaps.

Even for AMS/MMU users, grouping by color eliminates purge towers entirely (single-color plates need zero purging), saving significant material and time.

## Solution

### Core algorithm: Color-aware plate packing

```
Input:  N parts, each with an assigned color/material
Output: M build plates, each containing only parts of one color,
        packed to maximize bed utilization

1. Group parts by color/material
2. For each color group:
   a. Sort parts by footprint area (largest first — better packing)
   b. Bin-pack onto plates using arrangement algorithm (Phase 27)
   c. Respect build volume, sequential clearance, and plate constraints
3. Output ordered plate sequence: all White plates → all Red plates → ...
4. Optimize color order to minimize total swap count
```

### Configuration

```toml
[arrangement]
multi_plate_mode = "by_color"   # "mixed" | "by_color" | "by_material" | "manual"
plate_fill_target = 0.75        # Target bed utilization before starting new plate
color_order = "auto"            # "auto" | "manual" | ["White", "Red", "Blue"]

# For AMS/MMU users who want to minimize swaps but still mix colors
ams_mode = "minimize_swaps"     # "minimize_swaps" | "group_by_color" | "no_optimization"
```

### CLI workflow

```bash
# Import multi-part project with color assignments
slicecore arrange project.3mf --by-color
# → Plate 1: 8x White pieces (bed 92% full)
# → Plate 2: 5x White pieces + 3x White pieces (bed 78% full)
# → Plate 3: 6x Red pieces (bed 85% full)
# → Plate 4: 4x Blue pieces (bed 67% full)
# → Total: 4 plates, 2 filament swaps

# Slice all plates
slicecore slice --plates all --output plates/
# → plates/plate-01-white.gcode
# → plates/plate-02-white.gcode
# → plates/plate-03-red.gcode
# → plates/plate-04-blue.gcode

# Compare with mixed arrangement
slicecore arrange project.3mf --mixed
# → Plate 1: mixed colors (needs AMS, 12 color changes per layer, 45g purge waste)
# → Plate 2: mixed colors (needs AMS, 8 color changes per layer, 32g purge waste)
```

### Grouping strategies

| Strategy | How it works | Best for |
|----------|-------------|----------|
| **By color** | Group parts with identical color | Single-extruder printers, print farms |
| **By material** | Group parts with same material type (PLA, PETG) regardless of color | Mixed-material projects |
| **By color + material** | Group by exact filament match (e.g., "eSun PLA Red") | When exact spool matters |
| **Minimize swaps** | Allow color mixing but optimize part placement to minimize per-layer swaps | AMS/MMU users |
| **Hybrid** | Group by color where possible, mix remaining parts to fill plates | Balance utilization + swaps |

### Advanced: Swap-minimized mixed plates

For AMS/MMU printers that can handle multiple colors, the optimizer can still reduce swaps by co-locating parts that share colors:

```
Plate 1: 4 White parts + 2 parts that are mostly White with Red accents
  → Only 2 colors on this plate → minimal purging
Plate 2: 3 Red parts + 1 part that is Red with Blue trim
  → Only 2 colors → minimal purging
```

This is a bin-packing problem with a secondary objective: minimize the number of distinct colors per plate.

### Integration with existing systems

- **Phase 27 (Auto-arrangement)**: Provides the 2D bin-packing algorithm — this feature adds color-awareness as a grouping pre-pass
- **Multimaterial system** (`multimaterial.rs`): Handles per-part material assignment — this feature reads those assignments for grouping
- **Batch slicing** (todo): Color-grouped plates feed naturally into batch slicing workflow
- **Spool memory** (todo): Can check if user has enough of each color spool before committing to the plate plan

### Print farm optimization

For farms with multiple printers:

```bash
slicecore arrange project.3mf --by-color --printers 3
# → Printer 1: Plates 1-2 (White) — no swap needed
# → Printer 2: Plate 3 (Red)
# → Printer 3: Plate 4 (Blue)
# → All colors printing simultaneously, zero filament swaps total
```

## Dependencies

- **Phase 27 (Auto-arrangement)**: ✓ 2D bin-packing algorithm
- **Multimaterial** (existing): Part-to-material/color assignment
- **Batch slicing** (todo): Multi-plate output workflow
- **Spool memory** (todo): Filament quantity validation

## Phased implementation

1. **Phase A**: Color grouping — group parts by color, pack each group onto plates
2. **Phase B**: Color-ordered output — sequence plates to minimize filament swaps
3. **Phase C**: Utilization optimizer — fill partial plates with same-color parts from overflow
4. **Phase D**: AMS/MMU swap minimization — mixed plates with minimal distinct colors
5. **Phase E**: Multi-printer farm distribution — assign plates to printers by color
