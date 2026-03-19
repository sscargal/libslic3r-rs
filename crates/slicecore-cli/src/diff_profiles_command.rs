//! CLI subcommand for comparing two print profiles side by side.
//!
//! Resolves profile names or file paths to [`PrintConfig`] instances, runs
//! the diff engine, and displays results as a category-grouped table or JSON.

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Args;
use comfy_table::{ContentArrangement, Table};
use slicecore_config_schema::types::{SettingCategory, Tier};
use slicecore_engine::config::PrintConfig;
use slicecore_engine::profile_diff::{diff_configs, format_value, DiffEntry, DiffResult};
use slicecore_engine::profile_resolve::ProfileResolver;

// ---------------------------------------------------------------------------
// Tier filter (local copy -- schema_command's is private)
// ---------------------------------------------------------------------------

/// Progressive-disclosure tier filter for `--tier`.
#[derive(Clone, clap::ValueEnum)]
pub enum TierFilter {
    /// Beginner-level settings.
    Simple,
    /// Intermediate-level settings.
    Intermediate,
    /// Advanced-level settings.
    Advanced,
    /// Developer/debug settings.
    Developer,
}

impl TierFilter {
    fn to_tier(&self) -> Tier {
        match self {
            Self::Simple => Tier::Simple,
            Self::Intermediate => Tier::Intermediate,
            Self::Advanced => Tier::Advanced,
            Self::Developer => Tier::Developer,
        }
    }

    /// Returns `true` if `entry_tier` is at or below the selected filter level.
    fn includes(&self, entry_tier: &Tier) -> bool {
        (*entry_tier as u8) <= (self.to_tier() as u8)
    }
}

// ---------------------------------------------------------------------------
// Category parsing (local copy -- schema_command's is private)
// ---------------------------------------------------------------------------

/// Parses a category string into a [`SettingCategory`].
fn parse_category(s: &str) -> Option<SettingCategory> {
    match s.to_lowercase().replace('-', "_").as_str() {
        "quality" => Some(SettingCategory::Quality),
        "speed" => Some(SettingCategory::Speed),
        "line_width" => Some(SettingCategory::LineWidth),
        "cooling" => Some(SettingCategory::Cooling),
        "retraction" => Some(SettingCategory::Retraction),
        "support" => Some(SettingCategory::Support),
        "infill" => Some(SettingCategory::Infill),
        "adhesion" => Some(SettingCategory::Adhesion),
        "advanced" => Some(SettingCategory::Advanced),
        "machine" => Some(SettingCategory::Machine),
        "filament" => Some(SettingCategory::Filament),
        "acceleration" => Some(SettingCategory::Acceleration),
        "post_process" => Some(SettingCategory::PostProcess),
        "timelapse" => Some(SettingCategory::Timelapse),
        "multi_material" => Some(SettingCategory::MultiMaterial),
        "calibration" => Some(SettingCategory::Calibration),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Clap arguments
// ---------------------------------------------------------------------------

/// Compare two print profiles side by side.
#[derive(Args)]
pub struct DiffProfilesArgs {
    /// First profile (name like BBL/PLA_Basic or file path like config.toml).
    pub left: String,

    /// Second profile (name or file path; omit when using --defaults).
    pub right: Option<String>,

    /// Compare against built-in defaults instead of a second profile.
    #[arg(long)]
    pub defaults: bool,

    /// Show all settings, not just differences.
    #[arg(long)]
    pub all: bool,

    /// Show impact hints (affects list and setting descriptions).
    #[arg(short, long)]
    pub verbose: bool,

    /// Filter by category (repeatable: --category speed --category cooling).
    #[arg(long)]
    pub category: Vec<String>,

    /// Filter by tier level.
    #[arg(long)]
    pub tier: Option<TierFilter>,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,

    /// Profile library directory override.
    #[arg(long)]
    pub profiles_dir: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Profile loading helpers
// ---------------------------------------------------------------------------

/// Loads a [`PrintConfig`] from a file path, detecting format by extension.
fn load_config_from_file(path: &str) -> Result<PrintConfig, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    if path.ends_with(".json") {
        Ok(PrintConfig::from_json(&contents)?)
    } else {
        // Default to TOML (.toml or unknown extension)
        Ok(PrintConfig::from_toml(&contents)?)
    }
}

/// Resolves a profile query string to a `(PrintConfig, display_name)` pair.
///
/// If the query looks like a file path (exists on disk), loads it directly.
/// Otherwise, uses [`ProfileResolver`] to search the library.
fn resolve_profile(
    query: &str,
    profiles_dir: Option<&std::path::Path>,
) -> Result<(PrintConfig, String), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(query);
    if path.exists() && path.is_file() {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(query)
            .to_owned();
        let config = load_config_from_file(query)?;
        Ok((config, name))
    } else {
        let resolver = ProfileResolver::new(profiles_dir);
        let resolved = resolver.resolve(query, "process")?;
        let config = PrintConfig::from_toml_file(&resolved.path)?;
        Ok((config, resolved.name))
    }
}

// ---------------------------------------------------------------------------
// ANSI helpers
// ---------------------------------------------------------------------------

fn bold(s: &str, use_color: bool) -> String {
    if use_color {
        format!("\x1b[1m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn dim(s: &str, use_color: bool) -> String {
    if use_color {
        format!("\x1b[90m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn green(s: &str, use_color: bool) -> String {
    if use_color {
        format!("\x1b[32m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn red(s: &str, use_color: bool) -> String {
    if use_color {
        format!("\x1b[31m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Runs the `diff-profiles` subcommand.
///
/// Returns `Ok(true)` when differences are found, `Ok(false)` when the
/// profiles are identical.
///
/// # Errors
///
/// Returns an error on invalid arguments, unresolvable profiles, or I/O
/// failures.
pub fn run_diff_profiles_command(
    args: &DiffProfilesArgs,
    color: &str,
    quiet: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    // --- Argument validation ---
    if !args.defaults && args.right.is_none() {
        return Err("Provide two profiles or use --defaults".into());
    }

    // --- Resolve left profile ---
    let (left_config, left_name) = resolve_profile(&args.left, args.profiles_dir.as_deref())?;

    // --- Resolve right profile ---
    let (right_config, right_name) = if args.defaults {
        (PrintConfig::default(), "defaults".to_owned())
    } else {
        // Safe: validated above that right is Some when defaults is false.
        let right_query = args.right.as_deref().expect("right validated present");
        resolve_profile(right_query, args.profiles_dir.as_deref())?
    };

    // --- Compute diff ---
    let result = diff_configs(&left_config, &right_config, &left_name, &right_name);

    // --- Filter entries ---
    let mut filtered: Vec<&DiffEntry> = if args.all {
        result.entries.iter().collect()
    } else {
        result.entries.iter().filter(|e| e.changed).collect()
    };

    // Apply category filter
    let category_filters: Vec<SettingCategory> = args
        .category
        .iter()
        .filter_map(|c| parse_category(c))
        .collect();
    if !category_filters.is_empty() {
        filtered.retain(|e| {
            e.category
                .as_ref()
                .is_some_and(|cat| category_filters.contains(cat))
        });
    }

    // Apply tier filter
    if let Some(ref tier_filter) = args.tier {
        filtered.retain(|e| e.tier.as_ref().is_some_and(|t| tier_filter.includes(t)));
    }

    // --- Quiet mode: exit code only ---
    if quiet {
        return Ok(result.total_differences > 0);
    }

    // Determine color mode
    let use_color = match color {
        "always" => true,
        "never" => false,
        _ => std::io::stdout().is_terminal(),
    };

    // --- JSON output ---
    if args.json {
        display_json(&result, &filtered)?;
        return Ok(result.total_differences > 0);
    }

    // --- Table output ---
    display_table(&result, &filtered, args.verbose, use_color);
    Ok(result.total_differences > 0)
}

// ---------------------------------------------------------------------------
// JSON display
// ---------------------------------------------------------------------------

/// JSON output structure for the diff result.
#[derive(serde::Serialize)]
struct JsonOutput<'a> {
    left_name: &'a str,
    right_name: &'a str,
    total_differences: usize,
    category_counts: &'a BTreeMap<String, usize>,
    entries: &'a [&'a DiffEntry],
}

fn display_json(
    result: &DiffResult,
    entries: &[&DiffEntry],
) -> Result<(), Box<dyn std::error::Error>> {
    let output = JsonOutput {
        left_name: &result.left_name,
        right_name: &result.right_name,
        total_differences: result.total_differences,
        category_counts: &result.category_counts,
        entries,
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Table display
// ---------------------------------------------------------------------------

/// Formats a category name for display (title case, underscores to spaces).
fn display_category(cat: Option<&SettingCategory>) -> String {
    match cat {
        Some(c) => {
            let raw = c.as_str();
            raw.split('_')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(first) => {
                            let upper: String = first.to_uppercase().collect();
                            format!("{upper}{}", chars.as_str())
                        }
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
        None => "Uncategorized".to_owned(),
    }
}

fn display_table(result: &DiffResult, entries: &[&DiffEntry], verbose: bool, use_color: bool) {
    // --- Summary header ---
    println!(
        "{}",
        bold(
            &format!("Comparing: {} vs {}", result.left_name, result.right_name),
            use_color,
        )
    );

    if result.total_differences == 0 {
        println!("{}", green("Profiles are identical.", use_color));
        return;
    }

    // Per-category breakdown
    let cat_summary: String = result
        .category_counts
        .iter()
        .map(|(cat, count)| format!("{cat}: {count}"))
        .collect::<Vec<_>>()
        .join("  |  ");

    println!(
        "{} differences across {} categories:",
        result.total_differences,
        result.category_counts.len(),
    );
    println!("  {cat_summary}");
    println!();

    // --- Group entries by category ---
    let mut groups: BTreeMap<String, Vec<&DiffEntry>> = BTreeMap::new();
    for entry in entries {
        let cat_name = display_category(entry.category.as_ref());
        groups.entry(cat_name).or_default().push(entry);
    }

    for (cat_name, group_entries) in &groups {
        let header = format!("  {}", cat_name);
        let dashes = "-".repeat(header.len());
        println!("{}", bold(&header, use_color));
        println!("  {dashes}");

        let mut table = Table::new();
        table.set_content_arrangement(ContentArrangement::Dynamic);

        if verbose {
            table.set_header(vec![
                "Setting",
                &result.left_name,
                &result.right_name,
                "Affects",
            ]);
        } else {
            table.set_header(vec!["Setting", &result.left_name, &result.right_name]);
        }

        for entry in group_entries {
            let setting_label = format!("{} ({})", entry.display_name, entry.key);
            let left_str = format_value(&entry.left_value, &entry.units);
            let right_str = format_value(&entry.right_value, &entry.units);

            let (left_display, right_display) = if entry.changed && use_color {
                (green(&left_str, true), red(&right_str, true))
            } else {
                (left_str, right_str)
            };

            if verbose {
                let affects = entry
                    .affects
                    .iter()
                    .map(|k| k.0.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                table.add_row(vec![setting_label, left_display, right_display, affects]);
            } else {
                table.add_row(vec![setting_label, left_display, right_display]);
            }

            // In verbose mode, print description below the entry
            if verbose && !entry.description.is_empty() {
                println!("{table}");
                println!(
                    "    {}",
                    dim(&format!("  {}", entry.description), use_color)
                );
                table = Table::new();
                table.set_content_arrangement(ContentArrangement::Dynamic);
                if verbose {
                    table.set_header(vec![
                        "Setting",
                        &result.left_name,
                        &result.right_name,
                        "Affects",
                    ]);
                } else {
                    table.set_header(vec!["Setting", &result.left_name, &result.right_name]);
                }
            }
        }

        // Print the remaining table (if it has rows that haven't been printed)
        // comfy_table prints header even with no rows, so only print if we have entries
        // that weren't followed by a verbose description flush
        if !verbose || group_entries.is_empty() {
            println!("{table}");
        }
        println!();
    }
}
