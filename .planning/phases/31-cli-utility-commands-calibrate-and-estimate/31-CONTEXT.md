# Phase 31: CLI Utility Commands — Calibrate and Estimate - Context

**Gathered:** 2026-03-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `calibrate` subcommand group and enhance `analyze-gcode` with estimation/cost features. Calibrate generates printer-specific calibration G-code (temperature tower, retraction test, flow rate, first layer) using profile-aware defaults. Estimate merges into the existing `analyze-gcode` command with cost model, multi-config comparison, and model-based rough estimation.

**Not in scope:** Calibration result recording/auto-tuning, spool inventory, calibration wizards/sequences, AI-based calibration analysis, interactive Klipper integration, batch estimation, print job history database.

</domain>

<decisions>
## Implementation Decisions

### Calibration Print Types (Phase 31 Scope)
- **Temperature tower**: Stepped tower with programmed temperature changes per block. For dialing in surface finish, stringing, layer adhesion, and bridging per temperature
- **Retraction test**: Tower or multi-post model that sweeps retraction distance and speed to eliminate stringing and blobs
- **Flow rate calibration**: Hollow wall or stepped tower to set extrusion multiplier so wall thickness matches model
- **First layer calibration**: Large single-layer squares/lines/grids to tune Z-offset, elephant's foot compensation, and adhesion across the bed

### Geometry Generation — Hybrid Approach
- **Depends on the test type**: Some calibration models generated programmatically in code, some shipped as STL/3MF files that get sliced through the normal pipeline
- **Programmatic models must be deterministic**: Always generate the same geometry given the same parameters
- **G-code is printer/material-specific**: Generated G-code reflects the user's actual machine and filament profile
- **Can ship models or pre-made G-code with the product** as embedded resources
- **Temperature changes via G-code post-process injection**: Slice the tower model normally, then inject M104/M109 temperature change commands at the correct Z heights. Clean separation between mesh generation and temperature programming

### Profile Integration — Profile with Overrides
- Load machine + filament profiles via -m/-f/-p flags (reuse Phase 30's ProfileResolver)
- Profiles provide defaults: bed size, nozzle diameter, base temperature, retraction distance
- Calibration-specific parameters override via dedicated flags (--start-temp, --end-temp, --step, etc.)
- **Smart defaults from profile**: Temperature tower auto-derives range from filament profile's min_temp/max_temp. Retraction uses profile's retraction_length ± range. Flow uses extrusion_multiplier as baseline

### CLI Structure — Subcommands per Test
- `slicecore calibrate temp-tower -m "Prusa MK4" -f "Generic PLA" --start-temp 190 --end-temp 230 --step 5 -o temp_tower.gcode`
- `slicecore calibrate retraction -m "..." -f "..." --start-distance 0.5 --end-distance 3.0 --step 0.5 -o retraction_test.gcode`
- `slicecore calibrate flow -m "..." -f "..." -o flow_cube.gcode`
- `slicecore calibrate first-layer -m "..." -f "..." --pattern grid -o first_layer.gcode`
- `slicecore calibrate list` — Lists available calibration tests with descriptions

### Companion Instruction Files
- Generate G-code + companion instruction file (e.g., temp_tower.instructions.md)
- **Default format is Markdown**, user can choose output format
- Instructions explain how to read the printed result and what profile settings to adjust
- Embed same info as G-code comments in the G-code file

### Validation and Dry Run
- **Bed size validation**: Error if generated model exceeds printer bed dimensions (from machine profile). Suggest reducing range/steps
- **--dry-run**: Show model dimensions, number of steps/blocks, temperature/parameter range, estimated filament, estimated time — without slicing

### --save-model Flag
- Export the generated calibration mesh as STL/3MF before slicing
- Lets users inspect in a viewer, re-slice with different settings, or share the model

### Reproducibility Metadata
- Embed SliceCore version + calibration parameters in G-code header
- Consistent with Phase 30's reproduce command pattern in G-code header

### Estimate — Merged into analyze-gcode
- **No new `estimate` subcommand** — add estimation/cost features to existing `analyze-gcode` command
- Keeps CLI surface smaller, avoids duplicate functionality

### Estimate Input Types
- **G-code files**: Accurate post-slice estimation using existing trapezoid motion model (`estimate_print_time()`)
- **Model files (STL/3MF)**: Volume-based heuristic for rough estimation (<1 sec, ±30-50%). Good for "will this fit on my spool?" quick checks
- **Saved config TOML**: Accept Phase 30's `--save-config` output for estimation without re-specifying all profile flags

### Full Cost Model
- **Filament cost**: weight × price_per_kg (--filament-price flag)
- **Electricity cost**: print_time × printer_watts × electricity_rate (--electricity-rate, --printer-watts flags)
- **Machine depreciation**: printer_cost / expected_hours × print_time (--printer-cost, --expected-hours flags)
- **Labor rate**: setup_time × hourly_rate (--labor-rate, --setup-time flags)
- **Missing fields shown as "N/A" with hint**: e.g., `Electricity: N/A (provide --electricity-rate and --printer-watts to calculate)`
- Full transparency — user sees everything the tool CAN calculate and knows how to unlock each field

### Cost Defaults in Profiles
- Add `price_per_kg` field to filament profile schema
- Add `watts` field to machine profile schema
- Estimate reads these as defaults; CLI flags override
- Reduces required flags for common use

### Multi-Config Comparison
- Accept multiple filament/config combos in one invocation
- Shows comparison table: time diff, filament diff, cost diff
- "PLA vs PETG on same model" in one command

### Output Formats
- **Table** (default): Human-readable table for terminal use
- **--json**: Programmatic JSON output
- **--csv**: Spreadsheet import for cost tracking
- **--markdown**: For documentation, Discord, GitHub issues
- Consistent across calibrate and analyze-gcode commands

### Claude's Discretion
- Exact calibration model geometry design (tower dimensions, block heights, wall thicknesses)
- Which calibration tests use programmatic mesh vs shipped STL files
- Internal data structures for calibration parameter management
- Volume-based heuristic formula for model estimation
- Markdown instruction template design
- Table formatting and column layout
- Multi-config comparison table layout
- How to handle edge cases in cost model (zero values, very small prints)
- Performance optimization

</decisions>

<specifics>
## Specific Ideas

- Calibration is critical for reproducibility and accuracy — deterministic model generation is non-negotiable
- The hybrid approach (programmatic + shipped models) gives flexibility per test type
- Temperature changes should be injected as G-code post-processing, not baked into the engine
- Full cost model with progressive disclosure — show everything, hint at what's missing
- Multi-config comparison for quick material A vs B decisions
- Markdown as default instruction format aligns with developer/maker audience

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `estimate_print_time()` (estimation.rs): Trapezoid motion model with per-category breakdown — direct reuse for G-code estimation
- `PrintStatistics` / `StatisticsSummary` (statistics.rs): Existing stats with time, filament, cost fields — extend for full cost model
- `PrintTimeEstimate` struct: Already has total_seconds, move_time, travel_time, retraction_count
- `ProfileResolver` (Phase 30): Reuse -m/-f/-p profile resolution for calibration commands
- `cmd_analyze_gcode()`: Existing G-code analysis command to extend with estimation/cost features
- `cmd_compare_gcode()`: Existing comparison infrastructure — may inform multi-config comparison
- `slicecore-mesh`: Mesh generation primitives for programmatic calibration model creation
- `slicecore-gcode-io`: G-code writer for post-process injection of temperature changes
- `cmd_post_process()`: Existing post-processing command — pattern for G-code modification

### Established Patterns
- clap derive macros with nested subcommands (calibrate → temp-tower, retraction, etc.)
- stderr for progress/warnings, stdout for data output
- --json flag pattern for programmatic output
- Structured exit codes (0=success, 1-4 by category)
- Profile flags: -m/-f/-p with ProfileResolver

### Integration Points
- `Commands` enum in main.rs: Add `Calibrate` variant with sub-subcommands
- `cmd_analyze_gcode()`: Extend with cost estimation flags
- `ProfileResolver`: Reuse for calibrate commands
- `PrintConfig` schema: Add `price_per_kg` to filament section, `watts` to machine section
- G-code header embedding: Reuse Phase 30's reproduce command pattern for calibration metadata
- `Engine::new(config)`: Calibrate commands that slice will go through the standard engine pipeline

</code_context>

<deferred>
## Deferred Ideas

### Calibration Ecosystem (Future Phase)
- Calibration result recording & profile auto-update (`calibrate record --best-temp 210`)
- Multi-step calibration wizard sequences (temp → retraction → flow → PA, each feeding the next)
- Parametric calibration arrays — matrix plates sweeping two parameters simultaneously
- Calibration print gallery/registry via plugin system
- AI-assisted calibration analysis from photos of printed tests
- Interactive calibration mode for Klipper (real-time parameter adjustment via moonraker API)
- Print-time measurement models with vernier scales
- Calibration profiles per material-machine combo
- Slicer-driven wizard sequences with persistent profile updates

### Extended Calibration Print Types (Future Phase)
- Pressure advance / linear advance tower
- Max volumetric flow rate test
- Vertical fine artifacts test (Z-banding, resonance)
- Tolerance/clearance test
- Dimensional accuracy / XYZ calibration cube
- Overhang test (stepped angles 30-80°)
- Bridging test (increasing bridge lengths)
- Wall thickness test
- Layer adhesion pull test
- "All-in-one" printer test blocks
- Benchy-style benchmarks
- Organic form tests
- Lattice cube / infill showcase
- Spider/"torture toaster" stress tests
- Resonance / ringing towers and frequency sweeps
- Bed-mesh verification pattern
- Squareness / orthogonality test
- Circularity test (bearing seats)
- Backlash test
- Cooling/overhang tower with fan-speed changes
- Small-feature tower (minis and detail work)
- Support interface test (comparing support patterns)
- Top-surface/infill test plate
- Text/emboss/deboss plate
- Color change tower (AMS/MMU purge testing)
- Purge volume & waste block tests
- Multimaterial alignment test
- Support-material interface test (dual-material)
- Parametric material calibration arrays
- Application-specific "profile packs"
- Clearance cubes with integrated gauges
- Waste-to-useful hybrid designs (calibration strips as cable clips)
- Maker coins with profile info
- Diagnostics "failure atlas" models
- Vibration fingerprint plates
- Advanced supports and bridging suites
- Hyper-fine organic branch tests
- Interactive micro-towers for iterative calibration
- Multi-axis squareness jigs
- "Bed frame" full-area prints

### Estimation & Cost Tracking (Future Phase)
- Spool inventory tracking with remaining filament
- Print job history database for cost tracking over time
- Batch estimation across multiple G-code files
- Print farm cost reporting & aggregation
- Wear/maintenance cost model (nozzle wear, belt replacement)
- Currency and unit system preferences
- Energy cost from real power monitoring (OctoPrint/Moonraker)
- Filament weight estimation with per-material density database
- Multi-material estimation (AMS/MMU waste, purge tower filament)
- Estimation accuracy tracking (estimated vs actual print time)
- Confidence intervals on estimates

### CLI Utilities (Future Phase)
- `slicecore benchmark` — performance benchmark on reference models
- `slicecore doctor` — diagnose setup issues
- `slicecore diff` — semantic G-code diff
- `slicecore stats` — aggregate statistics across G-code directory
- `slicecore check-updates` — version and profile library update check
- Curated STL library (XYZ cube, overhang test, Benchy) as embedded/downloadable resources

</deferred>

---

*Phase: 31-cli-utility-commands-calibrate-and-estimate*
*Context gathered: 2026-03-16*
