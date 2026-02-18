# Phase 15: Printer and Filament Profile Library - Research

**Researched:** 2026-02-18
**Domain:** Profile library management, batch conversion, CLI profile discovery, directory organization
**Confidence:** HIGH

## Summary

Phase 15 builds an extensive library of printer and filament profiles by batch-converting upstream OrcaSlicer/BambuStudio JSON profiles into native TOML format using the conversion infrastructure built in Phases 13-14. The upstream data lives at `/home/steve/slicer-analysis/` and contains 9,533 OrcaSlicer JSON profiles (62 vendors, 5,129 filament + 2,961 process + 1,379 machine) and 3,019 BambuStudio JSON profiles (12 vendors). All upstream slicers (OrcaSlicer, BambuStudio, PrusaSlicer) are licensed under AGPL-3.0, which has significant implications for how converted profiles are stored and distributed.

The core technical challenge is NOT the conversion itself -- Phase 14 already built `import_upstream_profile()` + `convert_to_toml()` + `merge_import_results()` and these work correctly. The challenges for Phase 15 are: (1) designing a logical directory structure for `profiles/` that enables fast CLI search and browsing, (2) deciding which profiles to store (all 9,533 vs. curated subset vs. only "instantiated" leaf profiles), (3) handling the inheritance problem (most profiles override only 3-5 fields, relying on parent chains for full settings), (4) building CLI subcommands for profile discovery (`list`, `search`, `show`), and (5) creating integration tests that compare original JSON against converted TOML to verify conversion fidelity.

The recommended approach is to convert only the "instantiated" leaf profiles (7,628 of 9,533 OrcaSlicer profiles) since base/parent profiles are incomplete without inheritance resolution. Store converted profiles as TOML in `profiles/` organized by `source/vendor/type/`, and build a profile index (JSON or TOML manifest) that enables efficient CLI search without reading every file. For PrusaSlicer, defer INI import as it requires a fundamentally different parser -- the phase description says "import from slicer-analysis" but PrusaSlicer profiles are INI format (40K-line monolithic files), not JSON.

**Primary recommendation:** Build a batch conversion tool that walks the slicer-analysis directories, converts instantiated profiles to TOML, stores them in `profiles/` with a logical directory structure, generates a searchable index manifest, and provides `list-profiles`, `search-profiles`, and `show-profile` CLI subcommands. Store only converted TOML (not original JSON) in the repo to avoid AGPL licensing complexity.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| toml | 0.8 (already in workspace) | TOML serialization of converted profiles | Already used for config and profile_convert |
| serde_json | 1.x (already in workspace) | JSON deserialization of upstream profiles | Already used by Phase 13 profile import |
| serde | 1.x (already in workspace) | Serialize/Deserialize for profile index | Already used everywhere |
| clap | 4.5 (already in CLI crate) | New CLI subcommands | Already used for existing subcommands |
| walkdir | 2.x (NOT in workspace -- needs adding) | Recursive directory traversal for batch conversion | Standard Rust crate for directory walking, avoids manual recursion |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| glob | 0.3 | File pattern matching for profile discovery | Only if walkdir isn't sufficient for filtering |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| walkdir | std::fs::read_dir + manual recursion | walkdir is simpler, handles symlinks/errors, well-maintained |
| JSON index manifest | TOML index manifest | JSON is better for large indexes (faster parsing, smaller size). TOML tables get unwieldy with 7000+ entries. |
| Store TOML in repo | Store JSON in repo + convert at build time | Storing TOML avoids AGPL concerns about distributing derived works, and users get ready-to-use profiles without running conversion |

**Installation:**
```bash
# Add walkdir to workspace Cargo.toml
# [workspace.dependencies]
# walkdir = "2"
```

## Architecture Patterns

### Recommended Directory Structure for profiles/
```
profiles/
├── index.json                      # Searchable index of all profiles
├── orcaslicer/                     # Source slicer
│   ├── BBL/                        # Vendor
│   │   ├── filament/               # Profile type
│   │   │   ├── Bambu_PLA_Basic.toml
│   │   │   ├── Bambu_ABS.toml
│   │   │   └── ...
│   │   ├── process/
│   │   │   ├── 0.20mm_Standard_X1C.toml
│   │   │   └── ...
│   │   └── machine/
│   │       ├── Bambu_Lab_A1_0.4_nozzle.toml
│   │       └── ...
│   ├── Creality/
│   │   ├── filament/
│   │   ├── process/
│   │   └── machine/
│   └── ... (62 vendors)
└── bambustudio/                    # Separate source to avoid confusion
    ├── BBL/
    │   ├── filament/
    │   ├── process/
    │   └── machine/
    └── ... (12 vendors)
```

### Pattern 1: Profile Index Manifest
**What:** A JSON manifest file listing all profiles with searchable metadata, enabling fast CLI search without reading thousands of TOML files.
**When to use:** Always -- the CLI needs fast search across 7000+ profiles.
**Why JSON, not TOML:** A TOML array-of-tables with 7000 entries is awkward to parse and maintain. JSON arrays are natural for this.

**Example `index.json`:**
```json
{
  "version": 1,
  "generated": "2026-02-18T12:00:00Z",
  "profiles": [
    {
      "id": "orcaslicer/BBL/filament/Bambu_PLA_Basic",
      "name": "Bambu PLA Basic @BBL A1",
      "source": "orcaslicer",
      "vendor": "BBL",
      "type": "filament",
      "material": "PLA",
      "nozzle_size": null,
      "printer_model": "Bambu Lab A1",
      "path": "orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1.toml",
      "layer_height": null,
      "quality": null
    },
    {
      "id": "orcaslicer/BBL/process/0.20mm_Standard_X1C",
      "name": "0.20mm Standard @BBL X1C",
      "source": "orcaslicer",
      "vendor": "BBL",
      "type": "process",
      "material": null,
      "nozzle_size": 0.4,
      "printer_model": "Bambu Lab X1 Carbon",
      "path": "orcaslicer/BBL/process/0.20mm_Standard_X1C.toml",
      "layer_height": 0.20,
      "quality": "standard"
    }
  ]
}
```

### Pattern 2: Batch Conversion Tool (Build Script)
**What:** A standalone binary or build script that reads all profiles from slicer-analysis directories, converts them, and writes to profiles/.
**When to use:** During development to populate the profiles/ directory. Not shipped with the binary.
**Implementation approach:**
```rust
/// Walk a slicer profile directory tree and convert all instantiated profiles.
pub fn batch_convert_profiles(
    source_dir: &Path,      // e.g., /home/steve/slicer-analysis/OrcaSlicer/resources/profiles
    output_dir: &Path,      // e.g., profiles/orcaslicer
    source_name: &str,      // e.g., "orcaslicer"
) -> Result<BatchConvertResult, EngineError> {
    let mut index_entries = Vec::new();

    for vendor_dir in fs::read_dir(source_dir)? {
        let vendor_dir = vendor_dir?;
        if !vendor_dir.file_type()?.is_dir() { continue; }
        let vendor_name = vendor_dir.file_name().to_string_lossy().to_string();

        for profile_type in &["filament", "process", "machine"] {
            let type_dir = vendor_dir.path().join(profile_type);
            if !type_dir.exists() { continue; }

            for entry in fs::read_dir(&type_dir)? {
                let entry = entry?;
                if entry.path().extension() != Some("json".as_ref()) { continue; }

                // Read and parse JSON.
                let contents = fs::read_to_string(entry.path())?;
                let json: serde_json::Value = serde_json::from_str(&contents)?;

                // Skip non-instantiated (base/parent) profiles.
                let instantiation = json.get("instantiation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("false");
                if instantiation != "true" { continue; }

                // Import and convert.
                let import_result = import_upstream_profile(&json)?;
                let convert_result = convert_to_toml(&import_result);

                // Generate output filename (sanitize special chars).
                let filename = sanitize_filename(&import_result.metadata.name.unwrap_or_default());
                let out_path = output_dir.join(&vendor_name).join(profile_type).join(format!("{}.toml", filename));
                fs::create_dir_all(out_path.parent().unwrap())?;
                fs::write(&out_path, &convert_result.toml_output)?;

                // Build index entry.
                index_entries.push(build_index_entry(
                    source_name, &vendor_name, profile_type,
                    &import_result, &out_path, output_dir
                ));
            }
        }
    }

    Ok(BatchConvertResult { converted: index_entries.len(), index: index_entries })
}
```

### Pattern 3: CLI Profile Discovery Subcommands
**What:** Three new CLI subcommands: `list-profiles`, `search-profiles`, `show-profile`.
**When to use:** Users exploring the profile library from the command line.

**Example CLI interactions:**
```bash
# List all vendors
slicecore list-profiles --vendors

# List all profiles for a vendor
slicecore list-profiles --vendor BBL

# List all PLA filament profiles
slicecore list-profiles --type filament --material PLA

# Search profiles by keyword
slicecore search-profiles "Bambu Lab A1"

# Show a specific profile
slicecore show-profile orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1

# Use a library profile directly for slicing
slicecore slice model.stl --profile orcaslicer/BBL/process/0.20mm_Standard_X1C
```

**Implementation:**
```rust
/// List profiles with optional filters.
ListProfiles {
    /// Filter by vendor name (e.g., BBL, Creality, Prusa).
    #[arg(long)]
    vendor: Option<String>,

    /// Filter by profile type (filament, process, machine).
    #[arg(long, value_name = "TYPE")]
    profile_type: Option<String>,

    /// Filter by material type (PLA, ABS, PETG, TPU, etc.).
    #[arg(long)]
    material: Option<String>,

    /// List available vendors only (no individual profiles).
    #[arg(long)]
    vendors: bool,

    /// Output as JSON instead of human-readable table.
    #[arg(long)]
    json: bool,
},

/// Search profiles by keyword (matches name, vendor, material, printer model).
SearchProfiles {
    /// Search query (case-insensitive substring match).
    query: String,

    /// Maximum results to show.
    #[arg(short, long, default_value = "20")]
    limit: usize,

    /// Output as JSON.
    #[arg(long)]
    json: bool,
},

/// Show details of a specific profile.
ShowProfile {
    /// Profile ID (e.g., orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1).
    id: String,

    /// Show the full TOML content instead of summary.
    #[arg(long)]
    raw: bool,
},
```

### Pattern 4: Profile Location Discovery
**What:** The CLI needs to find the profiles/ directory at runtime.
**When to use:** For all profile commands.
**Approach:** Check in order: (1) `--profiles-dir` CLI flag, (2) `SLICECORE_PROFILES_DIR` env var, (3) relative to the binary `../profiles/`, (4) compiled-in default path.

```rust
fn find_profiles_dir(cli_override: Option<&Path>) -> Option<PathBuf> {
    // 1. CLI flag
    if let Some(dir) = cli_override {
        return Some(dir.to_path_buf());
    }
    // 2. Environment variable
    if let Ok(dir) = std::env::var("SLICECORE_PROFILES_DIR") {
        return Some(PathBuf::from(dir));
    }
    // 3. Relative to binary
    if let Ok(exe) = std::env::current_exe() {
        let profiles_dir = exe.parent()?.join("profiles");
        if profiles_dir.exists() {
            return Some(profiles_dir);
        }
        // One level up (for cargo run)
        let profiles_dir = exe.parent()?.parent()?.parent()?.parent()?.join("profiles");
        if profiles_dir.exists() {
            return Some(profiles_dir);
        }
    }
    None
}
```

### Anti-Patterns to Avoid
- **Storing original JSON in repo:** Do NOT store the upstream AGPL-3.0 JSON files in the MIT/Apache-2.0 licensed repo. Store only the converted TOML output (a derived work consisting of parameter values, which are likely factual data, not creative expression -- but safer to not bundle originals).
- **Converting base/parent profiles:** Do NOT convert profiles with `"instantiation": "false"`. These are abstract base profiles with many nil fields and only make sense in an inheritance chain. Only convert leaf (instantiated) profiles that have concrete values.
- **Scanning TOML files at search time:** Do NOT read and parse every TOML file when searching. With 7000+ profiles, this would be unacceptably slow. Use the pre-built JSON index.
- **Flattening directory hierarchy:** Do NOT put all 7000+ TOML files in a single directory. Use source/vendor/type/ hierarchy for human browsability and filesystem performance.
- **Including PrusaSlicer INI import:** Do NOT attempt to parse PrusaSlicer's 40K-line monolithic INI files in Phase 15. INI parsing is a different problem from JSON import (multi-profile sections, complex inheritance expressions, conditional compatibility). Defer to a future phase.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON import | Custom JSON parser | `import_upstream_profile()` from Phase 13 | Already handles all OrcaSlicer/BambuStudio quirks |
| TOML conversion | Manual TOML generation | `convert_to_toml()` from Phase 14 | Already handles selective output, float rounding, comments |
| Multi-file merge | Custom merge logic | `merge_import_results()` from Phase 14 | Already handles overlay with deduplication |
| Directory walking | Manual recursive traversal | `walkdir` crate (or `std::fs::read_dir` recursion) | Handles edge cases (symlinks, permissions, Unicode paths) |
| Filename sanitization | Regex-based cleaning | Simple character replacement (spaces->underscores, remove @) | Profile names have predictable characters |
| CLI argument parsing | Manual arg parsing | `clap` (already in CLI crate) | Consistent with existing subcommands |

**Key insight:** The hard work (JSON import + TOML conversion + field mapping) was done in Phases 13-14. Phase 15 is primarily about batch orchestration, directory structure, indexing, and CLI plumbing.

## Common Pitfalls

### Pitfall 1: Incomplete Profiles Due to Unresolved Inheritance
**What goes wrong:** A converted profile like "Bambu ABS @BBL X1C" only overrides 4-5 fields from its parent "Bambu ABS @base", which itself only overrides 4 fields from "fdm_filament_abs". The converted TOML will have most values at PrintConfig defaults, not the actual intended values.
**Why it happens:** OrcaSlicer profiles use deep inheritance chains. Leaf profiles are intentionally sparse -- they rely on parents for most settings.
**How to avoid:** Three strategies: (a) Accept that converted profiles are partial and document this clearly in headers -- users can use them as overlay configs alongside defaults; (b) Attempt to resolve inheritance by walking the `"inherits"` chain and merging parent profiles; (c) Focus on converting "complete" profiles that have many explicit settings (process profiles with `"instantiation": "true"` tend to have more fields than filament profiles).
**Recommendation:** Strategy (b) -- implement inheritance resolution for profiles within the same vendor directory. This is a manageable scope since `"inherits"` always references profiles in the same vendor's namespace. Walk the chain: load child, load parent, merge (child overrides parent). This produces much more useful profiles.
**Warning signs:** Converted filament profiles with default 0.0 bed_temp and 210.0 nozzle_temp -- these values came from PrintConfig::default(), not the actual profile.

### Pitfall 2: Filename Collision From Profile Naming
**What goes wrong:** Profile names like "Bambu ABS @BBL X1C" contain spaces and `@` characters that are problematic in filenames. Multiple profiles for the same material+printer with different nozzle sizes create near-identical names.
**Why it happens:** OrcaSlicer profile names use human-readable conventions with spaces, @, and punctuation.
**How to avoid:** Sanitize filenames: replace spaces with underscores, remove @ and parentheses, preserve the nozzle size suffix. Use the full original name (with sanitized characters) to ensure uniqueness.
**Warning signs:** Files overwriting each other, missing profiles in the output.

### Pitfall 3: Large Git Repository From Thousands of Profiles
**What goes wrong:** Committing 7,628 TOML files (~16MB uncompressed) adds significant weight to the repo, especially since profiles may be regenerated (changed) on every upstream update.
**Why it happens:** Each profile is a small file, but the sheer number adds up.
**How to avoid:** Consider: (a) Store profiles in a separate data submodule; (b) Compress profiles in a tarball that's unpacked at install time; (c) Accept the size -- git compresses text well and 16MB compressed is manageable; (d) Start with a curated subset (top 5-10 vendors: BBL, Creality, Prusa, Anycubic, Elegoo, Voron, Qidi) and expand later.
**Recommendation:** Option (d) -- start with the most popular vendors. 5-10 vendors covers 80%+ of user printers. Full expansion can happen incrementally.
**Warning signs:** `git status` showing thousands of untracked files, slow git operations.

### Pitfall 4: Index Staleness
**What goes wrong:** The `index.json` gets out of sync with the actual TOML files on disk.
**Why it happens:** Manual editing of profiles, partial re-conversion, or filesystem corruption.
**How to avoid:** Generate the index as part of the batch conversion process, never manually. Provide a `rebuild-index` subcommand that re-scans the profiles directory. Include a hash or timestamp in the index for staleness detection.
**Warning signs:** CLI search returning profiles that don't exist on disk, or missing profiles that are present.

### Pitfall 5: AGPL License Contamination
**What goes wrong:** Storing original AGPL-3.0 licensed JSON files in an MIT/Apache-2.0 repo could create licensing obligations.
**Why it happens:** All three upstream slicers (OrcaSlicer, BambuStudio, PrusaSlicer) use AGPL-3.0.
**How to avoid:** Store only converted TOML profiles (parameter values, which are factual data). Do NOT copy original JSON files into the repo. The conversion tool reads from the external slicer-analysis directory at development time but does not distribute the originals. Include clear attribution comments in converted profiles noting the source.
**Warning signs:** JSON files appearing in the profiles/ directory, AGPL license notices in the repo.

## Code Examples

### Batch Conversion with Inheritance Resolution
```rust
// Source: Derived from codebase analysis and upstream profile structure

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::profile_import::{import_upstream_profile, ImportResult};
use crate::profile_convert::convert_to_toml;

/// Resolve the inheritance chain for a profile within its vendor directory.
///
/// Loads the profile and all its ancestors, merging from root to leaf.
/// Returns a fully-resolved ImportResult with all inherited values applied.
fn resolve_inheritance(
    profile_path: &Path,
    vendor_type_dir: &Path,  // e.g., .../BBL/filament/
    cache: &mut HashMap<String, ImportResult>,
) -> Result<ImportResult, EngineError> {
    let contents = std::fs::read_to_string(profile_path)?;
    let json: serde_json::Value = serde_json::from_str(&contents)?;

    let name = json.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let inherits = json.get("inherits").and_then(|v| v.as_str());

    // If we have a parent, resolve it first.
    let base_result = if let Some(parent_name) = inherits {
        // Look for parent profile file in the same type directory.
        let parent_file = vendor_type_dir.join(format!("{}.json", parent_name));
        if parent_file.exists() {
            if let Some(cached) = cache.get(parent_name) {
                cached.clone()
            } else {
                let parent_result = resolve_inheritance(&parent_file, vendor_type_dir, cache)?;
                cache.insert(parent_name.to_string(), parent_result.clone());
                parent_result
            }
        } else {
            // Parent not found -- start from defaults.
            ImportResult {
                config: PrintConfig::default(),
                mapped_fields: vec![],
                unmapped_fields: vec![],
                metadata: ProfileMetadata::default(),
            }
        }
    } else {
        ImportResult {
            config: PrintConfig::default(),
            mapped_fields: vec![],
            unmapped_fields: vec![],
            metadata: ProfileMetadata::default(),
        }
    };

    // Import this profile's explicit fields.
    let this_result = import_upstream_profile(&json)?;

    // Merge: parent as base, this profile's fields overlay.
    let merged = merge_import_results(&[base_result, this_result]);
    Ok(merged)
}
```

### Profile Index Entry Construction
```rust
/// Extract searchable metadata from a profile name and its ImportResult.
fn build_index_entry(
    source: &str,
    vendor: &str,
    profile_type: &str,
    result: &ImportResult,
    out_path: &Path,
    profiles_root: &Path,
) -> ProfileIndexEntry {
    let name = result.metadata.name.clone().unwrap_or_default();

    // Extract material from filament profile name (e.g., "Bambu PLA Basic" -> "PLA").
    let material = if profile_type == "filament" {
        extract_material_from_name(&name)
    } else {
        None
    };

    // Extract layer height from process profile name (e.g., "0.20mm Standard" -> 0.20).
    let layer_height = if profile_type == "process" {
        extract_layer_height_from_name(&name)
    } else {
        None
    };

    // Extract printer model from @-suffix (e.g., "... @BBL X1C" -> "Bambu Lab X1 Carbon").
    let printer_model = extract_printer_model(&name);

    // Extract nozzle size from name (e.g., "... 0.4 nozzle" -> 0.4).
    let nozzle_size = extract_nozzle_size_from_name(&name);

    // Extract quality level from process name (e.g., "0.20mm Standard" -> "standard").
    let quality = if profile_type == "process" {
        extract_quality_from_name(&name)
    } else {
        None
    };

    let relative_path = out_path.strip_prefix(profiles_root)
        .unwrap_or(out_path)
        .to_string_lossy()
        .to_string();

    ProfileIndexEntry {
        id: relative_path.trim_end_matches(".toml").to_string(),
        name,
        source: source.to_string(),
        vendor: vendor.to_string(),
        profile_type: profile_type.to_string(),
        material,
        nozzle_size,
        printer_model,
        path: relative_path,
        layer_height,
        quality,
    }
}

/// Extract material type from a filament profile name.
/// "Bambu PLA Basic @BBL A1" -> Some("PLA")
/// "Generic PETG @System" -> Some("PETG")
fn extract_material_from_name(name: &str) -> Option<String> {
    let materials = [
        "PLA-CF", "PLA-GF", "PLA Silk", "PLA Matte", "PLA High Speed", "PLA+", "PLA",
        "PETG-CF", "PETG HF", "PETG", "ABS-GF", "ABS", "ASA",
        "PA-CF", "PA-GF", "PA", "PC", "PCTG",
        "TPU", "PVA", "HIPS", "PP-CF", "PP-GF", "PP",
        "PE-CF", "PE", "PPA-CF", "PPA-GF",
        "BVOH", "CoPE", "EVA", "PHA", "SBS",
    ];
    let upper_name = name.to_uppercase();
    for mat in &materials {
        if upper_name.contains(&mat.to_uppercase()) {
            return Some(mat.to_string());
        }
    }
    None
}
```

### CLI Search Implementation
```rust
/// Search the profile index by keyword.
fn search_profiles(index: &ProfileIndex, query: &str, limit: usize) -> Vec<&ProfileIndexEntry> {
    let query_lower = query.to_lowercase();
    let terms: Vec<&str> = query_lower.split_whitespace().collect();

    let mut matches: Vec<&ProfileIndexEntry> = index.profiles.iter()
        .filter(|entry| {
            // All terms must match at least one field.
            terms.iter().all(|term| {
                entry.name.to_lowercase().contains(term)
                    || entry.vendor.to_lowercase().contains(term)
                    || entry.material.as_ref().map_or(false, |m| m.to_lowercase().contains(term))
                    || entry.printer_model.as_ref().map_or(false, |p| p.to_lowercase().contains(term))
                    || entry.profile_type.to_lowercase().contains(term)
                    || entry.quality.as_ref().map_or(false, |q| q.to_lowercase().contains(term))
            })
        })
        .collect();

    matches.truncate(limit);
    matches
}
```

## Upstream Profile Data Analysis

### Profile Counts (Verified from filesystem)

| Source | Vendors | Filament | Process | Machine | Total | Instantiated |
|--------|---------|----------|---------|---------|-------|-------------|
| OrcaSlicer | 62 | 5,129 | 2,961 | 1,379 | 9,533* | 7,628 |
| BambuStudio | 12 | 1,935 | 684 | 386 | 3,019 | ~2,400 est. |
| PrusaSlicer | 33 | INI format | INI format | INI format | 36 INI files** | N/A |

*Includes vendor JSON metadata files.
**PrusaSlicer uses monolithic INI files (e.g., PrusaResearch.ini = 40,369 lines) containing all profiles for a vendor in a single file. Not JSON -- requires different parser.

### OrcaSlicer Profile Size (Verified)
- Total JSON size: ~16 MB (9,533 files)
- Average file size: 1,731 bytes
- Most profiles are small (3-20 fields) due to inheritance
- Instantiated profiles tend to be larger than base profiles
- Vendor metadata JSONs (BBL.json, Creality.json, etc.) list machine models and profile references

### Profile Inheritance Depth
- Typical chain: `"Bambu PLA @BBL X1C"` -> `"Bambu PLA @base"` -> `"fdm_filament_pla"` -> `"fdm_filament_common"`
- Depth: 2-4 levels typical
- Leaf profiles (instantiation=true) override only 3-10 fields from parent
- Base profiles have most concrete settings
- Without inheritance resolution, converted profiles are mostly defaults with a few overrides

### Top Vendors by Profile Count (OrcaSlicer)
1. BBL (Bambu Lab) -- largest, most profiles
2. Creality -- second largest
3. Anycubic
4. Prusa
5. Elegoo
6. Flashforge
7. Sovol
8. Voron
9. Qidi
10. Tronxy

### Overlap Between OrcaSlicer and BambuStudio
- BBL vendor: 1,578 profiles exist in both (identical names)
- BambuStudio is a subset of OrcaSlicer for shared vendors
- OrcaSlicer has 50 additional vendors not in BambuStudio
- Recommendation: Use OrcaSlicer as primary source (superset), skip BambuStudio duplicates

### File Naming Patterns
- Filament: `{Material} @{Vendor} {Printer} [{Nozzle}].json` (e.g., "Bambu ABS @BBL X1C.json")
- Process: `{LayerHeight} {Quality} @{Vendor} {Printer} [{Nozzle}].json` (e.g., "0.20mm Standard @BBL X1C.json")
- Machine: `{Vendor} {Model} {Nozzle}.json` (e.g., "Bambu Lab A1 0.4 nozzle.json")
- Base profiles: `fdm_{type}_{material}.json` or `fdm_{type}_common.json`

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single example TOML profile | 1 example profile in examples/profiles/ | Phase 2 | Minimal profile library |
| No upstream profile support | JSON import + TOML conversion | Phases 13-14 | Can import individual profiles |
| No profile discovery | Will add CLI search/list/show | Phase 15 | Users can explore profile library |

**Deprecated/outdated:**
- Nothing deprecated. Phase 15 extends Phases 13-14 without deprecating anything.

## Open Questions

1. **Should we resolve inheritance chains or store sparse profiles?**
   - What we know: Without inheritance resolution, most filament profiles will have only 3-5 non-default fields (e.g., flow_ratio, cost, vendor). With resolution, they would have full temperature, fan, and material settings.
   - What's unclear: How complex is inheritance resolution? Are there circular references or cross-vendor inheritance?
   - Recommendation: Implement inheritance resolution within each vendor directory. The `"inherits"` field always references a profile name (not path), and parent profiles are always in the same vendor's namespace. This is manageable and produces much more useful profiles.

2. **How many vendors should we include in the initial library?**
   - What we know: 62 OrcaSlicer vendors = ~7,628 instantiated profiles. Top 10 vendors likely cover 80%+ of user printers.
   - What's unclear: User demand for niche vendors.
   - Recommendation: Start with top 10-15 vendors for a curated, high-quality initial library. Provide the batch conversion tool so users can generate profiles for any vendor. Add a CI or make target for full regeneration.

3. **Should we merge process+filament+machine into unified TOML profiles?**
   - What we know: OrcaSlicer splits settings across 3 types. Our PrintConfig unifies them. Phase 14's merge_import_results() can combine them.
   - What's unclear: Which combinations to pre-merge? There are O(N^3) possible combinations of process x filament x machine.
   - Recommendation: Do NOT pre-merge. Store each profile type separately. Users can merge at slice time using multiple `--config` flags or the `convert-profile` command. Pre-merging would create an explosion of files.

4. **Where should the batch conversion tool live?**
   - What we know: It needs access to `import_upstream_profile()` and `convert_to_toml()` from slicecore-engine.
   - What's unclear: Should it be a CLI subcommand, a separate binary, or a build script?
   - Recommendation: Add it as a CLI subcommand (`slicecore import-profiles --source-dir ...`) for flexibility. This keeps it in the same binary and reuses the existing infrastructure. Gated behind a feature flag or cfg for builds that don't need it.

5. **PrusaSlicer INI support -- in scope or deferred?**
   - What we know: Phase description says "import profiles from slicer-analysis" which includes PrusaSlicer. But PrusaSlicer uses INI format (not JSON), with 40K-line monolithic files, complex inheritance expressions, and conditional compatibility filters.
   - What's unclear: How much effort INI parsing would add.
   - Recommendation: Defer PrusaSlicer INI import to a future phase. It requires a fundamentally different parser. Focus Phase 15 on OrcaSlicer/BambuStudio JSON (which share the same format and are the dominant ecosystem). Document this as a known limitation.

6. **Should the profiles directory be in the main repo or a separate data repo/submodule?**
   - What we know: 7,628 TOML files = ~16MB uncompressed, likely ~3-4MB compressed in git. Git handles thousands of small text files fine.
   - What's unclear: How often profiles will change and whether churn will bloat history.
   - Recommendation: Store in the main repo for simplicity. If size becomes an issue, refactor to a submodule later. Profile data is a core feature, not optional.

## Sources

### Primary (HIGH confidence)
- **Existing codebase** -- Direct inspection of:
  - `/home/steve/libslic3r-rs/crates/slicecore-engine/src/profile_import.rs` (Phase 13: 440 lines, import_upstream_profile, field mapping)
  - `/home/steve/libslic3r-rs/crates/slicecore-engine/src/profile_convert.rs` (Phase 14: 499 lines, convert_to_toml, merge_import_results)
  - `/home/steve/libslic3r-rs/crates/slicecore-cli/src/main.rs` (CLI: convert-profile subcommand, 709 lines)
  - `/home/steve/libslic3r-rs/crates/slicecore-engine/src/lib.rs` (re-exports)
  - `/home/steve/libslic3r-rs/examples/profiles/pla_standard_0.2mm.toml` (TOML format reference)
- **Actual upstream profile files** -- Direct filesystem inspection:
  - `/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/` (9,533 JSON files, 62 vendors, 16MB)
  - `/home/steve/slicer-analysis/BambuStudio/resources/profiles/` (3,019 JSON files, 12 vendors)
  - `/home/steve/slicer-analysis/PrusaSlicer/resources/profiles/` (36 INI files, 33 vendor directories)
  - Verified: file counts, directory structure, JSON schema, naming patterns, inheritance chains, instantiation flags
- **Phase 13/14 research** -- `/home/steve/libslic3r-rs/.planning/phases/13-json-profile-support/13-RESEARCH.md` and `/home/steve/libslic3r-rs/.planning/phases/14-profile-conversion-tool-json-to-toml/14-RESEARCH.md`
- **Upstream licenses** -- Verified AGPL-3.0 for OrcaSlicer, BambuStudio, and PrusaSlicer

### Secondary (MEDIUM confidence)
- **walkdir crate** -- Standard directory traversal crate for Rust. Widely used (90M+ downloads on crates.io). No Context7 verification performed, but the crate is well-known from training data.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All core libraries already in workspace; only walkdir needs adding
- Architecture: HIGH -- Directory structure, index design, and CLI patterns derived from direct analysis of upstream data and existing codebase
- Inheritance resolution: MEDIUM -- The approach is sound but the edge cases (missing parents, cross-vendor references) need validation during implementation
- Profile counts and structure: HIGH -- Verified directly from filesystem with exact counts
- Licensing analysis: MEDIUM -- AGPL-3.0 confirmed for all three slicers; impact on parameter data redistribution is a legal gray area (factual data vs. creative expression)
- PrusaSlicer scope: HIGH -- Clearly deferred; INI format is fundamentally different

**Research date:** 2026-02-18
**Valid until:** 2026-03-18 (stable domain -- upstream profile formats and directory structures have not changed significantly)
