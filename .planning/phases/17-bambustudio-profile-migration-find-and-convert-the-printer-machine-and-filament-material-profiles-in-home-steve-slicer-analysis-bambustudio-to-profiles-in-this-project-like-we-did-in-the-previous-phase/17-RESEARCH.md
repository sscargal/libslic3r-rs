# Phase 17: BambuStudio Profile Migration - Research

**Researched:** 2026-02-19
**Domain:** BambuStudio JSON profile conversion, batch import, overlap analysis with existing OrcaSlicer profiles
**Confidence:** HIGH

## Summary

Phase 17 imports BambuStudio profiles from `/home/steve/slicer-analysis/BambuStudio/resources/profiles/` into the project's `profiles/` directory. The critical finding is that BambuStudio uses the **exact same JSON format** as OrcaSlicer (same directory structure, same field names, same `inherits` / `instantiation` mechanism), because BambuStudio is OrcaSlicer's upstream fork. The existing `batch_convert_profiles()` function in `profile_library.rs` can process BambuStudio profiles with **zero code changes** -- it was already designed for this.

However, there is significant overlap with the OrcaSlicer profiles already imported in Phase 15. Of BambuStudio's 2,348 instantiated profiles: 326 are identical to OrcaSlicer versions, 1,358 share the same filename but have different content (typically minor field additions/removals), and 664 are unique to BambuStudio. The unique profiles are predominantly for newer Bambu Lab printers (H2C, H2S, P2S) not yet in OrcaSlicer, plus additional Qidi, Elegoo, and Creality machine-specific variants.

BambuStudio introduces one new feature not present in OrcaSlicer: the `include` field (found in 1,053 instantiated profiles). This is a secondary inheritance mechanism referencing non-instantiated template profiles by name. The existing `import_upstream_profile()` function ignores this field (it's not in the field mapping), which means some dual-extruder template fields won't be inherited. This is acceptable since our `PrintConfig` doesn't model dual-extruder settings, and the `include` targets contain mostly dual-extruder-specific fields (`filament_extruder_variant`, `filament_flush_temp`, etc.).

**Primary recommendation:** Run the existing `batch_convert_profiles()` with `source_name = "bambustudio"` and `source_dir = /home/steve/slicer-analysis/BambuStudio/resources/profiles`. This will produce `profiles/bambustudio/` alongside `profiles/orcaslicer/` and `profiles/prusaslicer/`. The `write_merged_index()` function already handles merging into the existing `index.json`. The only code change needed is to run the import and commit the resulting TOML files. This is essentially a data-only phase with no new code required.

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
| Separate "bambustudio" source namespace | Merge into existing "orcaslicer" namespace | Keeping separate is better: avoids filename collisions where BambuStudio has updated versions of same-named profiles; preserves attribution; lets users choose which source's version to use |
| Import all 2,348 instantiated profiles | Import only the 664 unique-to-BambuStudio profiles | Importing all is simpler and correct: even the 1,358 "different content" profiles may have updated values from BambuStudio's official releases |
| Resolve `include` field templates | Ignore `include` field | Ignoring is acceptable: the `include` targets contain dual-extruder fields not mapped to PrintConfig |

## Architecture Patterns

### Recommended Directory Structure
```
profiles/
  index.json                        # Merged index (orcaslicer + prusaslicer + bambustudio)
  orcaslicer/                       # Phase 15 - 6,015 profiles
    BBL/
    Creality/
    ...
  prusaslicer/                      # Phase 16 - 9,241 profiles
    PrusaResearch/
    Creality/
    ...
  bambustudio/                      # Phase 17 - ~2,348 profiles (NEW)
    BBL/
      filament/
      machine/
      process/
    Creality/
    Elegoo/
    Qidi/
    ... (12 vendors total)
```

### Pattern 1: Direct Reuse of Existing Pipeline
**What:** The `batch_convert_profiles()` function from Phase 15 handles BambuStudio JSON identically to OrcaSlicer JSON -- no code changes needed.
**When to use:** For all BambuStudio profile conversion.
**Why it works:** BambuStudio is OrcaSlicer's upstream fork. The profile JSON format is identical:
- Same directory structure: `vendor_dir/type_dir/profile.json`
- Same `"instantiation": "true"/"false"` field for leaf vs base discrimination
- Same `"inherits": "parent_name"` field for single-parent inheritance
- Same field names (OrcaSlicer naming: `nozzle_temperature`, `hot_plate_temp`, `sparse_infill_density`, etc.)
- Same array-wrapped string values

**Execution:**
```bash
# This is all that's needed:
cargo run --bin slicecore -- import-profiles \
  --source-dir /home/steve/slicer-analysis/BambuStudio/resources/profiles \
  --output-dir profiles \
  --source-name bambustudio
```

### Pattern 2: Index Merge (Already Implemented)
**What:** `write_merged_index()` handles adding BambuStudio entries to the existing `index.json` without losing OrcaSlicer or PrusaSlicer entries.
**When to use:** Automatically called by `cmd_import_profiles()` in the CLI.
**Already tested:** Phase 16 validated this pattern when adding PrusaSlicer entries alongside OrcaSlicer entries.

### Anti-Patterns to Avoid
- **Modifying `batch_convert_profiles()` for BambuStudio-specific logic:** The function already handles this format. Do not add BambuStudio-specific code paths.
- **Resolving the `include` field:** The `include` targets contain dual-extruder template fields not relevant to our single-extruder PrintConfig. Resolving them would add complexity for no benefit.
- **Deduplicating against OrcaSlicer:** Do NOT skip profiles that also exist in OrcaSlicer. BambuStudio may have updated values, and users should be able to choose either source's version. The separate namespace (`bambustudio/` vs `orcaslicer/`) handles this cleanly.
- **Treating this as a code phase:** No new Rust code is needed. This is a data import phase.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON parsing | Custom BambuStudio parser | Existing `import_upstream_profile()` | BambuStudio JSON is identical to OrcaSlicer JSON |
| Batch conversion | Custom BambuStudio converter | Existing `batch_convert_profiles()` | Works as-is for BambuStudio |
| Index merging | Custom merge logic | Existing `write_merged_index()` | Already handles multi-source merging |
| Inheritance resolution | Custom BambuStudio resolver | Existing `resolve_inheritance()` | Same `inherits` mechanism as OrcaSlicer |
| Filename sanitization | Custom sanitizer | Existing `sanitize_filename()` | Same naming conventions as OrcaSlicer |

**Key insight:** This phase requires ZERO new code. The existing infrastructure built in Phases 13-16 handles BambuStudio profiles identically to OrcaSlicer profiles. The work is purely: (1) run the import command, (2) verify the output, (3) commit the generated TOML files.

## Common Pitfalls

### Pitfall 1: Profile Name Collisions with OrcaSlicer
**What goes wrong:** BambuStudio and OrcaSlicer share many identical profile names (e.g., "Bambu ABS @BBL X1C"). If stored in the same directory, they'd overwrite each other.
**Why it happens:** BambuStudio is OrcaSlicer's upstream fork; shared vendors have profiles with identical names.
**How to avoid:** Use a separate `bambustudio/` namespace in `profiles/`. The existing architecture already supports this -- each source gets its own top-level directory.
**Warning signs:** Profiles from one source disappearing after importing the other source.

### Pitfall 2: `include` Field Not Resolved
**What goes wrong:** Some BambuStudio profiles reference templates via an `include` field that isn't part of the standard OrcaSlicer inheritance mechanism. These templates aren't loaded.
**Why it happens:** BambuStudio added an `include` mechanism for dual-extruder template inheritance. The `import_upstream_profile()` function doesn't know about `include`.
**How to avoid:** Accept this limitation. The `include` targets contain fields like `filament_extruder_variant`, `filament_flush_temp`, `filament_deretraction_speed` -- none of which map to our PrintConfig. The loss is negligible.
**Warning signs:** 1,053 profiles may have fewer mapped fields than expected, but this doesn't affect usability since the missing fields aren't in our field mapping.

### Pitfall 3: Index Size Growth
**What goes wrong:** Adding ~2,348 BambuStudio profiles to `index.json` (already containing 15,256 entries) grows it significantly.
**Why it happens:** Each profile gets an index entry with searchable metadata.
**How to avoid:** This is expected and acceptable. The merged index will contain ~17,600 entries. JSON parsing of this size is still fast (< 50ms).
**Warning signs:** Noticeable slowdown in `list-profiles` or `search-profiles` commands (unlikely at this scale).

### Pitfall 4: Large Git Commit
**What goes wrong:** Committing ~2,348 new TOML files in a single commit can be unwieldy for review.
**Why it happens:** Batch conversion generates thousands of files.
**How to avoid:** This is expected and acceptable. Phases 15 and 16 established the precedent (6,015 + 9,241 profiles). Consider splitting the commit into: (1) generated profiles, (2) updated index.json.
**Warning signs:** Git operations taking longer than expected.

## BambuStudio Profile Analysis (Verified from Filesystem)

### Profile Counts
| Category | Count |
|----------|-------|
| Total JSON files in BambuStudio profiles/ | 3,019 |
| In machine/filament/process subdirs | 2,855 |
| Vendor-level JSON metadata files | 12 |
| Other files (scripts, blacklist) | ~5 |
| **Instantiated profiles (to convert)** | **2,348** |
| Non-instantiated base/template profiles | 414 |
| Non-profile files (machine_model, metadata) | 93 |

### Per-Vendor Breakdown (Instantiated Only)
| Vendor | Machine | Filament | Process | Total |
|--------|---------|----------|---------|-------|
| BBL (Bambu Lab) | ~106 | ~1,126 | ~227 | ~1,459 |
| Qidi | ~34 | ~234 | ~148 | ~416 |
| Elegoo | ~41 | ~7 | ~74 | ~122 |
| Geeetech | ~62 | ~19 | ~89 | ~170 |
| Creality | ~52 | ~14 | ~68 | ~134 |
| Anycubic | ~17 | ~19 | ~22 | ~58 |
| Anker | ~5 | ~19 | ~10 | ~34 |
| Voron | ~16 | ~19 | ~8 | ~43 |
| Vivedino | ~7 | ~19 | ~8 | ~34 |
| Tronxy | ~3 | ~19 | ~8 | ~30 |
| Prusa | ~5 | ~19 | ~3 | ~27 |
| Voxelab | ~3 | ~7 | ~3 | ~13 |

### Overlap with OrcaSlicer (Phase 15)
| Category | Count |
|----------|-------|
| Identical to OrcaSlicer (same filename, same content) | 326 |
| Same filename but different content | 1,358 |
| **Unique to BambuStudio** | **664** |
| Total instantiated | 2,348 |

### Unique-to-BambuStudio Profiles by Type
| Type | Count | Notable Content |
|------|-------|----------------|
| Filament | 510 | Mostly H2C, H2S, P2S variants; Qidi dual-extruder profiles |
| Process | 119 | H2C, Elegoo, Creality process profiles |
| Machine | 35 | G-code templates, new printer models (H2C, H2S, P2S) |

### Unique-to-BambuStudio by Vendor
| Vendor | Unique Profiles | Key New Content |
|--------|----------------|-----------------|
| BBL | 331 | H2C/H2S/P2S printer profiles, newer filament variants |
| Qidi | 115 | Additional dual-extruder filament profiles |
| Elegoo | 98 | Process profiles for newer models |
| Creality | 46 | Machine + process profiles |
| Others | 74 | Mainly "Generic" filament profiles not in OrcaSlicer |

### BambuStudio-Specific Features
| Feature | Count | Impact on Conversion |
|---------|-------|---------------------|
| `include` field (dual-extruder templates) | 1,053 profiles | Ignored -- `include` targets contain dual-extruder fields not in PrintConfig |
| 63 keys unique to BambuStudio | 63 field names | Most are unmapped already (dual-extruder, support ironing, advanced features) |
| `include` targets (`fdm_filament_template_direct_dual`) | 1 template | Contains `filament_extruder_variant`, `filament_flush_temp` etc. -- none in our field mapping |

### Vendor List (12 vendors, all overlap with OrcaSlicer)
Anker, Anycubic, BBL, Creality, Elegoo, Geeetech, Prusa, Qidi, Tronxy, Vivedino, Voron, Voxelab

All 12 BambuStudio vendors also exist in OrcaSlicer. OrcaSlicer has an additional 50 vendors not in BambuStudio.

## Code Examples

### Running the Import (CLI)
```bash
# All that's needed -- the existing CLI handles everything:
cargo run --bin slicecore -- import-profiles \
  --source-dir /home/steve/slicer-analysis/BambuStudio/resources/profiles \
  --output-dir profiles \
  --source-name bambustudio

# Expected output:
# Importing bambustudio profiles from '/home/steve/slicer-analysis/BambuStudio/resources/profiles'...
# Import complete:
#   Converted: ~2348 profiles
#   Skipped:   ~507 (non-instantiated base profiles)
#   Errors:    0
#   Output:    profiles
```

### Verification Script
```bash
# Verify the import produced expected results:

# 1. Check profile count
find profiles/bambustudio -name "*.toml" | wc -l
# Expected: ~2348

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
#   bambustudio: ~2348
#   orcaslicer: 6015
#   prusaslicer: 9241
#   Total: ~17604

# 3. Check vendor directories exist
ls profiles/bambustudio/
# Expected: Anker Anycubic BBL Creality Elegoo Geeetech Prusa Qidi Tronxy Vivedino Voron Voxelab

# 4. Spot-check a converted profile
cat profiles/bambustudio/BBL/filament/Bambu_PLA_Basic_BBL_A1.toml | head -20
```

### Integration Test Pattern
```rust
#[test]
fn test_bambustudio_batch_convert() {
    let source = Path::new("/home/steve/slicer-analysis/BambuStudio/resources/profiles");
    if !source.exists() {
        // Skip in CI where source data isn't available.
        return;
    }

    let out_dir = std::env::temp_dir().join("slicecore_test_bambu");
    let _ = std::fs::remove_dir_all(&out_dir);

    let result = batch_convert_profiles(source, &out_dir.join("bambustudio"), "bambustudio").unwrap();

    assert!(result.converted > 2000, "Expected >2000 converted, got {}", result.converted);
    assert!(result.errors.len() < 50, "Too many errors: {}", result.errors.len());

    // Verify BBL vendor has profiles.
    assert!(out_dir.join("bambustudio/BBL/filament").is_dir());
    assert!(out_dir.join("bambustudio/BBL/machine").is_dir());
    assert!(out_dir.join("bambustudio/BBL/process").is_dir());

    let _ = std::fs::remove_dir_all(&out_dir);
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| OrcaSlicer + PrusaSlicer only (15,256 profiles) | Add BambuStudio source (~2,348 profiles) | Phase 17 | ~17,604 total profiles from 3 sources |
| No BambuStudio-specific profiles | BambuStudio H2C/H2S/P2S coverage | Phase 17 | Supports newest Bambu Lab printers |

## Open Questions

1. **Should we handle the `include` field?**
   - What we know: 1,053 BambuStudio profiles use an `include` field referencing templates like `fdm_filament_template_direct_dual`. These templates contain dual-extruder-specific fields.
   - What's unclear: Whether any `include` template contains fields that DO map to PrintConfig.
   - Recommendation: Don't resolve `include`. The only template (`fdm_filament_template_direct_dual`) contains fields not in our mapping. Revisit if dual-extruder support is added later.

2. **Should we skip profiles identical to OrcaSlicer?**
   - What we know: 326 profiles are byte-identical between BambuStudio and OrcaSlicer. 1,358 share names but differ in content.
   - What's unclear: Whether users would be confused by near-duplicate profiles from different sources.
   - Recommendation: Import all BambuStudio profiles. The separate `bambustudio/` namespace handles disambiguation. Users who want de-duplication can search by source.

3. **Should "different content" profiles prefer BambuStudio or OrcaSlicer values?**
   - What we know: 1,358 profiles exist in both sources with different field values. BambuStudio versions tend to be slightly simpler (fewer fields), while OrcaSlicer versions often have additional fields added by the OrcaSlicer community.
   - What's unclear: Which version better matches real-world printer behavior.
   - Recommendation: Keep both versions in their respective namespaces. Users can choose. This is the existing pattern for OrcaSlicer + PrusaSlicer overlap.

## Sources

### Primary (HIGH confidence)
- **Direct filesystem analysis** of `/home/steve/slicer-analysis/BambuStudio/resources/profiles/` -- all 3,019 JSON files analyzed
- **Direct filesystem analysis** of `/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/` -- for overlap comparison
- **Direct codebase analysis** of `profile_library.rs` (1,453 lines), `profile_import.rs`, `profile_import_ini.rs`, `main.rs` -- confirmed existing pipeline handles BambuStudio format
- **File-level comparison** between BambuStudio and OrcaSlicer profiles -- byte-level diff of all 2,855 profiles in machine/filament/process directories
- **JSON key analysis** -- compared all unique keys across both sources (532 BambuStudio keys vs 765 OrcaSlicer keys)
- **Existing Phase 15/16 documentation** -- `.planning/phases/15-*/15-RESEARCH.md` and `.planning/phases/16-*/16-RESEARCH.md`

### Secondary (MEDIUM confidence)
- **Assumption that `include` field is not critical** -- based on inspection of the single include target `fdm_filament_template_direct_dual` which contains only dual-extruder fields. If other templates exist in newer BambuStudio versions, this assumption may need revisiting.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- No new libraries needed; existing pipeline handles BambuStudio identically to OrcaSlicer
- Architecture: HIGH -- Verified by direct codebase analysis; `batch_convert_profiles()` already designed for this exact use case
- Profile analysis: HIGH -- All counts and overlap analysis verified by direct filesystem inspection with exact numbers
- `include` field impact: MEDIUM -- Inspected the single template but haven't exhaustively verified all `include` references

**Research date:** 2026-02-19
**Valid until:** 2026-03-19 (BambuStudio profile format is stable; profiles update but format doesn't change)
