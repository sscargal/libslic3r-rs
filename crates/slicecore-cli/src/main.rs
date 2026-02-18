//! SliceCore CLI -- command-line interface for the slicecore 3D slicing engine.
//!
//! Subcommands:
//! - `slice`: Slice an STL file to G-code
//! - `validate`: Validate a G-code file
//! - `analyze`: Analyze a mesh file (print stats)
//! - `ai-suggest`: Suggest print settings using AI mesh analysis

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

use slicecore_ai::AiConfig;
use slicecore_engine::{Engine, PrintConfig};
use slicecore_fileio::load_mesh;
use slicecore_gcode_io::validate_gcode;
use slicecore_mesh::{compute_stats, repair};
use slicecore_plugin::PluginRegistry;

/// SliceCore -- a 3D model slicer.
#[derive(Parser)]
#[command(
    name = "slicecore",
    about = "3D model slicer with plugin and AI integration",
    version,
    after_help = "\
PLUGIN SUPPORT:
  Plugins extend slicecore with custom infill patterns. Configure a plugin directory
  in your config TOML (plugin_dir = \"/path/to/plugins\") or use --plugin-dir on the
  slice command. Each plugin directory should contain subdirectories with plugin.toml
  manifests. Select a plugin infill pattern in config with:
    infill_pattern = { plugin = \"zigzag\" }

AI PROFILE SUGGESTIONS:
  The ai-suggest command analyzes mesh geometry and queries an LLM for optimal print
  settings. By default it connects to Ollama at localhost:11434 using llama3.2.

  To configure a different provider, create an AI config TOML file:
    # Ollama (default, no API key needed):
    provider = \"ollama\"
    model = \"llama3.2\"
    base_url = \"http://localhost:11434\"

    # OpenAI:
    provider = \"open_ai\"
    model = \"gpt-4o\"
    api_key = \"sk-...\"

    # Anthropic:
    provider = \"anthropic\"
    model = \"claude-sonnet-4-20250514\"
    api_key = \"sk-ant-...\"

  Then pass it with: slicecore ai-suggest model.stl --ai-config provider.toml"
)]
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

        /// Output slicing metadata as JSON to stdout
        #[arg(long)]
        json: bool,

        /// Output slicing metadata as MessagePack to stdout
        #[arg(long)]
        msgpack: bool,

        /// Directory to load plugins from (overrides config plugin_dir).
        /// Each subdirectory should contain a plugin.toml manifest.
        #[arg(long)]
        plugin_dir: Option<PathBuf>,
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

    /// Suggest optimal print settings using AI analysis of mesh geometry.
    ///
    /// Analyzes the input mesh and sends geometry features to an LLM provider
    /// (default: Ollama with llama3.2). Configure providers via --ai-config.
    AiSuggest {
        /// Input mesh file (STL, 3MF, or OBJ)
        input: PathBuf,

        /// AI provider configuration file (TOML).
        /// Uses Ollama defaults (localhost:11434, llama3.2) if not specified.
        #[arg(short = 'a', long = "ai-config")]
        ai_config: Option<PathBuf>,

        /// Output format: "text" (default) or "json"
        #[arg(long, default_value = "text")]
        format: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Slice {
            input,
            config,
            output,
            json,
            msgpack,
            plugin_dir,
        } => cmd_slice(
            &input,
            config.as_deref(),
            output.as_deref(),
            json,
            msgpack,
            plugin_dir.as_deref(),
        ),
        Commands::Validate { input } => cmd_validate(&input),
        Commands::Analyze { input } => cmd_analyze(&input),
        Commands::AiSuggest {
            input,
            ai_config,
            format,
        } => cmd_ai_suggest(&input, ai_config.as_deref(), &format),
    }
}

/// Slice an STL/mesh file to G-code.
fn cmd_slice(
    input: &PathBuf,
    config_path: Option<&std::path::Path>,
    output_path: Option<&std::path::Path>,
    json_output: bool,
    msgpack_output: bool,
    plugin_dir: Option<&std::path::Path>,
) {
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

    // 5. Load plugins (if applicable).
    // Determine effective plugin directory (CLI flag overrides config).
    let effective_plugin_dir = plugin_dir
        .map(|p| p.to_string_lossy().to_string())
        .or_else(|| print_config.plugin_dir.clone());

    // Check if plugin infill is requested but no plugin_dir is set.
    if matches!(
        print_config.infill_pattern,
        slicecore_engine::InfillPattern::Plugin(_)
    ) && effective_plugin_dir.is_none()
    {
        eprintln!(
            "Error: infill_pattern is set to a plugin pattern, but no plugin directory is configured."
        );
        eprintln!(
            "Set 'plugin_dir' in your config TOML or use --plugin-dir on the command line."
        );
        process::exit(1);
    }

    // Create engine (auto-loads plugins from config.plugin_dir when plugins feature enabled).
    let mut engine = Engine::new(print_config.clone());
    let cli_plugin_dir_provided = plugin_dir.is_some();

    if cli_plugin_dir_provided {
        // CLI --plugin-dir flag overrides config -- always load from specified dir.
        if let Some(ref dir) = effective_plugin_dir {
            let mut registry = PluginRegistry::new();
            match registry.discover_and_load(std::path::Path::new(dir)) {
                Ok(loaded) => {
                    if !loaded.is_empty() {
                        eprintln!("Loaded {} plugin(s):", loaded.len());
                        for info in &loaded {
                            eprintln!("  - {}: {}", info.name, info.description);
                        }
                    }
                    engine = engine.with_plugin_registry(registry);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to load plugins from '{}': {}", dir, e);
                }
            }
        }
    } else if engine.has_plugin_registry() {
        // Engine auto-loaded from config.plugin_dir -- report status.
        // Startup warnings will be emitted during slicing via EventBus.
        eprintln!("Plugins auto-loaded from config plugin_dir");
    } else if let Some(ref dir) = effective_plugin_dir {
        // Fallback: config had plugin_dir but engine didn't load (shouldn't normally happen).
        let mut registry = PluginRegistry::new();
        match registry.discover_and_load(std::path::Path::new(dir)) {
            Ok(loaded) => {
                if !loaded.is_empty() {
                    eprintln!("Loaded {} plugin(s):", loaded.len());
                    for info in &loaded {
                        eprintln!("  - {}: {}", info.name, info.description);
                    }
                }
                engine = engine.with_plugin_registry(registry);
            }
            Err(e) => {
                eprintln!("Warning: Failed to load plugins from '{}': {}", dir, e);
            }
        }
    }

    // 6. Slice.
    let result = match engine.slice(&repaired_mesh) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: Slicing failed: {}", e);
            process::exit(1);
        }
    };

    // 7. Determine output path.
    let out_path = if let Some(p) = output_path {
        p.to_path_buf()
    } else {
        input.with_extension("gcode")
    };

    // 8. Write G-code output.
    if let Err(e) = std::fs::write(&out_path, &result.gcode) {
        eprintln!("Error: Failed to write output '{}': {}", out_path.display(), e);
        process::exit(1);
    }

    // 9. Structured output (JSON or MessagePack to stdout).
    if json_output {
        match slicecore_engine::output::to_json(&result, &print_config) {
            Ok(json_str) => println!("{}", json_str),
            Err(e) => {
                eprintln!("Error: Failed to serialize JSON: {}", e);
                process::exit(1);
            }
        }
    } else if msgpack_output {
        match slicecore_engine::output::to_msgpack(&result, &print_config) {
            Ok(bytes) => {
                use std::io::Write;
                if let Err(e) = std::io::stdout().write_all(&bytes) {
                    eprintln!("Error: Failed to write MessagePack: {}", e);
                    process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Error: Failed to serialize MessagePack: {}", e);
                process::exit(1);
            }
        }
    }

    // 10. Print summary (to stderr if structured output was requested, to stdout otherwise).
    let time_minutes = result.estimated_time_seconds / 60.0;
    if json_output || msgpack_output {
        eprintln!("Slicing complete:");
        eprintln!("  Layers: {}", result.layer_count);
        eprintln!("  Estimated time: {:.1} min ({:.0} sec)", time_minutes, result.estimated_time_seconds);
        eprintln!("  Output: {}", out_path.display());
    } else {
        println!("Slicing complete:");
        println!("  Layers: {}", result.layer_count);
        println!("  Estimated time: {:.1} min ({:.0} sec)", time_minutes, result.estimated_time_seconds);
        println!("  Output: {}", out_path.display());
    }
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

/// Suggest print settings for a mesh using AI.
fn cmd_ai_suggest(
    input: &PathBuf,
    ai_config_path: Option<&std::path::Path>,
    format: &str,
) {
    // 1. Read input file.
    let data = match std::fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "Error: Failed to read input file '{}': {}",
                input.display(),
                e
            );
            process::exit(1);
        }
    };

    // 2. Load mesh.
    let mesh = match load_mesh(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "Error: Failed to parse mesh from '{}': {}",
                input.display(),
                e
            );
            process::exit(1);
        }
    };

    // 3. Load AI config.
    let ai_config = if let Some(path) = ai_config_path {
        let toml_str = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "Error: Failed to read AI config '{}': {}",
                    path.display(),
                    e
                );
                process::exit(1);
            }
        };
        match AiConfig::from_toml(&toml_str) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "Error: Failed to parse AI config '{}': {}",
                    path.display(),
                    e
                );
                process::exit(1);
            }
        }
    } else {
        AiConfig::default()
    };

    // 4. Create engine and suggest profile.
    let engine = Engine::new(PrintConfig::default());
    match engine.suggest_profile(&mesh, &ai_config) {
        Ok(suggestion) => {
            if format == "json" {
                match serde_json::to_string_pretty(&suggestion) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("Error: Failed to serialize suggestion: {}", e);
                        process::exit(1);
                    }
                }
            } else {
                // Human-readable text output.
                println!("AI Print Profile Suggestion");
                println!("==========================");
                println!();
                println!("  Layer height:     {:.2} mm", suggestion.layer_height);
                println!("  Wall count:       {}", suggestion.wall_count);
                println!(
                    "  Infill density:   {:.0}%",
                    suggestion.infill_density * 100.0
                );
                println!("  Infill pattern:   {}", suggestion.infill_pattern);
                println!(
                    "  Supports:         {}",
                    if suggestion.support_enabled {
                        "yes"
                    } else {
                        "no"
                    }
                );
                if suggestion.support_enabled {
                    println!(
                        "  Support angle:    {:.0} deg",
                        suggestion.support_overhang_angle
                    );
                }
                println!(
                    "  Perimeter speed:  {:.0} mm/s",
                    suggestion.perimeter_speed
                );
                println!("  Infill speed:     {:.0} mm/s", suggestion.infill_speed);
                println!("  Nozzle temp:      {:.0} C", suggestion.nozzle_temp);
                println!("  Bed temp:         {:.0} C", suggestion.bed_temp);
                if suggestion.brim_width > 0.0 {
                    println!("  Brim width:       {:.1} mm", suggestion.brim_width);
                }
                if !suggestion.reasoning.is_empty() {
                    println!();
                    println!("Reasoning: {}", suggestion.reasoning);
                }
            }
        }
        Err(e) => {
            let err_str = format!("{}", e);
            if err_str.contains("Connection refused")
                || err_str.contains("connection refused")
                || err_str.contains("ConnectError")
                || err_str.contains("error sending request")
            {
                eprintln!("Error: Failed to connect to AI provider.");
                if ai_config_path.is_none() {
                    eprintln!("The default provider is Ollama at localhost:11434.");
                    eprintln!(
                        "Start Ollama with 'ollama serve', or use --ai-config to configure a different provider."
                    );
                } else {
                    eprintln!(
                        "Check that the provider specified in your AI config is running and reachable."
                    );
                }
            } else {
                eprintln!("Error: AI suggestion failed: {}", e);
            }
            process::exit(1);
        }
    }
}
