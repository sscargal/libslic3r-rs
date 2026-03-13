---
phase: quick-4
plan: 1
subsystem: config
tags: [audit, parity, planning, config]
dependency_graph:
  requires: []
  provides: [config-parity-audit, phase-recommendations]
  affects: [config.rs, profile_import.rs]
tech_stack:
  added: []
  patterns: []
key_files:
  created:
    - designDocs/CONFIG_PARITY_AUDIT.md
  modified: []
decisions:
  - Categorized missing fields into P0/P1/P2 based on print quality impact
  - Recommended 4 phases (30-33) for systematic gap closure
  - Proposed proc-macro approach for ConfigSchema system
metrics:
  duration: 3min
  completed: "2026-03-13T21:08:33Z"
  tasks_completed: 1
  tasks_total: 1
---

# Quick Task 4: Config Parity Audit Summary

Comprehensive audit of PrintConfig fields vs OrcaSlicer/BambuStudio/PrusaSlicer with gap analysis and phase recommendations for systematic closure.

## What Was Done

### Task 1: Audit current PrintConfig fields and upstream mapping coverage

Created `designDocs/CONFIG_PARITY_AUDIT.md` (721 lines) containing:

1. **Executive Summary**: 258 typed fields across 16 sub-structs, ~120 upstream keys mapped, ~60-65% coverage estimate

2. **Complete Field Inventory**: Every field in PrintConfig and all sub-structs (LineWidthConfig, SpeedConfig, CoolingConfig, RetractionConfig, MachineConfig, AccelerationConfig, FilamentPropsConfig, SupportConfig, IroningConfig, ScarfJointConfig, MultiMaterialConfig, SequentialConfig, PerFeatureFlow, CustomGcodeHooks, PostProcessConfig) with upstream mapping status

3. **Gap Analysis (65+ fields)**:
   - P0 (15 fields): chamber_temperature, xy_hole/contour_compensation, top/bottom_surface_pattern, internal bridge settings, filament_shrink, z_offset, bed_type
   - P1 (30 fields): input shaping, advanced fan, fuzzy skin, brim improvements, draft shield, support interface filament selection
   - P2 (20 fields): timelapse types, thumbnails array, silent mode, AMS-specific fields

4. **Mapping Coverage Statistics**: Per-sub-struct breakdown showing 100% coverage for speeds/acceleration/line widths but 0% for support/scarf/multi-material import

5. **Phase Recommendations**:
   - Phase 30: P0 field gap closure (3-4 plans)
   - Phase 31: P1 field gap closure (4-5 plans)
   - Phase 32: Support/scarf/multi-material profile import (2-3 plans)
   - Phase 33: ConfigSchema system with proc-macro (5-6 plans)

6. **ConfigSchema Design Notes**: Proc-macro approach, runtime registry, progressive disclosure tiers, JSON Schema output

7. **Priority Matrix**: Recommended execution order with effort/impact analysis

## Deviations from Plan

None -- plan executed exactly as written.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 9eb8f61 | Config parity audit document |

## Self-Check

- [x] designDocs/CONFIG_PARITY_AUDIT.md exists (721 lines, PASS)
- [x] All PrintConfig sub-structs inventoried (16 sub-structs)
- [x] Gaps categorized P0/P1/P2 (65+ fields)
- [x] Phase recommendations with scope estimates (4 phases, ~18 plans)
- [x] ConfigSchema system design outlined (Section 6)
