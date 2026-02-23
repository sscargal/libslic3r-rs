# Phase 19: Slicing Summary and Print Statistics - Research

**Researched:** 2026-02-23
**Domain:** Post-slice statistics computation, multi-format CLI output, per-feature metric aggregation
**Confidence:** HIGH

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Output format design:**
- ASCII table format: Display both summary (total time, filament, cost) AND per-feature breakdown table
- Table columns: Feature name, Time (with pct total and pct print), Filament length (mm/m) with pct, Filament weight (g) with pct, Display toggle (for future GUI)
- CSV format: Machine-optimized flat structure, one row per feature, standardized column names: `feature,time_s,time_pct_total,time_pct_print,filament_mm,filament_g,filament_pct_total,filament_pct_print`, no section headers
- JSON format: Separate `PrintStatistics` structure (not extending SliceResult), independent type that can be embedded or standalone, includes summary fields + features array

**Metrics and calculations:**
- New metrics: Travel distance (total mm), Move/segment count, Retraction count + total retraction distance, Unretraction count, Wipe count + distance, Z-hop count + distance
- Time precision: Configurable via command option (default: seconds "38m18s", options: deciseconds, milliseconds)
- Filament units: Always show both length (mm/m) and weight (g), format "3.87m / 11.73g"
- Percentage calculations: Show BOTH pct of total time and pct of print time (two separate columns), same for filament

**Feature grouping:**
- Feature list: Extensible approach starting with GUI features (Inner wall, Outer wall, Overhang wall, Sparse infill, Internal solid infill, Top surface, Bottom surface, Bridge, Gap infill, Custom, Travel, Retract, Unretract, Wipe, Seams)
- Feature order: Default logical flow (walls -> infill -> top/bottom -> support -> travel/retract), configurable sort (time desc, filament desc, alphabetical)
- Support grouping: Subtotals for "Model total", "Support total", "Overall total"
- Zero features: Show all features even if unused (0 values clearly marked)

**CLI integration:**
- Display trigger: Default after successful slice unless `--quiet`, optional save to file
- Output stream: stdout for all statistics
- JSON integration: Include statistics in SliceResult when `--json` used, add `--json-no-stats` to exclude

### Claude's Discretion
- Exact ASCII table formatting and column widths
- CSV column ordering optimization
- JSON schema field naming conventions
- Flag naming for format selection and file output
- Error handling when statistics calculation fails
- Progress reporting during statistics calculation (if slow)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope

</user_constraints>

## Summary

This phase adds comprehensive post-slice statistics computation and multi-format display to the slicing pipeline. The existing codebase already has the two foundational pieces: (1) `PrintTimeEstimate` with total/move/travel time and retraction count from the trapezoid motion model, and (2) `FilamentUsage` with length/weight/cost. However, these are aggregate totals without per-feature breakdown. The core work is computing per-feature statistics from the `LayerToolpath` data (which already tags every segment with `FeatureType`), building a `PrintStatistics` type to hold the results, and rendering in three formats (ASCII table, CSV, JSON).

The `LayerToolpath` / `ToolpathSegment` structures are the ideal data source for per-feature statistics. Each segment carries `feature: FeatureType`, `e_value: f64`, `feedrate: f64`, `start/end: Point2`, and `z: f64`. By iterating all segments across all layers, we can accumulate per-feature time, filament length, filament weight, travel distance, and segment count. The existing `FeatureType` enum already covers 14 feature types (OuterPerimeter, InnerPerimeter, SolidInfill, SparseInfill, Skirt, Brim, GapFill, VariableWidthPerimeter, Support, SupportInterface, Bridge, Ironing, PurgeTower, Travel). Additional metrics like retraction/unretraction/wipe/z-hop counts must be extracted from the G-code command stream since those operations are generated in `gcode_gen.rs` and are not represented in toolpath segments.

The formatting layer should live in the CLI crate (not the engine) to avoid pulling table formatting dependencies into the WASM-compatible engine. The engine crate exports a `PrintStatistics` struct; the CLI formats it. For ASCII tables, `comfy-table` v7.2.2 is the recommended crate -- it is well-maintained, has no unsafe code, provides automatic column width calculation, and its default feature set is minimal (the `tty` feature auto-detects terminal width). CSV output is simple enough to hand-write (one header + N data rows with comma separation), avoiding an external dependency. JSON uses the existing `serde_json` already in the workspace.

**Primary recommendation:** Compute per-feature statistics from `LayerToolpath` segments in the engine, extract retraction/wipe/z-hop metrics from G-code commands, export a `PrintStatistics` struct from the engine, and format output in the CLI using `comfy-table` for ASCII tables.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde + serde_json | 1.x (workspace) | JSON serialization of PrintStatistics | Already in workspace, used everywhere |
| comfy-table | 7.2.2 | ASCII table formatting in CLI | Well-maintained, no unsafe, auto-width, MIT license |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| clap | 4.5 (workspace) | CLI flag parsing for --stats-format, --quiet, etc. | Already used for all CLI subcommands |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| comfy-table | tabled 0.20 | tabled has derive macros (nice for structs) but larger dependency; comfy-table is simpler and sufficient |
| comfy-table | prettytable-rs | prettytable-rs is less actively maintained (last release 2019) |
| csv crate | hand-written CSV | For simple flat rows, hand-written is fewer dependencies and trivially correct; csv crate adds ~100KB for features we don't need |
| hand-written CSV | csv 1.x | Use csv crate only if we need quoting of fields containing commas/newlines; our numeric data doesn't need it |

**Installation:**
```toml
# In crates/slicecore-cli/Cargo.toml
[dependencies]
comfy-table = "7"
```

No new dependencies needed in slicecore-engine (uses existing serde/serde_json).

## Architecture Patterns

### Recommended Module Structure
```
crates/slicecore-engine/src/
    statistics.rs           # PrintStatistics, FeatureStatistics, compute_statistics()
    estimation.rs           # (existing) PrintTimeEstimate, estimate_print_time()
    filament.rs             # (existing) FilamentUsage, estimate_filament_usage()
    output.rs               # (existing) SliceMetadata -- embed PrintStatistics here
    engine.rs               # (existing) SliceResult -- add statistics field

crates/slicecore-cli/src/
    main.rs                 # (existing) Add --stats-format, --quiet, --stats-file, --json-no-stats flags
    stats_display.rs        # NEW: format_table(), format_csv(), format_json() for statistics
```

### Pattern 1: Two-Phase Statistics Collection
**What:** Collect per-feature metrics from two sources: toolpath segments (for time, filament, travel) and G-code commands (for retraction, unretraction, wipe, z-hop counts).
**When to use:** Always -- the toolpath has feature type metadata, but retraction/wipe/z-hop are generated in gcode_gen and only appear in the command stream.
**Example:**
```rust
// Phase 1: Toolpath-based per-feature statistics
pub fn compute_toolpath_statistics(
    layer_toolpaths: &[LayerToolpath],
    config: &PrintConfig,
) -> ToolpathStats {
    let mut per_feature: HashMap<FeatureType, FeatureAccumulator> = HashMap::new();

    for toolpath in layer_toolpaths {
        for seg in &toolpath.segments {
            let acc = per_feature.entry(seg.feature).or_default();
            let seg_len = seg.length();
            acc.segment_count += 1;
            acc.total_distance_mm += seg_len;

            if seg.feature == FeatureType::Travel {
                acc.travel_distance_mm += seg_len;
            } else {
                acc.filament_mm += seg.e_value;
                // Time estimate: seg_len / (feedrate_mm_min / 60.0)
                let feedrate_mm_s = seg.feedrate / 60.0;
                if feedrate_mm_s > 0.0 {
                    acc.time_seconds += seg_len / feedrate_mm_s;
                }
            }
        }
    }
    // ...
}

// Phase 2: G-code-based metrics (retraction, wipe, z-hop counts)
pub fn compute_gcode_metrics(commands: &[GcodeCommand]) -> GcodeMetrics {
    let mut retraction_count = 0u32;
    let mut unretraction_count = 0u32;
    let mut total_retraction_distance = 0.0f64;
    let mut z_hop_count = 0u32;
    let mut z_hop_distance = 0.0f64;

    for cmd in commands {
        match cmd {
            GcodeCommand::Retract { distance, .. } => {
                retraction_count += 1;
                total_retraction_distance += distance;
            }
            GcodeCommand::Unretract { distance, .. } => {
                unretraction_count += 1;
            }
            // Z-hop detection: RapidMove with only Z changing after a Retract
            _ => {}
        }
    }
    // ...
}
```

### Pattern 2: Independent PrintStatistics Type
**What:** A standalone `PrintStatistics` struct that is not embedded in `SliceResult` but can optionally be included in JSON output.
**When to use:** Per user decision -- the type is independent, computed after slicing, and optionally embedded.
**Example:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintStatistics {
    /// Summary totals
    pub summary: StatisticsSummary,
    /// Per-feature breakdown
    pub features: Vec<FeatureStatistics>,
    /// G-code metrics (retraction, wipe, z-hop)
    pub gcode_metrics: GcodeMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsSummary {
    pub total_time_seconds: f64,
    pub print_time_seconds: f64,  // excludes travel/retract/prepare
    pub total_filament_mm: f64,
    pub total_filament_m: f64,
    pub total_filament_g: f64,
    pub total_filament_cost: f64,
    pub total_travel_distance_mm: f64,
    pub total_segments: u64,
    pub layer_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStatistics {
    pub feature: String,           // human-readable name
    pub time_seconds: f64,
    pub time_pct_total: f64,       // % of total time (including travel, retract)
    pub time_pct_print: f64,       // % of print time (excluding travel, retract)
    pub filament_mm: f64,
    pub filament_m: f64,
    pub filament_g: f64,
    pub filament_pct_total: f64,
    pub filament_pct_print: f64,
    pub segment_count: u64,
    pub is_support: bool,          // for support subtotaling
}
```

### Pattern 3: Formatting in CLI Layer
**What:** Statistics formatting (table/CSV/JSON) happens in the CLI crate, not the engine.
**When to use:** Always -- keeps the engine WASM-compatible and dependency-light.
**Example:**
```rust
// In crates/slicecore-cli/src/stats_display.rs
use comfy_table::{Table, ContentArrangement, presets::UTF8_FULL};

pub fn format_ascii_table(stats: &PrintStatistics, precision: TimePrecision) -> String {
    // Summary section
    let mut output = String::new();
    output.push_str(&format!("Total time: {}\n", format_time(stats.summary.total_time_seconds, precision)));
    output.push_str(&format!("Filament: {} / {:.2}g\n",
        format_length(stats.summary.total_filament_mm),
        stats.summary.total_filament_g));
    output.push_str(&format!("Cost: {:.2}\n\n", stats.summary.total_filament_cost));

    // Per-feature table
    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Feature", "Time", "% Total", "% Print", "Filament", "Weight", "% Total"]);

    for feat in &stats.features {
        table.add_row(vec![
            &feat.feature,
            &format_time(feat.time_seconds, precision),
            &format!("{:.1}%", feat.time_pct_total),
            &format!("{:.1}%", feat.time_pct_print),
            &format_length(feat.filament_mm),
            &format!("{:.2}g", feat.filament_g),
            &format!("{:.1}%", feat.filament_pct_total),
        ]);
    }

    output.push_str(&table.to_string());
    output
}
```

### Anti-Patterns to Avoid
- **Computing statistics from G-code text parsing:** Do NOT re-parse the G-code string output. Use the structured `GcodeCommand` stream and `LayerToolpath` segments which have typed, structured data.
- **Embedding comfy-table in slicecore-engine:** The engine crate must remain WASM-compatible. Table formatting is a CLI concern.
- **Extending SliceResult with statistics fields:** Per user decision, `PrintStatistics` is a separate type. The engine computes it, but it is not a field on `SliceResult`. When `--json` is used, the CLI embeds it in the output.
- **Using naive time estimates for per-feature breakdown:** The existing `estimate_print_time` uses trapezoid model on G-code. For per-feature time, use segment-level calculation (distance/feedrate) which is simpler but consistent with the per-segment toolpath data. The global trapezoid estimate is still used for total time.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| ASCII table formatting | Custom column-width calculator with padding | `comfy-table` 7.x | Column width auto-calculation, Unicode support, terminal width detection, well-tested |
| JSON serialization | Manual JSON string building | `serde_json` (workspace) | Already available, handles escaping, derives work on all types |
| Time formatting (38m18s) | - | Hand-write (trivial) | Only needs hours/minutes/seconds formatting, not worth a dependency |
| CSV output | - | Hand-write (trivial) | Flat numeric rows, no quoting needed, simpler than adding csv crate |

**Key insight:** The only non-trivial formatting task is ASCII tables with aligned columns. Everything else (CSV rows, JSON, time formatting) is simple enough to hand-write. Using comfy-table for the one complex case and hand-writing the rest is the right balance.

## Common Pitfalls

### Pitfall 1: Double-Counting Filament in Retraction Cycles
**What goes wrong:** Retraction moves have negative E-values, unretractions have positive E-values. If you count all positive E-values, you double-count filament that was retracted and then re-extruded.
**Why it happens:** The existing `estimate_filament_usage` already handles this correctly (only counts positive E from LinearMove/ArcMove, not Unretract commands). But when computing per-feature from toolpath segments, the retraction/unretraction is not in the toolpath -- it's in the G-code command stream.
**How to avoid:** For toolpath-based filament calculation, only sum `e_value` from extrusion segments (not Travel). Retraction/unretraction filament is not counted as "used" filament -- it's just the same filament moving back and forth.
**Warning signs:** Total filament from per-feature sum doesn't match `FilamentUsage.length_mm`.

### Pitfall 2: Per-Feature Time vs Total Time Mismatch
**What goes wrong:** Sum of per-feature times from toolpath (distance/feedrate) doesn't match the trapezoid-model total time.
**Why it happens:** The toolpath-based time estimate uses naive distance/feedrate. The trapezoid model accounts for acceleration/deceleration, adding 10-30% more time. Also, retraction overhead (0.5s each) and layer change overhead (0.2s each) are in the trapezoid model but not in toolpath segments.
**How to avoid:** Use the toolpath-based per-feature times for the BREAKDOWN (relative proportions), but scale them to match the trapezoid total time. Alternatively, document clearly that per-feature times are naive estimates while total time uses the trapezoid model. The user decided on percentage columns (pct_total, pct_print) which naturally handle this -- percentages are valid regardless of absolute accuracy.
**Warning signs:** Per-feature times sum to less than total time by 15-30%.

### Pitfall 3: Feature Mapping Mismatch Between User's List and FeatureType Enum
**What goes wrong:** The user's feature list (from GUI screenshots) uses names like "Inner wall", "Outer wall", "Overhang wall", "Top surface", "Bottom surface" etc. The codebase's `FeatureType` enum uses different names and groupings: `InnerPerimeter`, `OuterPerimeter`, `SolidInfill` (covers both top and bottom), etc.
**Why it happens:** The user's list is based on OrcaSlicer's GUI which has more granular feature types. The current `FeatureType` enum doesn't distinguish top vs bottom solid infill, or overhang wall vs normal wall.
**How to avoid:** Map the existing FeatureType variants to user-facing display names. Some features in the user's list (Overhang wall, Top surface, Bottom surface, Seams) don't have separate FeatureType variants yet. Either: (a) add new FeatureType variants for finer granularity, or (b) use the existing granularity and note that future phases can refine it. Given the "extensible approach" decision, option (b) is appropriate for now -- start with existing FeatureType variants and expand later.
**Warning signs:** User expects to see "Top surface" and "Bottom surface" as separate rows but they're both "Solid infill".

### Pitfall 4: Wipe Metrics Are Not Currently Tracked
**What goes wrong:** The user wants wipe count + distance, but the current G-code generation does not emit explicit wipe moves. The only wipe-related code is in `multimaterial.rs` for purge tower wipe moves.
**Why it happens:** Standard wipe-on-retraction (wiping the nozzle along the perimeter during retraction to reduce stringing) is not yet implemented in the retraction planner. The scarf joint module handles seam transitions but doesn't have a "wipe" move per se.
**How to avoid:** Report wipe metrics as 0 for now, with the infrastructure to track them when wipe-on-retraction is implemented. The architecture should support adding wipe tracking later without restructuring.
**Warning signs:** User expects non-zero wipe counts but sees all zeros.

### Pitfall 5: Z-hop Detection in G-code Stream
**What goes wrong:** Z-hops are not a single G-code command -- they're a sequence: Retract -> RapidMove(Z+hop) -> Travel -> RapidMove(Z-hop) -> Unretract. Counting z-hops requires stateful parsing of the command stream.
**Why it happens:** Z-hop is a coordinated sequence, not an atomic command.
**How to avoid:** Use a state machine approach: after seeing a Retract command, look for a subsequent RapidMove with only Z increasing. Count that as a z-hop. Track the z-hop distance as the delta between the pre-retract Z and the hop Z.
**Warning signs:** Z-hop count doesn't match retraction count (not all retractions have z-hops, depending on config).

## Code Examples

### Computing Per-Feature Statistics from Toolpaths
```rust
// Source: Derived from existing codebase patterns in toolpath.rs and estimation.rs

use std::collections::HashMap;
use crate::toolpath::{FeatureType, LayerToolpath};
use crate::config::PrintConfig;

#[derive(Default)]
struct FeatureAccumulator {
    time_seconds: f64,
    filament_mm: f64,
    distance_mm: f64,
    segment_count: u64,
}

pub fn compute_per_feature_stats(
    layer_toolpaths: &[LayerToolpath],
    config: &PrintConfig,
) -> HashMap<FeatureType, FeatureAccumulator> {
    let mut accumulators: HashMap<FeatureType, FeatureAccumulator> = HashMap::new();

    for toolpath in layer_toolpaths {
        for seg in &toolpath.segments {
            let acc = accumulators.entry(seg.feature).or_default();
            let seg_len = seg.length();
            acc.segment_count += 1;
            acc.distance_mm += seg_len;
            acc.filament_mm += seg.e_value; // 0 for travel

            let feedrate_mm_s = seg.feedrate / 60.0;
            if feedrate_mm_s > 0.0 {
                acc.time_seconds += seg_len / feedrate_mm_s;
            }
        }
    }

    accumulators
}
```

### Extracting Retraction/Z-hop Metrics from G-code Commands
```rust
// Source: Derived from existing estimation.rs pattern

use slicecore_gcode_io::GcodeCommand;

pub struct GcodeMetrics {
    pub retraction_count: u32,
    pub unretraction_count: u32,
    pub total_retraction_distance_mm: f64,
    pub z_hop_count: u32,
    pub total_z_hop_distance_mm: f64,
    pub total_travel_distance_mm: f64,
    pub total_move_count: u64,
}

pub fn extract_gcode_metrics(commands: &[GcodeCommand]) -> GcodeMetrics {
    let mut metrics = GcodeMetrics::default();
    let mut cur_z = 0.0f64;
    let mut just_retracted = false;

    for cmd in commands {
        match cmd {
            GcodeCommand::Retract { distance, .. } => {
                metrics.retraction_count += 1;
                metrics.total_retraction_distance_mm += distance;
                just_retracted = true;
            }
            GcodeCommand::Unretract { .. } => {
                metrics.unretraction_count += 1;
            }
            GcodeCommand::RapidMove { z: Some(z), x, y, .. } => {
                if just_retracted && x.is_none() && y.is_none() && *z > cur_z {
                    // Z-hop detected: Z increased right after retraction
                    metrics.z_hop_count += 1;
                    metrics.total_z_hop_distance_mm += z - cur_z;
                }
                cur_z = *z;
                just_retracted = false;
            }
            _ => {
                just_retracted = false;
            }
        }
    }

    metrics
}
```

### Time Formatting
```rust
pub enum TimePrecision {
    Seconds,       // "38m18s"
    Deciseconds,   // "38m18.3s"
    Milliseconds,  // "38m18.312s"
}

pub fn format_time(seconds: f64, precision: TimePrecision) -> String {
    let hours = (seconds / 3600.0).floor() as u64;
    let mins = ((seconds % 3600.0) / 60.0).floor() as u64;
    let secs = seconds % 60.0;

    let time_str = match precision {
        TimePrecision::Seconds => format!("{:.0}s", secs.floor()),
        TimePrecision::Deciseconds => format!("{:.1}s", secs),
        TimePrecision::Milliseconds => format!("{:.3}s", secs),
    };

    if hours > 0 {
        format!("{}h{}m{}", hours, mins, time_str)
    } else if mins > 0 {
        format!("{}m{}", mins, time_str)
    } else {
        time_str
    }
}
```

### Filament Length Formatting
```rust
pub fn format_length(mm: f64) -> String {
    if mm >= 1000.0 {
        format!("{:.2}m", mm / 1000.0)
    } else {
        format!("{:.1}mm", mm)
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Aggregate-only statistics (total time, total filament) | Per-feature breakdown with percentage columns | OrcaSlicer 2.0+ (2024) | Users expect per-feature visibility for optimization |
| Text-only output | Multiple output formats (table, CSV, JSON) | CLI tool best practice | Machine-parseable output enables automation and AI analysis |
| Time as single number | Percentage-of-total and percentage-of-print as separate metrics | OrcaSlicer GUI | Distinguishes "time printing" from "time total" for better optimization insight |

**Current codebase state:**
- `PrintTimeEstimate`: Has total, move, travel, retraction_count -- no per-feature breakdown
- `FilamentUsage`: Has total length, weight, cost -- no per-feature breakdown
- `FeatureType` enum: 14 variants covering all major print features
- `LayerToolpath.segments`: Each segment carries feature type, E-value, feedrate, start/end coordinates -- all needed for per-feature calculation
- G-code TYPE: comments: Already emitted at feature transitions in gcode_gen.rs

## Open Questions

1. **Per-feature time accuracy: naive vs trapezoid**
   - What we know: The toolpath gives us segment_length/feedrate for each feature. The global time uses the trapezoid model which adds 15-30% for acceleration. Per-feature trapezoid would require tracking previous speeds across feature boundaries.
   - What's unclear: Whether users expect per-feature times to sum exactly to total time, or if approximate relative proportions are sufficient.
   - Recommendation: Use naive per-feature times, display percentages (which are accurate relative to each other), and note in output that total time uses the trapezoid model. Alternatively, scale per-feature times by (trapezoid_total / naive_total) as a correction factor.

2. **Feature granularity gap**
   - What we know: The user wants "Top surface" and "Bottom surface" as separate rows. The codebase has only `SolidInfill` for both. Similarly, "Overhang wall" is not distinguished from regular outer perimeter, and "Seams" don't have dedicated segments.
   - What's unclear: Whether to add new FeatureType variants now or defer.
   - Recommendation: Use existing FeatureType variants for now. Map them to user-friendly display names. Add a note in output that finer granularity (top/bottom surface, overhang wall) will come in future phases. The extensible architecture makes this easy to add later.

3. **Wipe move tracking**
   - What we know: The user explicitly requested wipe count + distance. The codebase does not currently generate wipe moves (except for purge tower in multi-material).
   - What's unclear: Whether to implement wipe-on-retraction in this phase or just track zero wipes.
   - Recommendation: Track wipe metrics as 0 for now. The statistics infrastructure will be ready when wipe-on-retraction is implemented. Document this in the output.

## Sources

### Primary (HIGH confidence)
- Codebase: `crates/slicecore-engine/src/estimation.rs` -- PrintTimeEstimate struct and trapezoid model
- Codebase: `crates/slicecore-engine/src/filament.rs` -- FilamentUsage struct and computation
- Codebase: `crates/slicecore-engine/src/toolpath.rs` -- FeatureType enum, ToolpathSegment, LayerToolpath
- Codebase: `crates/slicecore-engine/src/gcode_gen.rs` -- G-code generation with TYPE: comments, feature_label()
- Codebase: `crates/slicecore-engine/src/engine.rs` -- SliceResult, slice pipeline, time/filament estimation
- Codebase: `crates/slicecore-cli/src/main.rs` -- CLI structure, existing flags, output patterns
- Codebase: `crates/slicecore-gcode-io/src/commands.rs` -- GcodeCommand enum (Retract, Unretract, RapidMove, etc.)

### Secondary (MEDIUM confidence)
- [comfy-table crates.io](https://crates.io/crates/comfy-table) -- v7.2.2, last updated 2026-01-13, MIT license
- [tabled crates.io](https://crates.io/crates/tabled) -- v0.20.0, last updated 2025-06-04
- [csv crate docs.rs](https://docs.rs/csv) -- v1.x, BurntSushi, widely used but overkill for our flat numeric CSV
- [comfy-table docs.rs](https://docs.rs/comfy-table) -- API documentation for table building

### Tertiary (LOW confidence)
- [OrcaSlicer per-feature statistics issue #2252](https://github.com/SoftFever/OrcaSlicer/issues/2252) -- Feature request for per-feature filament calculation
- [OrcaSlicer time display issue #8717](https://github.com/SoftFever/OrcaSlicer/issues/8717) -- Discussion about statistics display in preview

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries verified via crates.io, existing workspace dependencies confirmed
- Architecture: HIGH -- based on direct codebase analysis of existing types and pipeline
- Pitfalls: HIGH -- identified from actual codebase behavior (retraction handling, FeatureType mapping, z-hop sequences)

**Research date:** 2026-02-23
**Valid until:** 60 days (stable domain, no fast-moving dependencies)
