---
created: 2026-03-16T17:39:37.411Z
title: Comprehensive printer calibration model catalog
area: calibration
files:
  - crates/slicecore-engine/src/calibrate.rs
  - crates/slicecore-cli/src/calibrate/mod.rs
---

## Problem

Phase 31 implemented 4 calibration types (temp tower, retraction, flow rate, first layer). There is a much larger universe of calibration models and diagnostic prints that a full-featured slicer should support. This todo captures the complete catalog for future phase planning and prioritization.

**Already implemented (Phase 31):** temp tower, retraction test, flow/extrusion multiplier tower, first-layer calibration pattern.

## Calibration Model Catalog

### Category 1: Core Material Calibration (High Priority)

| # | Model | Description |
|---|-------|-------------|
| 1 | Max volumetric flow test | Tower/line pattern ramping speed and flow to find hotend limit; essential for speed profiles |
| 2 | Pressure advance / linear advance tower | Sweeps PA/LA value to remove corner bulging and improve dimensional accuracy |
| 3 | Dimensional accuracy cube (10/20mm XYZ) | Basic scaling check for steps/mm and equal dimensions on X, Y, Z |
| 4 | Wall thickness test | Single/multi-wall models to measure actual vs. intended line widths |
| 5 | Layer adhesion bar / pull test | Bars at various temps/flows to manually break, comparing strength |
| 6 | Parametric material calibration arrays | Matrix plates sweeping two parameters (temp vs PA, temp vs flow, etc.) |

### Category 2: Geometry & Quality Diagnostics

| # | Model | Description |
|---|-------|-------------|
| 7 | Overhang test | Overhangs at increasing angles (30-80deg) to find reliable limits |
| 8 | Bridging test | Bridges of increasing length to tune bridge speed, fan, flow |
| 9 | Tolerance/clearance test | Stepped pin/hole model for minimum reliable clearance |
| 10 | Vertical fine artifacts test | Tall wall with varying speed/accel to detect Z-banding, resonances |
| 11 | Small-feature tower | Tiny columns, thin walls, small gaps for minimum feature size |
| 12 | Cooling/overhang tower | Stepped overhangs with fan-speed changes per angle |
| 13 | Text/emboss/deboss plate | Small text and logos at different sizes/depths for legibility tuning |
| 14 | Top-surface/infill test plate | Different infill patterns underneath to tune top solid layers |

### Category 3: Motion System & Mechanical Diagnostics

| # | Model | Description |
|---|-------|-------------|
| 15 | Resonance/ringing towers | Varying speed or diagonal patterns to locate ringing frequencies |
| 16 | Squareness/orthogonality test | L-shaped or cross to check X/Y square and Z perpendicular |
| 17 | Circularity test | Disks and round holes to detect anisotropy and backlash |
| 18 | Backlash test | Direction-change patterns to reveal lost motion on reversal |
| 19 | Vibration fingerprint plates | Tall thin diagonal walls at high speed for motion tuning |
| 20 | Multi-axis squareness jigs | Large flat cross parts doubling as physical jigs for gantry alignment |
| 21 | Bed-mesh verification pattern | Single-layer grid covering whole bed to verify leveling/mesh compensation |
| 22 | "Bed frame" full-area prints | Models approaching full X/Y extents to reveal frame twist/skew |

### Category 4: Multi-Material & Color

| # | Model | Description |
|---|-------|-------------|
| 23 | Color change tower | Stepped tower to test purge lengths, color bleed on AMS/MMU |
| 24 | Purge volume & waste block tests | Sweep purge volume settings vs. color contamination |
| 25 | Multimaterial alignment test | Interlocking/checkerboard models for toolhead/AMS offset errors |
| 26 | Support-material interface test (dual) | Tune soluble/breakaway support Z-distance and interface thickness |

### Category 5: Support & Bridging Advanced

| # | Model | Description |
|---|-------|-------------|
| 27 | Support interface test | Different support patterns/densities under identical overhangs |
| 28 | Advanced supports & bridging suites | Combined rigs comparing tree vs classic vs organic supports |
| 29 | Hyper-fine organic branch tests | Tiny angled branch structures for brittle filaments and delicate supports |

### Category 6: Benchmark & Stress Tests

| # | Model | Description |
|---|-------|-------------|
| 30 | "All-in-one" printer tests | Multi-feature blocks (overhangs, bridges, pillars, text, fillets, corners) |
| 31 | Benchy-style benchmarks | Fast boats for visual baseline of ringing, overhangs, bridging |
| 32 | Organic form tests | Dragons, miniatures, statues stressing small details and supports |
| 33 | Lattice cube / infill showcase | Open-frame cubes for infill patterns and resonance behavior |
| 34 | Spider/"torture toaster" stress tests | Extreme feature-dense prints pushing all limits |
| 35 | Diagnostics "failure atlas" models | Parts producing distinct labeled issues for defect-to-tuning mapping |

### Category 7: Innovative / UX-Driven Calibration

| # | Model | Description |
|---|-------|-------------|
| 36 | Interactive multi-pass micro-towers | Short 5-10 min towers for iterative single-parameter refinement |
| 37 | Slicer-driven wizard sequences | Linked models printed in sequence, auto-updating material profile |
| 38 | Print-time measurement models | Parts with vernier scales so you read correct settings off the print |
| 39 | Clearance cubes with integrated gauges | Labeled clearance values on tightest freely-moving sections |
| 40 | Application-specific profile packs | Curated test bundles per use case (speed functional, miniatures, TPU/PETG/ASA) |

### Category 8: Sustainability & Physical Profile Library

| # | Model | Description |
|---|-------|-------------|
| 41 | Waste-to-useful hybrid designs | Calibration strips doubling as cable clips, plant markers, sample chips |
| 42 | Maker coins with profile info | Small tokens encoding profile data (material, temp, PA) as physical library |

## Solution

Future phases should prioritize by:
1. **Core material calibration** (Category 1) — highest user impact, builds on Phase 31 infrastructure
2. **Geometry diagnostics** (Category 2) — common user need for quality tuning
3. **Motion diagnostics** (Category 3) — important for speed tuning workflows
4. **Multi-material** (Category 4) — needed when MMU/AMS support matures
5. **Innovative UX** (Category 7) — differentiating features (wizard sequences, parametric arrays)
6. **Benchmarks** (Category 6) — nice-to-have, many users already have STLs for these

Implementation approach: extend `crates/slicecore-engine/src/calibrate.rs` mesh generation and `crates/slicecore-cli/src/calibrate/` command modules following the pattern established in Phase 31.
