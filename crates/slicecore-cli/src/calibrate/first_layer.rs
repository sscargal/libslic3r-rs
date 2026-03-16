//! First layer adhesion calibration command.
//!
//! Generates a flat plate mesh covering a configurable percentage of the
//! print bed, slices it with solid infill settings, and writes the resulting
//! G-code with a companion instruction file explaining how to evaluate
//! first layer quality and adjust Z-offset.

use std::path::PathBuf;

use slicecore_engine::calibrate::{generate_first_layer_mesh, FirstLayerParams, FirstLayerPattern};
use slicecore_engine::engine::Engine;

use super::common::{
    display_dry_run, format_calibration_header, resolve_calibration_config,
    save_calibration_model, write_instructions,
};

/// Arguments for the first layer calibration command, extracted from CLI.
pub struct FirstLayerArgs {
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
    /// Pattern type override (grid, lines, concentric).
    pub pattern: Option<String>,
}

/// Runs the first layer calibration command.
///
/// Pipeline: resolve config -> build params -> generate flat mesh at 80% bed
/// coverage -> optionally save model -> override config for solid first layer
/// -> slice via Engine -> write G-code with calibration header -> write
/// companion instructions for Z-offset tuning.
///
/// # Errors
///
/// Returns an error if profile resolution, mesh generation, slicing, or
/// file writing fails.
pub fn cmd_first_layer(args: FirstLayerArgs) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Resolve config from profiles
    let mut config = resolve_calibration_config(
        &args.machine,
        &args.filament,
        &args.process,
        &args.profiles_dir,
    )?;

    // 2. Build params
    let pattern = match args.pattern.as_deref() {
        Some("lines") => FirstLayerPattern::Lines,
        Some("concentric") => FirstLayerPattern::Concentric,
        _ => FirstLayerPattern::Grid,
    };

    let params = FirstLayerParams {
        pattern,
        coverage_percent: 80.0,
    };

    let bed_x = config.machine.bed_x;
    let bed_y = config.machine.bed_y;
    let plate_width = bed_x * params.coverage_percent / 100.0;
    let plate_depth = bed_y * params.coverage_percent / 100.0;

    let plate_height = config.first_layer_height;

    // Dry run: show model info and parameters without slicing
    if args.dry_run {
        let dry_params: Vec<(&str, String)> = vec![
            ("Pattern", format!("{pattern:?}")),
            ("Coverage", format!("{:.0}%", params.coverage_percent)),
            ("Plate Size", format!("{plate_width:.0}mm x {plate_depth:.0}mm")),
            ("Layer Height", format!("{plate_height:.2}mm")),
        ];
        display_dry_run(
            "First Layer Test",
            &dry_params,
            (plate_width, plate_depth, plate_height),
            (bed_x, bed_y),
            args.json,
        );
        return Ok(());
    }

    // 3. Generate mesh
    let mesh = generate_first_layer_mesh(&params, bed_x, bed_y);

    // 4. Optionally save mesh
    if let Some(ref model_path) = args.save_model {
        save_calibration_model(&mesh, model_path)?;
    }

    // 5. Override config for solid first layer
    config.layer_height = config.first_layer_height;
    config.infill_density = 1.0; // 100% infill for solid layer
    config.top_solid_layers = 1;
    config.bottom_solid_layers = 1;
    config.wall_count = 2;

    // 6. Slice through engine
    let engine = Engine::new(config.clone());
    let result = engine.slice(&mesh, None)?;

    // 7. Write G-code with header
    let gcode_text = String::from_utf8_lossy(&result.gcode);
    let header = format_calibration_header(
        "First Layer Test",
        &[
            ("Pattern", format!("{pattern:?}")),
            ("Coverage", format!("{:.0}%", params.coverage_percent)),
            ("Plate Size", format!("{plate_width:.0}mm x {plate_depth:.0}mm")),
            ("Layer Height", format!("{:.2}mm", config.first_layer_height)),
        ],
    );

    let mut output_gcode = String::with_capacity(header.len() + gcode_text.len() + 1);
    output_gcode.push_str(&header);
    output_gcode.push('\n');
    output_gcode.push_str(&gcode_text);

    // 8. Write G-code
    let output_path = args
        .output
        .unwrap_or_else(|| PathBuf::from("first_layer_test.gcode"));
    std::fs::write(&output_path, output_gcode)?;

    // 9. Write companion instructions
    let instructions_path = output_path.with_extension("instructions.md");
    let sections = build_first_layer_instructions(&params, plate_width, plate_depth, &config);
    write_instructions(
        &instructions_path,
        "First Layer Calibration Test",
        &sections,
    )?;

    // 10. Print summary
    eprintln!(
        "Generated first layer test: {plate_width:.0}mm x {plate_depth:.0}mm ({:.0}% bed coverage)",
        params.coverage_percent,
    );
    eprintln!("G-code: {}", output_path.display());
    eprintln!("Instructions: {}", instructions_path.display());

    Ok(())
}

/// Builds companion instruction sections for the first layer test.
fn build_first_layer_instructions(
    params: &FirstLayerParams,
    plate_width: f64,
    plate_depth: f64,
    config: &slicecore_engine::config::PrintConfig,
) -> Vec<(String, String)> {
    let mut sections = Vec::new();

    // Overview
    sections.push((
        "Overview".to_string(),
        format!(
            "This first layer test prints a {plate_width:.0}mm x {plate_depth:.0}mm plate \
             ({:.0}% of your {:.0}mm x {:.0}mm bed) at {:.2}mm layer height. It uses 100% \
             solid infill with 2 perimeters to clearly show first layer adhesion quality \
             across the entire build surface.",
            params.coverage_percent,
            config.machine.bed_x,
            config.machine.bed_y,
            config.first_layer_height,
        ),
    ));

    // What to look for
    sections.push((
        "What to Look For".to_string(),
        "Examine the printed first layer for these indicators:\n\
         \n\
         **Good first layer:**\n\
         - Smooth, consistent surface\n\
         - Lines merge together without gaps\n\
         - Slight squish but not overly flat\n\
         - Adheres well to bed, does not peel up at corners\n\
         \n\
         **Z-offset too high (nozzle too far from bed):**\n\
         - Visible gaps between extrusion lines\n\
         - Lines appear round/tubular instead of flat\n\
         - Poor adhesion, easy to peel off\n\
         - Lines may not stick to the bed at all\n\
         \n\
         **Z-offset too low (nozzle too close to bed):**\n\
         - Over-squished, very thin/transparent lines\n\
         - Rough surface texture (elephant's foot effect)\n\
         - Material pushes up at edges of lines\n\
         - May see ridges between lines\n\
         - First layer may be difficult to remove after printing"
            .to_string(),
    ));

    // How to adjust
    sections.push((
        "How to Adjust Z-Offset".to_string(),
        "1. **If gaps between lines**: Lower the nozzle (decrease Z-offset by 0.02-0.05mm)\n\
         2. **If over-squished**: Raise the nozzle (increase Z-offset by 0.02-0.05mm)\n\
         3. **If inconsistent across bed**: Level your bed (one area good, another bad)\n\
         4. Reprint and compare until the entire plate looks uniform\n\
         \n\
         **Typical adjustments:**\n\
         - Most printers: adjust in 0.02mm increments\n\
         - Large gaps: adjust by 0.05mm\n\
         - Fine-tuning: adjust by 0.01mm"
            .to_string(),
    ));

    // Bed leveling check
    sections.push((
        "Bed Leveling Check".to_string(),
        "This test also reveals bed leveling issues:\n\
         \n\
         - **Center good, corners bad**: Bed may be warped or need mesh leveling\n\
         - **One side good, other bad**: Bed is tilted, adjust leveling screws\n\
         - **Diagonal pattern**: Two opposite corners need adjustment\n\
         - **All edges bad, center good**: Bed is concave (common with glass beds)\n\
         - **All edges good, center bad**: Bed is convex\n\
         \n\
         If bed leveling varies significantly across the surface, consider enabling \
         automatic bed leveling (ABL) or manual mesh leveling in your firmware."
            .to_string(),
    ));

    sections
}
