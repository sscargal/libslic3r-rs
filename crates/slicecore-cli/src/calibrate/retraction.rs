//! Retraction calibration command.
//!
//! Generates a stacked-box retraction test mesh, slices it through the
//! engine using the profile's retraction setting, injects Z-boundary
//! comments labelling each section's target retraction distance, and writes
//! the resulting G-code with a companion instruction file explaining the
//! manual per-section reprint workflow.

use std::path::PathBuf;

use slicecore_engine::calibrate::{
    generate_retraction_mesh, retraction_schedule, validate_bed_fit, RetractionParams,
};
use slicecore_engine::engine::Engine;

use super::common::{
    display_dry_run, format_calibration_header, resolve_calibration_config,
    save_calibration_model, write_instructions,
};

/// Arguments for the retraction test command, extracted from CLI.
pub struct RetractionArgs {
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
    /// Starting retraction distance override.
    pub start_distance: Option<f64>,
    /// Ending retraction distance override.
    pub end_distance: Option<f64>,
    /// Retraction distance step override.
    pub step: Option<f64>,
}

/// Runs the retraction calibration command.
///
/// Pipeline: resolve config -> build params -> generate mesh -> validate bed fit
/// -> optionally save model -> slice (using profile's retraction throughout) ->
/// inject retraction comments at Z boundaries -> write G-code + instructions.
///
/// # Errors
///
/// Returns an error if profile resolution, mesh generation, slicing, or
/// file writing fails.
pub fn cmd_retraction(args: RetractionArgs) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Resolve config from profiles
    let config = resolve_calibration_config(
        &args.machine,
        &args.filament,
        &args.process,
        &args.profiles_dir,
    )?;

    // 2. Build params from filament defaults + CLI overrides
    let mut params = RetractionParams::from_filament(&config.filament);
    if let Some(d) = args.start_distance {
        params.start_distance = d;
    }
    if let Some(d) = args.end_distance {
        params.end_distance = d;
    }
    if let Some(s) = args.step {
        params.step = s;
    }

    let schedule = retraction_schedule(&params);
    let num_sections = schedule.len();
    let block_height = 8.0;
    let base_width = 30.0;
    let base_depth = 30.0;

    // Current profile retraction distance
    let profile_retraction = config
        .filament
        .filament_retraction_length
        .unwrap_or(1.0);

    let tower_height = 1.0 + num_sections as f64 * block_height;

    // Dry run: show model info and parameters without slicing
    if args.dry_run {
        let mut dry_params: Vec<(&str, String)> = vec![
            ("Start Distance", format!("{:.1}mm", params.start_distance)),
            ("End Distance", format!("{:.1}mm", params.end_distance)),
            ("Step", format!("{:.1}mm", params.step)),
            ("Sections", format!("{num_sections}")),
            ("Profile Retraction", format!("{profile_retraction:.1}mm")),
        ];
        for (z, dist) in &schedule {
            dry_params.push(("Schedule", format!("Z={z:.1}mm: {dist:.1}mm retraction")));
        }
        display_dry_run(
            "Retraction Test",
            &dry_params,
            (base_width, base_depth, tower_height),
            (config.machine.bed_x, config.machine.bed_y),
            args.json,
        );
        return Ok(());
    }

    // 3. Generate mesh
    let mesh = generate_retraction_mesh(&params);

    // 4. Validate bed fit
    validate_bed_fit(base_width, base_depth, tower_height, &config.machine)
        .map_err(|e| format!("Bed fit validation failed: {e}"))?;

    // 5. Optionally save mesh
    if let Some(ref model_path) = args.save_model {
        save_calibration_model(&mesh, model_path)?;
    }

    // 6. Slice through engine (using profile's retraction setting throughout)
    let engine = Engine::new(config);
    let result = engine.slice(&mesh, None)?;

    // 7. Post-process G-code: inject retraction comments at Z boundaries
    let gcode_text = String::from_utf8_lossy(&result.gcode);
    let header = format_calibration_header(
        "Retraction Test",
        &[
            (
                "Start Distance",
                format!("{:.1}mm", params.start_distance),
            ),
            ("End Distance", format!("{:.1}mm", params.end_distance)),
            ("Step", format!("{:.1}mm", params.step)),
            ("Sections", format!("{num_sections}")),
            (
                "Profile Retraction",
                format!("{profile_retraction:.1}mm"),
            ),
        ],
    );

    let output_gcode =
        inject_retraction_comments_text(&gcode_text, &schedule, &header);

    // 8. Write G-code
    let output_path = args
        .output
        .unwrap_or_else(|| PathBuf::from("retraction_test.gcode"));
    std::fs::write(&output_path, output_gcode)?;

    // 9. Write companion instructions
    let instructions_path = output_path.with_extension("instructions.md");
    let sections = build_retraction_instructions(&params, &schedule, profile_retraction);
    write_instructions(
        &instructions_path,
        "Retraction Calibration Test",
        &sections,
    )?;

    // 10. Print summary
    eprintln!(
        "Generated retraction test: {:.1}mm to {:.1}mm in {:.1}mm steps ({num_sections} sections, one reprint per section)",
        params.start_distance, params.end_distance, params.step,
    );
    eprintln!("G-code: {}", output_path.display());
    eprintln!("Instructions: {}", instructions_path.display());

    Ok(())
}

/// Injects retraction distance comments into raw G-code text at Z boundaries.
fn inject_retraction_comments_text(
    gcode: &str,
    schedule: &[(f64, f64)],
    header: &str,
) -> String {
    let mut output = String::with_capacity(gcode.len() + header.len() + schedule.len() * 100);
    output.push_str(header);
    output.push('\n');

    let mut current_z = 0.0_f64;
    let mut next_idx = 0_usize;

    for line in gcode.lines() {
        if let Some(z) = extract_z_from_line(line) {
            if z > current_z {
                while next_idx < schedule.len() && schedule[next_idx].0 <= z + f64::EPSILON {
                    let dist = schedule[next_idx].1;
                    output.push_str(&format!(
                        "; === RETRACTION SECTION: {dist:.1}mm (print this block, evaluate stringing, adjust, reprint) ===\n"
                    ));
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

/// Builds companion instruction sections for the retraction test.
fn build_retraction_instructions(
    params: &RetractionParams,
    schedule: &[(f64, f64)],
    profile_retraction: f64,
) -> Vec<(String, String)> {
    let mut sections = Vec::new();

    // Overview
    sections.push((
        "Overview".to_string(),
        format!(
            "This retraction test tower is sliced with your profile's current retraction \
             setting ({profile_retraction:.1}mm). Each section is labelled with a target \
             retraction distance to test, ranging from {:.1}mm to {:.1}mm in {:.1}mm steps.",
            params.start_distance, params.end_distance, params.step,
        ),
    ));

    // How it works
    sections.push((
        "How It Works".to_string(),
        "Unlike the temperature tower (which changes settings mid-print), the retraction \
         test requires you to manually adjust and reprint for each section:\n\
         \n\
         1. Each section of the tower is labelled with a target retraction distance\n\
         2. To test a section: set your profile retraction to that distance, reprint the tower\n\
         3. Evaluate stringing and blobs on that section\n\
         4. Work through sections from lowest to highest (or binary search) to find your optimal setting"
            .to_string(),
    ));

    // Section breakdown
    let mut breakdown = String::new();
    for (i, (z, dist)) in schedule.iter().enumerate() {
        breakdown.push_str(&format!(
            "- **Section {} (Z={:.1}mm)**: {:.1}mm retraction\n",
            i + 1,
            z,
            dist,
        ));
    }
    sections.push(("Section Distances".to_string(), breakdown));

    // What to look for
    sections.push((
        "What to Look For".to_string(),
        "- **Minimal stringing**: Thin threads between features indicate insufficient retraction\n\
         - **No blobs or ooze**: Excess material on outer walls means too much retraction (causes \
           pressure buildup on restart)\n\
         - **Clean travel moves**: Material should not be deposited during non-print moves\n\
         - **Good first layer adhesion**: Very high retraction can cause under-extrusion after retract"
            .to_string(),
    ));

    // Workflow
    sections.push((
        "Recommended Workflow".to_string(),
        format!(
            "1. Print the tower once with your current retraction ({profile_retraction:.1}mm)\n\
             2. If stringing is visible, increase retraction distance\n\
             3. If blobs appear, decrease retraction distance\n\
             4. Try the midpoint of the range first ({:.1}mm), then binary search\n\
             5. Once you find the optimal distance, update your filament profile",
            (params.start_distance + params.end_distance) / 2.0,
        ),
    ));

    sections
}
