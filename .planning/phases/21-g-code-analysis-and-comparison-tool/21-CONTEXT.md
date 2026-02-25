# Phase 21: G-code Analysis and Comparison Tool - Context

**Gathered:** 2026-02-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Build a G-code parser and analysis module that ingests G-code files from any slicer (BambuStudio, OrcaSlicer, PrusaSlicer, or our own output) and extracts structured metrics. Expose via two CLI subcommands: `analyze-gcode` for single-file analysis and `compare-gcode` for multi-file comparison with deltas. Plain `.gcode` files only — no compressed or binary formats in this phase.

</domain>

<decisions>
## Implementation Decisions

### Output formatting
- Default view shows full per-layer detail (not summary-only)
- ANSI colors enabled by default, with `--no-color` flag and TTY detection for piped output
- Three output formats: ASCII table (default), CSV (`--csv`), JSON (`--json`)
- Filament usage reported in all three units: length (mm/m), volume (mm³), and weight estimate (g) with configurable density (default PLA 1.24 g/cm³)
- Report both slicer header time estimate AND an independently computed time estimate, showing delta between them
- Metric units only (mm, mm/s, mm³) — no imperial conversion
- Plain `.gcode` files only — no .gcode.gz or .bgcode support in this phase

### Parser behavior
- Unknown/non-standard G-code commands: skip silently but count them; report "X unknown commands skipped" in summary
- Auto-detect slicer (BambuStudio, OrcaSlicer, PrusaSlicer) from header comments and adapt comment parsing rules accordingly (`;TYPE:` format and other annotations differ between slicers)
- Handle both absolute (M82) and relative (M83) extrusion modes transparently — track mode state and compute extrusion correctly regardless

### Comparison display
- Support comparing N files (2 or more), not limited to just 2
- First file argument is the baseline — all deltas computed against it
- Comparison available in same three formats: ASCII table, CSV, JSON

### Metric priorities
- Speed statistics: min/max/mean per feature type (perimeter, infill, support, etc.)
- Per-layer metrics include ALL of: Z height, move count, travel distance, extrusion distance, retraction count, and layer time estimate
- Full per-layer breakdown is the default view

### Claude's Discretion
- JSON output structure (flat vs hierarchical — pick what works best for scripting and programmatic use)
- CSV row layout (one row per layer vs per layer+feature — pick most useful for spreadsheet workflows)
- Per-layer feature display layout in ASCII table (inline sub-rows vs columns — pick based on readability)
- Missing header handling approach (best-effort vs warn+best-effort)
- Comparison column layout (two columns + delta vs interleaved)
- Delta significance thresholds and color-coding scheme for comparison
- Independent time estimate model complexity (feedrate-only vs acceleration-aware — balance accuracy vs complexity)
- Whether to support stdin piping
- Whether to add `--filter` flag for feature type filtering
- Top summary metric selection and ordering
- Whether to include extrusion width consistency analysis

</decisions>

<specifics>
## Specific Ideas

- Should work well for the primary use case: "I sliced the same STL in BambuStudio, OrcaSlicer, and PrusaSlicer — show me how they differ"
- Independent time estimate vs slicer header estimate delta is a key differentiator — shows how accurate each slicer's time prediction is
- Reuse existing `stats_display` patterns from the CLI for table formatting consistency

</specifics>

<deferred>
## Deferred Ideas

- .gcode.gz compressed file support — add in future phase
- .bgcode (BambuStudio binary G-code) support — add in future phase
- Speed distribution histograms (text-based) — could enhance metric depth later
- Extrusion width consistency analysis — deeper quality check for future phase

</deferred>

---

*Phase: 21-g-code-analysis-and-comparison-tool*
*Context gathered: 2026-02-25*
