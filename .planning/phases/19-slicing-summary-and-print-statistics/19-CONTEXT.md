# Phase 19: Slicing Summary and Print Statistics - Context

**Gathered:** 2026-02-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Generate detailed slicing statistics after G-code generation completes, presenting metrics about print time, filament usage, and per-feature breakdowns in user-selectable formats (ASCII table, CSV, JSON). This provides users with comprehensive print insights for optimization and comparison.

</domain>

<decisions>
## Implementation Decisions

### Output format design
- **ASCII table format:** Display both summary (total time, filament, cost) AND per-feature breakdown table
- **Table columns:** All metrics from GUI screenshots:
  - Feature name
  - Time (with percentage of total and percentage of print)
  - Filament length (mm/m) with percentage
  - Filament weight (g) with percentage
  - Display toggle (for future GUI integration)
- **CSV format:** Machine-optimized flat structure
  - One row per feature with all metrics as columns
  - Standardized column names for easy parsing: `feature,time_s,time_pct_total,time_pct_print,filament_mm,filament_g,filament_pct_total,filament_pct_print`
  - No section headers - pure data rows for tool consumption
- **JSON format:** Separate `PrintStatistics` structure (not extending SliceResult)
  - Independent type that can be embedded or standalone
  - Includes summary fields + features array

### Metrics and calculations
- **New metrics to calculate:**
  - Travel distance (total mm)
  - Move/segment count (total segments)
  - Retraction count + total retraction distance
  - Unretraction count
  - Wipe count + distance
  - Z-hop count + distance
- **Time precision:** Configurable via command option/argument
  - Default: seconds only (38m18s format)
  - Options: deciseconds, milliseconds
  - Applies to all time displays (summary and per-feature)
- **Filament units:** Always show both
  - Length (mm converted to m when > 1000mm)
  - Weight (g)
  - Format: "3.87m / 11.73g"
- **Percentage calculations:** Show BOTH
  - Percentage of total time (includes prepare time, travel, everything)
  - Percentage of print time (excludes prepare time, only actual printing features)
  - Two separate columns: `time_pct_total` and `time_pct_print`
  - Same for filament percentages

### Feature grouping
- **Feature list:** Extensible approach
  - Start with GUI features: Inner wall, Outer wall, Overhang wall, Sparse infill, Internal solid infill, Top surface, Bottom surface, Bridge, Gap infill, Custom, Travel, Retract, Unretract, Wipe, Seams
  - Architecture makes it easy to add new features as they're implemented
  - Each feature is a distinct category for tracking
- **Feature order:** Default to logical flow, user-configurable
  - Default: Print sequence order (walls → infill → top/bottom → support → travel/retract)
  - Command option to sort by: time descending, filament descending, or alphabetical
  - Consistent ordering across all output formats
- **Support grouping:** Subtotal both
  - Support features appear in main list
  - Show subtotals: "Model total", "Support total", "Overall total"
  - Helps users understand support material cost separately
- **Zero features:** Show all features even if unused
  - Display all possible features, even if 0 time/filament
  - Shows what's NOT being used (useful for debugging configs)
  - 0 values clearly marked in output

### CLI integration
- **Display trigger:** Default unless quiet, with save option
  - Statistics display by default after successful slice
  - Suppress with `--quiet` flag
  - Optional save to file: command option to specify filename and path
  - When saving to file, still display to stdout unless quiet
- **Output stream:** stdout
  - All statistics output goes to stdout for easy redirection
  - Consistent with standard command-line tool conventions
- **Format selection:** Claude's discretion
  - Design ergonomic CLI flags for format selection
  - Suggestions: `--stats-format=table|csv|json` or separate flags
  - Ensure mutually exclusive format options
  - Default format: ASCII table for human readability
- **JSON integration:** Optional field
  - When `--json` flag is used, include statistics in SliceResult by default
  - Add `--json-no-stats` flag to exclude statistics from JSON output
  - Allows users to get compact JSON when they only need core slice data
  - Statistics structure is separate but embedded in the result

### Claude's Discretion
- Exact ASCII table formatting and column widths
- CSV column ordering optimization
- JSON schema field naming conventions
- Flag naming for format selection and file output
- Error handling when statistics calculation fails
- Progress reporting during statistics calculation (if slow)

</decisions>

<specifics>
## Specific Ideas

- User referenced GUI screenshots showing two views:
  - Summary view: Total filament (3.87m / 11.73g), Cost (0.29), Total time (38m18s)
  - Line Type view: Per-feature breakdown with columns for Line Type, Time, Percent, Used filament, Display toggle
- User wants this for future TSP optimization comparisons
  - Statistics should be detailed enough to compare different travel path algorithms
  - Travel distance and segment count are critical for optimization analysis
- User asked: "What metrics and telemetry are already included? What new metrics must be implemented to be useful to users and AI?"
  - Implies AI/ML analysis as a future use case for this data
  - Statistics should be machine-parseable for automated analysis

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 19-slicing-summary-and-print-statistics*
*Context gathered: 2026-02-23*
