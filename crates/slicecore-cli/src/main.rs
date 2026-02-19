//! SliceCore CLI -- command-line interface for the slicecore 3D slicing engine.
//!
//! Subcommands:
//! - `slice`: Slice an STL file to G-code
//! - `validate`: Validate a G-code file
//! - `analyze`: Analyze a mesh file (print stats)
//! - `ai-suggest`: Suggest print settings using AI mesh analysis
//! - `import-profiles`: Import upstream slicer profiles to native TOML format
//! - `list-profiles`: List profiles from the profile library
//! - `search-profiles`: Search profiles by keyword
//! - `show-profile`: Show details of a specific profile

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

use slicecore_ai::AiConfig;
use slicecore_engine::{
    batch_convert_profiles, batch_convert_prusaslicer_profiles, load_index, write_merged_index,
    Engine, PrintConfig, ProfileIndexEntry,
};
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

  Then pass it with: slicecore ai-suggest model.stl --ai-config provider.toml

PROFILE CONVERSION:
  Convert OrcaSlicer/BambuStudio JSON profiles to native TOML:
    slicecore convert-profile profile.json > my_config.toml
    slicecore convert-profile process.json filament.json machine.json -o config.toml
  Multiple files are merged in order (later files override earlier ones for shared fields).

PROFILE LIBRARY:
  Import upstream slicer profiles:
    slicecore import-profiles --source-dir /path/to/OrcaSlicer/resources/profiles
  This converts JSON profiles to native TOML and generates a searchable index.
  Profiles are stored in profiles/ organized by source/vendor/type/.

PROFILE DISCOVERY:
  List available vendors:
    slicecore list-profiles --vendors
  List PLA filament profiles from BBL:
    slicecore list-profiles --vendor BBL --profile-type filament --material PLA
  Search for a specific printer or material:
    slicecore search-profiles \"Bambu Lab A1 PLA\"
  View a profile's details:
    slicecore show-profile orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1
  View raw TOML content:
    slicecore show-profile orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1 --raw"
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

        /// Print config file (TOML or JSON, auto-detected; optional -- uses defaults if not provided)
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

    /// Convert OrcaSlicer/BambuStudio JSON profiles to native TOML format.
    ///
    /// Reads one or more JSON profile files, maps fields to PrintConfig,
    /// and outputs clean TOML with only the converted fields.
    ConvertProfile {
        /// Input JSON profile file(s) to convert.
        /// Multiple files are merged in order (e.g., process + filament + machine).
        #[arg(required = true)]
        input: Vec<PathBuf>,

        /// Output TOML file path (default: stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Show detailed conversion report on stderr.
        #[arg(short, long)]
        verbose: bool,
    },

    /// Import upstream slicer profiles and convert to native TOML format.
    ///
    /// Walks a slicer resource directory (e.g., OrcaSlicer/resources/profiles/),
    /// resolves inheritance chains, and writes converted TOML profiles with
    /// a searchable index.json manifest.
    ImportProfiles {
        /// Source directory containing vendor profile directories
        /// (e.g., path to OrcaSlicer/resources/profiles/)
        #[arg(long)]
        source_dir: PathBuf,

        /// Output directory for converted TOML profiles (default: profiles/)
        #[arg(short, long, default_value = "profiles")]
        output_dir: PathBuf,

        /// Source slicer name (orcaslicer or bambustudio)
        #[arg(long, default_value = "orcaslicer")]
        source_name: String,
    },

    /// List profiles from the profile library.
    ///
    /// Loads the profile index and displays matching profiles in a tabular
    /// or JSON format. Supports filtering by vendor, type, and material.
    ListProfiles {
        /// Filter by vendor name (e.g., BBL, Creality, Prusa).
        #[arg(long)]
        vendor: Option<String>,

        /// Filter by profile type (filament, process, machine).
        #[arg(long, value_name = "TYPE")]
        profile_type: Option<String>,

        /// Filter by material type (PLA, ABS, PETG, TPU, etc.).
        #[arg(long)]
        material: Option<String>,

        /// List available vendors only (no individual profiles).
        #[arg(long)]
        vendors: bool,

        /// Path to profiles directory (overrides auto-detection).
        #[arg(long)]
        profiles_dir: Option<PathBuf>,

        /// Output as JSON instead of human-readable table.
        #[arg(long)]
        json: bool,
    },

    /// Search profiles by keyword (matches name, vendor, material, printer model).
    ///
    /// All search terms must match at least one field in the profile entry
    /// (AND logic). Matching is case-insensitive.
    SearchProfiles {
        /// Search query (case-insensitive substring match across all fields).
        query: String,

        /// Maximum results to show (default: 20).
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Path to profiles directory (overrides auto-detection).
        #[arg(long)]
        profiles_dir: Option<PathBuf>,

        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Show details of a specific profile from the library.
    ///
    /// Displays metadata summary or raw TOML content for a profile
    /// identified by its ID (e.g., orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1).
    ShowProfile {
        /// Profile ID (e.g., orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1).
        id: String,

        /// Show the full TOML content instead of metadata summary.
        #[arg(long)]
        raw: bool,

        /// Path to profiles directory (overrides auto-detection).
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
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
        Commands::ConvertProfile {
            input,
            output,
            verbose,
        } => cmd_convert_profile(&input, output.as_deref(), verbose),
        Commands::ImportProfiles {
            source_dir,
            output_dir,
            source_name,
        } => cmd_import_profiles(&source_dir, &output_dir, &source_name),
        Commands::ListProfiles {
            vendor,
            profile_type,
            material,
            vendors,
            profiles_dir,
            json,
        } => cmd_list_profiles(
            vendor.as_deref(),
            profile_type.as_deref(),
            material.as_deref(),
            vendors,
            profiles_dir.as_deref(),
            json,
        ),
        Commands::SearchProfiles {
            query,
            limit,
            profiles_dir,
            json,
        } => cmd_search_profiles(&query, limit, profiles_dir.as_deref(), json),
        Commands::ShowProfile {
            id,
            raw,
            profiles_dir,
        } => cmd_show_profile(&id, raw, profiles_dir.as_deref()),
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
        match PrintConfig::from_file(cfg_path) {
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

/// Convert OrcaSlicer/BambuStudio JSON profiles to native TOML format.
fn cmd_convert_profile(
    input: &[PathBuf],
    output_path: Option<&std::path::Path>,
    verbose: bool,
) {
    let mut results: Vec<slicecore_engine::ImportResult> = Vec::new();

    for path in input {
        // Read file contents.
        let contents = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "Error: Failed to read '{}': {}",
                    path.display(),
                    e
                );
                process::exit(1);
            }
        };

        // Parse JSON.
        let value: serde_json::Value = match serde_json::from_str(&contents) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "Error: Failed to parse JSON from '{}': {}",
                    path.display(),
                    e
                );
                process::exit(1);
            }
        };

        // Import the upstream profile.
        let result =
            match slicecore_engine::profile_import::import_upstream_profile(&value) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!(
                        "Error: Failed to import profile from '{}': {}",
                        path.display(),
                        e
                    );
                    process::exit(1);
                }
            };

        if verbose {
            eprintln!(
                "  File: {} -- {} mapped, {} unmapped",
                path.display(),
                result.mapped_fields.len(),
                result.unmapped_fields.len()
            );
        }

        results.push(result);
    }

    // Merge if multiple, or use single result directly.
    let final_result = if results.len() > 1 {
        slicecore_engine::merge_import_results(&results)
    } else {
        results.into_iter().next().unwrap()
    };

    // Convert to TOML.
    let converted = slicecore_engine::convert_to_toml(&final_result);

    // Output the TOML.
    if let Some(out_path) = output_path {
        if let Err(e) = std::fs::write(out_path, &converted.toml_output) {
            eprintln!(
                "Error: Failed to write output '{}': {}",
                out_path.display(),
                e
            );
            process::exit(1);
        }
    } else {
        print!("{}", converted.toml_output);
    }

    // Print conversion summary to stderr.
    let output_desc = if let Some(p) = output_path {
        p.display().to_string()
    } else {
        "stdout".to_string()
    };

    if let Some(ref name) = converted.source_name {
        if let Some(ref stype) = converted.source_type {
            eprintln!("Converted \"{}\" ({})", name, stype);
        } else {
            eprintln!("Converted \"{}\"", name);
        }
    } else {
        eprintln!("Converted profile");
    }
    eprintln!("  Mapped: {} fields", converted.mapped_count);
    eprintln!("  Unmapped: {} fields", converted.unmapped_fields.len());
    eprintln!("  Output: {}", output_desc);

    // Verbose: list field names.
    if verbose {
        eprintln!();
        eprintln!("Mapped fields:");
        for field in &final_result.mapped_fields {
            eprintln!("  - {}", field);
        }
        if !converted.unmapped_fields.is_empty() {
            eprintln!();
            eprintln!("Unmapped fields:");
            for field in &converted.unmapped_fields {
                eprintln!("  - {}", field);
            }
        }
    }
}

/// Import upstream slicer profiles and convert to native TOML format.
fn cmd_import_profiles(
    source_dir: &std::path::Path,
    output_dir: &std::path::Path,
    source_name: &str,
) {
    let target_dir = output_dir.join(source_name);

    eprintln!(
        "Importing {} profiles from '{}'...",
        source_name,
        source_dir.display()
    );

    // Dispatch to the appropriate batch conversion pipeline based on source name.
    let result = if source_name == "prusaslicer" {
        match batch_convert_prusaslicer_profiles(source_dir, &target_dir, source_name) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: Batch conversion failed: {}", e);
                process::exit(1);
            }
        }
    } else {
        match batch_convert_profiles(source_dir, &target_dir, source_name) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: Batch conversion failed: {}", e);
                process::exit(1);
            }
        }
    };

    // Write the index, merging with any existing index to preserve other sources.
    if let Err(e) = write_merged_index(&result.index, output_dir) {
        eprintln!("Error: Failed to write index: {}", e);
        process::exit(1);
    }

    // Print summary to stderr.
    let skip_label = if source_name == "prusaslicer" {
        "abstract/SLA profiles"
    } else {
        "non-instantiated base profiles"
    };
    eprintln!("Import complete:");
    eprintln!("  Converted: {} profiles", result.converted);
    eprintln!("  Skipped:   {} ({})", result.skipped, skip_label);
    eprintln!("  Errors:    {}", result.errors.len());
    eprintln!("  Output:    {}", output_dir.display());

    if !result.errors.is_empty() {
        let show_count = result.errors.len().min(10);
        eprintln!();
        eprintln!("First {} error(s):", show_count);
        for err in result.errors.iter().take(10) {
            eprintln!("  - {}", err);
        }
        if result.errors.len() > 10 {
            eprintln!("  ... and {} more", result.errors.len() - 10);
        }
    }
}

// ---------------------------------------------------------------------------
// Profile discovery helpers
// ---------------------------------------------------------------------------

/// Auto-detect the profiles directory using multiple strategies.
///
/// Priority:
/// 1. CLI flag override (`--profiles-dir`)
/// 2. `SLICECORE_PROFILES_DIR` environment variable
/// 3. Relative to binary (installed location, or cargo target dir)
/// 4. Current working directory `./profiles`
fn find_profiles_dir(cli_override: Option<&std::path::Path>) -> Option<PathBuf> {
    // 1. CLI flag override.
    if let Some(dir) = cli_override {
        return Some(dir.to_path_buf());
    }
    // 2. Environment variable.
    if let Ok(dir) = std::env::var("SLICECORE_PROFILES_DIR") {
        let p = PathBuf::from(dir);
        if p.exists() {
            return Some(p);
        }
    }
    // 3. Relative to binary (for installed location).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let profiles = parent.join("profiles");
            if profiles.exists() {
                return Some(profiles);
            }
            // For cargo run: target/debug/slicecore -> ../../profiles
            if let Some(gp) = parent
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
            {
                let profiles = gp.join("profiles");
                if profiles.exists() {
                    return Some(profiles);
                }
            }
        }
    }
    // 4. Current directory.
    let cwd_profiles = PathBuf::from("profiles");
    if cwd_profiles.exists() {
        return Some(cwd_profiles);
    }
    None
}

/// List profiles from the profile library.
fn cmd_list_profiles(
    vendor: Option<&str>,
    profile_type: Option<&str>,
    material: Option<&str>,
    vendors_only: bool,
    profiles_dir_override: Option<&std::path::Path>,
    json_output: bool,
) {
    let profiles_dir = match find_profiles_dir(profiles_dir_override) {
        Some(d) => d,
        None => {
            eprintln!("Error: Could not find profiles directory.");
            eprintln!("Use --profiles-dir, set SLICECORE_PROFILES_DIR, or run from the project root.");
            process::exit(1);
        }
    };

    let index = match load_index(&profiles_dir) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Error: Failed to load profile index: {}", e);
            process::exit(1);
        }
    };

    if vendors_only {
        // Collect unique vendor names, sorted.
        let mut vendors: Vec<String> = index
            .profiles
            .iter()
            .map(|p| p.vendor.clone())
            .collect();
        vendors.sort();
        vendors.dedup();

        if json_output {
            let json = serde_json::to_string_pretty(&vendors).unwrap_or_else(|e| {
                eprintln!("Error: Failed to serialize JSON: {}", e);
                process::exit(1);
            });
            println!("{}", json);
        } else {
            for v in &vendors {
                println!("{}", v);
            }
            eprintln!("{} vendor(s) found", vendors.len());
        }
        return;
    }

    // Filter profiles.
    let filtered: Vec<&ProfileIndexEntry> = index
        .profiles
        .iter()
        .filter(|p| {
            if let Some(v) = vendor {
                if !p.vendor.to_lowercase().contains(&v.to_lowercase()) {
                    return false;
                }
            }
            if let Some(t) = profile_type {
                if p.profile_type != t {
                    return false;
                }
            }
            if let Some(m) = material {
                match &p.material {
                    Some(mat) => {
                        if !mat.to_lowercase().contains(&m.to_lowercase()) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        })
        .collect();

    if json_output {
        let json = serde_json::to_string_pretty(&filtered).unwrap_or_else(|e| {
            eprintln!("Error: Failed to serialize JSON: {}", e);
            process::exit(1);
        });
        println!("{}", json);
    } else {
        // Print header.
        println!(
            "{:<10} {:<12} {:<50} {:<10}",
            "TYPE", "VENDOR", "NAME", "MATERIAL"
        );
        println!("{}", "-".repeat(86));

        for p in &filtered {
            println!(
                "{:<10} {:<12} {:<50} {:<10}",
                p.profile_type,
                p.vendor,
                truncate_str(&p.name, 48),
                p.material.as_deref().unwrap_or("-"),
            );
        }
        eprintln!("{} profile(s) found", filtered.len());
    }
}

/// Search profiles by keyword.
fn cmd_search_profiles(
    query: &str,
    limit: usize,
    profiles_dir_override: Option<&std::path::Path>,
    json_output: bool,
) {
    let profiles_dir = match find_profiles_dir(profiles_dir_override) {
        Some(d) => d,
        None => {
            eprintln!("Error: Could not find profiles directory.");
            eprintln!("Use --profiles-dir, set SLICECORE_PROFILES_DIR, or run from the project root.");
            process::exit(1);
        }
    };

    let index = match load_index(&profiles_dir) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Error: Failed to load profile index: {}", e);
            process::exit(1);
        }
    };

    // Split query into whitespace-separated terms (lowercase).
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .collect();

    // Filter profiles where ALL terms match at least one field.
    let matching: Vec<&ProfileIndexEntry> = index
        .profiles
        .iter()
        .filter(|p| {
            let fields = [
                p.name.to_lowercase(),
                p.vendor.to_lowercase(),
                p.material
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase(),
                p.printer_model
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase(),
                p.profile_type.to_lowercase(),
                p.quality
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase(),
            ];

            terms.iter().all(|term| {
                fields.iter().any(|f| f.contains(term.as_str()))
            })
        })
        .take(limit)
        .collect();

    if json_output {
        let json = serde_json::to_string_pretty(&matching).unwrap_or_else(|e| {
            eprintln!("Error: Failed to serialize JSON: {}", e);
            process::exit(1);
        });
        println!("{}", json);
    } else {
        println!(
            "{:<10} {:<12} {:<50} {:<10}",
            "TYPE", "VENDOR", "NAME", "MATERIAL"
        );
        println!("{}", "-".repeat(86));

        for p in &matching {
            println!(
                "{:<10} {:<12} {:<50} {:<10}",
                p.profile_type,
                p.vendor,
                truncate_str(&p.name, 48),
                p.material.as_deref().unwrap_or("-"),
            );
        }
        eprintln!(
            "{} result(s) (showing up to {})",
            matching.len(),
            limit
        );
    }
}

/// Show details of a specific profile.
fn cmd_show_profile(
    id: &str,
    raw: bool,
    profiles_dir_override: Option<&std::path::Path>,
) {
    let profiles_dir = match find_profiles_dir(profiles_dir_override) {
        Some(d) => d,
        None => {
            eprintln!("Error: Could not find profiles directory.");
            eprintln!("Use --profiles-dir, set SLICECORE_PROFILES_DIR, or run from the project root.");
            process::exit(1);
        }
    };

    let index = match load_index(&profiles_dir) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Error: Failed to load profile index: {}", e);
            process::exit(1);
        }
    };

    // Find entry: exact match on id, or path without .toml extension.
    let entry = index
        .profiles
        .iter()
        .find(|e| e.id == id || e.path.trim_end_matches(".toml") == id);

    let entry = match entry {
        Some(e) => e,
        None => {
            // Try case-insensitive match for suggestion.
            let id_lower = id.to_lowercase();
            let suggestion = index.profiles.iter().find(|e| {
                e.id.to_lowercase() == id_lower
                    || e.path.trim_end_matches(".toml").to_lowercase() == id_lower
            });

            if let Some(s) = suggestion {
                eprintln!(
                    "Error: Profile '{}' not found. Did you mean '{}'?",
                    id, s.id
                );
            } else {
                eprintln!("Error: Profile '{}' not found.", id);
                eprintln!("Use 'list-profiles' or 'search-profiles' to find available profiles.");
            }
            process::exit(1);
        }
    };

    if raw {
        // Read and print the TOML file.
        let toml_path = profiles_dir.join(&entry.path);
        let contents = match std::fs::read_to_string(&toml_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "Error: Failed to read profile file '{}': {}",
                    toml_path.display(),
                    e
                );
                process::exit(1);
            }
        };
        print!("{}", contents);
    } else {
        // Print structured metadata summary.
        println!("Profile: {}", entry.name);
        println!("Source:  {}", entry.source);
        println!("Vendor:  {}", entry.vendor);
        println!("Type:    {}", entry.profile_type);
        if let Some(ref mat) = entry.material {
            println!("Material: {}", mat);
        }
        if let Some(ref model) = entry.printer_model {
            println!("Printer: {}", model);
        }
        if let Some(height) = entry.layer_height {
            println!("Layer height: {:.2}mm", height);
        }
        if let Some(nozzle) = entry.nozzle_size {
            println!("Nozzle: {:.1}mm", nozzle);
        }
        if let Some(ref quality) = entry.quality {
            println!("Quality: {}", quality);
        }
        println!("ID:      {}", entry.id);
        println!("Path:    {}", entry.path);
    }
}

/// Truncate a string to `max_len` characters, appending ".." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}..", &s[..max_len - 2])
    }
}

// ---------------------------------------------------------------------------
// AI suggestion
// ---------------------------------------------------------------------------

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
