//! SliceCore CLI -- command-line interface for the slicecore 3D slicing engine.
//!
//! Subcommands:
//! - `slice`: Slice an STL file to G-code
//! - `validate`: Validate a G-code file
//! - `analyze`: Analyze a mesh file (print stats)

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

use slicecore_engine::{Engine, PrintConfig};
use slicecore_fileio::load_mesh;
use slicecore_gcode_io::validate_gcode;
use slicecore_mesh::{compute_stats, repair};

/// SliceCore -- a 3D model slicer.
#[derive(Parser)]
#[command(name = "slicecore", about = "3D model slicer", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Slice an STL file to G-code
    Slice {
        /// Input STL file path
        input: PathBuf,

        /// Print config TOML file (optional -- uses defaults if not provided)
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Output G-code file path (default: input with .gcode extension)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Validate a G-code file
    Validate {
        /// G-code file to validate
        input: PathBuf,
    },

    /// Analyze a mesh file (print stats)
    Analyze {
        /// Mesh file to analyze
        input: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Slice {
            input,
            config,
            output,
        } => cmd_slice(&input, config.as_deref(), output.as_deref()),
        Commands::Validate { input } => cmd_validate(&input),
        Commands::Analyze { input } => cmd_analyze(&input),
    }
}

/// Slice an STL/mesh file to G-code.
fn cmd_slice(input: &PathBuf, config_path: Option<&std::path::Path>, output_path: Option<&std::path::Path>) {
    // 1. Read input file.
    let data = match std::fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: Failed to read input file '{}': {}", input.display(), e);
            process::exit(1);
        }
    };

    // 2. Load mesh.
    let mesh = match load_mesh(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: Failed to parse mesh from '{}': {}", input.display(), e);
            process::exit(1);
        }
    };

    // 3. Repair mesh.
    let vertices = mesh.vertices().to_vec();
    let indices = mesh.indices().to_vec();
    let (repaired_mesh, report) = match repair(vertices, indices) {
        Ok((m, r)) => (m, r),
        Err(e) => {
            eprintln!("Error: Failed to repair mesh: {}", e);
            process::exit(1);
        }
    };

    if !report.was_already_clean {
        eprintln!(
            "Note: Mesh repaired ({} degenerates removed, {} edges stitched, {} holes filled, {} normals fixed)",
            report.degenerate_removed,
            report.edges_stitched,
            report.holes_filled,
            report.normals_fixed,
        );
    }

    // 4. Load config.
    let print_config = if let Some(cfg_path) = config_path {
        match PrintConfig::from_toml_file(cfg_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: Failed to load config '{}': {}", cfg_path.display(), e);
                process::exit(1);
            }
        }
    } else {
        PrintConfig::default()
    };

    // 5. Slice.
    let engine = Engine::new(print_config);
    let result = match engine.slice(&repaired_mesh) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Slicing failed: {}", e);
            process::exit(1);
        }
    };

    // 6. Determine output path.
    let out_path = if let Some(p) = output_path {
        p.to_path_buf()
    } else {
        input.with_extension("gcode")
    };

    // 7. Write output.
    if let Err(e) = std::fs::write(&out_path, &result.gcode) {
        eprintln!("Error: Failed to write output '{}': {}", out_path.display(), e);
        process::exit(1);
    }

    // 8. Print summary.
    let time_minutes = result.estimated_time_seconds / 60.0;
    println!("Slicing complete:");
    println!("  Layers: {}", result.layer_count);
    println!("  Estimated time: {:.1} min ({:.0} sec)", time_minutes, result.estimated_time_seconds);
    println!("  Output: {}", out_path.display());
}

/// Validate a G-code file.
fn cmd_validate(input: &PathBuf) {
    let contents = match std::fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: Failed to read '{}': {}", input.display(), e);
            process::exit(1);
        }
    };

    let result = validate_gcode(&contents);

    println!("Validation of '{}':", input.display());
    println!("  Lines: {}", result.line_count);

    if result.valid {
        println!("  Status: VALID");
    } else {
        println!("  Status: INVALID ({} errors)", result.errors.len());
        for err in &result.errors {
            println!("  ERROR: {}", err);
        }
    }

    if !result.warnings.is_empty() {
        println!("  Warnings: {}", result.warnings.len());
        for warn in &result.warnings {
            println!("  WARN: {}", warn);
        }
    }

    if !result.valid {
        process::exit(1);
    }
}

/// Analyze a mesh file and print statistics.
fn cmd_analyze(input: &PathBuf) {
    let data = match std::fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: Failed to read '{}': {}", input.display(), e);
            process::exit(1);
        }
    };

    let mesh = match load_mesh(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: Failed to parse mesh from '{}': {}", input.display(), e);
            process::exit(1);
        }
    };

    let stats = compute_stats(&mesh);

    println!("Mesh analysis of '{}':", input.display());
    println!("  Vertices: {}", stats.vertex_count);
    println!("  Triangles: {}", stats.triangle_count);
    println!(
        "  Bounding box: ({:.3}, {:.3}, {:.3}) - ({:.3}, {:.3}, {:.3})",
        stats.aabb.min.x, stats.aabb.min.y, stats.aabb.min.z,
        stats.aabb.max.x, stats.aabb.max.y, stats.aabb.max.z,
    );
    println!("  Volume: {:.3} mm^3", stats.volume);
    println!("  Surface area: {:.3} mm^2", stats.surface_area);
    println!("  Manifold: {}", if stats.is_manifold { "yes" } else { "no" });
    println!("  Watertight: {}", if stats.is_watertight { "yes" } else { "no" });
    println!(
        "  Consistent winding: {}",
        if stats.has_consistent_winding { "yes" } else { "no" }
    );
    if stats.degenerate_count > 0 {
        println!("  Degenerate triangles: {}", stats.degenerate_count);
    }
}
