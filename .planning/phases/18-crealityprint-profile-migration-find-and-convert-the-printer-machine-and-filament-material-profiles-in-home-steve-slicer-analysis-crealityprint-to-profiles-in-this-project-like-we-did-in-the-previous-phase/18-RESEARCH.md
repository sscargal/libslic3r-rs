# Phase 18: CrealityPrint Profile Migration - Research

**Researched:** 2026-02-19
**Domain:** CrealityPrint JSON profile conversion, batch import, overlap analysis with existing OrcaSlicer/BambuStudio/PrusaSlicer profiles
**Confidence:** HIGH

## Summary

Phase 18 imports CrealityPrint profiles from `/home/steve/slicer-analysis/CrealityPrint/resources/profiles/` into the project's `profiles/` directory. The critical finding is that CrealityPrint uses the **exact same JSON format** as OrcaSlicer and BambuStudio (same directory structure: `vendor/type/profile.json`, same `inherits`/`instantiation` mechanism, same field names). This is expected because CrealityPrint is a fork of OrcaSlicer. The existing `batch_convert_profiles()` function in `profile_library.rs` can process CrealityPrint profiles with **zero code changes**.

CrealityPrint has **3,940 instantiated profiles** across **36 vendors** -- significantly larger than BambuStudio (2,348 profiles, 12 vendors). The dominant vendor is Creality itself (1,519 profiles, 39% of total), reflecting CrealityPrint's primary role as Creality's official slicer. Of these 3,940 profiles: 141 are byte-identical to OrcaSlicer versions, 2,176 share the same filename but have different content, and **1,623 are unique to CrealityPrint** (not found in either OrcaSlicer or BambuStudio by filename). The unique profiles are overwhelmingly Creality-branded (1,469 of 1,623), covering newer models (K2, K2 SE, GS-01/02/03, SPARKX i7, Hi, Ender-3 V4, CFS-C multi-color variants) and extensive filament-per-printer-per-nozzle combinations.

CrealityPrint introduces **18 JSON keys not found in OrcaSlicer**, but none of these map to the existing `PrintConfig` field set. The most common are `epoxy_resin_plate_temp` (1,177 profiles), `cool_cds_fan_start_at_height` (953), and `enable_special_area_additional_cooling_fan` (707). These are CrealityPrint-specific bed/cooling features that will simply be listed as unmapped fields during conversion -- the same behavior as any other unmapped field. Critically, CrealityPrint does **not** use the `include` field that BambuStudio added (0 profiles), making this a cleaner conversion than Phase 17.

**Primary recommendation:** Run the existing `batch_convert_profiles()` with `source_name = "crealityprint"` and `source_dir = /home/steve/slicer-analysis/CrealityPrint/resources/profiles`. This will produce `profiles/crealityprint/` alongside the existing three source directories. The `write_merged_index()` function already handles merging into the existing `index.json`. The only code change needed is to create integration tests. This is a data-only phase with no new pipeline code required.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| (all existing) | - | No new dependencies needed | `batch_convert_profiles()` already handles this exact JSON format |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none needed) | - | - | - |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Separate "crealityprint" source namespace | Merge into existing "orcaslicer" namespace | Keeping separate is better: avoids filename collisions where CrealityPrint has updated versions of 2,176 same-named profiles; preserves attribution; lets users choose which source's version to use |
| Import all 3,940 instantiated profiles | Import only the 1,623 unique-to-CrealityPrint profiles | Importing all is simpler and correct: even the 2,176 "different content" profiles may have Creality-optimized values from official releases; separate namespace handles disambiguation |

## Architecture Patterns

### Recommended Directory Structure
```
profiles/
  index.json                        # Merged index (4 sources)
  orcaslicer/                       # Phase 15 - 6,015 profiles
    BBL/
    Creality/
    ...
  prusaslicer/                      # Phase 16 - 9,241 profiles
    PrusaResearch/
    Creality/
    ...
  bambustudio/                      # Phase 17 - 2,348 profiles
    BBL/
    Creality/
    ...
  crealityprint/                    # Phase 18 - ~3,940 profiles (NEW)
    Creality/                       # 1,519 profiles (largest vendor)
      filament/                     # 1,187 profiles
      machine/                      # 84 profiles
      process/                      # 248 profiles
    BBL/                            # 568 profiles
    Qidi/                           # 256 profiles
    Snapmaker/                      # 231 profiles
    Prusa/                          # 178 profiles
    Elegoo/                         # 178 profiles
    Flashforge/                     # 106 profiles
    Voron/                          # 100 profiles
    OrcaArena/                      # 85 profiles
    ... (36 vendors total)
```

### Pattern 1: Direct Reuse of Existing Pipeline
**What:** The `batch_convert_profiles()` function from Phase 15 handles CrealityPrint JSON identically to OrcaSlicer/BambuStudio JSON -- no code changes needed.
**When to use:** For all CrealityPrint profile conversion.
**Why it works:** CrealityPrint is an OrcaSlicer fork. The profile JSON format is identical:
- Same directory structure: `vendor_dir/type_dir/profile.json`
- Same `"instantiation": "true"/"false"` field for leaf vs base discrimination
- Same `"inherits": "parent_name"` field for single-parent inheritance
- Same field names (OrcaSlicer naming: `nozzle_temperature`, `hot_plate_temp`, `sparse_infill_density`, etc.)
- Same array-wrapped string values
- No `include` field (unlike BambuStudio)

**Execution:**
```bash
# This is all that's needed:
cargo run -p slicecore-cli -- import-profiles \
  --source-dir /home/steve/slicer-analysis/CrealityPrint/resources/profiles \
  --output-dir profiles \
  --source-name crealityprint
```

### Pattern 2: Index Merge (Already Implemented)
**What:** `write_merged_index()` handles adding CrealityPrint entries to the existing `index.json` without losing OrcaSlicer, PrusaSlicer, or BambuStudio entries.
**When to use:** Automatically called by `cmd_import_profiles()` in the CLI.
**Already tested:** Phases 16 and 17 validated this pattern when adding sources alongside existing ones.

### Anti-Patterns to Avoid
- **Modifying `batch_convert_profiles()` for CrealityPrint-specific logic:** The function already handles this format. Do not add CrealityPrint-specific code paths.
- **Importing from `resources/profiles_template/`:** The `profiles_template/Template/` directory contains non-instantiated base templates separate from the main profiles. The batch converter only processes `resources/profiles/` and correctly skips non-instantiated files.
- **Deduplicating against OrcaSlicer:** Do NOT skip profiles that also exist in OrcaSlicer. CrealityPrint may have Creality-optimized values, and users should be able to choose either source's version. The separate namespace handles this cleanly.
- **Treating this as a code phase:** No new Rust code is needed beyond integration tests. This is a data import phase.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON parsing | Custom CrealityPrint parser | Existing `import_upstream_profile()` | CrealityPrint JSON is identical to OrcaSlicer JSON |
| Batch conversion | Custom CrealityPrint converter | Existing `batch_convert_profiles()` | Works as-is for CrealityPrint |
| Index merging | Custom merge logic | Existing `write_merged_index()` | Already handles multi-source merging (tested with 3 sources in Phase 17) |
| Inheritance resolution | Custom CrealityPrint resolver | Existing `resolve_inheritance()` | Same `inherits` mechanism as OrcaSlicer |
| Filename sanitization | Custom sanitizer | Existing `sanitize_filename()` | Same naming conventions as OrcaSlicer |

**Key insight:** This phase requires ZERO new pipeline code. The existing infrastructure built in Phases 13-17 handles CrealityPrint profiles identically to OrcaSlicer profiles. The work is: (1) run the import command, (2) verify the output, (3) create and run integration tests.

## Common Pitfalls

### Pitfall 1: Profile Name Collisions with OrcaSlicer
**What goes wrong:** CrealityPrint and OrcaSlicer share 2,317 profile filenames. If stored in the same directory, they'd overwrite each other.
**Why it happens:** CrealityPrint is an OrcaSlicer fork; all 36 vendors are shared with OrcaSlicer.
**How to avoid:** Use a separate `crealityprint/` namespace in `profiles/`. The existing architecture already supports this -- each source gets its own top-level directory.
**Warning signs:** Profiles from one source disappearing after importing the other source.

### Pitfall 2: Large Profile Count
**What goes wrong:** Adding ~3,940 CrealityPrint profiles to `index.json` (already containing 17,604 entries) grows it to ~21,500 entries.
**Why it happens:** CrealityPrint has extensive per-printer-per-nozzle-per-filament combinations, especially for Creality brand printers.
**How to avoid:** This is expected and acceptable. JSON parsing of ~21,500 entries is still fast (< 100ms). The index was designed for this scale.
**Warning signs:** Noticeable slowdown in `list-profiles` or `search-profiles` commands (unlikely at this scale).

### Pitfall 3: Machine Model Files Without Instantiation
**What goes wrong:** 198 `machine_model` type JSON files exist in machine/ directories (e.g., `Creality K1.json` alongside `Creality K1 0.4 nozzle.json`). These have `"type": "machine_model"` and no `instantiation` field.
**Why it happens:** OrcaSlicer/CrealityPrint stores printer metadata (3D bed model, nozzle sizes) alongside machine profiles.
**How to avoid:** The existing `batch_convert_profiles()` already handles this correctly: files without `"instantiation": "true"` are skipped. The 198 machine_model files + 714 non-instantiated base profiles = ~912 files will be skipped, leaving ~3,940 converted.
**Warning signs:** Conversion errors on machine_model files (shouldn't happen -- they parse as valid JSON and are silently skipped).

### Pitfall 4: 18 Unique CrealityPrint-Specific Keys
**What goes wrong:** 18 JSON keys are unique to CrealityPrint (not in OrcaSlicer or BambuStudio). They will appear as unmapped fields.
**Why it happens:** CrealityPrint adds features like epoxy resin plate temperature, CDS cooling fan control, and chamber layer activation.
**How to avoid:** This is expected and acceptable. These fields are listed in `unmapped_fields` during import but don't cause errors. None map to existing `PrintConfig` fields. The most frequent are: `epoxy_resin_plate_temp` (1,177 profiles), `cool_cds_fan_start_at_height` (953), `enable_special_area_additional_cooling_fan` (707).
**Warning signs:** Unexpectedly high unmapped field counts in conversion output (this is normal for these 18 keys).

### Pitfall 5: Large Git Commit
**What goes wrong:** Committing ~3,940 new TOML files in a single commit can be unwieldy for review.
**Why it happens:** Batch conversion generates thousands of files.
**How to avoid:** Profiles are in `.gitignore` (generated data), so this is not an issue for git. Only the integration test file needs committing.
**Warning signs:** None -- profiles are gitignored.

## CrealityPrint Profile Analysis (Verified from Filesystem)

### Profile Counts
| Category | Count |
|----------|-------|
| Total JSON files in profiles/ directory | 4,893 |
| In machine/filament/process subdirs | 4,852 |
| Vendor-level JSON metadata files | 38 |
| Other files (scripts, blacklist, images) | 3 |
| **Instantiated profiles (to convert)** | **3,940** |
| Non-instantiated base/template profiles | 714 |
| Machine model metadata files (no instantiation field) | 198 (~5 without instantiation field in type dirs) |

### Per-Vendor Breakdown (Instantiated Only)
| Vendor | Machine | Filament | Process | Total |
|--------|---------|----------|---------|-------|
| Creality | 84 | 1,187 | 248 | 1,519 |
| BBL (Bambu Lab) | 28 | 421 | 119 | 568 |
| Qidi | 19 | 134 | 103 | 256 |
| Snapmaker | 72 | 91 | 68 | 231 |
| Prusa | 22 | 63 | 93 | 178 |
| Elegoo | 25 | 3 | 150 | 178 |
| Flashforge | 14 | 50 | 42 | 106 |
| Voron | 56 | 10 | 34 | 100 |
| OrcaArena | 4 | 57 | 24 | 85 |
| Anker | 12 | 29 | 20 | 61 |
| Ratrig | 25 | 10 | 25 | 60 |
| Sovol | 11 | 10 | 21 | 42 |
| Anycubic | 8 | 10 | 22 | 40 |
| Custom | 6 | 11 | 22 | 39 |
| MagicMaker | 5 | 7 | 26 | 38 |
| Vzbot | 6 | 10 | 18 | 34 |
| TwoTrees | 2 | 17 | 14 | 33 |
| FlyingBear | 2 | 21 | 10 | 33 |
| Folgertech | 6 | 12 | 13 | 31 |
| FLSun | 4 | 10 | 16 | 30 |
| BIQU | 3 | 12 | 12 | 27 |
| Artillery | 5 | 6 | 13 | 24 |
| Positron3D | 4 | 10 | 9 | 23 |
| Comgrow | 3 | 3 | 16 | 22 |
| Dremel | 3 | 4 | 13 | 20 |
| InfiMech | 1 | 14 | 5 | 20 |
| Peopoly | 3 | 7 | 9 | 19 |
| SecKit | 2 | 10 | 7 | 19 |
| Kingroon | 3 | 10 | 5 | 18 |
| Vivedino | 2 | 10 | 6 | 18 |
| Tronxy | 1 | 10 | 6 | 17 |
| Raise3D | 6 | 5 | 6 | 17 |
| CONSTRUCT3D | 2 | 3 | 8 | 13 |
| Wanhao | 1 | 3 | 4 | 8 |
| UltiMaker | 1 | 3 | 3 | 7 |
| Voxelab | 1 | 3 | 2 | 6 |
| **Total** | **452** | **2,276** | **1,212** | **3,940** |

### Overlap Analysis
| Comparison | Count |
|------------|-------|
| **vs OrcaSlicer:** | |
| Same filename in both | 2,317 |
| Identical content (byte-equal) | 141 |
| Same filename, different content | 2,176 |
| Unique to CrealityPrint (not in OrcaSlicer) | 1,623 |
| **vs BambuStudio:** | |
| Same filename in both | 895 |
| Unique to CrealityPrint (not in BambuStudio) | 3,045 |
| **vs Both OrcaSlicer AND BambuStudio:** | |
| **Truly unique to CrealityPrint** | **1,614** |

### Truly-Unique-to-CrealityPrint by Vendor
| Vendor | Unique Profiles | Key New Content |
|--------|----------------|-----------------|
| Creality | 1,469 | K2, K2 SE, GS-01/02/03, Hi, SPARKX i7, Ender-3 V4, CFS-C multi-color, Sermoon M300 |
| TwoTrees | 15 | Additional TwoTrees printer profiles |
| BIQU | 12 | Additional BIQU profiles |
| Folgertech | 12 | Additional Folgertech profiles |
| Custom | 11 | Generic RRF/Klipper/Marlin printer profiles |
| Kingroon | 11 | Additional Kingroon profiles |
| Flashforge | 10 | Additional Flashforge profiles |
| Others (16 vendors) | 73 | Various small additions across remaining vendors |

### CrealityPrint-Specific Features
| Feature | Impact on Conversion |
|---------|---------------------|
| 18 unique JSON keys | All unmapped -- CrealityPrint-specific bed/cooling features not in PrintConfig |
| No `include` field (unlike BambuStudio) | Cleaner conversion than Phase 17 |
| `creality_flush_time` (63 machine profiles) | Creality-specific multi-color purge timing, unmapped |
| 542 total unique keys (vs 788 in OrcaSlicer) | Fewer keys than OrcaSlicer, 524 shared keys |

### Vendor List (36 vendors, all overlap with OrcaSlicer)
Anker, Anycubic, Artillery, BBL, BIQU, Comgrow, CONSTRUCT3D, Creality, Custom, Dremel, Elegoo, Flashforge, FLSun, FlyingBear, Folgertech, InfiMech, Kingroon, MagicMaker, OrcaArena, Peopoly, Positron3D, Prusa, Qidi, Raise3D, Ratrig, SecKit, Snapmaker, Sovol, Tronxy, TwoTrees, UltiMaker, Vivedino, Voron, Voxelab, Vzbot, Wanhao

All 36 CrealityPrint vendors also exist in OrcaSlicer. 11 also exist in BambuStudio.

### Creality-Unique Printer Models (48 machine profiles not in OrcaSlicer or BambuStudio)
Notable new models: K2, K2 SE, GS-01, GS-02, GS-03, Hi, SPARKX i7, Ender-3 V4, Sermoon M300, CFS-C multi-color variants (K1 CFS-C, K1C CFS-C, K1 Max CFS-C, K1C 2025 CFS-C, K1 Max 2025 CFS-C, K1 SE CFS-C), Sonic Ender-3/5 S1

## Code Examples

### Running the Import (CLI)
```bash
# All that's needed -- the existing CLI handles everything:
cargo run -p slicecore-cli -- import-profiles \
  --source-dir /home/steve/slicer-analysis/CrealityPrint/resources/profiles \
  --output-dir profiles \
  --source-name crealityprint

# Expected output:
# Importing crealityprint profiles from '/home/steve/slicer-analysis/CrealityPrint/resources/profiles'...
# Import complete:
#   Converted: ~3940 profiles
#   Skipped:   ~912 (non-instantiated base profiles + machine_model metadata)
#   Errors:    0
#   Output:    profiles
```

### Verification Script
```bash
# Verify the import produced expected results:

# 1. Check profile count
find profiles/crealityprint -name "*.toml" | wc -l
# Expected: ~3940

# 2. Check index merged correctly
python3 -c "
import json
with open('profiles/index.json') as f:
    data = json.load(f)
sources = {}
for p in data['profiles']:
    sources[p['source']] = sources.get(p['source'], 0) + 1
for src, count in sorted(sources.items()):
    print(f'{src}: {count}')
print(f'Total: {len(data[\"profiles\"])}')
"
# Expected:
#   bambustudio: 2348
#   crealityprint: ~3940
#   orcaslicer: 6015
#   prusaslicer: 9241
#   Total: ~21544

# 3. Check vendor directories exist (should be 36)
ls profiles/crealityprint/ | wc -l
# Expected: 36

# 4. Spot-check a converted profile
cat profiles/crealityprint/Creality/filament/$(ls profiles/crealityprint/Creality/filament/ | head -1) | head -20
```

### Integration Test Pattern
```rust
#[test]
fn test_crealityprint_batch_convert_synthetic() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();

    // Create synthetic CrealityPrint-style profiles
    // (identical format to OrcaSlicer/BambuStudio)
    let filament_json = r#"{
        "type": "filament",
        "name": "CR-PLA @Creality K1 0.4 nozzle",
        "instantiation": "true",
        "inherits": "fdm_filament_pla",
        "nozzle_temperature": ["210"],
        "hot_plate_temp": ["60"],
        "filament_type": ["PLA"],
        "epoxy_resin_plate_temp": "0"
    }"#;
    // ... similar to BambuStudio integration tests
}

#[test]
#[ignore]
fn test_real_crealityprint_batch_convert() {
    let source = Path::new("/home/steve/slicer-analysis/CrealityPrint/resources/profiles");
    if !source.exists() { return; }

    let out_dir = std::env::temp_dir().join("slicecore_test_crealityprint");
    let result = batch_convert_profiles(source, &out_dir.join("crealityprint"), "crealityprint").unwrap();

    assert!(result.converted > 3500, "Expected >3500 converted, got {}", result.converted);
    assert!(result.errors.len() < 50, "Too many errors: {}", result.errors.len());

    // Verify Creality vendor has profiles (largest vendor)
    assert!(out_dir.join("crealityprint/Creality/filament").is_dir());
    assert!(out_dir.join("crealityprint/Creality/machine").is_dir());
    assert!(out_dir.join("crealityprint/Creality/process").is_dir());
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 3 sources: OrcaSlicer + PrusaSlicer + BambuStudio (17,604 profiles) | Add CrealityPrint source (~3,940 profiles) | Phase 18 | ~21,544 total profiles from 4 sources |
| No Creality-optimized profiles from official slicer | CrealityPrint K2, GS-03, Hi, SPARKX coverage | Phase 18 | Supports newest Creality printers with official settings |

## Open Questions

1. **Should "different content" profiles prefer CrealityPrint or OrcaSlicer values for Creality printers?**
   - What we know: 2,176 profiles exist in both sources with different field values. For Creality printers specifically, CrealityPrint values may be more authoritative since it's Creality's official slicer.
   - What's unclear: Whether CrealityPrint values are actually better-tuned for Creality hardware.
   - Recommendation: Keep both versions in their respective namespaces. Users who own Creality printers may prefer CrealityPrint source profiles. This is the existing pattern for all multi-source overlap.

2. **Should we handle the 18 CrealityPrint-specific keys?**
   - What we know: `epoxy_resin_plate_temp` (1,177 profiles), `cool_cds_fan_start_at_height` (953), and `enable_special_area_additional_cooling_fan` (707) are the most common. These are Creality-specific hardware features.
   - What's unclear: Whether users need these values for Creality printers.
   - Recommendation: Don't add PrintConfig fields for these now. They're hardware-specific and our PrintConfig doesn't model per-brand hardware features. Revisit if Creality-specific hardware support is added later.

## Sources

### Primary (HIGH confidence)
- **Direct filesystem analysis** of `/home/steve/slicer-analysis/CrealityPrint/resources/profiles/` -- all 4,893 files analyzed, 4,852 JSON profiles parsed, 3,940 instantiated profiles counted
- **Direct filesystem analysis** of `/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/` -- for overlap comparison (2,317 shared filenames, 141 byte-identical, 2,176 different content)
- **Direct filesystem analysis** of `/home/steve/slicer-analysis/BambuStudio/resources/profiles/` -- for overlap comparison (895 shared filenames)
- **Direct codebase analysis** of `profile_library.rs`, `profile_import.rs`, `main.rs` -- confirmed existing pipeline handles CrealityPrint format identically to OrcaSlicer
- **JSON key analysis** -- 542 CrealityPrint keys vs 788 OrcaSlicer keys, 524 shared, 18 unique to CrealityPrint
- **Phase 17 execution summary** -- confirmed data-only import pattern works, zero code changes needed
- **Existing integration tests** in `integration_profile_library_bambu.rs` -- template for CrealityPrint tests

### Secondary (MEDIUM confidence)
- **Assumption that CrealityPrint is an OrcaSlicer fork** -- based on identical JSON format, shared vendor set, and consistent field naming. CrealityPrint's GitHub and documentation confirm this lineage.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- No new libraries needed; existing pipeline handles CrealityPrint identically to OrcaSlicer/BambuStudio (verified by format analysis)
- Architecture: HIGH -- Verified by direct codebase analysis; `batch_convert_profiles()` already designed for this exact use case; Phase 17 proved the pattern
- Profile analysis: HIGH -- All counts and overlap analysis verified by direct filesystem inspection with exact numbers from parsing all 4,852 JSON files
- Unique key impact: HIGH -- All 18 unique keys inspected; none map to PrintConfig fields

**Research date:** 2026-02-19
**Valid until:** 2026-03-19 (CrealityPrint profile format is stable; profiles update but format doesn't change)
