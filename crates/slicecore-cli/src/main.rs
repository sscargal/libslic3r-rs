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
//! - `convert`: Convert a mesh file between formats (STL, 3MF, OBJ)
//! - `analyze-gcode`: Analyze a G-code file with structured metrics output
//! - `compare-gcode`: Compare multiple G-code files with deltas
//! - `arrange`: Arrange multiple mesh files on a build plate
//! - `post-process`: Post-process an existing G-code file
//! - `csg`: CSG boolean operations, splitting, hollowing, primitives, and mesh info
//! - `schema`: Query the setting schema registry (JSON Schema, metadata, search)
//! - `diff-profiles`: Compare two print profiles side by side
//! - `profile`: Manage profiles (clone, set, get, reset, edit, validate, delete, rename)

mod analysis_display;
mod calibrate;
pub mod cli_output;
mod csg_command;
mod csg_info;
mod diff_profiles_command;
mod plugins_command;
mod profile_command;
mod profile_wizard;
mod schema_command;
mod slice_workflow;
mod stats_display;

use std::io::{BufReader, IsTerminal, Read as _};
use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

use slicecore_ai::AiConfig;
use slicecore_engine::profile_resolve::{ProfileResolver, ProfileSource};
use slicecore_engine::{
    batch_convert_profiles, batch_convert_prusaslicer_profiles, config::PostProcessConfig,
    create_builtin_postprocessors, load_index, write_merged_index, Engine, PrintConfig,
    ProfileIndexEntry,
};
use slicecore_fileio::{load_mesh, save_mesh};
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
    slicecore show-profile orcaslicer/BBL/filament/Bambu_PLA_Basic_BBL_A1 --raw

MESH CONVERSION:
  Convert between mesh file formats:
    slicecore convert model.stl model.3mf
    slicecore convert model.3mf model.obj
  Supported output formats: .stl (binary), .3mf, .obj

G-CODE ANALYSIS:
  Analyze a single G-code file:
    slicecore analyze-gcode output.gcode
    slicecore analyze-gcode output.gcode --json
    slicecore analyze-gcode output.gcode --csv --summary
    cat output.gcode | slicecore analyze-gcode -

  Compare G-code files from different slicers:
    slicecore compare-gcode bambu.gcode orca.gcode prusa.gcode
    slicecore compare-gcode baseline.gcode variant.gcode --json

PROFILE MANAGEMENT:
  Clone a profile for customization:
    slicecore profile clone BBL/PLA_Basic my-pla
  Edit a single setting:
    slicecore profile set my-pla speed.perimeter 60
  Open in editor:
    slicecore profile edit my-pla
  Validate against schema:
    slicecore profile validate my-pla
  Delete a custom profile:
    slicecore profile delete my-pla --yes

PLUGIN MANAGEMENT:
  List installed plugins:
    slicecore plugins list
    slicecore plugins list --json
    slicecore plugins list --category infill --status enabled
  Manage plugins:
    slicecore plugins enable <name>
    slicecore plugins disable <name>
    slicecore plugins info <name>
    slicecore plugins validate <name>"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Plugin directory (overrides config plugin_dir)
    #[arg(long, global = true)]
    plugin_dir: Option<PathBuf>,

    /// Suppress progress output, warnings, and informational messages
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Color output mode
    #[arg(long, global = true, default_value = "auto", value_parser = ["always", "never", "auto"])]
    color: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Slice an STL file to G-code
    Slice {
        /// Input STL file path
        input: PathBuf,

        /// Print config file (TOML or JSON, auto-detected; optional -- uses defaults if not provided)
        #[arg(short, long, conflicts_with_all = ["machine", "filament", "process"])]
        config: Option<PathBuf>,

        /// Machine profile name or path
        #[arg(short, long, conflicts_with = "config")]
        machine: Option<String>,

        /// Filament profile name or path
        #[arg(short, long, conflicts_with = "config")]
        filament: Option<String>,

        /// Process profile name or path
        #[arg(short, long, conflicts_with = "config")]
        process: Option<String>,

        /// TOML/JSON override file (applied after profiles)
        #[arg(long, conflicts_with = "config")]
        overrides: Option<PathBuf>,

        /// Override a config key (repeatable, format: key=value)
        #[arg(long = "set", conflicts_with = "config")]
        set_overrides: Vec<String>,

        /// Resolve profiles and validate config without slicing
        #[arg(long)]
        dry_run: bool,

        /// Save merged config to a TOML file
        #[arg(long, value_name = "FILE")]
        save_config: Option<PathBuf>,

        /// Print merged config with source annotations
        #[arg(long)]
        show_config: bool,

        /// Allow slicing without profiles (dev/testing only)
        #[arg(long)]
        unsafe_defaults: bool,

        /// Override safety validation errors
        #[arg(long)]
        force: bool,

        /// Suppress log file creation
        #[arg(long)]
        no_log: bool,

        /// Custom log file path
        #[arg(long, value_name = "FILE")]
        log_file: Option<PathBuf>,

        /// Profile library directory override
        #[arg(long, value_name = "DIR")]
        profiles_dir: Option<PathBuf>,

        /// Output G-code file path (default: input with .gcode extension)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Output slicing metadata as JSON to stdout
        #[arg(long)]
        json: bool,

        /// Output slicing metadata as MessagePack to stdout
        #[arg(long)]
        msgpack: bool,

        /// Statistics output format (table, csv, json). Default: table.
        #[arg(long, default_value = "table", value_parser = ["table", "csv", "json"])]
        stats_format: String,

        /// Save statistics to a file (in addition to stdout display).
        #[arg(long, value_name = "FILE")]
        stats_file: Option<PathBuf>,

        /// Exclude statistics from JSON output (only with --json).
        #[arg(long)]
        json_no_stats: bool,

        /// Time precision for statistics display.
        #[arg(long, default_value = "seconds", value_parser = ["seconds", "deciseconds", "milliseconds"])]
        time_precision: String,

        /// Sort order for feature statistics.
        #[arg(long, default_value = "default", value_parser = ["default", "time", "filament", "alpha"])]
        sort_stats: String,

        /// Embed thumbnail images in output (3MF or G-code).
        #[arg(long)]
        thumbnails: bool,

        /// Thumbnail image format (png or jpeg)
        #[arg(long, default_value = "png")]
        thumbnail_format: String,

        /// Thumbnail JPEG quality (1-100, default: 85). Ignored for PNG
        #[arg(long)]
        thumbnail_quality: Option<u8>,

        /// Auto-arrange input mesh(es) on bed before slicing.
        #[arg(long)]
        auto_arrange: bool,

        /// Disable travel move optimization (for debugging/comparison).
        #[arg(long)]
        no_travel_opt: bool,

        /// Use a saved profile set (expands to -m/-f/-p)
        #[arg(long = "profile-set", value_name = "SET_NAME", conflicts_with_all = ["config", "machine", "filament", "process"])]
        profile_set: Option<String>,
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

        /// Output as JSON instead of TOML.
        #[arg(long)]
        json: bool,
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

        /// Output as JSON instead of human-readable progress.
        #[arg(long)]
        json: bool,
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

    /// Compare two print profiles side by side
    DiffProfiles(diff_profiles_command::DiffProfilesArgs),

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

    /// Analyze a G-code file and display structured metrics
    AnalyzeGcode {
        /// Input G-code file path (use "-" for stdin)
        input: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Output as CSV
        #[arg(long)]
        csv: bool,

        /// Disable ANSI color output
        // TODO(phase-40): consider migrating to global --color flag
        #[arg(long)]
        no_color: bool,

        /// Filament density in g/cm3 (default: 1.24 for PLA)
        #[arg(long, default_value = "1.24")]
        density: f64,

        /// Filament diameter in mm (default: 1.75)
        #[arg(long, default_value = "1.75")]
        diameter: f64,

        /// Filter output to specific feature types (comma-separated)
        #[arg(long)]
        filter: Option<String>,

        /// Summary only (no per-layer detail)
        #[arg(long)]
        summary: bool,

        /// Filament price per kg (currency units)
        #[arg(long)]
        filament_price: Option<f64>,

        /// Printer power consumption in watts
        #[arg(long)]
        printer_watts: Option<f64>,

        /// Electricity rate per kWh (currency units)
        #[arg(long)]
        electricity_rate: Option<f64>,

        /// Printer purchase cost (currency units)
        #[arg(long)]
        printer_cost: Option<f64>,

        /// Expected printer lifetime in hours
        #[arg(long)]
        expected_hours: Option<f64>,

        /// Labor hourly rate (currency units)
        #[arg(long)]
        labor_rate: Option<f64>,

        /// Setup/post-processing time in minutes
        #[arg(long, default_value = "5.0")]
        setup_time: f64,

        /// Output as markdown table
        #[arg(long)]
        markdown: bool,

        /// Treat input as model file (STL/3MF) for rough volume-based estimation
        #[arg(long)]
        model: bool,

        /// Compare with additional filament profiles (side-by-side cost/time table)
        #[arg(long = "compare-filament", num_args = 1..)]
        compare_filament: Vec<String>,

        /// Profile library directory for resolving filament profile names
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Convert a mesh file between formats (STL, 3MF, OBJ).
    ///
    /// Input format is auto-detected from file content.
    /// Output format is determined by the output file extension.
    Convert {
        /// Input mesh file path
        input: PathBuf,
        /// Output mesh file path (format detected from extension: .stl, .3mf, .obj)
        output: PathBuf,
    },

    /// Generate thumbnail preview images from a mesh file.
    Thumbnail {
        /// Input mesh file (STL, 3MF, OBJ)
        input: PathBuf,
        /// Output directory or file path (default: input filename with .png extension)
        #[arg(short, long)]
        output: Option<String>,
        /// Camera angles to render (comma-separated: front,back,left,right,top,isometric)
        /// Default: isometric only
        #[arg(long, default_value = "isometric")]
        angles: String,
        /// Resolution WxH (e.g., "300x300", "220x124", "640x480")
        /// Default: 300x300
        #[arg(long, default_value = "300x300")]
        resolution: String,
        /// Background color as hex (e.g., "transparent", "FFFFFF", "000000")
        /// Default: transparent
        #[arg(long, default_value = "transparent")]
        background: String,
        /// Model color as hex (e.g., "C8C8C8", "FF0000")
        /// Default: C8C8C8 (light gray)
        #[arg(long, default_value = "C8C8C8")]
        color: String,

        /// Image format for output (png or jpeg)
        #[arg(long, default_value = "png")]
        format: String,

        /// JPEG quality (1-100, default: 85). Ignored for PNG
        #[arg(long)]
        quality: Option<u8>,
    },

    /// Compare multiple G-code files (first file is baseline)
    CompareGcode {
        /// G-code files to compare (first is baseline, need at least 2)
        files: Vec<PathBuf>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Output as CSV
        #[arg(long)]
        csv: bool,

        /// Disable ANSI color output
        // TODO(phase-40): consider migrating to global --color flag
        #[arg(long)]
        no_color: bool,

        /// Filament density in g/cm3 (default: 1.24 for PLA)
        #[arg(long, default_value = "1.24")]
        density: f64,

        /// Filament diameter in mm (default: 1.75)
        #[arg(long, default_value = "1.75")]
        diameter: f64,
    },

    /// Arrange multiple mesh files on a build plate.
    ///
    /// Outputs a JSON arrangement plan by default. Use --apply to write
    /// transformed mesh files, or --format 3mf to produce a positioned 3MF.
    Arrange {
        /// Input mesh files to arrange
        #[arg(required = true)]
        input: Vec<PathBuf>,

        /// Print config file (TOML or JSON; optional)
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Bed shape string (e.g., "0x0,220x0,220x220,0x220"). Overrides config.
        #[arg(long)]
        bed_shape: Option<String>,

        /// Part spacing in mm
        #[arg(long, default_value = "2.0")]
        spacing: f64,

        /// Bed edge margin in mm
        #[arg(long, default_value = "5.0")]
        margin: f64,

        /// Rotation step in degrees
        #[arg(long, default_value = "45.0")]
        rotation_step: f64,

        /// Disable auto-orient
        #[arg(long)]
        no_auto_orient: bool,

        /// Enable sequential printing mode
        #[arg(long)]
        sequential: bool,

        /// Apply transforms and write output files
        #[arg(long)]
        apply: bool,

        /// Output format: "json" (default) or "3mf" (positioned 3MF)
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// Calibration test print generation.
    ///
    /// Generate G-code for temperature tower, retraction, flow rate, and
    /// first layer adhesion calibration tests.
    #[command(subcommand)]
    Calibrate(calibrate::CalibrateCommand),

    /// CSG (Constructive Solid Geometry) operations on meshes.
    ///
    /// Boolean operations (union, difference, intersection, xor), plane splitting,
    /// hollowing, primitive generation, and mesh information display.
    #[command(subcommand)]
    Csg(csg_command::CsgCommand),

    /// Query the setting schema registry.
    ///
    /// Outputs JSON Schema or flat metadata JSON for all registered settings.
    /// Supports filtering by tier, category, and full-text search.
    Schema(schema_command::SchemaArgs),

    /// Manage installed plugins.
    ///
    /// List, enable, disable, inspect, and validate plugins from the
    /// configured plugin directory.
    #[command(subcommand)]
    Plugins(plugins_command::PluginsCommand),

    /// Manage profiles: clone, edit, validate, delete, rename.
    ///
    /// Create custom profiles from library presets and modify them.
    /// Use `profile clone` to start, then `profile set` or `profile edit`
    /// to customize settings.
    #[command(subcommand)]
    Profile(profile_command::ProfileCommand),

    /// Post-process an existing G-code file.
    ///
    /// Reads a G-code file, applies configured post-processors (pause-at-layer,
    /// timelapse, fan override, custom G-code injection), and writes the result.
    /// Works with G-code from any slicer, not just slicecore.
    PostProcess {
        /// Input G-code file path
        input: PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Path to TOML config file with `[post_process]` section
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Insert pause at layer N (repeatable)
        #[arg(long = "pause-at-layer", value_name = "LAYER")]
        pause_at_layers: Vec<usize>,

        /// Pause command (default: "M0", alternative: "M600")
        #[arg(long = "pause-command", default_value = "M0")]
        pause_command: String,

        /// Enable timelapse mode
        #[arg(long)]
        timelapse: bool,

        /// Park X position for timelapse
        #[arg(long = "timelapse-park-x", default_value = "0.0")]
        timelapse_park_x: f64,

        /// Park Y position for timelapse
        #[arg(long = "timelapse-park-y", default_value = "200.0")]
        timelapse_park_y: f64,

        /// Fan speed override rule (repeatable, format "start_layer:end_layer:speed",
        /// use "*" for end_layer to mean until end)
        #[arg(long = "fan-override", value_name = "START:END:SPEED")]
        fan_overrides: Vec<String>,

        /// Custom G-code injection (repeatable, format "trigger:gcode" where trigger
        /// is "every_N", "at_N", "before_retract", or "after_retract")
        #[arg(long = "inject-gcode", value_name = "TRIGGER:GCODE")]
        inject_gcode: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    let global_plugin_dir = cli.plugin_dir;
    let global_quiet = cli.quiet;
    let global_color = cli.color;

    let color_mode = match global_color.as_str() {
        "always" => cli_output::ColorMode::Always,
        "never" => cli_output::ColorMode::Never,
        _ => cli_output::ColorMode::Auto,
    };

    match cli.command {
        Commands::Slice {
            input,
            config,
            machine,
            filament,
            process,
            overrides,
            set_overrides,
            dry_run,
            save_config,
            show_config,
            unsafe_defaults,
            force,
            no_log,
            log_file,
            profiles_dir,
            output,
            json,
            msgpack,
            stats_format,
            stats_file,
            json_no_stats,
            time_precision,
            sort_stats,
            thumbnails,
            thumbnail_format,
            thumbnail_quality,
            auto_arrange,
            no_travel_opt,
            profile_set,
        } => {
            // Expand --profile-set flag to -m/-f/-p
            let (machine, filament, process) = if let Some(ref set_name) = profile_set {
                let ep_path = slicecore_engine::enabled_profiles::EnabledProfiles::default_path()
                    .or_else(|| profiles_dir.as_ref().map(|d| d.join("enabled-profiles.toml")));
                let ep = ep_path
                    .as_ref()
                    .and_then(|p| {
                        slicecore_engine::enabled_profiles::EnabledProfiles::load(p)
                            .ok()
                            .flatten()
                    });
                match ep.and_then(|e| e.get_set(set_name).cloned()) {
                    Some(set) => (Some(set.machine), Some(set.filament), Some(set.process)),
                    None => {
                        eprintln!("Error: Profile set '{set_name}' not found.");
                        eprintln!("Run: slicecore profile set list");
                        std::process::exit(1);
                    }
                }
            } else if machine.is_none()
                && filament.is_none()
                && process.is_none()
                && config.is_none()
            {
                // Try default set
                let ep_path = slicecore_engine::enabled_profiles::EnabledProfiles::default_path()
                    .or_else(|| profiles_dir.as_ref().map(|d| d.join("enabled-profiles.toml")));
                let ep = ep_path
                    .as_ref()
                    .and_then(|p| {
                        slicecore_engine::enabled_profiles::EnabledProfiles::load(p)
                            .ok()
                            .flatten()
                    });
                match ep.and_then(|e| e.default_set().map(|(_, s)| s.clone())) {
                    Some(set) => {
                        eprintln!(
                            "Using default profile set: machine={}, filament={}, process={}",
                            set.machine, set.filament, set.process
                        );
                        (Some(set.machine), Some(set.filament), Some(set.process))
                    }
                    None => (machine, filament, process),
                }
            } else {
                (machine, filament, process)
            };

            cmd_slice(
                &input,
                config.as_deref(),
                machine.as_deref(),
                filament.as_deref(),
                process.as_deref(),
                overrides.as_deref(),
                &set_overrides,
                dry_run,
                save_config.as_deref(),
                show_config,
                unsafe_defaults,
                force,
                no_log,
                log_file.as_deref(),
                profiles_dir.as_deref(),
                output.as_deref(),
                json,
                msgpack,
                global_plugin_dir.as_deref(),
                &stats_format,
                global_quiet,
                stats_file.as_deref(),
                json_no_stats,
                &time_precision,
                &sort_stats,
                thumbnails,
                &thumbnail_format,
                thumbnail_quality,
                auto_arrange,
                no_travel_opt,
                color_mode,
            );
        }
        Commands::Validate { input } => cmd_validate(&input),
        Commands::Analyze { input } => cmd_analyze(&input),
        Commands::ConvertProfile {
            input,
            output,
            verbose,
            json,
        } => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, json, color_mode);
            let spinner = output_ctx.spinner("Converting profile");
            cmd_convert_profile(&input, output.as_deref(), verbose);
            output_ctx.finish_spinner(&spinner, "Profile converted");
        }
        Commands::ImportProfiles {
            source_dir,
            output_dir,
            source_name,
            json,
        } => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, json, color_mode);
            let spinner = output_ctx.spinner("Importing profiles");
            cmd_import_profiles(&source_dir, &output_dir, &source_name);
            output_ctx.finish_spinner(&spinner, "Profiles imported");
        }
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
        Commands::DiffProfiles(args) => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, false, color_mode);
            match diff_profiles_command::run_diff_profiles_command(
                &args,
                &global_color,
                global_quiet,
            ) {
                Ok(has_differences) => {
                    if has_differences {
                        process::exit(1);
                    }
                }
                Err(e) => {
                    output_ctx.error_msg(&format!("{e}"));
                    process::exit(2);
                }
            }
        }
        Commands::AiSuggest {
            input,
            ai_config,
            format,
        } => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, format == "json", color_mode);
            let spinner = output_ctx.spinner("Analyzing mesh and querying AI");
            cmd_ai_suggest(&input, ai_config.as_deref(), &format);
            output_ctx.finish_spinner(&spinner, "AI suggestion complete");
        }
        Commands::AnalyzeGcode {
            input,
            json,
            csv,
            no_color,
            density,
            diameter,
            filter,
            summary,
            filament_price,
            printer_watts,
            electricity_rate,
            printer_cost,
            expected_hours,
            labor_rate,
            setup_time,
            markdown,
            model,
            compare_filament,
            profiles_dir,
        } => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, json, color_mode);
            let spinner = output_ctx.spinner("Analyzing G-code");
            cmd_analyze_gcode(
                &input,
                json,
                csv,
                no_color,
                density,
                diameter,
                filter,
                summary,
                filament_price,
                printer_watts,
                electricity_rate,
                printer_cost,
                expected_hours,
                labor_rate,
                setup_time,
                markdown,
                model,
                &compare_filament,
                profiles_dir.as_deref(),
            );
            output_ctx.finish_spinner(&spinner, "Analysis complete");
        }
        Commands::Convert { input, output } => cmd_convert(&input, &output),
        Commands::Thumbnail {
            input,
            output,
            angles,
            resolution,
            background,
            color,
            format,
            quality,
        } => cmd_thumbnail(
            &input,
            output.as_deref(),
            &angles,
            &resolution,
            &background,
            &color,
            &format,
            quality,
        ),
        Commands::CompareGcode {
            files,
            json,
            csv,
            no_color,
            density,
            diameter,
        } => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, json, color_mode);
            let spinner = output_ctx.spinner("Comparing G-code files");
            cmd_compare_gcode(&files, json, csv, no_color, density, diameter);
            output_ctx.finish_spinner(&spinner, "Comparison complete");
        }
        Commands::Arrange {
            input,
            config,
            bed_shape,
            spacing,
            margin,
            rotation_step,
            no_auto_orient,
            sequential,
            apply,
            format,
        } => cmd_arrange(
            &input,
            config.as_deref(),
            bed_shape.as_deref(),
            spacing,
            margin,
            rotation_step,
            no_auto_orient,
            sequential,
            apply,
            &format,
        ),
        Commands::Calibrate(cal_cmd) => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, false, color_mode);
            let spinner = output_ctx.spinner("Generating calibration G-code");
            if let Err(e) = calibrate::run_calibrate(cal_cmd, &output_ctx) {
                output_ctx.finish_spinner(&spinner, "");
                output_ctx.error_msg(&format!("{e}"));
                process::exit(1);
            }
            output_ctx.finish_spinner(&spinner, "Calibration G-code generated");
        }
        Commands::Csg(csg_cmd) => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, false, color_mode);
            let spinner = output_ctx.spinner("Running CSG operation");
            if let Err(e) = csg_command::run_csg(csg_cmd, &output_ctx) {
                output_ctx.finish_spinner(&spinner, "");
                output_ctx.error_msg(&format!("{e}"));
                process::exit(1);
            }
            output_ctx.finish_spinner(&spinner, "CSG operation complete");
        }
        Commands::Schema(args) => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, false, color_mode);
            if let Err(e) = schema_command::run_schema_command(&args) {
                output_ctx.error_msg(&format!("{e}"));
                process::exit(1);
            }
        }
        Commands::Plugins(plugins_cmd) => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, false, color_mode);
            let dir = match global_plugin_dir.as_deref() {
                Some(d) => d.to_path_buf(),
                None => {
                    output_ctx.error_msg("No plugin directory configured.");
                    output_ctx.error_msg(
                        "Set 'plugin_dir' in your config TOML or use --plugin-dir on the command line.",
                    );
                    process::exit(1);
                }
            };
            if let Err(e) = plugins_command::run_plugins(plugins_cmd, &dir) {
                output_ctx.error_msg(&format!("{e}"));
                process::exit(1);
            }
        }
        Commands::Profile(profile_cmd) => {
            let output_ctx = cli_output::CliOutput::new(global_quiet, false, color_mode);
            if let Err(e) = profile_command::run_profile_command(profile_cmd) {
                output_ctx.error_msg(&format!("{e}"));
                process::exit(1);
            }
        }
        Commands::PostProcess {
            input,
            output,
            config,
            pause_at_layers,
            pause_command,
            timelapse,
            timelapse_park_x,
            timelapse_park_y,
            fan_overrides,
            inject_gcode,
        } => cmd_post_process(
            &input,
            output.as_deref(),
            config.as_deref(),
            &pause_at_layers,
            &pause_command,
            timelapse,
            timelapse_park_x,
            timelapse_park_y,
            &fan_overrides,
            &inject_gcode,
        ),
    }
}

/// Slice an STL/mesh file to G-code.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn cmd_slice(
    input: &PathBuf,
    config_path: Option<&std::path::Path>,
    machine: Option<&str>,
    filament: Option<&str>,
    process: Option<&str>,
    overrides_file: Option<&std::path::Path>,
    set_overrides: &[String],
    dry_run: bool,
    save_config: Option<&std::path::Path>,
    show_config: bool,
    unsafe_defaults: bool,
    force: bool,
    no_log: bool,
    log_file: Option<&std::path::Path>,
    profiles_dir: Option<&std::path::Path>,
    output_path: Option<&std::path::Path>,
    json_output: bool,
    msgpack_output: bool,
    plugin_dir: Option<&std::path::Path>,
    stats_format: &str,
    quiet: bool,
    stats_file: Option<&std::path::Path>,
    json_no_stats: bool,
    time_precision: &str,
    sort_stats: &str,
    thumbnails: bool,
    thumbnail_format: &str,
    thumbnail_quality: Option<u8>,
    auto_arrange: bool,
    no_travel_opt: bool,
    color_mode: cli_output::ColorMode,
) {
    // Determine if we're using the new profile-based workflow or legacy --config path.
    let use_profile_workflow = machine.is_some()
        || filament.is_some()
        || process.is_some()
        || !set_overrides.is_empty()
        || overrides_file.is_some()
        || dry_run
        || save_config.is_some()
        || show_config
        || unsafe_defaults;

    // Check if profile setup is needed (first-run wizard trigger)
    // Skip when profiles are explicitly specified via -m/-f/-p or --profiles-dir
    let has_explicit_profiles =
        machine.is_some() || filament.is_some() || process.is_some() || unsafe_defaults;
    if use_profile_workflow && !has_explicit_profiles && profiles_dir.is_none() {
        let enabled_path = slicecore_engine::enabled_profiles::EnabledProfiles::default_path();
        if let Some(ref path) = enabled_path {
            if !path.exists() && !force {
                if std::io::stdin().is_terminal() {
                    eprintln!("No profiles enabled yet. Starting setup wizard...");
                    eprintln!("(Use --force to skip, or run 'slicecore profile setup' manually)\n");
                    if let Err(e) = crate::profile_wizard::run_setup_wizard(profiles_dir, false) {
                        eprintln!("Setup wizard failed: {e}");
                        eprintln!("Run 'slicecore profile setup' manually or use --force to skip.");
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("Error: No enabled profiles found.");
                    eprintln!("Run: slicecore profile setup --machine <id> --filament <id>");
                    eprintln!("Or use --force to proceed without profile activation.");
                    std::process::exit(1);
                }
            }
        }
    }

    let total_steps = if use_profile_workflow { 5 } else { 4 };
    let output = cli_output::CliOutput::new(quiet, json_output, color_mode);

    // Step 1: Load mesh (read + parse + repair).
    let step1 = output.start_step(1, total_steps, "Load mesh");

    let data = match std::fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            output.error_msg(&format!(
                "Failed to read input file '{}': {}",
                input.display(),
                e
            ));
            process::exit(1);
        }
    };

    let mesh = match load_mesh(&data) {
        Ok(m) => m,
        Err(e) => {
            output.error_msg(&format!(
                "Failed to parse mesh from '{}': {}",
                input.display(),
                e
            ));
            process::exit(1);
        }
    };

    let vertices = mesh.vertices().to_vec();
    let indices = mesh.indices().to_vec();
    let (repaired_mesh, report) = match repair(vertices, indices) {
        Ok((m, r)) => (m, r),
        Err(e) => {
            output.error_msg(&format!("Failed to repair mesh: {}", e));
            process::exit(1);
        }
    };

    if !report.was_already_clean {
        output.info(&format!(
            "Mesh repaired ({} degenerates removed, {} edges stitched, {} holes filled, {} normals fixed)",
            report.degenerate_removed,
            report.edges_stitched,
            report.holes_filled,
            report.normals_fixed,
        ));
    }

    output.finish_step(&step1, "Load mesh");

    // Step 2 (& 3 for profile workflow): Load config / Resolve profiles / Validate.
    let (mut print_config, gcode_header_opt) = if use_profile_workflow {
        let step2 = output.start_step(2, total_steps, "Resolve profiles");

        let workflow_options = slice_workflow::SliceWorkflowOptions {
            machine: machine.map(String::from),
            filament: filament.map(String::from),
            process: process.map(String::from),
            overrides_file: overrides_file.map(PathBuf::from),
            set_overrides: set_overrides.to_vec(),
            dry_run,
            save_config: save_config.map(PathBuf::from),
            show_config,
            unsafe_defaults,
            force,
            no_log,
            log_file: log_file.map(PathBuf::from),
            profiles_dir: profiles_dir.map(PathBuf::from),
            input_path: input.clone(),
            json_output,
        };

        match slice_workflow::run_slice_workflow(&workflow_options, &output) {
            Ok(result) => {
                output.finish_step(&step2, "Resolve profiles");

                let step3 = output.start_step(3, total_steps, "Validate config");
                // Validation already ran inside run_slice_workflow; step is informational.
                output.finish_step(&step3, "Validate config");

                let header =
                    slice_workflow::generate_gcode_header(&result.composed, &workflow_options);
                (result.composed.config, Some(header))
            }
            Err(code) => process::exit(code),
        }
    } else {
        let step2 = output.start_step(2, total_steps, "Load config");

        let config = if let Some(cfg_path) = config_path {
            match PrintConfig::from_file(cfg_path) {
                Ok(c) => c,
                Err(e) => {
                    output.error_msg(&format!(
                        "Failed to load config '{}': {}",
                        cfg_path.display(),
                        e
                    ));
                    process::exit(1);
                }
            }
        } else {
            PrintConfig::default()
        };

        output.finish_step(&step2, "Load config");
        (config, None)
    };

    // Load plugins (if applicable).
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
        output.error_msg(
            "infill_pattern is set to a plugin pattern, but no plugin directory is configured.",
        );
        output
            .info("Set 'plugin_dir' in your config TOML or use --plugin-dir on the command line.");
        process::exit(1);
    }

    // Apply CLI flag overrides.
    if no_travel_opt {
        print_config.travel_opt.enabled = false;
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
                        output.info(&format!("Loaded {} plugin(s):", loaded.len()));
                        for info in &loaded {
                            output.info(&format!("  - {}: {}", info.name, info.description));
                        }
                    }
                    engine = engine.with_plugin_registry(registry);
                }
                Err(e) => {
                    output.warn(&format!("Failed to load plugins from '{}': {}", dir, e));
                }
            }
        }
    } else if engine.has_plugin_registry() {
        // Engine auto-loaded from config.plugin_dir -- report status.
        output.info("Plugins auto-loaded from config plugin_dir");
    } else if let Some(ref dir) = effective_plugin_dir {
        // Fallback: config had plugin_dir but engine didn't load (shouldn't normally happen).
        let mut registry = PluginRegistry::new();
        match registry.discover_and_load(std::path::Path::new(dir)) {
            Ok(loaded) => {
                if !loaded.is_empty() {
                    output.info(&format!("Loaded {} plugin(s):", loaded.len()));
                    for info in &loaded {
                        output.info(&format!("  - {}: {}", info.name, info.description));
                    }
                }
                engine = engine.with_plugin_registry(registry);
            }
            Err(e) => {
                output.warn(&format!("Failed to load plugins from '{}': {}", dir, e));
            }
        }
    }

    // 5b. Auto-arrange (if requested).
    if auto_arrange {
        let part = slicecore_arrange::ArrangePart {
            id: input
                .file_stem()
                .map_or_else(|| "input".into(), |s| s.to_string_lossy().into_owned()),
            vertices: repaired_mesh.vertices().to_vec(),
            mesh_height: {
                let zs: Vec<f64> = repaired_mesh.vertices().iter().map(|v| v.z).collect();
                let z_min = zs.iter().copied().fold(f64::INFINITY, f64::min);
                let z_max = zs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                z_max - z_min
            },
            ..Default::default()
        };
        let arrange_config = slicecore_arrange::ArrangeConfig::default();
        let bed_shape_str = &print_config.machine.bed_shape;
        let bed_x = print_config.machine.bed_x;
        let bed_y = print_config.machine.bed_y;
        match slicecore_arrange::arrange(&[part], &arrange_config, bed_shape_str, bed_x, bed_y) {
            Ok(result) => {
                let json = serde_json::to_string_pretty(&result).unwrap_or_default();
                output.info(&format!("Auto-arrange plan:\n{json}"));
                if !result.unplaced_parts.is_empty() {
                    output.warn(&format!(
                        "{} part(s) could not be placed on the bed",
                        result.unplaced_parts.len()
                    ));
                }
            }
            Err(e) => {
                output.warn(&format!("Auto-arrange failed: {e}"));
            }
        }
    }

    // Step N: Slice.
    let slice_step_num = if use_profile_workflow { 4 } else { 3 };
    let step_slice = output.start_step(slice_step_num, total_steps, "Slice");

    let result = match engine.slice(&repaired_mesh, None) {
        Ok(r) => r,
        Err(e) => {
            output.error_msg(&format!("Slicing failed: {}", e));
            process::exit(1);
        }
    };

    output.finish_step(&step_slice, "Slice");

    // 7. Generate and embed thumbnails if requested.
    let mut gcode_output = result.gcode.clone();
    if thumbnails {
        let image_format = match thumbnail_format {
            "jpeg" | "jpg" => slicecore_render::ImageFormat::Jpeg,
            _ => slicecore_render::ImageFormat::Png,
        };
        let quality = validate_quality(thumbnail_quality, image_format);

        // 3MF always requires PNG thumbnails per spec
        let is_3mf = output_path.is_some_and(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("3mf"))
        });
        let (thumb_format, thumb_q) =
            if is_3mf && image_format == slicecore_render::ImageFormat::Jpeg {
                output.warn("JPEG not supported for 3MF thumbnails, using PNG");
                (slicecore_render::ImageFormat::Png, None)
            } else {
                (image_format, quality)
            };

        let thumb_config = slicecore_render::ThumbnailConfig {
            width: print_config.thumbnail_resolution[0],
            height: print_config.thumbnail_resolution[1],
            angles: vec![slicecore_render::CameraAngle::Isometric],
            output_format: thumb_format,
            quality: thumb_q,
            ..slicecore_render::ThumbnailConfig::default()
        };
        let thumbs = slicecore_render::render_mesh(&repaired_mesh, &thumb_config);
        if let Some(thumb) = thumbs.first() {
            // Determine dialect name for thumbnail format selection
            let dialect_name = format!("{:?}", print_config.gcode_dialect).to_lowercase();
            if let Some(fmt) = slicecore_render::thumbnail_format_for_dialect(&dialect_name) {
                let block = slicecore_render::format_gcode_thumbnail_block(thumb, fmt);
                // Prepend thumbnail block to G-code output
                let mut new_output = block.into_bytes();
                new_output.extend_from_slice(&gcode_output);
                gcode_output = new_output;
            }
        }
    }

    // 7b. Prepend profile composition header if available.
    if let Some(ref header) = gcode_header_opt {
        let mut new_output = header.as_bytes().to_vec();
        new_output.extend_from_slice(&gcode_output);
        gcode_output = new_output;
    }

    // Step N: Write G-code.
    let write_step_num = if use_profile_workflow { 5 } else { 4 };
    let step_write = output.start_step(write_step_num, total_steps, "Write G-code");

    let out_path = if let Some(p) = output_path {
        p.to_path_buf()
    } else {
        input.with_extension("gcode")
    };

    if let Err(e) = std::fs::write(&out_path, &gcode_output) {
        output.error_msg(&format!(
            "Failed to write output '{}': {}",
            out_path.display(),
            e
        ));
        process::exit(1);
    }

    output.finish_step(&step_write, "Write G-code");

    // 9. Structured output (JSON or MessagePack to stdout).
    if json_output {
        match slicecore_engine::output::to_json(&result, &print_config) {
            Ok(json_str) => {
                if !json_no_stats {
                    if let Some(ref statistics) = result.statistics {
                        // Parse base JSON, inject statistics, re-serialize.
                        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&json_str)
                        {
                            if let Ok(stats_val) = serde_json::to_value(statistics) {
                                value["statistics"] = stats_val;
                            }
                            if let Ok(combined) = serde_json::to_string_pretty(&value) {
                                println!("{}", combined);
                            } else {
                                println!("{}", json_str);
                            }
                        } else {
                            println!("{}", json_str);
                        }
                    } else {
                        println!("{}", json_str);
                    }
                } else {
                    println!("{}", json_str);
                }
            }
            Err(e) => {
                output.error_msg(&format!("Failed to serialize JSON: {}", e));
                process::exit(1);
            }
        }
    } else if msgpack_output {
        match slicecore_engine::output::to_msgpack(&result, &print_config) {
            Ok(bytes) => {
                use std::io::Write;
                if let Err(e) = std::io::stdout().write_all(&bytes) {
                    output.error_msg(&format!("Failed to write MessagePack: {}", e));
                    process::exit(1);
                }
            }
            Err(e) => {
                output.error_msg(&format!("Failed to serialize MessagePack: {}", e));
                process::exit(1);
            }
        }
    }

    // 10. Display statistics.
    let time_precision_enum = stats_display::parse_time_precision(time_precision);
    let sort_order = stats_display::parse_sort_order(sort_stats);

    if let Some(ref statistics) = result.statistics {
        if !quiet {
            let stats_output = match stats_format {
                "csv" => stats_display::format_csv(statistics, &sort_order),
                "json" => stats_display::format_json(statistics),
                _ => {
                    stats_display::format_ascii_table(statistics, &time_precision_enum, &sort_order)
                }
            };

            // When structured output (--json/--msgpack) is active, stats go to stderr.
            if json_output || msgpack_output {
                output.info(&stats_output);
            } else {
                println!("{}", stats_output);
            }
        }

        // Save to file if requested (regardless of quiet).
        if let Some(file_path) = stats_file {
            let stats_output = match stats_format {
                "csv" => stats_display::format_csv(statistics, &sort_order),
                "json" => stats_display::format_json(statistics),
                _ => {
                    stats_display::format_ascii_table(statistics, &time_precision_enum, &sort_order)
                }
            };
            if let Err(e) = std::fs::write(file_path, &stats_output) {
                output.warn(&format!(
                    "Failed to write statistics to '{}': {}",
                    file_path.display(),
                    e
                ));
            }
        }
    } else if !quiet {
        // Fallback: basic summary when statistics is None.
        let time_minutes = result.estimated_time_seconds / 60.0;
        if json_output || msgpack_output {
            output.info("Slicing complete:");
            output.info(&format!("  Layers: {}", result.layer_count));
            output.info(&format!("  Estimated time: {:.1} min", time_minutes));
        } else {
            println!("Slicing complete:");
            println!("  Layers: {}", result.layer_count);
            println!("  Estimated time: {:.1} min", time_minutes);
        }
    }

    // Summary line with output path, layers, estimated time, filament usage.
    let filament_m = result
        .statistics
        .as_ref()
        .map_or(0.0, |s| s.summary.total_filament_m);
    let time_min = result.estimated_time_seconds / 60.0;
    // Summary line: goes to stderr via CliOutput for structured/profile workflows,
    // and to stdout for the default human-readable output mode.
    let summary_line = format!(
        "Output: {} ({} layers, {:.1}m filament, est. {:.1}min)",
        out_path.display(),
        result.layer_count,
        filament_m,
        time_min,
    );
    if json_output || msgpack_output {
        output.info(&summary_line);
    } else {
        // Print to stdout for human-readable output and to stderr via CliOutput
        // so profile workflow tests can verify it on stderr.
        if !quiet {
            println!("\n{summary_line}");
        }
        output.info(&summary_line);
    }

    // Log file creation (profile workflow only, unless --no-log).
    if use_profile_workflow && !no_log {
        let log_path = if let Some(lf) = log_file {
            lf.to_path_buf()
        } else {
            out_path.with_extension("log")
        };
        let log_content = format!(
            "SliceCore Slice Log\nInput: {}\nOutput: {}\nLayers: {}\nEstimated time: {:.1}s\n",
            input.display(),
            out_path.display(),
            result.layer_count,
            result.estimated_time_seconds,
        );
        if let Err(e) = std::fs::write(&log_path, &log_content) {
            output.warn(&format!(
                "Failed to create log file '{}': {e}",
                log_path.display()
            ));
        }
    }
}

/// Post-process an existing G-code file.
#[allow(clippy::too_many_arguments)]
fn cmd_post_process(
    input: &PathBuf,
    output_path: Option<&std::path::Path>,
    config_path: Option<&std::path::Path>,
    pause_at_layers: &[usize],
    pause_command: &str,
    timelapse: bool,
    timelapse_park_x: f64,
    timelapse_park_y: f64,
    fan_override_strs: &[String],
    inject_gcode_strs: &[String],
) {
    use slicecore_engine::config::{
        CustomGcodeRule, CustomGcodeTrigger, FanOverrideRule, TimelapseConfig,
    };
    use slicecore_gcode_io::{GcodeCommand, GcodeDialect, GcodeWriter};
    use slicecore_plugin::postprocess::run_post_processors;
    use slicecore_plugin_api::FfiPrintConfigSnapshot;

    // 1. Read input G-code file.
    let gcode_text = match std::fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: Failed to read '{}': {}", input.display(), e);
            process::exit(1);
        }
    };

    // 2. Parse G-code lines into Vec<GcodeCommand>.
    //    Wrap each line as Raw -- post-processors pattern-match layer comments
    //    and specific commands regardless of typed vs raw representation.
    let commands: Vec<GcodeCommand> = gcode_text
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let trimmed = line.trim();
            // Detect comment lines for layer tracking.
            if let Some(comment_text) = trimmed.strip_prefix(';') {
                GcodeCommand::Comment(comment_text.trim().to_string())
            } else {
                GcodeCommand::Raw(trimmed.to_string())
            }
        })
        .collect();

    // 3. Build PostProcessConfig from config file and CLI flags.
    let mut pp_config = if let Some(cfg_path) = config_path {
        match PrintConfig::from_file(cfg_path) {
            Ok(c) => c.post_process,
            Err(e) => {
                eprintln!(
                    "Error: Failed to load config '{}': {}",
                    cfg_path.display(),
                    e
                );
                process::exit(1);
            }
        }
    } else {
        PostProcessConfig::default()
    };

    // CLI flags override config file values.
    if !pause_at_layers.is_empty() {
        pp_config.pause_at_layers = pause_at_layers.to_vec();
    }
    if pause_command != "M0" || pp_config.pause_command == "M0" {
        pp_config.pause_command = pause_command.to_string();
    }
    if timelapse {
        pp_config.timelapse = TimelapseConfig {
            enabled: true,
            park_x: timelapse_park_x,
            park_y: timelapse_park_y,
            ..TimelapseConfig::default()
        };
    }

    // Parse --fan-override flags: "start:end:speed" (end = "*" means None).
    for spec in fan_override_strs {
        let parts: Vec<&str> = spec.split(':').collect();
        if parts.len() != 3 {
            eprintln!(
                "Error: Invalid fan-override format '{}'. Expected start_layer:end_layer:speed",
                spec
            );
            process::exit(1);
        }
        let start_layer = match parts[0].parse::<usize>() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("Error: Invalid start_layer in fan-override '{}'", spec);
                process::exit(1);
            }
        };
        let end_layer = if parts[1] == "*" {
            None
        } else {
            match parts[1].parse::<usize>() {
                Ok(v) => Some(v),
                Err(_) => {
                    eprintln!("Error: Invalid end_layer in fan-override '{}'", spec);
                    process::exit(1);
                }
            }
        };
        let fan_speed = match parts[2].parse::<u8>() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("Error: Invalid speed in fan-override '{}'", spec);
                process::exit(1);
            }
        };
        pp_config.fan_overrides.push(FanOverrideRule {
            start_layer,
            end_layer,
            fan_speed,
        });
    }

    // Parse --inject-gcode flags: "trigger:gcode".
    for spec in inject_gcode_strs {
        let (trigger_str, gcode) = match spec.split_once(':') {
            Some((t, g)) => (t, g.to_string()),
            None => {
                eprintln!(
                    "Error: Invalid inject-gcode format '{}'. Expected trigger:gcode",
                    spec
                );
                process::exit(1);
            }
        };
        let trigger = if trigger_str == "before_retract" {
            CustomGcodeTrigger::BeforeRetraction
        } else if trigger_str == "after_retract" {
            CustomGcodeTrigger::AfterRetraction
        } else if let Some(n_str) = trigger_str.strip_prefix("every_") {
            match n_str.parse::<usize>() {
                Ok(n) => CustomGcodeTrigger::EveryNLayers { n },
                Err(_) => {
                    eprintln!("Error: Invalid trigger '{}' in inject-gcode", trigger_str);
                    process::exit(1);
                }
            }
        } else if let Some(n_str) = trigger_str.strip_prefix("at_") {
            match n_str.parse::<usize>() {
                Ok(n) => CustomGcodeTrigger::AtLayers { layers: vec![n] },
                Err(_) => {
                    eprintln!("Error: Invalid trigger '{}' in inject-gcode", trigger_str);
                    process::exit(1);
                }
            }
        } else {
            eprintln!(
                "Error: Unknown trigger '{}'. Use every_N, at_N, before_retract, or after_retract",
                trigger_str
            );
            process::exit(1);
        };
        pp_config
            .custom_gcode
            .push(CustomGcodeRule { trigger, gcode });
    }

    pp_config.enabled = true;

    // 4. Create built-in post-processors.
    let plugins = create_builtin_postprocessors(&pp_config);
    if plugins.is_empty() {
        eprintln!("Warning: No post-processors are configured. Output will be unchanged.");
    }

    // 5. Build a default FfiPrintConfigSnapshot.
    let config_snapshot = FfiPrintConfigSnapshot {
        nozzle_diameter: 0.4,
        layer_height: 0.2,
        first_layer_height: 0.3,
        bed_x: 220.0,
        bed_y: 220.0,
        print_speed: 60.0,
        travel_speed: 120.0,
        retract_length: 0.8,
        retract_speed: 45.0,
        nozzle_temp: 200.0,
        bed_temp: 60.0,
        fan_speed: 255,
        total_layers: 100,
    };

    // 6. Run post-processors.
    let plugin_refs: Vec<&dyn slicecore_plugin::postprocess::PostProcessorPluginAdapter> =
        plugins.iter().map(|p| p.as_ref()).collect();
    let processed = match run_post_processors(commands, &plugin_refs, &config_snapshot) {
        Ok(cmds) => cmds,
        Err(e) => {
            eprintln!("Error: Post-processing failed: {}", e);
            process::exit(1);
        }
    };

    // 7. Write output.
    if let Some(out_path) = output_path {
        let file = match std::fs::File::create(out_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "Error: Failed to create output file '{}': {}",
                    out_path.display(),
                    e
                );
                process::exit(1);
            }
        };
        let mut writer = GcodeWriter::new(file, GcodeDialect::Marlin);
        if let Err(e) = writer.write_commands(&processed) {
            eprintln!("Error: Failed to write output: {}", e);
            process::exit(1);
        }
    } else {
        // Write to stdout.
        let mut buf = Vec::new();
        let mut writer = GcodeWriter::new(&mut buf, GcodeDialect::Marlin);
        if let Err(e) = writer.write_commands(&processed) {
            eprintln!("Error: Failed to format output: {}", e);
            process::exit(1);
        }
        let output_str = String::from_utf8_lossy(&buf);
        print!("{output_str}");
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
            eprintln!(
                "Error: Failed to parse mesh from '{}': {}",
                input.display(),
                e
            );
            process::exit(1);
        }
    };

    let stats = compute_stats(&mesh);

    println!("Mesh analysis of '{}':", input.display());
    println!("  Vertices: {}", stats.vertex_count);
    println!("  Triangles: {}", stats.triangle_count);
    println!(
        "  Bounding box: ({:.3}, {:.3}, {:.3}) - ({:.3}, {:.3}, {:.3})",
        stats.aabb.min.x,
        stats.aabb.min.y,
        stats.aabb.min.z,
        stats.aabb.max.x,
        stats.aabb.max.y,
        stats.aabb.max.z,
    );
    println!("  Volume: {:.3} mm^3", stats.volume);
    println!("  Surface area: {:.3} mm^2", stats.surface_area);
    println!(
        "  Manifold: {}",
        if stats.is_manifold { "yes" } else { "no" }
    );
    println!(
        "  Watertight: {}",
        if stats.is_watertight { "yes" } else { "no" }
    );
    println!(
        "  Consistent winding: {}",
        if stats.has_consistent_winding {
            "yes"
        } else {
            "no"
        }
    );
    if stats.degenerate_count > 0 {
        println!("  Degenerate triangles: {}", stats.degenerate_count);
    }
}

/// Convert OrcaSlicer/BambuStudio JSON profiles to native TOML format.
fn cmd_convert_profile(input: &[PathBuf], output_path: Option<&std::path::Path>, verbose: bool) {
    let mut results: Vec<slicecore_engine::ImportResult> = Vec::new();

    for path in input {
        // Read file contents.
        let contents = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: Failed to read '{}': {}", path.display(), e);
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
        let result = match slicecore_engine::profile_import::import_upstream_profile(&value) {
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
#[deprecated(note = "Use ProfileResolver instead for consistent profile discovery")]
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

/// List profiles from the profile library using `ProfileResolver`.
fn cmd_list_profiles(
    vendor: Option<&str>,
    profile_type: Option<&str>,
    material: Option<&str>,
    vendors_only: bool,
    profiles_dir_override: Option<&std::path::Path>,
    json_output: bool,
) {
    let resolver = ProfileResolver::new(profiles_dir_override);

    // Use resolver search with empty query to get all profiles.
    let all_profiles = resolver.search("", profile_type, usize::MAX);

    if all_profiles.is_empty() {
        // Fall back to index-based listing for vendor/material filtering.
        #[allow(deprecated)]
        let profiles_dir = match find_profiles_dir(profiles_dir_override) {
            Some(d) => d,
            None => {
                eprintln!("Error: Could not find profiles directory.");
                eprintln!(
                    "Use --profiles-dir, set SLICECORE_PROFILES_DIR, or run from the project root."
                );
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
            let mut vendors: Vec<String> =
                index.profiles.iter().map(|p| p.vendor.clone()).collect();
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

        // Filter profiles from index.
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
            println!(
                "{:<10} {:<12} {:<50} {:<10} {:<15}",
                "TYPE", "VENDOR", "NAME", "MATERIAL", "SOURCE"
            );
            println!("{}", "-".repeat(101));

            for p in &filtered {
                println!(
                    "{:<10} {:<12} {:<50} {:<10} {:<15}",
                    p.profile_type,
                    p.vendor,
                    truncate_str(&p.name, 48),
                    p.material.as_deref().unwrap_or("-"),
                    format!("library/{}", p.vendor),
                );
            }
            eprintln!("{} profile(s) found", filtered.len());
        }
        return;
    }

    // We have resolver results -- use them.
    if vendors_only {
        // Extract unique vendors from resolved profiles.
        let mut vendors: Vec<String> = all_profiles
            .iter()
            .filter_map(|p| match &p.source {
                ProfileSource::Library { vendor } => Some(vendor.clone()),
                _ => None,
            })
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

    // Apply vendor and material filters on resolved profiles.
    let filtered: Vec<_> = all_profiles
        .iter()
        .filter(|p| {
            if let Some(v) = vendor {
                let vendor_name = match &p.source {
                    ProfileSource::Library { vendor } => vendor.to_lowercase(),
                    _ => String::new(),
                };
                if !vendor_name.contains(&v.to_lowercase()) {
                    return false;
                }
            }
            if let Some(m) = material {
                if !p.name.to_lowercase().contains(&m.to_lowercase()) {
                    return false;
                }
            }
            true
        })
        .collect();

    if json_output {
        // Build JSON with source field.
        let entries: Vec<serde_json::Value> = filtered
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "profile_type": p.profile_type,
                    "source": p.source.to_string(),
                    "path": p.path.display().to_string(),
                })
            })
            .collect();
        let json = serde_json::to_string_pretty(&entries).unwrap_or_else(|e| {
            eprintln!("Error: Failed to serialize JSON: {}", e);
            process::exit(1);
        });
        println!("{}", json);
    } else {
        println!(
            "{:<10} {:<12} {:<50} {:<15}",
            "TYPE", "VENDOR", "NAME", "SOURCE"
        );
        println!("{}", "-".repeat(91));

        for p in &filtered {
            let vendor_name = match &p.source {
                ProfileSource::Library { vendor } => vendor.as_str(),
                ProfileSource::User => "-",
                ProfileSource::BuiltIn => "-",
            };
            println!(
                "{:<10} {:<12} {:<50} {:<15}",
                p.profile_type,
                vendor_name,
                truncate_str(&p.name, 48),
                p.source,
            );
        }
        eprintln!("{} profile(s) found", filtered.len());
    }
}

/// Search profiles by keyword using `ProfileResolver`.
fn cmd_search_profiles(
    query: &str,
    limit: usize,
    profiles_dir_override: Option<&std::path::Path>,
    json_output: bool,
) {
    let resolver = ProfileResolver::new(profiles_dir_override);

    // Use resolver search with the query.
    let matching = resolver.search(query, None, limit);

    if matching.is_empty() {
        // Try to provide "did you mean?" suggestions.
        // Attempt to resolve as each type to trigger NotFound suggestions.
        let mut suggestions = Vec::new();
        for profile_type in &["machine", "filament", "process"] {
            if let Err(slicecore_engine::profile_resolve::ProfileError::NotFound {
                suggestions: s,
                ..
            }) = resolver.resolve(query, profile_type)
            {
                for sug in s {
                    if !suggestions.contains(&sug) {
                        suggestions.push(sug);
                    }
                }
            }
        }

        if suggestions.is_empty() {
            eprintln!("No profiles found matching '{}'.", query);
            eprintln!("Use 'list-profiles' to see all available profiles.");
        } else {
            eprintln!("No profiles found matching '{}'.", query);
            eprintln!(
                "Did you mean: {}?",
                suggestions
                    .into_iter()
                    .take(5)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        return;
    }

    if json_output {
        let entries: Vec<serde_json::Value> = matching
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "profile_type": p.profile_type,
                    "source": p.source.to_string(),
                    "path": p.path.display().to_string(),
                })
            })
            .collect();
        let json = serde_json::to_string_pretty(&entries).unwrap_or_else(|e| {
            eprintln!("Error: Failed to serialize JSON: {}", e);
            process::exit(1);
        });
        println!("{}", json);
    } else {
        println!(
            "{:<10} {:<12} {:<50} {:<15}",
            "TYPE", "VENDOR", "NAME", "SOURCE"
        );
        println!("{}", "-".repeat(91));

        for p in &matching {
            let vendor_name = match &p.source {
                ProfileSource::Library { vendor } => vendor.as_str(),
                ProfileSource::User => "-",
                ProfileSource::BuiltIn => "-",
            };
            println!(
                "{:<10} {:<12} {:<50} {:<15}",
                p.profile_type,
                vendor_name,
                truncate_str(&p.name, 48),
                p.source,
            );
        }
        eprintln!("{} result(s) (showing up to {})", matching.len(), limit);
    }
}

/// Show details of a specific profile using `ProfileResolver`.
fn cmd_show_profile(id: &str, raw: bool, profiles_dir_override: Option<&std::path::Path>) {
    let resolver = ProfileResolver::new(profiles_dir_override);

    // Try to resolve using ProfileResolver -- accept any type.
    // First try file-path style (with /), then each type.
    let resolved = if id.contains('/') || id.ends_with(".toml") {
        resolver
            .resolve(id, "machine")
            .or_else(|_| resolver.resolve(id, "filament"))
            .or_else(|_| resolver.resolve(id, "process"))
    } else {
        // Try each type.
        resolver
            .resolve(id, "machine")
            .or_else(|_| resolver.resolve(id, "filament"))
            .or_else(|_| resolver.resolve(id, "process"))
    };

    let resolved = match resolved {
        Ok(r) => r,
        Err(e) => {
            // Fall back to index-based lookup for backward compatibility.
            #[allow(deprecated)]
            let profiles_dir = match find_profiles_dir(profiles_dir_override) {
                Some(d) => d,
                None => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };

            let index = match load_index(&profiles_dir) {
                Ok(idx) => idx,
                Err(_) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };

            // Find entry by id in the index.
            let entry = index
                .profiles
                .iter()
                .find(|entry| entry.id == id || entry.path.trim_end_matches(".toml") == id);

            if let Some(entry) = entry {
                if raw {
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
                    println!("Profile: {}", entry.name);
                    println!("Source:  library/{}", entry.vendor);
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
                return;
            }

            eprintln!("Error: {e}");
            process::exit(1);
        }
    };

    if raw {
        // Read and print the TOML file.
        let contents = match std::fs::read_to_string(&resolved.path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "Error: Failed to read profile file '{}': {}",
                    resolved.path.display(),
                    e
                );
                process::exit(1);
            }
        };
        print!("{}", contents);
    } else {
        // Print structured metadata summary.
        println!("Profile: {}", resolved.name);
        println!("Source:  {}", resolved.source);
        println!("Type:    {}", resolved.profile_type);

        // Show inheritance chain if available.
        match resolver.resolve_inheritance(&resolved.path) {
            Ok(chain) if chain.len() > 1 => {
                let names: Vec<_> = chain.iter().map(|p| p.name.as_str()).collect();
                println!("Inherits: {}", names.join(" -> "));
            }
            _ => {}
        }

        println!("Checksum: {}", resolved.checksum);
        println!("Path:    {}", resolved.path.display());
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
fn cmd_ai_suggest(input: &PathBuf, ai_config_path: Option<&std::path::Path>, format: &str) {
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
                println!("  Perimeter speed:  {:.0} mm/s", suggestion.perimeter_speed);
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

// ---------------------------------------------------------------------------
// G-code analysis commands
// ---------------------------------------------------------------------------

/// Analyze a G-code file and display structured metrics.
#[allow(clippy::too_many_arguments)]
fn cmd_analyze_gcode(
    input: &str,
    json: bool,
    csv: bool,
    no_color: bool,
    density: f64,
    diameter: f64,
    filter: Option<String>,
    summary_only: bool,
    filament_price: Option<f64>,
    printer_watts: Option<f64>,
    electricity_rate: Option<f64>,
    printer_cost: Option<f64>,
    expected_hours: Option<f64>,
    labor_rate: Option<f64>,
    setup_time: f64,
    markdown: bool,
    model: bool,
    compare_filament: &[String],
    profiles_dir: Option<&std::path::Path>,
) {
    use slicecore_engine::cost_model::{self, CostInputs};

    let has_cost_flags = filament_price.is_some()
        || printer_watts.is_some()
        || electricity_rate.is_some()
        || printer_cost.is_some()
        || expected_hours.is_some()
        || labor_rate.is_some();

    if model {
        // Treat input as a mesh file for rough volume-based estimation
        let data = match std::fs::read(input) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error: Failed to read '{}': {}", input, e);
                process::exit(1);
            }
        };
        let mesh = match slicecore_fileio::load_mesh(&data) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Error: Failed to parse mesh '{}': {}", input, e);
                process::exit(1);
            }
        };
        let stats = slicecore_mesh::compute_stats(&mesh);
        let vol_est = cost_model::volume_estimate(stats.volume, diameter, density);

        // Build cost inputs from volume estimate
        let cost_inputs = CostInputs {
            filament_weight_g: vol_est.filament_weight_g,
            print_time_seconds: vol_est.rough_time_seconds,
            filament_price_per_kg: filament_price,
            electricity_rate,
            printer_watts,
            printer_cost,
            expected_hours,
            labor_rate,
            setup_time_minutes: Some(setup_time),
        };
        let cost_est = cost_model::compute_cost(&cost_inputs);

        // Display
        let use_color = !no_color && std::io::stdout().is_terminal();
        if json {
            let combined = serde_json::json!({
                "volume_estimate": vol_est,
                "cost_estimate": cost_est,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&combined)
                    .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
            );
        } else if csv {
            analysis_display::display_volume_estimate_csv(&vol_est);
            analysis_display::display_cost_csv(&cost_est);
        } else if markdown {
            analysis_display::display_volume_estimate_markdown(&vol_est);
            analysis_display::display_cost_markdown(&cost_est);
        } else {
            analysis_display::display_volume_estimate(&vol_est, use_color);
            if has_cost_flags {
                analysis_display::display_cost_table(&cost_est, use_color);
            }
        }
        return;
    }

    // Read input: stdin or file.
    let (contents, filename) = if input == "-" {
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().lock().read_to_string(&mut buf) {
            eprintln!("Error: Failed to read from stdin: {}", e);
            process::exit(1);
        }
        (buf, "<stdin>".to_string())
    } else {
        let contents = match std::fs::read_to_string(input) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: Failed to read '{}': {}", input, e);
                process::exit(1);
            }
        };
        (contents, input.to_string())
    };

    // Parse using BufReader over the content bytes.
    let reader = BufReader::new(contents.as_bytes());
    let analysis = slicecore_engine::parse_gcode_file(reader, &filename, diameter, density);

    // Parse filter list.
    let filter_list: Option<Vec<String>> = filter.map(|f| {
        f.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });

    // Build cost estimate if any cost flags provided
    let cost_est = if has_cost_flags {
        let cost_inputs = CostInputs {
            filament_weight_g: analysis.total_filament_weight_g,
            print_time_seconds: analysis.total_time_estimate_s,
            filament_price_per_kg: filament_price,
            electricity_rate,
            printer_watts,
            printer_cost,
            expected_hours,
            labor_rate,
            setup_time_minutes: Some(setup_time),
        };
        Some(cost_model::compute_cost(&cost_inputs))
    } else {
        None
    };

    // Dispatch to output format.
    if json {
        if let Some(ref cost) = cost_est {
            let combined = serde_json::json!({
                "analysis": analysis,
                "cost_estimate": cost,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&combined)
                    .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
            );
        } else {
            analysis_display::display_analysis_json(&analysis);
        }
    } else if csv {
        analysis_display::display_analysis_csv(&analysis, summary_only);
        if let Some(ref cost) = cost_est {
            analysis_display::display_cost_csv(cost);
        }
    } else if markdown {
        let use_color = false;
        analysis_display::display_analysis_table(&analysis, use_color, summary_only, &filter_list);
        if let Some(ref cost) = cost_est {
            analysis_display::display_cost_markdown(cost);
        }
    } else {
        let use_color = !no_color && std::io::stdout().is_terminal();
        analysis_display::display_analysis_table(&analysis, use_color, summary_only, &filter_list);
        if let Some(ref cost) = cost_est {
            println!();
            analysis_display::display_cost_table(cost, use_color);
        }
    }

    // Multi-config comparison: re-estimate with different filament profiles
    if !compare_filament.is_empty() {
        let format = analysis_display::determine_output_format(json, csv, markdown);

        let resolver = ProfileResolver::new(profiles_dir);

        // Build baseline row
        let baseline_cost_inputs = cost_model::CostInputs {
            filament_weight_g: analysis.total_filament_weight_g,
            print_time_seconds: analysis.total_time_estimate_s,
            filament_price_per_kg: filament_price,
            electricity_rate,
            printer_watts,
            printer_cost,
            expected_hours,
            labor_rate,
            setup_time_minutes: Some(setup_time),
        };
        let baseline_cost = cost_model::compute_cost(&baseline_cost_inputs);
        let mut rows = vec![analysis_display::ComparisonRow {
            name: "baseline".to_string(),
            time_seconds: analysis.total_time_estimate_s,
            filament_weight_g: analysis.total_filament_weight_g,
            filament_cost: baseline_cost.filament_cost,
            total_cost: baseline_cost.total_cost,
        }];

        // Build comparison rows for each filament profile
        for fil_name in compare_filament {
            // Try to resolve filament profile and extract density/price
            let (comp_density, comp_price) = match resolver.resolve(fil_name, "filament") {
                Ok(resolved) => match std::fs::read_to_string(&resolved.path) {
                    Ok(toml_str) => {
                        let partial: PrintConfig = toml::from_str(&toml_str).unwrap_or_default();
                        let d = partial.filament.density;
                        let p = if partial.filament.cost_per_kg > 0.0 {
                            Some(partial.filament.cost_per_kg)
                        } else {
                            filament_price
                        };
                        (d, p)
                    }
                    Err(e) => {
                        eprintln!("Warning: Could not read filament profile '{fil_name}': {e}");
                        (density, filament_price)
                    }
                },
                Err(e) => {
                    eprintln!("Warning: Could not resolve filament profile '{fil_name}': {e}");
                    (density, filament_price)
                }
            };

            // Recompute weight using the comparison density
            let comp_weight = if comp_density != density && density > 0.0 {
                analysis.total_filament_weight_g * (comp_density / density)
            } else {
                analysis.total_filament_weight_g
            };

            let comp_cost_inputs = cost_model::CostInputs {
                filament_weight_g: comp_weight,
                print_time_seconds: analysis.total_time_estimate_s,
                filament_price_per_kg: comp_price,
                electricity_rate,
                printer_watts,
                printer_cost,
                expected_hours,
                labor_rate,
                setup_time_minutes: Some(setup_time),
            };
            let comp_cost = cost_model::compute_cost(&comp_cost_inputs);

            rows.push(analysis_display::ComparisonRow {
                name: fil_name.clone(),
                time_seconds: analysis.total_time_estimate_s,
                filament_weight_g: comp_weight,
                filament_cost: comp_cost.filament_cost,
                total_cost: comp_cost.total_cost,
            });
        }

        println!();
        analysis_display::display_config_comparison(&rows, format, no_color);
    }
}

/// Compare multiple G-code files (first file is baseline).
/// Convert a mesh file between formats.
fn cmd_convert(input: &std::path::Path, output: &std::path::Path) {
    let data = match std::fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error reading {}: {}", input.display(), e);
            process::exit(1);
        }
    };
    let mesh = match load_mesh(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error parsing {}: {}", input.display(), e);
            process::exit(1);
        }
    };
    if let Err(e) = save_mesh(&mesh, output) {
        eprintln!("Error writing {}: {}", output.display(), e);
        process::exit(1);
    }
    eprintln!(
        "Converted {} ({} triangles) -> {}",
        input.display(),
        mesh.triangle_count(),
        output.display()
    );
}

/// Generate thumbnail preview images from a mesh file.
fn cmd_thumbnail(
    input: &PathBuf,
    output: Option<&str>,
    angles_str: &str,
    resolution_str: &str,
    background_str: &str,
    color_str: &str,
    format_str: &str,
    quality: Option<u8>,
) {
    // Load mesh
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
            eprintln!(
                "Error: Failed to parse mesh from '{}': {}",
                input.display(),
                e
            );
            process::exit(1);
        }
    };

    // Parse resolution
    let (width, height) = parse_resolution(resolution_str);

    // Parse angles
    let angles = parse_camera_angles(angles_str);

    // Parse background color
    let background = parse_background(background_str);

    // Parse model color
    let model_color = parse_hex_color(color_str);

    // Determine image format and validate quality
    let image_format = detect_image_format(output, format_str);
    let quality = validate_quality(quality, image_format);

    // Warn if JPEG with transparent background
    if image_format == slicecore_render::ImageFormat::Jpeg && background == [0, 0, 0, 0] {
        eprintln!("Warning: JPEG does not support transparency; using white background");
    }

    // Build config and render
    let config = slicecore_render::ThumbnailConfig {
        width,
        height,
        angles: angles.clone(),
        background,
        model_color,
        output_format: image_format,
        quality,
    };
    let thumbnails = slicecore_render::render_mesh(&mesh, &config);

    // Write output
    if thumbnails.len() == 1 {
        let out_path = if let Some(out) = output {
            PathBuf::from(out)
        } else {
            input.with_extension(image_format.extension())
        };
        if let Err(e) = std::fs::write(&out_path, &thumbnails[0].encoded_data) {
            eprintln!("Error: Failed to write '{}': {}", out_path.display(), e);
            process::exit(1);
        }
        eprintln!("Thumbnail: {}", out_path.display());
    } else {
        let out_dir = if let Some(out) = output {
            PathBuf::from(out)
        } else {
            PathBuf::from(".")
        };
        if !out_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&out_dir) {
                eprintln!(
                    "Error: Failed to create directory '{}': {}",
                    out_dir.display(),
                    e
                );
                process::exit(1);
            }
        }
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        for thumb in &thumbnails {
            let angle_name = format!("{:?}", thumb.angle).to_lowercase();
            let filename = format!("{}_{}.{}", stem, angle_name, image_format.extension());
            let path = out_dir.join(&filename);
            if let Err(e) = std::fs::write(&path, &thumb.encoded_data) {
                eprintln!("Error: Failed to write '{}': {}", path.display(), e);
                process::exit(1);
            }
            eprintln!("Thumbnail: {}", path.display());
        }
    }
}

fn detect_image_format(
    output: Option<&str>,
    explicit_format: &str,
) -> slicecore_render::ImageFormat {
    // Explicit --format takes priority (unless it's the default "png")
    if explicit_format != "png" {
        return match explicit_format {
            "jpeg" | "jpg" => slicecore_render::ImageFormat::Jpeg,
            "png" => slicecore_render::ImageFormat::Png,
            other => {
                eprintln!("Error: Unknown image format '{}'. Valid: png, jpeg", other);
                process::exit(1);
            }
        };
    }
    // Auto-detect from output extension
    if let Some(out) = output {
        let path = std::path::Path::new(out);
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            return match ext.to_ascii_lowercase().as_str() {
                "jpg" | "jpeg" => slicecore_render::ImageFormat::Jpeg,
                _ => slicecore_render::ImageFormat::Png,
            };
        }
    }
    slicecore_render::ImageFormat::Png
}

fn validate_quality(quality: Option<u8>, format: slicecore_render::ImageFormat) -> Option<u8> {
    if let Some(q) = quality {
        if !(1..=100).contains(&q) {
            eprintln!("Error: --quality must be between 1 and 100, got {}", q);
            process::exit(1);
        }
        if format == slicecore_render::ImageFormat::Png {
            eprintln!("Warning: --quality is ignored for PNG format");
            return None;
        }
        return Some(q);
    }
    None
}

fn parse_resolution(s: &str) -> (u32, u32) {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        eprintln!(
            "Error: Invalid resolution '{}'. Expected WxH (e.g., 300x300)",
            s
        );
        process::exit(1);
    }
    let w: u32 = parts[0].parse().unwrap_or_else(|_| {
        eprintln!("Error: Invalid width in resolution '{}'", s);
        process::exit(1);
    });
    let h: u32 = parts[1].parse().unwrap_or_else(|_| {
        eprintln!("Error: Invalid height in resolution '{}'", s);
        process::exit(1);
    });
    (w, h)
}

fn parse_camera_angles(s: &str) -> Vec<slicecore_render::CameraAngle> {
    if s.eq_ignore_ascii_case("all") {
        return slicecore_render::CameraAngle::all();
    }
    s.split(',')
        .map(|a| match a.trim().to_lowercase().as_str() {
            "front" => slicecore_render::CameraAngle::Front,
            "back" => slicecore_render::CameraAngle::Back,
            "left" => slicecore_render::CameraAngle::Left,
            "right" => slicecore_render::CameraAngle::Right,
            "top" => slicecore_render::CameraAngle::Top,
            "isometric" | "iso" => slicecore_render::CameraAngle::Isometric,
            other => {
                eprintln!("Error: Unknown camera angle '{}'. Valid: front, back, left, right, top, isometric", other);
                process::exit(1);
            }
        })
        .collect()
}

fn parse_background(s: &str) -> [u8; 4] {
    if s.eq_ignore_ascii_case("transparent") {
        return [0, 0, 0, 0];
    }
    let rgb = parse_hex_color(s);
    [rgb[0], rgb[1], rgb[2], 255]
}

fn parse_hex_color(s: &str) -> [u8; 3] {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        eprintln!(
            "Error: Invalid hex color '{}'. Expected 6-digit hex (e.g., C8C8C8)",
            s
        );
        process::exit(1);
    }
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or_else(|_| {
        eprintln!("Error: Invalid hex color '{}'", s);
        process::exit(1);
    });
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or_else(|_| {
        eprintln!("Error: Invalid hex color '{}'", s);
        process::exit(1);
    });
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or_else(|_| {
        eprintln!("Error: Invalid hex color '{}'", s);
        process::exit(1);
    });
    [r, g, b]
}

fn cmd_compare_gcode(
    files: &[PathBuf],
    json: bool,
    csv: bool,
    no_color: bool,
    density: f64,
    diameter: f64,
) {
    if files.len() < 2 {
        eprintln!("Error: compare-gcode requires at least 2 files (first is baseline).");
        eprintln!("Usage: slicecore compare-gcode <baseline.gcode> <other.gcode> [more.gcode ...]");
        process::exit(1);
    }

    // Parse all files.
    let mut analyses: Vec<slicecore_engine::GcodeAnalysis> = Vec::new();
    for file in files {
        let contents = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: Failed to read '{}': {}", file.display(), e);
                process::exit(1);
            }
        };
        let reader = BufReader::new(contents.as_bytes());
        let filename = file
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| file.display().to_string());
        let analysis = slicecore_engine::parse_gcode_file(reader, &filename, diameter, density);
        analyses.push(analysis);
    }

    // Split into baseline and others.
    let baseline = analyses.remove(0);
    let others = analyses;

    // Compare.
    let result = slicecore_engine::compare_gcode_analyses(baseline, others);

    // Dispatch to output format.
    if json {
        analysis_display::display_comparison_json(&result);
    } else if csv {
        analysis_display::display_comparison_csv(&result);
    } else {
        let use_color = !no_color && std::io::stdout().is_terminal();
        analysis_display::display_comparison_table(&result, use_color);
    }
}

/// Arrange multiple mesh files on a build plate.
#[allow(clippy::too_many_arguments)]
fn cmd_arrange(
    inputs: &[PathBuf],
    config_path: Option<&std::path::Path>,
    bed_shape_override: Option<&str>,
    spacing: f64,
    margin: f64,
    rotation_step: f64,
    no_auto_orient: bool,
    sequential: bool,
    apply: bool,
    format: &str,
) {
    // Validate format.
    if format != "json" && format != "3mf" {
        eprintln!("Error: --format must be \"json\" or \"3mf\", got \"{format}\"");
        process::exit(1);
    }

    // Load optional config.
    let print_config = if let Some(cfg_path) = config_path {
        match PrintConfig::from_file(cfg_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "Error: Failed to load config '{}': {}",
                    cfg_path.display(),
                    e
                );
                process::exit(1);
            }
        }
    } else {
        PrintConfig::default()
    };

    // Load meshes.
    let mut meshes = Vec::new();
    let mut parts = Vec::new();
    for path in inputs {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error: Failed to read '{}': {}", path.display(), e);
                process::exit(1);
            }
        };
        let mesh = match load_mesh(&data) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Error: Failed to parse mesh '{}': {}", path.display(), e);
                process::exit(1);
            }
        };

        let id = path
            .file_stem()
            .map_or_else(|| "input".into(), |s| s.to_string_lossy().into_owned());
        let vertices = mesh.vertices().to_vec();
        let z_min = vertices.iter().map(|v| v.z).fold(f64::INFINITY, f64::min);
        let z_max = vertices
            .iter()
            .map(|v| v.z)
            .fold(f64::NEG_INFINITY, f64::max);
        let mesh_height = z_max - z_min;

        parts.push(slicecore_arrange::ArrangePart {
            id,
            vertices,
            mesh_height,
            ..Default::default()
        });
        meshes.push(mesh);
    }

    // Build ArrangeConfig from CLI flags + optional PrintConfig.
    let arrange_config = slicecore_arrange::ArrangeConfig {
        part_spacing: spacing,
        bed_margin: margin,
        rotation_step,
        auto_orient: !no_auto_orient,
        sequential_mode: sequential,
        brim_width: print_config.brim_width,
        skirt_distance: print_config.skirt_distance,
        skirt_loops: print_config.skirt_loops,
        nozzle_diameter: print_config.machine.nozzle_diameter(),
        ..Default::default()
    };

    // Determine bed shape.
    let bed_shape_str = bed_shape_override
        .map(String::from)
        .unwrap_or_else(|| print_config.machine.bed_shape.clone());
    let bed_x = print_config.machine.bed_x;
    let bed_y = print_config.machine.bed_y;

    // Run arrangement.
    let result =
        match slicecore_arrange::arrange(&parts, &arrange_config, &bed_shape_str, bed_x, bed_y) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: Arrangement failed: {e}");
                process::exit(1);
            }
        };

    // Warn about unplaced parts.
    if !result.unplaced_parts.is_empty() {
        eprintln!(
            "Warning: {} part(s) could not be placed: {}",
            result.unplaced_parts.len(),
            result.unplaced_parts.join(", ")
        );
    }

    // Output handling.
    match format {
        "3mf" => {
            // Write a single 3MF file with all parts at their arranged positions.
            let first_stem = inputs
                .first()
                .and_then(|p| p.file_stem())
                .map_or_else(|| "arranged".into(), |s| s.to_string_lossy().into_owned());
            let out_path = PathBuf::from(format!("{first_stem}_arranged.3mf"));

            // Apply transforms and save the first plate's parts as a combined mesh.
            // For 3MF, we write each mesh individually with transforms applied.
            if let Some(plate) = result.plates.first() {
                // Build a combined mesh from all placed parts with transforms applied.
                let mut all_vertices = Vec::new();
                let mut all_indices = Vec::new();

                for placement in &plate.placements {
                    // Find the corresponding loaded mesh by matching part_id to input filenames.
                    let mesh_idx = inputs
                        .iter()
                        .position(|p| {
                            p.file_stem()
                                .is_some_and(|s| s.to_string_lossy() == placement.part_id)
                        })
                        .unwrap_or(0);

                    let mesh = &meshes[mesh_idx];
                    let offset = all_vertices.len();

                    // Apply translation to vertices.
                    let (tx, ty) = placement.position;
                    for v in mesh.vertices() {
                        all_vertices.push(slicecore_math::Point3::new(v.x + tx, v.y + ty, v.z));
                    }
                    for idx in mesh.indices() {
                        all_indices.push([
                            idx[0] + offset as u32,
                            idx[1] + offset as u32,
                            idx[2] + offset as u32,
                        ]);
                    }
                }

                let combined = match slicecore_mesh::TriangleMesh::new(all_vertices, all_indices) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("Error: Failed to build combined mesh: {e}");
                        process::exit(1);
                    }
                };
                if let Err(e) = save_mesh(&combined, &out_path) {
                    eprintln!("Error: Failed to write 3MF '{}': {}", out_path.display(), e);
                    process::exit(1);
                }
                eprintln!("Wrote positioned 3MF: {}", out_path.display());
            }

            // Also print JSON plan to stdout.
            let json = serde_json::to_string_pretty(&result).unwrap_or_default();
            println!("{json}");
        }
        _ => {
            // JSON format (default).
            if apply {
                // Apply transforms and write output files.
                for plate in &result.plates {
                    for placement in &plate.placements {
                        let mesh_idx = inputs
                            .iter()
                            .position(|p| {
                                p.file_stem()
                                    .is_some_and(|s| s.to_string_lossy() == placement.part_id)
                            })
                            .unwrap_or(0);

                        let mesh = &meshes[mesh_idx];
                        let (tx, ty) = placement.position;

                        // Apply translation to create a new transformed mesh.
                        let transformed_vertices: Vec<slicecore_math::Point3> = mesh
                            .vertices()
                            .iter()
                            .map(|v| slicecore_math::Point3::new(v.x + tx, v.y + ty, v.z))
                            .collect();
                        let transformed = match slicecore_mesh::TriangleMesh::new(
                            transformed_vertices,
                            mesh.indices().to_vec(),
                        ) {
                            Ok(m) => m,
                            Err(e) => {
                                eprintln!("Error: Failed to build transformed mesh: {e}");
                                process::exit(1);
                            }
                        };

                        // Output as {original_stem}_arranged.stl.
                        let stem = inputs[mesh_idx]
                            .file_stem()
                            .map_or_else(|| "output".into(), |s| s.to_string_lossy().into_owned());
                        let ext = inputs[mesh_idx].extension().map_or("stl", |e| {
                            if e == "3mf" || e == "obj" {
                                e.to_str().unwrap_or("stl")
                            } else {
                                "stl"
                            }
                        });
                        let out_path = PathBuf::from(format!("{stem}_arranged.{ext}"));
                        if let Err(e) = save_mesh(&transformed, &out_path) {
                            eprintln!("Error: Failed to write '{}': {}", out_path.display(), e);
                            process::exit(1);
                        }
                        eprintln!("Wrote: {}", out_path.display());
                    }
                }
            }

            // Print JSON plan to stdout.
            let json = serde_json::to_string_pretty(&result).unwrap_or_default();
            println!("{json}");
        }
    }
}
