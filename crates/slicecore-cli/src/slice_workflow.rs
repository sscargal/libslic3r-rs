//! Slice workflow orchestrator for profile-based slicing.
//!
//! Implements the resolve -> compose -> validate -> slice pipeline when
//! the user specifies `-m`/`-f`/`-p` profile flags instead of `--config`.

use std::path::{Path, PathBuf};

use slicecore_engine::config::PrintConfig;
use slicecore_engine::config_validate::{
    resolve_template_variables, validate_config, ValidationIssue, ValidationSeverity,
};
use slicecore_engine::profile_compose::{
    ComposedConfig, ProfileComposer, SourceType, validate_set_key,
};
use slicecore_engine::profile_resolve::ProfileResolver;
use slicecore_engine::get_builtin_profile;

/// All options from the CLI flags that control the slice workflow.
#[allow(dead_code)]
pub struct SliceWorkflowOptions {
    /// Machine profile name or file path (-m/--machine).
    pub machine: Option<String>,
    /// Filament profile name or file path (-f/--filament).
    pub filament: Option<String>,
    /// Process profile name or file path (-p/--process).
    pub process: Option<String>,
    /// Path to a TOML/JSON override file (--overrides).
    pub overrides_file: Option<PathBuf>,
    /// Repeatable key=value overrides (--set).
    pub set_overrides: Vec<String>,
    /// Resolve + merge + validate only, skip slicing (--dry-run).
    pub dry_run: bool,
    /// Save merged config to file (--save-config).
    pub save_config: Option<PathBuf>,
    /// Print merged config with provenance annotations (--show-config).
    pub show_config: bool,
    /// Allow slicing without profiles (--unsafe-defaults).
    pub unsafe_defaults: bool,
    /// Override safety validation errors (--force).
    pub force: bool,
    /// Suppress log file creation (--no-log).
    pub no_log: bool,
    /// Custom log file path (--log-file).
    pub log_file: Option<PathBuf>,
    /// Profile library directory override (--profiles-dir).
    pub profiles_dir: Option<PathBuf>,
    /// Input file path (for provenance comments).
    pub input_path: PathBuf,
    /// Whether JSON output was requested.
    pub json_output: bool,
}

/// Result of the slice workflow before actual slicing begins.
#[allow(dead_code)]
pub struct WorkflowResult {
    /// The composed configuration ready for slicing.
    pub composed: ComposedConfig,
    /// Log lines accumulated during the workflow.
    pub log_lines: Vec<String>,
}

/// Runs the full profile-based slice workflow: resolve -> compose -> validate.
///
/// Returns the composed config ready for the engine, or an exit code on failure.
///
/// # Errors
///
/// Returns `Err(exit_code)` where:
/// - 2 = profile resolution or composition error
/// - 4 = safety validation error (and `--force` not set)
#[allow(clippy::too_many_lines)]
pub fn run_slice_workflow(options: &SliceWorkflowOptions) -> Result<WorkflowResult, i32> {
    let mut log_lines: Vec<String> = Vec::new();

    // 1. Create resolver
    let resolver = ProfileResolver::new(options.profiles_dir.as_deref());

    // 2. Check unsafe-defaults mode
    if options.unsafe_defaults
        && options.machine.is_none()
        && options.filament.is_none()
        && options.process.is_none()
    {
        eprintln!("Warning: --unsafe-defaults active, using PrintConfig::default() with no profiles");
        log_lines.push("Using default config (--unsafe-defaults)".to_string());

        let mut composer = ProfileComposer::new();
        apply_set_overrides(&mut composer, &options.set_overrides)?;
        apply_overrides_file(&mut composer, &options.overrides_file)?;

        let composed = match composer.compose() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: Failed to compose config: {e}");
                return Err(2);
            }
        };

        let composed = resolve_gcode_templates(composed);
        let composed = run_validation(composed, options)?;

        return handle_workflow_outputs(options, composed, &log_lines);
    }

    let mut composer = ProfileComposer::new();

    // 3. Resolve and add machine profile
    if let Some(ref machine_query) = options.machine {
        match resolve_and_add_profile(
            &resolver,
            machine_query,
            "machine",
            SourceType::Machine,
            &mut composer,
            &mut log_lines,
        ) {
            Ok(()) => {}
            Err(code) => return Err(code),
        }
    }

    // 4. Resolve and add filament profile
    if let Some(ref filament_query) = options.filament {
        match resolve_and_add_profile(
            &resolver,
            filament_query,
            "filament",
            SourceType::Filament,
            &mut composer,
            &mut log_lines,
        ) {
            Ok(()) => {}
            Err(code) => return Err(code),
        }
    }

    // 5. Resolve and add process profile (default to "standard" built-in)
    if let Some(ref process_query) = options.process {
        match resolve_and_add_profile(
            &resolver,
            process_query,
            "process",
            SourceType::Process,
            &mut composer,
            &mut log_lines,
        ) {
            Ok(()) => {}
            Err(code) => return Err(code),
        }
    } else {
        // Use built-in standard process profile
        if let Some(builtin) = get_builtin_profile("standard") {
            if let Err(e) =
                composer.add_toml_layer(SourceType::Process, "(built-in:standard)", builtin.toml_content)
            {
                eprintln!("Error: Failed to load built-in standard process profile: {e}");
                return Err(2);
            }
            log_lines.push("Using built-in 'standard' process profile".to_string());
            eprintln!("Note: Using built-in 'Standard Quality' process profile (no -p specified)");
        }
    }

    // 6. Apply overrides file
    apply_overrides_file(&mut composer, &options.overrides_file)?;

    // 7. Apply --set overrides
    apply_set_overrides(&mut composer, &options.set_overrides)?;

    // 8. Compose
    let composed = match composer.compose() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Failed to compose config: {e}");
            return Err(2);
        }
    };

    // Print composition warnings
    for warning in &composed.warnings {
        eprintln!("Warning: {warning}");
    }

    // 9. Resolve template variables in start/end gcode
    let composed = resolve_gcode_templates(composed);

    // 10. Validate
    let composed = run_validation(composed, options)?;

    // 11. Handle workflow outputs (dry-run, show-config, save-config)
    handle_workflow_outputs(options, composed, &log_lines)
}

/// Resolves a profile query and adds it (with inheritance) to the composer.
fn resolve_and_add_profile(
    resolver: &ProfileResolver,
    query: &str,
    expected_type: &str,
    source_type: SourceType,
    composer: &mut ProfileComposer,
    log_lines: &mut Vec<String>,
) -> Result<(), i32> {
    // Check built-in profiles first
    if let Some(builtin) = get_builtin_profile(query) {
        if builtin.profile_type == expected_type {
            if let Err(e) = composer.add_toml_layer(
                source_type,
                &format!("(built-in:{query})"),
                builtin.toml_content,
            ) {
                eprintln!("Error: Failed to load built-in profile '{query}': {e}");
                return Err(2);
            }
            log_lines.push(format!(
                "Resolved {expected_type} '{query}' -> built-in '{}'",
                builtin.display_name
            ));
            eprintln!(
                "Profile: {expected_type} = {} (built-in)",
                builtin.display_name
            );
            return Ok(());
        }
    }

    // Try resolver
    let resolved = match resolver.resolve(query, expected_type) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return Err(2);
        }
    };

    // Handle inheritance chain
    let chain = match resolver.resolve_inheritance(&resolved.path) {
        Ok(c) if !c.is_empty() => c,
        _ => vec![resolved.clone()],
    };

    // Add each profile in the chain (ancestors first)
    for profile in &chain {
        let content = match std::fs::read_to_string(&profile.path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "Error: Failed to read profile '{}': {e}",
                    profile.path.display()
                );
                return Err(2);
            }
        };
        if let Err(e) = composer.add_toml_layer(
            source_type.clone(),
            &profile.path.to_string_lossy(),
            &content,
        ) {
            eprintln!(
                "Error: Failed to parse profile '{}': {e}",
                profile.path.display()
            );
            return Err(2);
        }
    }

    log_lines.push(format!(
        "Resolved {expected_type} '{query}' -> {} ({})",
        resolved.path.display(),
        resolved.source
    ));
    eprintln!(
        "Profile: {expected_type} = {} ({})",
        resolved.name, resolved.source
    );

    Ok(())
}

/// Applies a TOML/JSON override file to the composer.
fn apply_overrides_file(
    composer: &mut ProfileComposer,
    overrides_file: &Option<PathBuf>,
) -> Result<(), i32> {
    if let Some(ref path) = overrides_file {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "Error: Failed to read overrides file '{}': {e}",
                    path.display()
                );
                return Err(2);
            }
        };

        // Try TOML first, then JSON
        if composer
            .add_toml_layer(
                SourceType::UserOverride,
                &path.to_string_lossy(),
                &content,
            )
            .is_err()
        {
            // Try JSON: parse as PrintConfig then serialize back to TOML
            match serde_json::from_str::<PrintConfig>(&content) {
                Ok(config) => {
                    let toml_str = toml::to_string_pretty(&config).map_err(|e| {
                        eprintln!("Error: Failed to convert JSON overrides to TOML: {e}");
                        2
                    })?;
                    if let Err(e) = composer.add_toml_layer(
                        SourceType::UserOverride,
                        &path.to_string_lossy(),
                        &toml_str,
                    ) {
                        eprintln!("Error: Failed to parse overrides file '{}': {e}", path.display());
                        return Err(2);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Error: Overrides file '{}' is neither valid TOML nor JSON: {e}",
                        path.display()
                    );
                    return Err(2);
                }
            }
        }
    }
    Ok(())
}

/// Parses and applies `--set key=value` overrides to the composer.
fn apply_set_overrides(
    composer: &mut ProfileComposer,
    set_overrides: &[String],
) -> Result<(), i32> {
    for entry in set_overrides {
        let (key, value) = match entry.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => {
                eprintln!("Error: Invalid --set format '{entry}', expected key=value");
                return Err(2);
            }
        };

        if let Err(e) = validate_set_key(key) {
            eprintln!("Error: {e}");
            return Err(2);
        }

        if let Err(e) = composer.add_set_override(key, value) {
            eprintln!("Error: Failed to add override '{key}={value}': {e}");
            return Err(2);
        }
    }
    Ok(())
}

/// Resolves template variables in start_gcode and end_gcode fields.
fn resolve_gcode_templates(mut composed: ComposedConfig) -> ComposedConfig {
    let start = composed.config.machine.start_gcode.clone();
    let end = composed.config.machine.end_gcode.clone();
    composed.config.machine.start_gcode = resolve_template_variables(&start, &composed.config);
    composed.config.machine.end_gcode = resolve_template_variables(&end, &composed.config);
    composed
}

/// Runs validation and handles --force.
fn run_validation(
    composed: ComposedConfig,
    options: &SliceWorkflowOptions,
) -> Result<ComposedConfig, i32> {
    let issues = validate_config(&composed.config);

    let errors: Vec<&ValidationIssue> = issues
        .iter()
        .filter(|i| i.severity == ValidationSeverity::Error)
        .collect();
    let warnings: Vec<&ValidationIssue> = issues
        .iter()
        .filter(|i| i.severity == ValidationSeverity::Warning)
        .collect();

    for w in &warnings {
        eprintln!("Validation warning: {} - {}", w.field, w.message);
    }

    if !errors.is_empty() {
        for e in &errors {
            if options.force {
                eprintln!(
                    "Validation error (overridden by --force): {} - {}",
                    e.field, e.message
                );
            } else {
                eprintln!("Validation error: {} - {}", e.field, e.message);
            }
        }
        if !options.force {
            eprintln!(
                "Error: {} validation error(s). Use --force to override.",
                errors.len()
            );
            return Err(4);
        }
    }

    Ok(composed)
}

/// Handles --dry-run, --show-config, --save-config outputs.
fn handle_workflow_outputs(
    options: &SliceWorkflowOptions,
    composed: ComposedConfig,
    log_lines: &[String],
) -> Result<WorkflowResult, i32> {
    // --dry-run: print summary and exit
    if options.dry_run {
        eprintln!("\n--- Dry Run Summary ---");
        for line in log_lines {
            eprintln!("  {line}");
        }
        eprintln!("\nProfile checksums:");
        for (path, checksum) in &composed.profile_checksums {
            eprintln!("  {path}: {}", &checksum[..16.min(checksum.len())]);
        }
        if !composed.warnings.is_empty() {
            eprintln!("\nWarnings:");
            for w in &composed.warnings {
                eprintln!("  {w}");
            }
        }
        eprintln!("\nConfig validated successfully. Slicing would proceed.");
        std::process::exit(0);
    }

    // --show-config: print annotated config
    if options.show_config {
        print_annotated_config(&composed, options.json_output);
    }

    // --save-config: write merged TOML
    if let Some(ref save_path) = options.save_config {
        save_merged_config(&composed, save_path, options);
    }

    Ok(WorkflowResult {
        composed,
        log_lines: log_lines.to_vec(),
    })
}

/// Prints annotated config with provenance comments.
fn print_annotated_config(composed: &ComposedConfig, json_output: bool) {
    if json_output {
        match serde_json::to_string_pretty(&composed.config) {
            Ok(json) => println!("{json}"),
            Err(e) => eprintln!("Error: Failed to serialize config as JSON: {e}"),
        }
    } else {
        match toml::to_string_pretty(&composed.config) {
            Ok(toml_str) => {
                println!("; Merged configuration with provenance");
                println!(";");
                for line in toml_str.lines() {
                    if let Some((key, _)) = line.split_once('=') {
                        let key = key.trim();
                        if let Some(source) = composed.provenance.get(key) {
                            let source_desc = match &source.file_path {
                                Some(path) => format!("{} ({})", source.source_type, path),
                                None => source.source_type.to_string(),
                            };
                            println!("{line}  ; Source: {source_desc}");
                        } else {
                            println!("{line}");
                        }
                    } else {
                        println!("{line}");
                    }
                }
            }
            Err(e) => eprintln!("Error: Failed to serialize config as TOML: {e}"),
        }
    }
}

/// Saves merged config as TOML with provenance header comments.
fn save_merged_config(
    composed: &ComposedConfig,
    save_path: &Path,
    options: &SliceWorkflowOptions,
) {
    match toml::to_string_pretty(&composed.config) {
        Ok(toml_str) => {
            let mut output = String::new();

            // Reproduce command
            output.push_str("# Generated by SliceCore\n");
            output.push_str("# Reproduce: slicecore slice ");
            output.push_str(&options.input_path.to_string_lossy());
            if let Some(ref m) = options.machine {
                output.push_str(&format!(" -m {m}"));
            }
            if let Some(ref f) = options.filament {
                output.push_str(&format!(" -f {f}"));
            }
            if let Some(ref p) = options.process {
                output.push_str(&format!(" -p {p}"));
            }
            for s in &options.set_overrides {
                output.push_str(&format!(" --set {s}"));
            }
            output.push('\n');

            // Profile checksums
            output.push_str("#\n");
            for (path, checksum) in &composed.profile_checksums {
                output.push_str(&format!(
                    "# Profile: {path} (sha256:{})\n",
                    &checksum[..16.min(checksum.len())]
                ));
            }
            output.push_str("#\n\n");

            output.push_str(&toml_str);

            if let Err(e) = std::fs::write(save_path, &output) {
                eprintln!(
                    "Warning: Failed to save config to '{}': {e}",
                    save_path.display()
                );
            } else {
                eprintln!("Saved merged config to '{}'", save_path.display());
            }
        }
        Err(e) => eprintln!("Error: Failed to serialize config for saving: {e}"),
    }
}

/// Generates G-code header comment block with version, timestamp, profiles, and config.
pub fn generate_gcode_header(
    composed: &ComposedConfig,
    options: &SliceWorkflowOptions,
) -> String {
    let mut header = String::new();

    // Version line
    let version = env!("CARGO_PKG_VERSION");
    header.push_str(&format!("; Generated by SliceCore v{version}\n"));

    // Timestamp (manual UTC formatting, no chrono dependency)
    let now = std::time::SystemTime::now();
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Convert epoch to approximate ISO 8601 UTC (good enough without chrono)
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    // Approximate year/month/day from days since 1970-01-01
    let (year, month, day) = epoch_days_to_date(days_since_epoch);
    header.push_str(&format!(
        "; Timestamp: {year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z\n"
    ));

    // Reproduce command
    header.push_str("; Reproduce: slicecore slice ");
    header.push_str(&options.input_path.to_string_lossy());
    if let Some(ref m) = options.machine {
        header.push_str(&format!(" -m {m}"));
    }
    if let Some(ref f) = options.filament {
        header.push_str(&format!(" -f {f}"));
    }
    if let Some(ref p) = options.process {
        header.push_str(&format!(" -p {p}"));
    }
    for s in &options.set_overrides {
        header.push_str(&format!(" --set {s}"));
    }
    header.push('\n');
    header.push_str(";\n");

    // Profile checksums
    for (path, checksum) in &composed.profile_checksums {
        header.push_str(&format!(
            "; Profile: {path} (sha256:{})\n",
            &checksum[..16.min(checksum.len())]
        ));
    }
    header.push_str(";\n");

    // Full merged config as TOML comments
    if let Ok(toml_str) = toml::to_string_pretty(&composed.config) {
        for line in toml_str.lines() {
            header.push_str(&format!("; {line}\n"));
        }
    }
    header.push_str(";\n");

    header
}

/// Converts days since Unix epoch (1970-01-01) to (year, month, day).
///
/// Uses a simple iterative approach -- accurate for dates from 1970 through 2100+.
fn epoch_days_to_date(days: u64) -> (u64, u64, u64) {
    let mut remaining = days;
    let mut year = 1970_u64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let days_in_months: [u64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1_u64;
    for &dim in &days_in_months {
        if remaining < dim {
            break;
        }
        remaining -= dim;
        month += 1;
    }

    (year, month, remaining + 1)
}

/// Returns true if the given year is a leap year.
const fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
