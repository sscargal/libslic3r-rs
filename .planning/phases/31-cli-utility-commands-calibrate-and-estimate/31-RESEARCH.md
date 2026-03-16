# Phase 31: CLI Utility Commands -- Calibrate and Estimate - Research

**Researched:** 2026-03-16
**Domain:** CLI calibration G-code generation + cost estimation for a Rust 3D slicer
**Confidence:** HIGH

## Summary

Phase 31 adds two major CLI feature areas: (1) a `calibrate` subcommand group that generates printer-specific calibration G-code (temperature tower, retraction test, flow rate, first layer), and (2) cost estimation features merged into the existing `analyze-gcode` command. Both build heavily on existing infrastructure -- the engine's `PrintConfig`, `ProfileResolver`, mesh primitives, `estimate_print_time()`, and the established CLI patterns (clap derive, `--json`, `comfy-table`, exit codes).

The calibration system uses a hybrid approach: some models generated programmatically via the existing CSG primitives (`primitive_box`, `primitive_cylinder`, etc.), others potentially shipped as embedded STL resources. Temperature changes are injected as G-code post-processing (using existing `GcodeCommand::SetExtruderTemp`). The estimation/cost features extend the existing `GcodeAnalysis` with cost fields and add volume-based rough estimation for model files.

**Primary recommendation:** Structure as two parallel work streams (calibrate infrastructure + estimate/cost), with calibrate using the existing CSG primitives + mesh union for model generation, and estimate extending the existing `cmd_analyze_gcode` flow with cost model and multi-config comparison.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Calibration print types (Phase 31 scope):** Temperature tower, retraction test, flow rate calibration, first layer calibration
- **Geometry generation -- hybrid approach:** Some models programmatic, some shipped STL/3MF; temperature changes via G-code post-process injection (M104/M109 at correct Z heights)
- **Profile integration:** Load profiles via -m/-f/-p flags (reuse Phase 30's ProfileResolver); calibration-specific params override via dedicated flags
- **CLI structure:** Subcommands per test under `calibrate` group (temp-tower, retraction, flow, first-layer, list)
- **Companion instruction files:** G-code + markdown instruction file; embed info as G-code comments too
- **Validation and dry run:** Bed size validation, --dry-run shows dimensions/steps/range/estimates without slicing
- **--save-model flag:** Export calibration mesh as STL/3MF before slicing
- **Reproducibility metadata:** Embed SliceCore version + calibration params in G-code header
- **Estimate merged into analyze-gcode:** No new `estimate` subcommand; add estimation/cost features to existing `analyze-gcode`
- **Estimate input types:** G-code files (accurate), model files STL/3MF (rough heuristic), saved config TOML
- **Full cost model:** Filament cost, electricity, machine depreciation, labor; missing fields shown as "N/A" with hints
- **Cost defaults in profiles:** Add `price_per_kg` to filament, `watts` to machine profile schema
- **Multi-config comparison:** Accept multiple filament/config combos, show comparison table
- **Output formats:** Table (default), --json, --csv, --markdown

### Claude's Discretion
- Exact calibration model geometry design (tower dimensions, block heights, wall thicknesses)
- Which calibration tests use programmatic mesh vs shipped STL files
- Internal data structures for calibration parameter management
- Volume-based heuristic formula for model estimation
- Markdown instruction template design
- Table formatting and column layout
- Multi-config comparison table layout
- Edge cases in cost model (zero values, very small prints)
- Performance optimization

### Deferred Ideas (OUT OF SCOPE)
- Calibration result recording/auto-tuning, multi-step wizard sequences, parametric arrays, calibration gallery, AI-assisted analysis, interactive Klipper mode
- Extended calibration print types (PA tower, max flow, tolerance test, etc.)
- Spool inventory, print job history, batch estimation, print farm reporting, wear cost model, currency preferences, energy monitoring, multi-material estimation, accuracy tracking, confidence intervals
- benchmark, doctor, diff, stats, check-updates CLI commands
</user_constraints>

## Standard Stack

### Core (already in project)
| Library | Purpose | Why Standard |
|---------|---------|--------------|
| clap (derive) | CLI argument parsing with nested subcommands | Already used throughout -- `Csg` command provides exact pattern for nested subcommands |
| comfy-table | ASCII table output formatting | Already used in `analysis_display.rs` and `stats_display.rs` |
| serde + serde_json | JSON serialization for --json output | Already used everywhere |
| slicecore-mesh primitives | `primitive_box`, `primitive_cylinder`, `mesh_union` | Already available in CSG module for calibration model generation |
| slicecore-gcode-io | `GcodeCommand::SetExtruderTemp` for temperature injection | Already supports M104/M109 write |
| slicecore-engine estimation | `estimate_print_time()` trapezoid model | Already implemented and tested |
| slicecore-engine statistics | `PrintStatistics`, `filament_mm_to_grams()` | Already has weight/cost computation |

### Supporting
| Library | Purpose | When to Use |
|---------|---------|-------------|
| csv (crate) | CSV output format | May already be a dependency; check if manual CSV is sufficient |
| include_bytes!/include_str! | Embed instruction templates or pre-made STL models | For shipped calibration resources |

### No New Dependencies Required
This phase can be implemented entirely with existing dependencies. The `primitive_box`, `primitive_cylinder`, and `mesh_union` from `slicecore-mesh::csg` provide all geometry generation needed. The `comfy-table` crate handles table formatting. JSON/CSV output follows existing patterns.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-cli/src/
  calibrate/
    mod.rs                    # CalibrateCommand enum + dispatch
    temp_tower.rs             # Temperature tower generation
    retraction.rs             # Retraction test generation
    flow.rs                   # Flow rate calibration generation
    first_layer.rs            # First layer calibration generation
    common.rs                 # Shared types, instruction generation, validation
  main.rs                     # Add Calibrate variant to Commands enum
  analysis_display.rs         # Extend with cost display functions

crates/slicecore-engine/src/
  calibrate.rs                # Core calibration model generation (mesh + gcode injection)
  cost_model.rs               # Cost estimation data structures and computation
  config.rs                   # Add watts to MachineConfig (price_per_kg already exists)
```

### Pattern 1: Nested Subcommand (matching CSG pattern)
**What:** Add `Calibrate` as a subcommand group with sub-subcommands, exactly like the existing `Csg` command.
**When to use:** For the `calibrate temp-tower`, `calibrate retraction`, etc. structure.
**Example:**
```rust
// In main.rs Commands enum:
/// Calibration print generation
#[command(subcommand)]
Calibrate(calibrate::CalibrateCommand),

// In calibrate/mod.rs:
#[derive(Subcommand)]
pub enum CalibrateCommand {
    /// Generate a temperature tower calibration print
    TempTower { /* fields */ },
    /// Generate a retraction test calibration print
    Retraction { /* fields */ },
    /// Generate a flow rate calibration print
    Flow { /* fields */ },
    /// Generate a first layer calibration print
    FirstLayer { /* fields */ },
    /// List available calibration tests
    List,
}
```

### Pattern 2: Calibration Pipeline (generate -> validate -> slice -> inject -> write)
**What:** Each calibration command follows: generate mesh -> validate against bed size -> slice through Engine -> inject calibration G-code (temperature changes etc.) -> write output + instructions.
**When to use:** For all calibration commands that produce G-code.
**Example flow:**
```rust
// 1. Build calibration parameters from profile + CLI overrides
let params = TempTowerParams::from_profile_and_args(&config, &cli_args);

// 2. Generate mesh programmatically
let mesh = generate_temp_tower_mesh(&params);

// 3. Validate against bed
validate_bed_fit(&mesh, &config.machine)?;

// 4. Optionally save model (--save-model)
if let Some(path) = &args.save_model { save_mesh(&mesh, path)?; }

// 5. Slice through normal engine pipeline
let engine = Engine::new(config.clone());
let result = engine.slice(&mesh)?;

// 6. Inject temperature changes at correct Z heights
let commands = inject_temp_changes(result.commands, &params);

// 7. Write G-code with calibration metadata header
write_gcode_with_header(&commands, &params, output_path)?;

// 8. Write companion instructions
write_instructions(&params, instruction_path)?;
```

### Pattern 3: Cost Model Progressive Disclosure
**What:** Calculate all cost components independently; display "N/A" with hint for missing inputs.
**When to use:** For the `analyze-gcode` cost output.
**Example:**
```rust
pub struct CostEstimate {
    pub filament_cost: Option<f64>,
    pub electricity_cost: Option<f64>,
    pub depreciation_cost: Option<f64>,
    pub labor_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub missing_hints: Vec<String>,
}
```

### Pattern 4: Volume-Based Rough Estimation
**What:** For STL/3MF input (no G-code), use mesh volume to estimate filament usage and time.
**When to use:** Quick "will this fit on my spool?" checks.
**Formula:**
```rust
// Volume -> filament length
let volume_mm3 = mesh_stats.volume.abs();
let infill_factor = 0.20; // assume 20% infill default
let shell_fraction = 0.30; // walls + top/bottom ~30% of bounding volume
let effective_volume = volume_mm3 * (infill_factor + shell_fraction);
let filament_cross_section = PI * (diameter/2.0).powi(2);
let filament_length_mm = effective_volume / filament_cross_section;

// Filament length -> weight -> cost
let weight_g = filament_mm_to_grams(filament_length_mm, diameter, density);

// Very rough time: assume average 40mm/s effective speed
let rough_time_s = filament_length_mm / 40.0;
```
This is intentionally rough (+/- 30-50%) as stated in CONTEXT.md.

### Anti-Patterns to Avoid
- **Baking temperature logic into the slicing engine:** Temperature changes should be post-process injected, not woven into the engine's temperature planning. This keeps the engine clean and the calibration concern isolated.
- **Creating a separate `estimate` subcommand:** The user explicitly decided to merge into `analyze-gcode` to avoid duplicate functionality.
- **Hand-rolling table formatting:** Use `comfy-table` consistently with existing display code.
- **Making calibration models overly complex:** Simple stepped boxes/cylinders are sufficient. Complex geometry adds no calibration value and complicates generation code.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Mesh primitives | Custom vertex/index generation | `primitive_box()`, `primitive_cylinder()`, `mesh_union()` | CSG primitives already tested and correct |
| Table formatting | Manual string padding | `comfy-table` | Already used, handles terminal width |
| Profile resolution | Custom profile loading | `ProfileResolver` from Phase 30 | Handles library paths, name resolution, validation |
| Time estimation | New estimation model | `estimate_print_time()` | Trapezoid model already implemented |
| Weight from length | Custom density calculation | `filament_mm_to_grams()` | Already matches filament.rs |
| G-code writing | Manual string concatenation | `GcodeCommand::fmt()` Display impl | All commands already have correct formatting |

## Common Pitfalls

### Pitfall 1: Temperature Injection at Wrong Z Heights
**What goes wrong:** Temperature changes land mid-layer instead of at layer boundaries, causing inconsistent results.
**Why it happens:** Z heights in calibration model don't align with actual slice layer heights due to adaptive layers or first layer height differences.
**How to avoid:** Calculate block Z boundaries from actual layer heights post-slice, not from the model geometry. Inject temperature commands at the first G-code command after a Z change that crosses a block boundary.
**Warning signs:** Temperature changes at fractional layer heights, blocks with different number of layers.

### Pitfall 2: Bed Size Validation Off-by-One
**What goes wrong:** Model generates fine but extends slightly beyond printable area due to skirt/brim.
**Why it happens:** Validating only the model footprint, forgetting that slicing adds skirt, brim, or purge line around the model.
**How to avoid:** Add margin (5-10mm) when validating bed fit, or explicitly account for skirt_distance + skirt_loops * line_width.
**Warning signs:** Prints that start but clip on the bed edges.

### Pitfall 3: Cost Model Division by Zero
**What goes wrong:** Panic or NaN when electricity rate is 0 or expected printer hours is 0.
**Why it happens:** Users may not set all cost parameters; zero is a valid "not set" sentinel.
**How to avoid:** Treat zero as "not provided" -- return `None` for that cost component, display as "N/A" with hint.
**Warning signs:** Tests with zero or negative input values.

### Pitfall 4: Multi-Config Comparison With Incompatible Profiles
**What goes wrong:** Comparing configs that don't share the same machine leads to misleading results.
**Why it happens:** User compares PLA on machine A vs PETG on machine B.
**How to avoid:** Warn (don't error) when machine profiles differ between configs. Show the machine name in the comparison table header.
**Warning signs:** Wildly different time estimates that are driven by machine differences, not material differences.

### Pitfall 5: Hardcoded Calibration Dimensions
**What goes wrong:** Temperature tower doesn't fit on a small bed (e.g., 120x120mm printers).
**Why it happens:** Tower designed for 220x220 beds without considering smaller form factors.
**How to avoid:** Parameterize all dimensions, derive sensible defaults from bed size. Auto-scale tower footprint to fit within 60% of bed area.

## Code Examples

### Temperature Tower Mesh Generation (using existing primitives)
```rust
use slicecore_mesh::csg::{primitive_box, mesh_union};
use slicecore_mesh::transform::translate;

fn generate_temp_tower(
    block_count: usize,
    block_height: f64,  // e.g., 8.0mm per temp step
    base_width: f64,    // e.g., 30.0mm
    base_depth: f64,    // e.g., 30.0mm
    wall_thickness: f64, // e.g., 1.2mm (3 walls * 0.4)
) -> TriangleMesh {
    // Start with a base plate
    let base = primitive_box(base_width, base_depth, 1.0);
    let mut tower = translate(&base, 0.0, 0.0, 0.5);

    // Stack blocks
    for i in 0..block_count {
        let z = 1.0 + (i as f64) * block_height + block_height / 2.0;
        let block = primitive_box(base_width, base_depth, block_height);
        let positioned = translate(&block, 0.0, 0.0, z);
        tower = mesh_union(&tower, &positioned, &Default::default())
            .expect("union should succeed for simple boxes");
    }
    tower
}
```

### Temperature Injection Post-Processing
```rust
use slicecore_gcode_io::GcodeCommand;

fn inject_temp_changes(
    commands: Vec<GcodeCommand>,
    temp_schedule: &[(f64, f64)],  // (z_height, temperature)
) -> Vec<GcodeCommand> {
    let mut result = Vec::with_capacity(commands.len() + temp_schedule.len());
    let mut schedule_idx = 0;
    let mut current_z = 0.0_f64;

    for cmd in &commands {
        // Track Z position changes
        match cmd {
            GcodeCommand::LinearMove { z: Some(z), .. }
            | GcodeCommand::RapidMove { z: Some(z), .. } => {
                let new_z = *z;
                // Check if we crossed a temperature change boundary
                while schedule_idx < temp_schedule.len()
                    && new_z >= temp_schedule[schedule_idx].0
                    && current_z < temp_schedule[schedule_idx].0
                {
                    let temp = temp_schedule[schedule_idx].1;
                    result.push(GcodeCommand::Comment(
                        format!(" Temperature change to {:.0}C", temp)
                    ));
                    result.push(GcodeCommand::SetExtruderTemp {
                        temp,
                        wait: false,  // M104, don't wait
                    });
                    schedule_idx += 1;
                }
                current_z = new_z;
            }
            _ => {}
        }
        result.push(cmd.clone());
    }
    result
}
```

### Cost Model Computation
```rust
pub struct CostInputs {
    pub filament_weight_g: f64,
    pub print_time_seconds: f64,
    pub filament_price_per_kg: Option<f64>,
    pub electricity_rate: Option<f64>,   // per kWh
    pub printer_watts: Option<f64>,
    pub printer_cost: Option<f64>,
    pub expected_hours: Option<f64>,
    pub labor_rate: Option<f64>,         // per hour
    pub setup_time_minutes: Option<f64>,
}

pub fn compute_cost(inputs: &CostInputs) -> CostEstimate {
    let filament_cost = inputs.filament_price_per_kg
        .map(|ppk| inputs.filament_weight_g / 1000.0 * ppk);

    let electricity_cost = match (inputs.electricity_rate, inputs.printer_watts) {
        (Some(rate), Some(watts)) => {
            let hours = inputs.print_time_seconds / 3600.0;
            Some(hours * watts / 1000.0 * rate)
        }
        _ => None,
    };

    let depreciation_cost = match (inputs.printer_cost, inputs.expected_hours) {
        (Some(cost), Some(hours)) if hours > 0.0 => {
            let print_hours = inputs.print_time_seconds / 3600.0;
            Some(cost / hours * print_hours)
        }
        _ => None,
    };

    let labor_cost = match (inputs.labor_rate, inputs.setup_time_minutes) {
        (Some(rate), Some(mins)) if mins > 0.0 => Some(rate * mins / 60.0),
        _ => None,
    };

    // ... build CostEstimate with missing_hints for None values
}
```

### Extending analyze-gcode CLI (adding cost flags)
```rust
// Add to AnalyzeGcode variant in Commands enum:
/// Filament price per kg (for cost estimation)
#[arg(long)]
filament_price: Option<f64>,

/// Printer power consumption in watts (for electricity cost)
#[arg(long)]
printer_watts: Option<f64>,

/// Electricity rate per kWh (for electricity cost)
#[arg(long)]
electricity_rate: Option<f64>,

/// Printer purchase cost (for depreciation)
#[arg(long)]
printer_cost: Option<f64>,

/// Expected printer lifetime in hours (for depreciation)
#[arg(long)]
expected_hours: Option<f64>,

/// Labor hourly rate (for labor cost)
#[arg(long)]
labor_rate: Option<f64>,

/// Setup time in minutes (for labor cost)
#[arg(long, default_value = "5")]
setup_time: Option<f64>,

/// Output as Markdown table
#[arg(long)]
markdown: bool,

/// Accept a model file for rough estimation (STL/3MF)
#[arg(long)]
model: bool,
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate estimate command | Merged into analyze-gcode | Phase 31 design decision | Smaller CLI surface, avoids duplication |
| Hardcoded calibration STLs | Programmatic generation from primitives | Phase 31 design decision | Parameterizable, profile-aware defaults |
| Simple distance/speed estimation | Trapezoid motion model | Phase 6 (estimation.rs) | 30-50% more accurate |

**Key existing infrastructure:**
- `FilamentPropsConfig.cost_per_kg` already exists (default: 25.0)
- `FilamentPropsConfig.nozzle_temperature_range_low/high` already exists -- perfect for temperature tower defaults
- `FilamentPropsConfig.filament_retraction_length` already exists -- perfect for retraction test defaults
- `MachineConfig.bed_x/bed_y` already exists -- bed size validation ready
- `PrintConfig.extrusion_multiplier` already exists -- flow calibration baseline ready
- `MachineConfig` does NOT have a `watts` field yet -- needs to be added

## Open Questions

1. **Calibration model complexity tradeoff**
   - What we know: Simple stacked boxes work for temp towers, cylinders for retraction
   - What's unclear: How much geometric detail (e.g., bridging test sections, temperature labels embossed) to include
   - Recommendation: Start simple (stacked boxes), add complexity in future phases. Labels can go in companion instructions instead of embossed text

2. **First layer calibration pattern**
   - What we know: Needs to cover large bed area, single layer height
   - What's unclear: Best pattern (grid lines vs concentric squares vs zigzag)
   - Recommendation: Grid pattern -- simplest to generate programmatically, covers bed well, easy to read results

3. **Multi-config comparison for model estimation**
   - What we know: Works well for G-code (re-analyze same file), less clear for model-based
   - What's unclear: Should model-based rough estimation support multi-config (needs to slice multiple times)?
   - Recommendation: For Phase 31, multi-config comparison only for G-code files. Model-based estimation is single-config quick check only.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml per crate |
| Quick run command | `cargo test -p slicecore-cli --lib` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| (none specified) | Calibrate temp-tower generates valid G-code | integration | `cargo test -p slicecore-cli --test cli_calibrate` | No -- Wave 0 |
| (none specified) | Calibrate retraction generates valid G-code | integration | `cargo test -p slicecore-cli --test cli_calibrate` | No -- Wave 0 |
| (none specified) | Calibrate flow generates valid G-code | integration | `cargo test -p slicecore-cli --test cli_calibrate` | No -- Wave 0 |
| (none specified) | Calibrate first-layer generates valid G-code | integration | `cargo test -p slicecore-cli --test cli_calibrate` | No -- Wave 0 |
| (none specified) | analyze-gcode cost estimation computes correctly | unit | `cargo test -p slicecore-engine cost_model` | No -- Wave 0 |
| (none specified) | Volume-based rough estimation produces reasonable values | unit | `cargo test -p slicecore-engine rough_estimate` | No -- Wave 0 |
| (none specified) | Multi-config comparison shows deltas | integration | `cargo test -p slicecore-cli --test cli_calibrate` | No -- Wave 0 |
| (none specified) | Bed size validation rejects oversized models | unit | `cargo test -p slicecore-engine calibrate` | No -- Wave 0 |
| (none specified) | Temperature injection at correct Z heights | unit | `cargo test -p slicecore-engine calibrate` | No -- Wave 0 |
| (none specified) | Companion instruction file generated | integration | `cargo test -p slicecore-cli --test cli_calibrate` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-cli -p slicecore-engine --lib`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-cli/tests/cli_calibrate.rs` -- integration tests for calibrate subcommands
- [ ] `crates/slicecore-engine/src/calibrate.rs` -- core calibration module with unit tests
- [ ] `crates/slicecore-engine/src/cost_model.rs` -- cost model module with unit tests

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `crates/slicecore-cli/src/main.rs` -- CLI structure, Commands enum, existing patterns
- Codebase inspection: `crates/slicecore-cli/src/csg_command.rs` -- nested subcommand pattern (CsgCommand enum)
- Codebase inspection: `crates/slicecore-engine/src/estimation.rs` -- trapezoid motion model, `estimate_print_time()`
- Codebase inspection: `crates/slicecore-engine/src/statistics.rs` -- `PrintStatistics`, `filament_mm_to_grams()`
- Codebase inspection: `crates/slicecore-engine/src/config.rs` -- `PrintConfig`, `FilamentPropsConfig`, `MachineConfig`
- Codebase inspection: `crates/slicecore-mesh/src/csg/primitives.rs` -- `primitive_box`, `primitive_cylinder`, `mesh_union`
- Codebase inspection: `crates/slicecore-gcode-io/src/commands.rs` -- `GcodeCommand::SetExtruderTemp`, Display impl
- Codebase inspection: `crates/slicecore-mesh/src/stats.rs` -- `MeshStats` with volume computation
- Codebase inspection: `crates/slicecore-cli/src/analysis_display.rs` -- existing G-code analysis display
- Codebase inspection: `crates/slicecore-cli/src/slice_workflow.rs` -- `ProfileResolver` usage, `SliceWorkflowOptions`

### Secondary (MEDIUM confidence)
- CONTEXT.md (31-CONTEXT.md) -- user decisions and phase boundary definition

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in the project, no new dependencies needed
- Architecture: HIGH -- directly follows established patterns (CSG subcommands, ProfileResolver, comfy-table)
- Pitfalls: HIGH -- based on analysis of actual code paths and data structures
- Calibration geometry: MEDIUM -- specific dimensions are Claude's discretion, general approach is validated by existing primitives
- Volume-based estimation accuracy: MEDIUM -- heuristic by nature, documented as +/- 30-50%

**Research date:** 2026-03-16
**Valid until:** 2026-04-16 (stable -- internal project, no external dependency changes)
