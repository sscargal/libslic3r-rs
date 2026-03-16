//! Calibrate CLI subcommands for generating calibration test prints.
//!
//! Provides subcommands for temperature tower, retraction, flow rate,
//! and first layer calibration tests. Each subcommand generates G-code
//! tailored to help users dial in specific printer settings.
//!
//! # Subcommands
//!
//! - `temp-tower`: Generate a temperature tower test print
//! - `retraction`: Generate a retraction calibration test
//! - `flow`: Generate a flow rate calibration test
//! - `first-layer`: Generate a first layer adhesion test
//! - `list`: Show available calibration tests

#[allow(dead_code)]
pub mod common;
pub mod first_layer;
pub mod flow;
pub mod retraction;
pub mod temp_tower;

use std::path::PathBuf;

use clap::Subcommand;
use comfy_table::{presets::UTF8_FULL, Table};

/// Calibration test subcommands.
///
/// Each variant generates G-code for a specific calibration test print.
/// Profile flags (`--machine`, `--filament`, `--process`) can be used to
/// derive test parameters from an existing printer configuration.
#[derive(Subcommand)]
pub enum CalibrateCommand {
    /// Generate a temperature tower calibration test
    TempTower {
        /// Machine profile name or path
        #[arg(short, long)]
        machine: Option<String>,
        /// Filament profile name or path
        #[arg(short, long)]
        filament: Option<String>,
        /// Process profile name or path
        #[arg(short, long)]
        process: Option<String>,
        /// Profile library directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
        /// Output G-code file path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Resolve parameters without generating G-code
        #[arg(long)]
        dry_run: bool,
        /// Save generated mesh model to file
        #[arg(long)]
        save_model: Option<PathBuf>,
        /// Starting temperature (overrides filament config)
        #[arg(long)]
        start_temp: Option<f64>,
        /// Ending temperature (overrides filament config)
        #[arg(long)]
        end_temp: Option<f64>,
        /// Temperature step between blocks
        #[arg(long)]
        step: Option<f64>,
    },

    /// Generate a retraction calibration test
    Retraction {
        /// Machine profile name or path
        #[arg(short, long)]
        machine: Option<String>,
        /// Filament profile name or path
        #[arg(short, long)]
        filament: Option<String>,
        /// Process profile name or path
        #[arg(short, long)]
        process: Option<String>,
        /// Profile library directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
        /// Output G-code file path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Resolve parameters without generating G-code
        #[arg(long)]
        dry_run: bool,
        /// Save generated mesh model to file
        #[arg(long)]
        save_model: Option<PathBuf>,
        /// Starting retraction distance in mm
        #[arg(long)]
        start_distance: Option<f64>,
        /// Ending retraction distance in mm
        #[arg(long)]
        end_distance: Option<f64>,
        /// Retraction distance step in mm
        #[arg(long)]
        step: Option<f64>,
    },

    /// Generate a flow rate calibration test
    Flow {
        /// Machine profile name or path
        #[arg(short, long)]
        machine: Option<String>,
        /// Filament profile name or path
        #[arg(short, long)]
        filament: Option<String>,
        /// Process profile name or path
        #[arg(short, long)]
        process: Option<String>,
        /// Profile library directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
        /// Output G-code file path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Resolve parameters without generating G-code
        #[arg(long)]
        dry_run: bool,
        /// Save generated mesh model to file
        #[arg(long)]
        save_model: Option<PathBuf>,
        /// Baseline extrusion multiplier
        #[arg(long)]
        baseline: Option<f64>,
        /// Multiplier step between sections
        #[arg(long)]
        step: Option<f64>,
        /// Number of test steps
        #[arg(long)]
        steps: Option<usize>,
    },

    /// Generate a first layer adhesion calibration test
    FirstLayer {
        /// Machine profile name or path
        #[arg(short, long)]
        machine: Option<String>,
        /// Filament profile name or path
        #[arg(short, long)]
        filament: Option<String>,
        /// Process profile name or path
        #[arg(short, long)]
        process: Option<String>,
        /// Profile library directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
        /// Output G-code file path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Resolve parameters without generating G-code
        #[arg(long)]
        dry_run: bool,
        /// Save generated mesh model to file
        #[arg(long)]
        save_model: Option<PathBuf>,
        /// Pattern type: grid, lines, or concentric
        #[arg(long)]
        pattern: Option<String>,
    },

    /// List available calibration tests
    List,
}

/// Runs a calibrate subcommand.
///
/// # Errors
///
/// Returns an error if the subcommand fails.
#[allow(clippy::too_many_lines)]
pub fn run_calibrate(cmd: CalibrateCommand) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CalibrateCommand::TempTower {
            machine,
            filament,
            process,
            profiles_dir,
            output,
            json,
            dry_run,
            save_model,
            start_temp,
            end_temp,
            step,
        } => temp_tower::cmd_temp_tower(temp_tower::TempTowerArgs {
            machine,
            filament,
            process,
            profiles_dir,
            output,
            json,
            dry_run,
            save_model,
            start_temp,
            end_temp,
            step,
        }),
        CalibrateCommand::Retraction {
            machine,
            filament,
            process,
            profiles_dir,
            output,
            json,
            dry_run,
            save_model,
            start_distance,
            end_distance,
            step,
        } => retraction::cmd_retraction(retraction::RetractionArgs {
            machine,
            filament,
            process,
            profiles_dir,
            output,
            json,
            dry_run,
            save_model,
            start_distance,
            end_distance,
            step,
        }),
        CalibrateCommand::Flow {
            machine,
            filament,
            process,
            profiles_dir,
            output,
            json,
            dry_run,
            save_model,
            baseline,
            step,
            steps,
        } => flow::cmd_flow(flow::FlowArgs {
            machine,
            filament,
            process,
            profiles_dir,
            output,
            json,
            dry_run,
            save_model,
            baseline,
            step,
            steps,
        }),
        CalibrateCommand::FirstLayer {
            machine,
            filament,
            process,
            profiles_dir,
            output,
            json,
            dry_run,
            save_model,
            pattern,
        } => first_layer::cmd_first_layer(first_layer::FirstLayerArgs {
            machine,
            filament,
            process,
            profiles_dir,
            output,
            json,
            dry_run,
            save_model,
            pattern,
        }),
        CalibrateCommand::List => {
            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(vec!["Test", "Description", "Key Parameters"]);
            table.add_row(vec![
                "temp-tower",
                "Temperature tower - find optimal nozzle temp",
                "--start-temp, --end-temp, --step",
            ]);
            table.add_row(vec![
                "retraction",
                "Retraction test - minimize stringing",
                "--start-distance, --end-distance, --step",
            ]);
            table.add_row(vec![
                "flow",
                "Flow rate test - correct wall thickness",
                "--baseline, --step, --steps",
            ]);
            table.add_row(vec![
                "first-layer",
                "First layer adhesion - dial in Z offset",
                "--pattern (grid/lines/concentric)",
            ]);
            println!("{table}");
            Ok(())
        }
    }
}
