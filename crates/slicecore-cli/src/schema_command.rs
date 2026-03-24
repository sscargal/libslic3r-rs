//! CLI subcommand for querying the setting schema registry.
//!
//! Supports JSON Schema output, flat metadata JSON, tier/category filtering,
//! and full-text search across setting definitions.

use clap::Args;
use slicecore_config_schema::{OverrideSafety, SettingCategory, Tier};
use slicecore_engine::setting_registry;

/// Arguments for the `schema` subcommand.
#[derive(Args)]
pub struct SchemaArgs {
    /// Output format: json-schema or json.
    #[arg(long, default_value = "json-schema")]
    format: SchemaFormat,

    /// Filter by maximum tier: simple, intermediate, advanced, developer.
    #[arg(long)]
    tier: Option<TierFilter>,

    /// Filter by category: quality, speed, line-width, cooling, retraction,
    /// support, infill, adhesion, advanced, machine, filament, acceleration,
    /// post-process, timelapse, multi-material, calibration.
    #[arg(long)]
    category: Option<String>,

    /// Full-text search across key, display name, description, and tags.
    #[arg(long)]
    search: Option<String>,

    /// Filter by override safety level: safe, warn, ignored.
    #[arg(long)]
    override_safety: Option<SafetyFilter>,
}

/// Override safety filter matching the `OverrideSafety` enum.
#[derive(Clone, clap::ValueEnum)]
enum SafetyFilter {
    /// Safe to override per-object/per-region.
    Safe,
    /// Nonsensical per-region but allowed (warns).
    Warn,
    /// Machine property, silently ignored as override.
    Ignored,
}

impl SafetyFilter {
    fn to_override_safety(&self) -> OverrideSafety {
        match self {
            Self::Safe => OverrideSafety::Safe,
            Self::Warn => OverrideSafety::Warn,
            Self::Ignored => OverrideSafety::Ignored,
        }
    }
}

/// Output format for the schema command.
#[derive(Clone, clap::ValueEnum)]
enum SchemaFormat {
    /// JSON Schema 2020-12 document.
    JsonSchema,
    /// Flat metadata JSON array.
    Json,
}

/// Tier filter matching the progressive disclosure levels.
#[derive(Clone, clap::ValueEnum)]
enum TierFilter {
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
}

/// Parses a category string into a `SettingCategory`.
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

/// Runs the schema subcommand with the given arguments.
///
/// # Errors
///
/// Returns an error if category parsing fails or JSON serialization fails.
pub fn run_schema_command(args: &SchemaArgs) -> Result<(), Box<dyn std::error::Error>> {
    let registry = setting_registry();

    // Handle search mode (returns early)
    if let Some(ref query) = args.search {
        let results = registry.search(query);
        let json_results: Vec<serde_json::Value> = results
            .iter()
            .map(|def| serde_json::to_value(def).unwrap_or_default())
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
        return Ok(());
    }

    match args.format {
        SchemaFormat::JsonSchema => {
            if args.tier.is_some() || args.category.is_some() {
                eprintln!(
                    "Warning: --tier and --category filters are not supported with json-schema format. \
                     Outputting full schema."
                );
            }
            let schema = registry.to_json_schema();
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
        SchemaFormat::Json => {
            let tier = args.tier.as_ref().map(TierFilter::to_tier);
            let category = match &args.category {
                Some(cat_str) => {
                    let cat = parse_category(cat_str).ok_or_else(|| {
                        format!("Unknown category: '{cat_str}'. Valid categories: quality, speed, \
                            line-width, cooling, retraction, support, infill, adhesion, advanced, machine, \
                            filament, acceleration, post-process, timelapse, multi-material, calibration")
                    })?;
                    Some(cat)
                }
                None => None,
            };
            let safety = args
                .override_safety
                .as_ref()
                .map(SafetyFilter::to_override_safety);
            let metadata =
                registry.to_filtered_metadata_json_with_safety(tier, category, safety);
            println!("{}", serde_json::to_string_pretty(&metadata)?);
        }
    }

    Ok(())
}
