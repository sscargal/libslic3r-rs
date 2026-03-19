//! Temperature tower calibration command.
//!
//! Generates a stacked-box temperature tower mesh, slices it through the
//! engine, injects temperature change commands at Z boundaries, and writes
//! the resulting G-code with a companion instruction file.

use std::path::PathBuf;

use slicecore_engine::calibrate::{
    generate_temp_tower_mesh, temp_schedule, validate_bed_fit, TempTowerParams,
};
use slicecore_engine::engine::Engine;

use super::common::{
    display_dry_run, format_calibration_header, resolve_calibration_config, save_calibration_model,
    write_instructions,
};

/// Arguments for the temperature tower command, extracted from CLI.
pub struct TempTowerArgs {
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
    /// Starting temperature override.
    pub start_temp: Option<f64>,
    /// Ending temperature override.
    pub end_temp: Option<f64>,
    /// Temperature step override.
    pub step: Option<f64>,
}

/// Runs the temperature tower calibration command.
///
/// Pipeline: resolve config -> build params -> generate mesh -> validate bed fit
/// -> optionally save model -> slice via Engine -> inject temp changes -> write G-code
/// with calibration header -> write companion instructions.
///
/// # Errors
///
/// Returns an error if profile resolution, mesh generation, slicing, or
/// file writing fails.
pub fn cmd_temp_tower(args: TempTowerArgs, output: &crate::cli_output::CliOutput) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Resolve config from profiles
    let config = resolve_calibration_config(
        &args.machine,
        &args.filament,
        &args.process,
        &args.profiles_dir,
    )?;

    // 2. Build params from filament defaults + CLI overrides
    let mut params = TempTowerParams::from_filament(&config.filament);
    if let Some(t) = args.start_temp {
        params.start_temp = t;
    }
    if let Some(t) = args.end_temp {
        params.end_temp = t;
    }
    if let Some(s) = args.step {
        params.step = s;
    }

    let schedule = temp_schedule(&params);
    let num_blocks = schedule.len();

    let tower_height = 1.0 + num_blocks as f64 * params.block_height;

    // Dry run: show model info and parameters without slicing
    if args.dry_run {
        let mut dry_params: Vec<(&str, String)> = vec![
            ("Start Temperature", format!("{:.0}C", params.start_temp)),
            ("End Temperature", format!("{:.0}C", params.end_temp)),
            ("Step", format!("{:.0}C", params.step)),
            ("Block Height", format!("{:.1}mm", params.block_height)),
            ("Blocks", format!("{num_blocks}")),
        ];
        for (z, temp) in &schedule {
            dry_params.push(("Schedule", format!("Z={z:.1}mm: {temp:.0}C")));
        }
        display_dry_run(
            "Temperature Tower",
            &dry_params,
            (params.base_width, params.base_depth, tower_height),
            (config.machine.bed_x, config.machine.bed_y),
            args.json,
        );
        return Ok(());
    }

    // 3. Generate mesh
    let mesh = generate_temp_tower_mesh(&params);

    // 4. Validate bed fit
    validate_bed_fit(
        params.base_width,
        params.base_depth,
        tower_height,
        &config.machine,
    )
    .map_err(|e| format!("Bed fit validation failed: {e}"))?;

    // 5. Optionally save mesh
    if let Some(ref model_path) = args.save_model {
        save_calibration_model(&mesh, model_path)?;
    }

    // 6. Slice through engine
    let engine = Engine::new(config);
    let result = engine.slice(&mesh, None)?;

    // 7. Parse raw G-code and inject temperature changes
    let gcode_text = String::from_utf8_lossy(&result.gcode);
    let header = format_calibration_header(
        "Temperature Tower",
        &[
            ("Start Temperature", format!("{:.0}C", params.start_temp)),
            ("End Temperature", format!("{:.0}C", params.end_temp)),
            ("Step", format!("{:.0}C", params.step)),
            ("Block Height", format!("{:.1}mm", params.block_height)),
            ("Blocks", format!("{num_blocks}")),
        ],
    );

    // Inject temp changes by post-processing text lines
    let output_gcode = inject_temp_changes_text(&gcode_text, &schedule, &header);

    // 8. Write G-code
    let output_path = args
        .output
        .unwrap_or_else(|| PathBuf::from("temp_tower.gcode"));
    std::fs::write(&output_path, output_gcode)?;

    // 9. Write companion instructions
    let instructions_path = output_path.with_extension("instructions.md");
    let sections = build_temp_tower_instructions(&params, &schedule);
    write_instructions(
        &instructions_path,
        "Temperature Tower Calibration",
        &sections,
    )?;

    // 10. Print summary
    output.info(&format!(
        "Generated temperature tower: {:.0}C to {:.0}C in {:.0}C steps ({num_blocks} blocks)",
        params.start_temp, params.end_temp, params.step,
    ));
    output.info(&format!("G-code: {}", output_path.display()));
    output.info(&format!("Instructions: {}", instructions_path.display()));

    Ok(())
}

/// Injects temperature change comments into raw G-code text at Z boundaries.
fn inject_temp_changes_text(gcode: &str, schedule: &[(f64, f64)], header: &str) -> String {
    let mut output = String::with_capacity(gcode.len() + header.len() + schedule.len() * 80);
    output.push_str(header);
    output.push('\n');

    let mut current_z = 0.0_f64;
    let mut next_idx = 0_usize;

    for line in gcode.lines() {
        // Detect Z moves in G0/G1 commands
        if let Some(z) = extract_z_from_line(line) {
            if z > current_z {
                while next_idx < schedule.len() && schedule[next_idx].0 <= z + f64::EPSILON {
                    let (sched_z, temp) = schedule[next_idx];
                    output.push_str(&format!(
                        "; === TEMPERATURE CHANGE: {temp:.0}C at Z={sched_z:.1}mm ===\n"
                    ));
                    output.push_str(&format!("M104 S{temp:.0} ; set nozzle temp\n"));
                    next_idx += 1;
                }
                current_z = z;
            }
        }
        output.push_str(line);
        output.push('\n');
    }

    output
}

/// Extracts Z value from a G0/G1 line.
fn extract_z_from_line(line: &str) -> Option<f64> {
    let trimmed = line.trim();
    if !trimmed.starts_with("G0 ") && !trimmed.starts_with("G1 ") {
        return None;
    }
    for token in trimmed.split_whitespace() {
        if let Some(z_str) = token.strip_prefix('Z') {
            return z_str.parse::<f64>().ok();
        }
    }
    None
}

/// Builds companion instruction sections for the temperature tower.
fn build_temp_tower_instructions(
    params: &TempTowerParams,
    schedule: &[(f64, f64)],
) -> Vec<(String, String)> {
    let mut sections = Vec::new();

    // Overview
    sections.push((
        "Overview".to_string(),
        format!(
            "This temperature tower tests nozzle temperatures from {:.0}C to {:.0}C \
             in {:.0}C increments. Each block is {:.1}mm tall.",
            params.start_temp, params.end_temp, params.step, params.block_height,
        ),
    ));

    // Block breakdown
    let mut breakdown = String::new();
    for (i, (z, temp)) in schedule.iter().enumerate() {
        breakdown.push_str(&format!(
            "- **Block {} (Z={:.1}mm)**: {:.0}C\n",
            i + 1,
            z,
            temp,
        ));
    }
    sections.push(("Block Temperatures".to_string(), breakdown));

    // How to read results
    sections.push((
        "How to Read Results".to_string(),
        "Inspect each block for:\n\
         - **Layer adhesion**: Are layers bonding well? Too-cold blocks will delaminate.\n\
         - **Surface quality**: Smooth surfaces indicate good flow. Rough or blobby surfaces suggest too-hot temperature.\n\
         - **Stringing**: Thin threads between features indicate temperature is too high.\n\
         - **Bridging**: If the model has overhangs, check for drooping (too hot) or poor adhesion (too cold).\n\
         \n\
         Choose the temperature that gives the best balance of adhesion, surface quality, and minimal stringing."
            .to_string(),
    ));

    // Selection guide
    sections.push((
        "Selecting Your Temperature".to_string(),
        "1. Identify the block with the best overall quality\n\
         2. Note the temperature printed on the block (or count from the bottom)\n\
         3. Update your filament profile's nozzle temperature to that value\n\
         4. For borderline cases, print a second tower with a narrower range and smaller step"
            .to_string(),
    ));

    sections
}
