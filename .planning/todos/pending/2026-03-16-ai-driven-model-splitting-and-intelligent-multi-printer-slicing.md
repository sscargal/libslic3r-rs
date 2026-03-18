---
created: 2026-03-16T18:40:00.000Z
title: AI-driven model splitting and intelligent multi-printer slicing
area: engine
files:
  - crates/slicecore-ai/src/lib.rs
  - crates/slicecore-mesh/src/csg/split.rs
  - crates/slicecore-engine/src/engine.rs
  - crates/slicecore-arrange/src/lib.rs
---

## Problem

When a model exceeds the build volume, users must manually split it in CAD, choose joint types, orient parts, and manage multi-plate printing. This is tedious, error-prone, and requires expertise. No slicer today does this intelligently end-to-end.

The vision: give the slicer an oversized model + a printer (or fleet of printers) and it automatically:
1. Analyzes the model for optimal split planes
2. Chooses appropriate joint types per split (dovetail, pin/peg, snap-fit, lap joint, tongue-and-groove — not limited to one type)
3. Orients each resulting part for optimal print quality, strength, and time
4. Arranges parts across one or more plates
5. Slices everything with appropriate settings

This is a **major differentiator** — no open-source or commercial slicer does this today.

## Solution

### MVP (v1): Single printer, auto-split

**Inputs:**
- Oversized model (STL/3MF/OBJ)
- Target printer (build volume from profile)
- Target filament (single material)

**Pipeline:**
1. **Fit analysis**: Determine if model fits. If yes, skip splitting.
2. **Split plane detection**: Use AI (LLM) + heuristics to find optimal split planes:
   - Minimize visible seams (split along natural boundaries — flat surfaces, concavities)
   - Maximize structural integrity (avoid splitting across load-bearing cross-sections)
   - Minimize number of parts
   - Ensure each part fits the build volume with margin for joints
3. **Joint selection per split**: AI selects joint type based on:
   - Split plane geometry (flat → dovetail/lap, curved → pin alignment)
   - Load direction at joint (tension → pin, shear → dovetail, compression → flat)
   - Assembly ergonomics (accessible for gluing? snap-fit possible?)
   - Can mix joint types across different splits of the same model
4. **Joint geometry generation**: Use CSG operations (Phase 29) to:
   - Cut model along split planes
   - Add joint geometry (positive on one side, negative on the other)
   - Ensure tight tolerances (material shrinkage-aware from profile)
5. **Orientation optimization**: For each part:
   - Minimize support material
   - Maximize strength along primary load direction
   - Minimize print time
   - Ensure joint surfaces print accurately (avoid overhangs on mating faces)
6. **Plate arrangement**: Use auto-arrangement (Phase 27) to pack parts onto plates
7. **Slice all parts**: Standard slicing pipeline

**AI integration:**
- Uses existing slicecore-ai crate (Phase 8) for LLM reasoning about split planes and joint selection
- Fallback to geometric heuristics when LLM unavailable (offline mode)
- LLM prompt includes: model bounding box, cross-section analysis at candidate planes, material properties, printer capabilities

### v2: Multi-printer fleet

**Additional inputs:**
- Fleet of printers (different makes/models with different build volumes)
- Available filaments per printer

**Additional intelligence:**
- Route parts to printers based on: build volume fit, material requirement, estimated print time (load balance across fleet)
- Example: A full-size character model (1.5m tall) → split into 12 parts → 4 sent to X1C (detail parts: head, hands), 4 to A2 (large flat panels: torso plates), 4 to P2S (structural: legs)
- Structural parts in PETG, cosmetic parts in PLA, flexible gaskets in TPU
- Generate per-printer job packages (ties into job directory and daemon todos)

### v3: Assembly instructions

- Generate assembly guide (SVG/PDF) showing:
  - Part numbering and orientation
  - Joint type per connection
  - Assembly order (dependency graph)
  - Adhesive recommendations per joint type and material
  - Estimated assembly time

### Dependencies

- **Phase 29 (CSG)**: ✓ Already implemented — needed for cutting and joint geometry
- **Phase 27 (Arrangement)**: ✓ Already implemented — needed for plate packing
- **Phase 8 (AI)**: ✓ Already implemented — needed for intelligent decisions
- **Material database** (todo): Needed for shrinkage compensation and joint tolerances
- **Headless daemon** (todo): Needed for fleet job routing in v2
- **Network printer discovery** (todo): Needed for fleet awareness in v2

### Complexity assessment

This is a **milestone-level feature** (v2.0+). Recommend breaking into phases:
1. Research: survey existing split algorithms (Chopper, RevoMaker, BSP-based)
2. Geometric split (no AI): heuristic plane detection + simple pin joints
3. AI-enhanced split: LLM-guided plane selection and joint choice
4. Multi-printer routing
5. Assembly instruction generation
