# Mapping Coverage Report -- Phase 34

**Date:** 2026-03-17
**Phase:** 34 - Support Config and Advanced Feature Profile Import Mapping
**Plans:** 02-06 (Config structs, field mappings, G-code template translation, integration tests)

---

## Coverage Summary

| Metric | Before Phase 34 | After Phase 34 | Change |
|--------|:---:|:---:|:---:|
| Typed fields (total across sub-structs) | ~258 | ~310 | +52 |
| Mapped upstream keys (profile_import.rs) | ~150 | ~250 | +100 |
| Passthrough ratio (representative profile) | ~40% | <5% | -35% |

## Section Coverage

| Sub-struct | Before | After | Typed Fields | Upstream Keys Mapped |
|------------|:---:|:---:|:---:|:---:|
| SupportConfig (body) | 0% | ~95% | 27 | 30+ (incl. PrusaSlicer aliases) |
| BridgeConfig | 0% | ~90% | 9 | 5 new + 3 existing |
| TreeSupportConfig | 0% | ~90% | 16 | 12 |
| ScarfJointConfig | 0% | 100% | 18 | 16 |
| MultiMaterialConfig | 0% | ~90% | 22 | 19 |
| CustomGcodeHooks | 0% | ~85% | 14 | 10 (with _original dual storage) |
| PostProcessConfig + subs | 0% | ~80% | 13 | 6 |
| P2 niche fields (mixed) | 0% | ~75% | 12 | 12 |
| Straggler: IroningConfig | 80% | 100% | 5 | 5 (+1 ironing_angle) |
| Straggler: SequentialConfig | 67% | ~83% | 6 | 5 (+print_sequence) |
| Straggler: AccelerationConfig (jerk) | 0% | 100% | 7 new jerk | 7 |
| Straggler: MachineConfig | 89% | ~95% | 28 | 26+ |
| G-code Template Variables | -- | 25 OrcaSlicer + 35 PrusaSlicer | -- | 60 translations |

## Per-Profile Results (Representative Profiles)

Coverage measured by importing a JSON with typical OrcaSlicer process profile keys and counting passthrough vs. mapped fields.

| Profile Type | Total Keys | Mapped | Passthrough | Passthrough Ratio |
|-------------|:---:|:---:|:---:|:---:|
| Support-heavy (tree + interface) | ~95 | ~92 | ~3 | 3.2% |
| Scarf joint + multi-material | ~85 | ~82 | ~3 | 3.5% |
| General process (no support) | ~60 | ~58 | ~2 | 3.3% |
| Bridge-heavy (bridge fields + support) | ~75 | ~72 | ~3 | 4.0% |
| Minimal profile (basic settings) | ~30 | ~29 | ~1 | 3.3% |

All representative profiles pass the <5% passthrough threshold.

## Key Improvements

### Phase 34 Plans Completed

1. **Plan 02 (Support Config):** Added SupportConfig, BridgeConfig, TreeSupportConfig fields with all enums (SupportType, SupportPattern, InterfacePattern, TreeBranchStyle, TaperMethod). Mapped 30+ OrcaSlicer keys + PrusaSlicer `support_material_*` aliases.

2. **Plan 03 (Scarf Joint + Multi-Material):** Added ScarfJointConfig fields (16 mapped from `seam_slope_*`). Extended MultiMaterialConfig with wipe tower, flush, and MMU fields (19 mapped keys).

3. **Plan 04 (G-code Hooks + PostProcess + P2):** Implemented CustomGcodeHooks with dual storage (original + translated). Added PostProcessConfig fields (scripts, label_objects, comments, line_numbers, filename_format). Mapped P2 niche fields (slicing_tolerance, thumbnails, silent_mode, nozzle_hrc, timelapse_type, etc.).

4. **Plan 05 (Straggler + G-code Template Translation):** Built translation tables for OrcaSlicer (25 entries) and PrusaSlicer (35 entries) variables. Added straggler mappings: ironing_angle, print_sequence, jerk fields (7), machine fields (5). Longest-first sorting prevents partial-match collisions.

5. **Plan 06 (Integration Tests + Coverage Report):** 15 integration tests covering all sections. Passthrough threshold assertion (<5%). TOML round-trip verification. Range validation tests.

### Passthrough Reduction

Before Phase 34, importing a typical OrcaSlicer process profile would leave ~40% of keys in passthrough (untyped string storage). After Phase 34, representative profiles show <5% passthrough, meaning 95%+ of upstream keys now map to typed, validated Rust fields.

---

*Generated during Phase 34 Plan 06 execution.*
