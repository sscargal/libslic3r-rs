# Phase 21: G-code Analysis and Comparison Tool - Research

**Researched:** 2026-02-25
**Domain:** G-code parsing, analysis, and CLI tooling (Rust)
**Confidence:** HIGH

## Summary

Phase 21 builds a G-code ingestion and analysis pipeline that reads plain `.gcode` files from any major slicer (BambuStudio, OrcaSlicer, PrusaSlicer, or slicecore's own output), extracts structured metrics, and exposes them via two new CLI subcommands: `analyze-gcode` and `compare-gcode`.

The core technical challenges are: (1) a line-by-line G-code parser that tracks machine state (position, extrusion mode, feedrate), (2) auto-detecting the source slicer from header comments and adapting annotation parsing accordingly (BambuStudio uses `; FEATURE:` while PrusaSlicer/OrcaSlicer use `;TYPE:`), (3) accumulating per-layer and per-feature metrics, (4) an independent time estimation model, and (5) N-file comparison with delta computation. The project already has strong infrastructure for this: the `estimation.rs` trapezoid time model, the `statistics.rs` per-feature accumulator pattern, and the `stats_display.rs` three-format output (ASCII table via comfy-table, CSV, JSON).

**Primary recommendation:** Build the G-code analyzer as a new module in `slicecore-engine` (not `slicecore-gcode-io`) since it depends on the time estimation and filament computation logic already in the engine crate. Expose the analysis structs and functions as a public API, then wire two new CLI subcommands in `slicecore-cli`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Default view shows full per-layer detail (not summary-only)
- ANSI colors enabled by default, with `--no-color` flag and TTY detection for piped output
- Three output formats: ASCII table (default), CSV (`--csv`), JSON (`--json`)
- Filament usage reported in all three units: length (mm/m), volume (mm3), and weight estimate (g) with configurable density (default PLA 1.24 g/cm3)
- Report both slicer header time estimate AND an independently computed time estimate, showing delta between them
- Metric units only (mm, mm/s, mm3) -- no imperial conversion
- Plain `.gcode` files only -- no .gcode.gz or .bgcode support in this phase
- Unknown/non-standard G-code commands: skip silently but count them; report "X unknown commands skipped" in summary
- Auto-detect slicer (BambuStudio, OrcaSlicer, PrusaSlicer) from header comments and adapt comment parsing rules accordingly
- Handle both absolute (M82) and relative (M83) extrusion modes transparently
- Support comparing N files (2 or more), not limited to just 2
- First file argument is the baseline -- all deltas computed against it
- Comparison available in same three formats: ASCII table, CSV, JSON
- Speed statistics: min/max/mean per feature type
- Per-layer metrics include ALL of: Z height, move count, travel distance, extrusion distance, retraction count, and layer time estimate
- Full per-layer breakdown is the default view

### Claude's Discretion
- JSON output structure (flat vs hierarchical)
- CSV row layout (one row per layer vs per layer+feature)
- Per-layer feature display layout in ASCII table
- Missing header handling approach
- Comparison column layout
- Delta significance thresholds and color-coding scheme
- Independent time estimate model complexity (feedrate-only vs acceleration-aware)
- Whether to support stdin piping
- Whether to add `--filter` flag for feature type filtering
- Top summary metric selection and ordering
- Whether to include extrusion width consistency analysis

### Deferred Ideas (OUT OF SCOPE)
- .gcode.gz compressed file support
- .bgcode (BambuStudio binary G-code) support
- Speed distribution histograms (text-based)
- Extrusion width consistency analysis
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| comfy-table | 7 | ASCII table rendering | Already used in stats_display.rs, ContentArrangement::Dynamic for auto-sizing |
| serde + serde_json | 1 | JSON serialization | Already workspace dependency, used throughout the project |
| clap | 4.5 | CLI argument parsing | Already used in slicecore-cli with derive feature |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none new) | - | - | All needed dependencies already in workspace |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom G-code parser | nom/pest parser combinator | G-code line grammar is simple enough that line-by-line string parsing is clearer and faster; parser combinators add complexity without benefit |
| comfy-table for comparison | tabled crate | comfy-table already in use, adding another table crate creates inconsistency |
| ANSI color via custom code | colored or owo-colors crate | For Phase 21 ANSI coloring, use inline `\x1b[...m` escape sequences directly -- avoids new dependency for a small feature set |

**Installation:**
No new crate dependencies required. All needed crates are already workspace dependencies.

## Architecture Patterns

### Recommended Module Structure
```
crates/slicecore-engine/src/
├── gcode_analysis.rs        # Core parser + analyzer (new)
├── gcode_analysis/
│   ├── mod.rs               # Re-exports
│   ├── parser.rs            # Line-by-line G-code parser with state tracking
│   ├── slicer_detect.rs     # Auto-detect slicer from header comments
│   ├── metrics.rs           # Metric accumulation structs (per-layer, per-feature)
│   └── comparison.rs        # N-file comparison with delta computation
├── estimation.rs            # (existing) Reuse trapezoid_time for independent estimates
├── statistics.rs            # (existing) Reuse filament_mm_to_grams
└── ...

crates/slicecore-cli/src/
├── main.rs                  # Add AnalyzeGcode + CompareGcode subcommands
├── stats_display.rs         # (existing) Reuse format_time, format_length, format_filament
└── analysis_display.rs      # (new) Display formatting for analysis + comparison output
```

### Pattern 1: Line-by-Line State Machine Parser
**What:** Parse G-code one line at a time, maintaining machine state (X/Y/Z position, current feedrate, extrusion mode, current feature type, current layer).
**When to use:** Always -- G-code is inherently sequential and line-oriented.
**Example:**
```rust
pub struct GcodeParserState {
    // Machine position
    x: f64,
    y: f64,
    z: f64,
    e: f64,                    // Absolute E position (converted from relative if needed)
    feedrate: f64,             // mm/min

    // Extrusion mode
    absolute_extrusion: bool,  // true = M82, false = M83
    absolute_positioning: bool, // true = G90, false = G91

    // Layer tracking
    current_layer_z: f64,
    current_layer_index: usize,

    // Feature tracking (from slicer comments)
    current_feature: Option<String>,

    // Slicer identification
    detected_slicer: Option<SlicerType>,

    // Unknown command tracking
    unknown_command_count: u32,
}
```

### Pattern 2: Slicer Auto-Detection from Header Comments
**What:** Scan the first ~100 lines for slicer identification patterns.
**When to use:** At the start of parsing, before processing moves.
**Key patterns to match:**
```rust
pub enum SlicerType {
    BambuStudio,
    OrcaSlicer,
    PrusaSlicer,
    Slicecore,
    Unknown,
}

// Detection heuristics (from real G-code examination):
// BambuStudio: "; BambuStudio 02.05.00.66" in HEADER_BLOCK
// OrcaSlicer:  "; generated by OrcaSlicer" or "; OrcaSlicer ..."
// PrusaSlicer: "; generated by PrusaSlicer"
// Slicecore:   "; Generated by slicecore" (our own output)
```

### Pattern 3: Feature Annotation Mapping
**What:** Different slicers use different comment formats for feature types.
**BambuStudio annotations (confirmed from real G-code):**
```
; FEATURE: Outer wall
; FEATURE: Inner wall
; FEATURE: Sparse infill
; FEATURE: Internal solid infill
; FEATURE: Bottom surface
; FEATURE: Top surface
; FEATURE: Bridge
; FEATURE: Gap infill
; FEATURE: Overhang wall
; FEATURE: Floating vertical shell
; FEATURE: Custom
```

**PrusaSlicer annotations (confirmed from docs):**
```
;TYPE:External perimeter
;TYPE:Perimeter
;TYPE:Overhang perimeter
;TYPE:Internal infill
;TYPE:Solid infill
;TYPE:Top solid infill
;TYPE:Bridge infill
;TYPE:Internal bridge infill
;TYPE:Gap fill
;TYPE:Skirt/Brim
;TYPE:Support material
;TYPE:Support material interface
;TYPE:Thin wall
;TYPE:Custom
```

**OrcaSlicer annotations:** Uses same format as PrusaSlicer (`; TYPE:`) since it's a PrusaSlicer fork. Feature names may vary slightly.

**Slicecore annotations (confirmed from gcode_gen.rs):**
```
; TYPE:Outer perimeter
; TYPE:Inner perimeter
; TYPE:Solid infill
; TYPE:Sparse infill
; TYPE:Skirt
; TYPE:Brim
; TYPE:Travel
; TYPE:Gap fill
; TYPE:Variable width perimeter
; TYPE:Support
; TYPE:Support interface
; TYPE:Bridge
; TYPE:Ironing
; TYPE:Purge tower
```

### Pattern 4: Header Metadata Extraction
**What:** Parse slicer-specific header comment blocks for metadata.
**BambuStudio header format (confirmed from real G-code):**
```
; HEADER_BLOCK_START
; BambuStudio 02.05.00.66
; model printing time: 9m 48s; total estimated time: 18m 1s
; total layer number: 100
; total filament length [mm] : 1393.21
; total filament volume [cm^3] : 3351.07
; total filament weight [g] : 4.22
; filament_density: 1.26,1.24,1.25,1.24
; filament_diameter: 1.75,1.75,1.75,1.75
; max_z_height: 20.00
; HEADER_BLOCK_END
```

**BambuStudio layer annotations:**
```
; CHANGE_LAYER
; Z_HEIGHT: 0.2
; LAYER_HEIGHT: 0.2
; layer num/total_layer_count: 1/100
```

**PrusaSlicer header format:**
```
; generated by PrusaSlicer 2.8.0
; estimated printing time (normal mode) = 1h 15m 30s
; filament used [mm] = 3870.0
; filament used [cm3] = 9.31
; filament used [g] = 11.73
; filament cost = 0.29
;LAYER_CHANGE
;Z:0.2
;HEIGHT:0.2
```

### Pattern 5: Reuse Existing stats_display Patterns
**What:** The CLI already has format_time, format_length, format_filament, format_ascii_table, format_csv, format_json in stats_display.rs. Reuse these directly for the analysis output formatting.
**Why:** Consistent output style across CLI commands, zero code duplication.

### Anti-Patterns to Avoid
- **Loading entire file into memory as a String then splitting:** G-code files can be hundreds of MB. Use `BufReader` with line-by-line iteration.
- **Parsing G-code into GcodeCommand enum then analyzing:** The existing `GcodeCommand` enum is for *generating* G-code, not parsing it. It lacks the flexibility to handle the diverse comment formats from different slicers. Use a lightweight inline parser instead.
- **Storing every line's data:** Only accumulate metrics (counters, sums, min/max). Don't store individual move data.
- **Sharing GcodeCommand between parser and analyzer:** The parser should work on raw strings; converting to GcodeCommand would lose comment metadata and add unnecessary allocations.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| ASCII table formatting | Manual column alignment | comfy-table 7 | Already used in stats_display, handles dynamic column widths |
| JSON serialization | Manual string building | serde_json | Already workspace dependency |
| Time formatting | New time formatter | stats_display::format_time | Already handles h/m/s with configurable precision |
| Length formatting | New length formatter | stats_display::format_length | Already handles mm/m formatting |
| Filament weight from length | New cross-section computation | statistics::filament_mm_to_grams (make pub) | Already correct, same model used throughout |
| Trapezoid time estimation | New estimation model | estimation::trapezoid_time | Already proven accurate, same function |
| CLI argument parsing | Manual arg parsing | clap 4.5 derive | Already the standard in slicecore-cli |

**Key insight:** About 60% of the needed infrastructure already exists in the codebase. The main new work is the G-code parser (line-by-line state machine) and the comparison logic. Everything around display, formatting, estimation, and CLI structure is a reuse story.

## Common Pitfalls

### Pitfall 1: Extrusion Mode Confusion
**What goes wrong:** Incorrect filament usage totals because relative (M83) vs absolute (M82) extrusion is not tracked.
**Why it happens:** PrusaSlicer defaults to M83 (relative), BambuStudio defaults to M83, but some firmware start in M82 (absolute). The E-axis interpretation differs dramatically.
**How to avoid:** Track `absolute_extrusion` state flag. When M82 (absolute), extrusion amount = current_E - previous_E. When M83 (relative), extrusion amount = E value directly. Reset on G92 E0.
**Warning signs:** Filament totals that are wildly off from header values.

### Pitfall 2: G92 E0 Resets
**What goes wrong:** Accumulated E-axis position becomes incorrect after extruder reset commands.
**Why it happens:** G92 E0 resets the extruder position to zero. In absolute mode, failure to track this reset makes all subsequent E-delta calculations wrong.
**How to avoid:** On G92 E{value}, set the tracked E position to the specified value. This is common: BambuStudio emits G92 E0 at every layer change.
**Warning signs:** Negative extrusion amounts appearing in analysis, or extrusion totals that are orders of magnitude wrong.

### Pitfall 3: Large File Memory Pressure
**What goes wrong:** Loading a 200MB G-code file entirely into a String or Vec causes OOM or poor performance.
**Why it happens:** Real-world G-code files (especially from Benchy or complex prints) can be enormous.
**How to avoid:** Use `BufReader::lines()` for streaming line-by-line parsing. Only accumulate aggregate metrics, never store individual moves.
**Warning signs:** Memory usage scaling linearly with file size, slow startup.

### Pitfall 4: BambuStudio Uses FEATURE, Not TYPE
**What goes wrong:** Parser only looks for `;TYPE:` and misses BambuStudio feature annotations entirely.
**Why it happens:** BambuStudio (and OrcaSlicer based on BambuStudio builds) uses `; FEATURE: Outer wall` format with a space before the keyword, while PrusaSlicer uses `;TYPE:External perimeter` without a space.
**How to avoid:** After slicer auto-detection, use the correct comment parsing rules. BambuStudio: `; FEATURE: {name}`, PrusaSlicer: `;TYPE:{name}`, OrcaSlicer: can use either (detect which is present).
**Warning signs:** Zero feature annotations found despite the file having them.

### Pitfall 5: Feedrate Unit Confusion
**What goes wrong:** Time estimates are off by 60x because feedrate units are confused.
**Why it happens:** G-code feedrates (F parameter) are in mm/min, but the estimation functions work in mm/s.
**How to avoid:** Always convert immediately: `feedrate_mm_per_s = f_value / 60.0`. This is already the pattern in `estimation.rs`.
**Warning signs:** Time estimates that are 60x too short or too long.

### Pitfall 6: Inline Comments Breaking Parameter Parsing
**What goes wrong:** A line like `G1 X10 Y20 ; move to start` has the comment parsed as a parameter.
**Why it happens:** Naive split-on-whitespace includes comment text.
**How to avoid:** Strip everything after the first `;` on non-comment lines before parsing parameters. The existing `validate.rs` already does this correctly (see `trimmed.find(';')`).
**Warning signs:** Parse errors or unknown parameter letters on lines with inline comments.

## Code Examples

### G-code Line Parsing (core pattern)
```rust
// Parse a single G-code line into relevant metrics updates.
fn parse_gcode_line(line: &str, state: &mut GcodeParserState, metrics: &mut LayerMetrics) {
    let trimmed = line.trim();

    // Empty line
    if trimmed.is_empty() {
        return;
    }

    // Full-line comment -- check for annotations
    if trimmed.starts_with(';') {
        parse_comment(trimmed, state, metrics);
        return;
    }

    // Strip inline comment
    let code = if let Some(pos) = trimmed.find(';') {
        trimmed[..pos].trim()
    } else {
        trimmed
    };

    let parts: Vec<&str> = code.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    match parts[0] {
        "G0" => parse_rapid_move(&parts[1..], state, metrics),
        "G1" => parse_linear_move(&parts[1..], state, metrics),
        "G2" | "G3" => parse_arc_move(&parts[1..], state, metrics),
        "G28" => { /* Home -- reset position */ }
        "G90" => state.absolute_positioning = true,
        "G91" => state.absolute_positioning = false,
        "G92" => parse_position_reset(&parts[1..], state),
        "M82" => state.absolute_extrusion = true,
        "M83" => state.absolute_extrusion = false,
        _ => { state.unknown_command_count += 1; }
    }
}
```

### Slicer Detection Pattern
```rust
fn detect_slicer(first_lines: &[String]) -> SlicerType {
    for line in first_lines {
        let lower = line.to_lowercase();
        if lower.contains("bambustudio") {
            return SlicerType::BambuStudio;
        }
        if lower.contains("orcaslicer") {
            return SlicerType::OrcaSlicer;
        }
        if lower.contains("prusaslicer") || lower.contains("prusa slicer") {
            return SlicerType::PrusaSlicer;
        }
        if lower.contains("generated by slicecore") {
            return SlicerType::Slicecore;
        }
    }
    SlicerType::Unknown
}
```

### Independent Time Estimate (reuse trapezoid_time)
```rust
// Source: crates/slicecore-engine/src/estimation.rs
use crate::estimation::trapezoid_time;

fn estimate_move_time(distance: f64, feedrate_mm_s: f64, prev_feedrate: f64, accel: f64) -> f64 {
    let entry_speed = if prev_feedrate > 0.0 {
        feedrate_mm_s.min(prev_feedrate)
    } else {
        0.0
    };
    trapezoid_time(distance, entry_speed, feedrate_mm_s, 0.0, accel)
}
```

### N-File Comparison with Delta
```rust
pub struct ComparisonResult {
    pub baseline: GcodeAnalysis,
    pub others: Vec<GcodeAnalysis>,
    pub deltas: Vec<ComparisonDelta>, // One per non-baseline file
}

pub struct ComparisonDelta {
    pub filename: String,
    pub total_time_delta_s: f64,
    pub total_time_delta_pct: f64,
    pub filament_delta_mm: f64,
    pub filament_delta_pct: f64,
    pub layer_count_delta: i64,
    pub retraction_count_delta: i64,
    // ... per-feature deltas
}
```

## Discretion Recommendations

Based on evidence from the codebase and user's stated use case ("I sliced the same STL in BambuStudio, OrcaSlicer, and PrusaSlicer -- show me how they differ"):

### JSON output structure: Hierarchical
**Recommendation:** Use hierarchical JSON for scripting friendliness.
```json
{
  "header": { "slicer": "BambuStudio", "version": "02.05.00.66", ... },
  "summary": { "total_time_s": 1081, "filament_mm": 1393.21, ... },
  "layers": [ { "z": 0.2, "moves": 42, "extrusion_mm": 14.3, ... } ],
  "features": { "outer_wall": { "time_s": 320, "filament_mm": 450, ... } },
  "speed_stats": { "outer_wall": { "min": 20, "max": 60, "mean": 45 }, ... }
}
```

### CSV row layout: One row per layer
**Recommendation:** One row per layer for spreadsheet workflows. Feature-level breakdown goes into JSON only. CSV is for layer-by-layer comparison in Excel/Google Sheets.

### Missing header handling: Warn + best-effort
**Recommendation:** Parse whatever is available, emit warnings for missing metadata, never fail due to missing header info. All analysis from actual G-code parsing is independent of headers anyway.

### Comparison column layout: Baseline + file columns + delta columns
**Recommendation:** For ASCII table with N files: `| Metric | File1 (base) | File2 | Delta2 | File3 | Delta3 |`

### Delta significance: Color-code >5% deltas
**Recommendation:** Green for improvement (less time/filament), red for regression (more time/filament), white/default for <5% delta. Use `--no-color` to disable.

### Independent time estimate: Feedrate-only (not acceleration-aware)
**Recommendation:** Use naive distance/feedrate as the primary independent estimate for Phase 21. This is simpler and more portable (doesn't assume acceleration values). The delta between naive and header will still be informative. The trapezoid model from `estimation.rs` is available but requires acceleration parameters the user may not know for external G-code files. Offer `--acceleration` flag as optional override for trapezoid estimation.

### stdin piping: Yes, support it
**Recommendation:** Support `slicecore analyze-gcode -` for reading from stdin. This is a standard Unix convention and costs almost nothing to implement (already using BufReader).

### Feature type filtering: Yes, add --filter flag
**Recommendation:** `--filter outer_wall,inner_wall` to show only selected feature types. Small feature, high utility for comparison workflows.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| BambuStudio `;TYPE:` annotations | BambuStudio `; FEATURE:` annotations | BambuStudio 02.x | Must detect both; newer versions use FEATURE |
| Single `;TYPE:` format | Slicer-specific annotation formats | Always (PrusaSlicer vs Bambu fork) | Must auto-detect and adapt |
| G-code text only | Binary G-code (.bgcode) emerging | PrusaSlicer 2.7+ | Out of scope for this phase (deferred) |

**Deprecated/outdated:**
- Older BambuStudio may have used `;TYPE:` format (like PrusaSlicer). Current versions use `; FEATURE:`. The parser should handle both.

## Open Questions

1. **OrcaSlicer annotation format: FEATURE vs TYPE**
   - What we know: OrcaSlicer is a BambuStudio fork. BambuStudio uses `; FEATURE:`. PrusaSlicer uses `;TYPE:`.
   - What's unclear: Does OrcaSlicer use FEATURE (from BambuStudio heritage) or TYPE (from PrusaSlicer heritage)? Likely depends on version.
   - Recommendation: Support both. Auto-detect which is present in the file. This is low-risk since both patterns are simple to check.

2. **Acceleration parameter for independent time estimation**
   - What we know: `trapezoid_time()` requires acceleration. External G-code doesn't specify it.
   - What's unclear: What default acceleration to assume for external slicers.
   - Recommendation: Default to 3000 mm/s^2 (typical for most printers), allow override with `--acceleration` flag. Document that feedrate-only estimate is the baseline.

3. **Filament density for weight estimation**
   - What we know: User specified default PLA 1.24 g/cm^3. BambuStudio headers include filament_density.
   - What's unclear: Whether to prefer header density or CLI-specified density.
   - Recommendation: Use header-reported density when available, fall back to CLI `--density` flag or default 1.24.

## Sources

### Primary (HIGH confidence)
- BambuStudio G-code format: Examined real G-code files at `/home/steve/libslic3r-rs/tmp/gcode-bambu/Cube_PLA.gcode` and `3DBenchy_PLA.gcode`
- Slicecore G-code format: Examined `crates/slicecore-engine/src/gcode_gen.rs` feature_label() and generate_layer_gcode()
- Existing stats infrastructure: Examined `crates/slicecore-cli/src/stats_display.rs` and `crates/slicecore-engine/src/statistics.rs`
- Existing time estimation: Examined `crates/slicecore-engine/src/estimation.rs` trapezoid_time()
- Existing CLI patterns: Examined `crates/slicecore-cli/src/main.rs` Commands enum and subcommand structure

### Secondary (MEDIUM confidence)
- [PrusaSlicer G-code viewer documentation](https://help.prusa3d.com/article/prusaslicer-g-code-viewer_193152) - Confirmed ;TYPE:, ;HEIGHT:, ;LAYER_CHANGE annotations
- [OrcaSlicer G-code output wiki](https://github.com/OrcaSlicer/OrcaSlicer/wiki/others_settings_g_code_output) - General G-code output settings

### Tertiary (LOW confidence)
- OrcaSlicer exact annotation format (FEATURE vs TYPE) -- needs validation with real OrcaSlicer G-code files. Assumption: supports both.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - No new dependencies, all reuse from existing codebase
- Architecture: HIGH - Parser state machine pattern is well-established; module placement follows existing crate conventions
- Pitfalls: HIGH - Confirmed from real G-code file examination (BambuStudio FEATURE vs TYPE, extrusion modes, G92 resets)

**Research date:** 2026-02-25
**Valid until:** 2026-03-25 (stable domain, G-code format rarely changes)
