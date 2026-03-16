//! Flow rate calibration command.
//!
//! Generates a stacked-box flow rate tower mesh, slices it through the
//! engine, injects M221 flow rate override commands at Z boundaries, and
//! writes the resulting G-code with a companion instruction file explaining
//! how to measure wall thickness and calculate the correct extrusion multiplier.

use std::path::PathBuf;

use slicecore_engine::calibrate::{
    flow_schedule, generate_flow_mesh, inject_flow_changes_text, validate_bed_fit, FlowParams,
};
use slicecore_engine::engine::Engine;

use super::common::{format_calibration_header, resolve_calibration_config, write_instructions};

/// Arguments for the flow rate calibration command, extracted from CLI.
pub struct FlowArgs {
    /// Machine profile name or path.
    pub machine: Option<String>,
    /// Filament profile name or path.
    pub filament: Option<String>,
    /// Process profile name or path.
    pub process: Option<String>,
    /// Profile library directory.
    pub profiles_dir: Option<PathBuf>,
    /// Output G-code file path.
    pub output: Option<PathBuf>,
    /// Output as JSON.
    pub json: bool,
    /// Resolve parameters without generating G-code.
    pub dry_run: bool,
    /// Save generated mesh model to file.
    pub save_model: Option<PathBuf>,
    /// Baseline extrusion multiplier override.
    pub baseline: Option<f64>,
    /// Multiplier step override.
    pub step: Option<f64>,
    /// Number of test steps override.
    pub steps: Option<usize>,
}

/// Runs the flow rate calibration command.
///
/// Pipeline: resolve config -> build params -> generate mesh -> validate bed fit
/// -> optionally save model -> slice via Engine -> inject M221 flow changes at
/// Z boundaries -> write G-code with calibration header -> write companion
/// instructions with wall thickness measurement guidance.
///
/// # Errors
///
/// Returns an error if profile resolution, mesh generation, slicing, or
/// file writing fails.
pub fn cmd_flow(args: FlowArgs) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Resolve config from profiles
    let config = resolve_calibration_config(
        &args.machine,
        &args.filament,
        &args.process,
        &args.profiles_dir,
    )?;

    // 2. Build params from config defaults + CLI overrides
    let mut params = FlowParams::from_config(&config);
    if let Some(b) = args.baseline {
        params.baseline_multiplier = b;
    }
    if let Some(s) = args.step {
        params.step = s;
    }
    if let Some(n) = args.steps {
        params.steps = n;
    }

    let schedule = flow_schedule(&params);
    let num_sections = schedule.len();
    let block_height = 5.0;
    let base_width = 30.0;
    let base_depth = 30.0;

    let start_pct = schedule.first().map_or(100.0, |s| s.1);
    let end_pct = schedule.last().map_or(100.0, |s| s.1);

    // Dry run: just print params
    if args.dry_run {
        if args.json {
            println!("{}", serde_json::to_string_pretty(&params)?);
        } else {
            eprintln!(
                "Flow calibration: {start_pct:.0}% to {end_pct:.0}% in {:.0}% steps ({num_sections} sections)",
                params.step * 100.0,
            );
            for (z, pct) in &schedule {
                eprintln!("  Z={z:.1}mm: {pct:.0}% flow");
            }
        }
        return Ok(());
    }

    // 3. Generate mesh
    let mesh = generate_flow_mesh(&params);

    // 4. Validate bed fit
    let tower_height = 1.0 + num_sections as f64 * block_height;
    validate_bed_fit(base_width, base_depth, tower_height, &config.machine)
        .map_err(|e| format!("Bed fit validation failed: {e}"))?;

    // 5. Optionally save mesh
    if let Some(ref model_path) = args.save_model {
        slicecore_fileio::export::save_mesh(&mesh, model_path)?;
        eprintln!("Saved mesh to {}", model_path.display());
    }

    // 6. Slice through engine
    let engine = Engine::new(config);
    let result = engine.slice(&mesh, None)?;

    // 7. Post-process G-code: inject M221 flow rate changes at Z boundaries
    let gcode_text = String::from_utf8_lossy(&result.gcode);
    let header = format_calibration_header(
        "Flow Rate Test",
        &[
            ("Baseline Multiplier", format!("{:.2}", params.baseline_multiplier)),
            ("Step", format!("{:.0}%", params.step * 100.0)),
            ("Start Flow", format!("{start_pct:.0}%")),
            ("End Flow", format!("{end_pct:.0}%")),
            ("Sections", format!("{num_sections}")),
        ],
    );

    let output_gcode = inject_flow_changes_text(&gcode_text, &schedule, &header);

    // 8. Write G-code
    let output_path = args.output.unwrap_or_else(|| PathBuf::from("flow_test.gcode"));
    std::fs::write(&output_path, output_gcode)?;

    // 9. Write companion instructions
    let instructions_path = output_path.with_extension("instructions.md");
    let sections = build_flow_instructions(&params, &schedule);
    write_instructions(&instructions_path, "Flow Rate Calibration Test", &sections)?;

    // 10. Print summary
    eprintln!(
        "Generated flow calibration: {start_pct:.0}% to {end_pct:.0}% in {:.0}% steps ({num_sections} sections)",
        params.step * 100.0,
    );
    eprintln!("G-code: {}", output_path.display());
    eprintln!("Instructions: {}", instructions_path.display());

    Ok(())
}

/// Builds companion instruction sections for the flow rate test.
fn build_flow_instructions(
    params: &FlowParams,
    schedule: &[(f64, f64)],
) -> Vec<(String, String)> {
    let mut sections = Vec::new();

    let start_pct = schedule.first().map_or(100.0, |s| s.1);
    let end_pct = schedule.last().map_or(100.0, |s| s.1);

    // Overview
    sections.push((
        "Overview".to_string(),
        format!(
            "This flow rate calibration tower tests extrusion multipliers from {start_pct:.0}% \
             to {end_pct:.0}% in {:.0}% steps. Each 5mm section prints at a different flow \
             rate using M221 overrides. Measure wall thickness with calipers to determine \
             the optimal extrusion multiplier.",
            params.step * 100.0,
        ),
    ));

    // Section breakdown
    let mut breakdown = String::new();
    for (i, (z, pct)) in schedule.iter().enumerate() {
        let multiplier = pct / 100.0;
        breakdown.push_str(&format!(
            "- **Section {} (Z={z:.1}mm)**: {pct:.0}% flow (multiplier: {multiplier:.2})\n",
            i + 1,
        ));
    }
    sections.push(("Section Flow Rates".to_string(), breakdown));

    // How to measure
    sections.push((
        "How to Measure".to_string(),
        "1. Print the tower and let it cool completely\n\
         2. Using digital calipers, measure the wall thickness of each section\n\
         3. Measure in multiple spots per section and average the readings\n\
         4. Compare measured thickness to expected thickness:\n\
         \n\
         **Expected wall thickness** = number_of_perimeters x nozzle_diameter\n\
         (e.g., 2 perimeters x 0.4mm nozzle = 0.80mm expected)\n\
         \n\
         5. For each section, calculate:\n\
         `correct_multiplier = expected_thickness / measured_thickness * section_multiplier`"
            .to_string(),
    ));

    // Interpreting results
    sections.push((
        "Interpreting Results".to_string(),
        "- **Walls too thin**: Flow rate too low -- increase extrusion multiplier\n\
         - **Walls too thick**: Flow rate too high -- decrease extrusion multiplier\n\
         - **Exact match**: The flow rate for that section is correct\n\
         \n\
         The ideal section is the one where measured wall thickness matches expected \
         wall thickness. Use that section's multiplier as your new extrusion multiplier."
            .to_string(),
    ));

    // Applying results
    sections.push((
        "Applying Your Result".to_string(),
        format!(
            "1. Identify the section with the most accurate wall thickness\n\
             2. Note its flow rate percentage from the section list above\n\
             3. Convert to multiplier: new_multiplier = flow_percent / 100\n\
             4. Update your profile's extrusion_multiplier to this value\n\
             5. For fine-tuning, reprint with a narrower range centered on your result\n\
             \n\
             Current baseline multiplier: {:.2}",
            params.baseline_multiplier,
        ),
    ));

    sections
}
