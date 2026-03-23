//! Profile management CLI subcommands.
//!
//! Provides the `slicecore profile` command group with subcommands for cloning,
//! editing, validating, and managing print profiles. The primary workflow starts
//! with `profile clone` to create a custom profile from a library preset, then
//! uses `profile set`, `profile edit`, or `profile validate` to customize and
//! verify settings.
//!
//! Available subcommands:
//! - `clone`: Create a custom profile from an existing preset
//! - `setting`: Set a single setting value
//! - `set`: Manage saved profile sets (create, delete, list, show, default)
//! - `get`: Get a single setting value
//! - `reset`: Reset a setting to its inherited value
//! - `edit`: Open profile in `$EDITOR`
//! - `validate`: Validate profile against schema
//! - `delete`: Delete a custom profile
//! - `rename`: Rename a custom profile
//! - `enable`: Enable one or more profiles by ID
//! - `disable`: Disable one or more profiles by ID
//! - `status`: Show enabled profile summary
//! - `setup`: Interactive first-run wizard or non-interactive setup
//! - `list`: List profiles with activation-aware filtering and `--compat` column
//! - `search`: Search profiles with `--material`, `--vendor`, `--nozzle`, `--type` filter flags
//! - `compat`: Show detailed compatibility breakdown for a profile

use std::path::{Path, PathBuf};

use clap::Subcommand;
use slicecore_config_schema::SettingKey;
use slicecore_engine::config::PrintConfig;
use slicecore_engine::enabled_profiles::{CompatibilityInfo, EnabledProfiles, ProfileSet};
use slicecore_engine::profile_library::{matches_filters, ProfileFilters, ProfileIndex};
use slicecore_engine::profile_resolve::{
    ProfileError, ProfileResolver, ProfileSource, ResolvedProfile,
};

/// Shared filter flags for profile search and list commands.
///
/// All specified filters use AND logic: every set filter must match.
#[derive(clap::Args, Debug, Default, Clone)]
pub struct CliProfileFilters {
    /// Filter by material type (PLA, ABS, PETG, etc.)
    #[arg(short = 'm', long)]
    pub material: Option<String>,

    /// Filter by vendor name
    #[arg(short = 'v', long)]
    pub vendor: Option<String>,

    /// Filter by nozzle diameter (mm)
    #[arg(short = 'n', long)]
    pub nozzle: Option<f64>,

    /// Filter by profile type (machine, filament, process)
    #[arg(short = 't', long = "type")]
    pub profile_type: Option<String>,
}

impl From<&CliProfileFilters> for ProfileFilters {
    fn from(cli: &CliProfileFilters) -> Self {
        Self {
            material: cli.material.clone(),
            vendor: cli.vendor.clone(),
            nozzle: cli.nozzle,
            profile_type: cli.profile_type.clone(),
        }
    }
}

/// Profile management subcommands.
#[derive(Subcommand)]
pub enum ProfileCommand {
    /// Create a custom profile by cloning an existing preset.
    ///
    /// Copies the source profile to ~/.slicecore/profiles/{type}/ with a
    /// \[metadata\] section recording the clone lineage.
    Clone {
        /// Source profile name or path (e.g., BBL/PLA_Basic)
        source: String,

        /// Name for the new custom profile
        name: String,

        /// Overwrite if the target profile already exists
        #[arg(long)]
        force: bool,

        /// Profile type hint (machine, filament, or process)
        #[arg(long)]
        r#type: Option<String>,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Set a single setting value in a custom profile.
    #[command(name = "setting")]
    Setting {
        /// Profile name
        name: String,

        /// Setting key (e.g., speed.perimeter)
        key: String,

        /// New value
        value: String,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Manage saved profile sets (machine + filament + process combos).
    #[command(name = "set")]
    Set {
        #[command(subcommand)]
        command: ProfileSetCommand,
    },

    /// Get a single setting value from a profile.
    Get {
        /// Profile name
        name: String,

        /// Setting key (e.g., speed.perimeter)
        key: String,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Reset a setting to its inherited value.
    Reset {
        /// Profile name
        name: String,

        /// Setting key to reset
        key: String,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Open a profile in $EDITOR for manual editing.
    Edit {
        /// Profile name
        name: String,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Validate a profile against the setting schema.
    Validate {
        /// Profile name
        name: String,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,

        /// Output validation results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Delete a custom profile.
    Delete {
        /// Profile name
        name: String,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Rename a custom profile.
    Rename {
        /// Current profile name
        old_name: String,

        /// New profile name
        new_name: String,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Enable one or more profiles by ID.
    ///
    /// Auto-detects profile type from library index metadata.
    /// Omit IDs to launch interactive picker (requires terminal).
    Enable {
        /// Profile IDs to enable (omit for interactive picker)
        ids: Vec<String>,

        /// Profile type filter for interactive picker
        #[arg(long)]
        r#type: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Disable one or more profiles.
    ///
    /// Omit IDs to launch interactive picker showing enabled profiles.
    Disable {
        /// Profile IDs to disable (omit for interactive picker)
        ids: Vec<String>,

        /// Profile type filter for interactive picker
        #[arg(long)]
        r#type: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Show enabled profile summary.
    ///
    /// Displays count of enabled profiles by type (machine, filament, process).
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// List profiles from the profile library.
    ///
    /// Alias for the top-level `list-profiles` command, available under the
    /// `profile` command group for convenience. Supports `--material`, `--vendor`,
    /// `--nozzle`, and `--type` filter flags via flattened `CliProfileFilters`.
    List {
        /// Shared filter flags (--material, --vendor, --nozzle, --type)
        #[command(flatten)]
        filters: CliProfileFilters,

        /// List available vendors only (no individual profiles)
        #[arg(long)]
        vendors: bool,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Show only enabled profiles (default when enabled-profiles.toml exists)
        #[arg(long)]
        enabled: bool,

        /// Show only disabled profiles
        #[arg(long)]
        disabled: bool,

        /// Show all profiles regardless of activation status
        #[arg(long, conflicts_with_all = ["enabled", "disabled"])]
        all: bool,

        /// Show compatibility column
        #[arg(long)]
        compat: bool,
    },

    /// Show details of a specific profile.
    ///
    /// Alias for the top-level `show-profile` command.
    Show {
        /// Profile ID or name
        id: String,

        /// Show raw TOML content instead of formatted output
        #[arg(long)]
        raw: bool,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Search profiles by keyword with filter flags.
    ///
    /// Returns case-insensitive substring matches across name, vendor, material,
    /// and ID. By default shows only profiles compatible with enabled printers;
    /// use `--include-incompatible` to show all.
    Search {
        /// Search query (matches name, vendor, material, ID)
        query: String,

        /// Maximum number of results
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Shared filter flags (--material, --vendor, --nozzle, --type)
        #[command(flatten)]
        filters: CliProfileFilters,

        /// Show all profiles including those incompatible with enabled printers
        #[arg(long)]
        include_incompatible: bool,

        /// Enable matched profiles after search
        #[arg(long)]
        enable: bool,

        /// Show all profiles regardless of activation status
        #[arg(long)]
        all: bool,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show detailed compatibility information for a profile.
    ///
    /// Displays nozzle, temperature, and type compatibility checks against
    /// enabled printers. Use `--json` for structured output.
    Compat {
        /// Profile ID to check compatibility for
        id: String,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Interactive first-run setup wizard.
    ///
    /// Guides through vendor -> printer -> filament selection.
    /// Re-runnable to add/remove profiles. Use --reset to start fresh.
    Setup {
        /// Clear all enabled profiles and start fresh
        #[arg(long)]
        reset: bool,

        /// Machine profile ID (non-interactive mode)
        #[arg(long)]
        machine: Vec<String>,

        /// Filament profile ID (non-interactive mode)
        #[arg(long)]
        filament: Vec<String>,

        /// Process profile ID (non-interactive mode)
        #[arg(long)]
        process: Vec<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },

    /// Compare two print profiles side by side.
    ///
    /// Alias for the top-level `diff-profiles` command.
    Diff(crate::diff_profiles_command::DiffProfilesArgs),
}

/// Profile set management subcommands.
///
/// Manages named profile sets (machine + filament + process triples)
/// for quick slicing with pre-configured profile combos.
#[derive(Subcommand)]
pub enum ProfileSetCommand {
    /// Create a named profile set (machine + filament + process combo).
    Create {
        /// Name for the profile set (e.g., my-x1c-pla)
        name: String,
        /// Machine profile ID
        #[arg(long)]
        machine: String,
        /// Filament profile ID
        #[arg(long)]
        filament: String,
        /// Process profile ID
        #[arg(long)]
        process: String,
        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },
    /// Delete a saved profile set.
    Delete {
        /// Name of the set to delete
        name: String,
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },
    /// List all saved profile sets.
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },
    /// Show details of a saved profile set.
    Show {
        /// Name of the set to show
        name: String,
        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },
    /// Set the default profile set for slicing.
    Default {
        /// Name of the set to make default (omit to show current default)
        name: Option<String>,
        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
    },
}

/// Runs a profile management subcommand.
///
/// # Errors
///
/// Returns an error if the subcommand fails (e.g., profile not found,
/// invalid name, I/O error).
pub fn run_profile_command(cmd: ProfileCommand) -> Result<(), anyhow::Error> {
    match cmd {
        ProfileCommand::Clone {
            source,
            name,
            force,
            r#type,
            profiles_dir,
        } => cmd_clone(
            &source,
            &name,
            force,
            r#type.as_deref(),
            profiles_dir.as_deref(),
        ),
        ProfileCommand::Setting {
            name,
            key,
            value,
            profiles_dir,
        } => cmd_set(&name, &key, &value, profiles_dir.as_deref()),
        ProfileCommand::Set { command } => match command {
            ProfileSetCommand::Create {
                name,
                machine,
                filament,
                process,
                profiles_dir,
            } => cmd_set_create(&name, &machine, &filament, &process, profiles_dir.as_deref()),
            ProfileSetCommand::Delete {
                name,
                yes: _,
                profiles_dir,
            } => cmd_set_delete(&name, profiles_dir.as_deref()),
            ProfileSetCommand::List {
                json,
                profiles_dir,
            } => cmd_set_list(json, profiles_dir.as_deref()),
            ProfileSetCommand::Show {
                name,
                profiles_dir,
            } => cmd_set_show(&name, profiles_dir.as_deref()),
            ProfileSetCommand::Default {
                name,
                profiles_dir,
            } => cmd_set_default(name.as_deref(), profiles_dir.as_deref()),
        },
        ProfileCommand::Get {
            name,
            key,
            profiles_dir,
        } => cmd_get(&name, &key, profiles_dir.as_deref()),
        ProfileCommand::Reset {
            name,
            key,
            profiles_dir,
        } => cmd_reset(&name, &key, profiles_dir.as_deref()),
        ProfileCommand::Edit { name, profiles_dir } => cmd_edit(&name, profiles_dir.as_deref()),
        ProfileCommand::Validate {
            name,
            profiles_dir,
            json,
        } => cmd_validate(&name, json, profiles_dir.as_deref()),
        ProfileCommand::Delete {
            name,
            yes,
            profiles_dir,
        } => cmd_delete(&name, yes, profiles_dir.as_deref()),
        ProfileCommand::Rename {
            old_name,
            new_name,
            profiles_dir,
        } => cmd_rename(&old_name, &new_name, profiles_dir.as_deref()),
        ProfileCommand::Enable {
            ids,
            r#type,
            json,
            profiles_dir,
        } => cmd_enable(&ids, r#type.as_deref(), json, profiles_dir.as_deref()),
        ProfileCommand::Disable {
            ids,
            r#type,
            json,
            profiles_dir,
        } => cmd_disable(&ids, r#type.as_deref(), json, profiles_dir.as_deref()),
        ProfileCommand::Status { json, profiles_dir } => cmd_status(json, profiles_dir.as_deref()),
        ProfileCommand::List {
            filters,
            vendors,
            profiles_dir,
            json,
            enabled,
            disabled,
            all,
            compat,
        } => cmd_list(
            &filters,
            vendors,
            profiles_dir.as_deref(),
            json,
            enabled,
            disabled,
            all,
            compat,
        ),
        ProfileCommand::Show {
            id,
            raw,
            profiles_dir,
        } => cmd_show(&id, raw, profiles_dir.as_deref()),
        ProfileCommand::Search {
            query,
            limit,
            filters,
            include_incompatible,
            enable,
            all,
            profiles_dir,
            json,
        } => cmd_search(
            &query,
            limit,
            &filters,
            include_incompatible,
            enable,
            all,
            profiles_dir.as_deref(),
            json,
        ),
        ProfileCommand::Compat {
            id,
            profiles_dir,
            json,
        } => cmd_compat(&id, profiles_dir.as_deref(), json),
        ProfileCommand::Setup {
            reset,
            machine,
            filament,
            process,
            json: _json,
            profiles_dir,
        } => {
            if !machine.is_empty() || !filament.is_empty() || !process.is_empty() {
                crate::profile_wizard::run_setup_noninteractive(
                    &machine,
                    &filament,
                    &process,
                    profiles_dir.as_deref(),
                    reset,
                )
            } else {
                crate::profile_wizard::run_setup_wizard(profiles_dir.as_deref(), reset)
            }
        }
        ProfileCommand::Diff(args) => {
            crate::diff_profiles_command::run_diff_profiles_command(&args, "auto", false)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(())
        }
    }
}

/// Validates that a profile name is safe for use as a filename.
///
/// A valid name:
/// - Is not empty
/// - Has at most 128 characters
/// - Contains only ASCII letters, digits, hyphens, or underscores
/// - Does not start with a hyphen
fn is_valid_profile_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 128 {
        return false;
    }
    if name.starts_with('-') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Resolves a profile query across all profile types when no type hint is given.
///
/// If `type_hint` is provided, resolves directly against that type. Otherwise,
/// tries `machine`, `filament`, and `process` in order and returns the single
/// match, or errors on zero or multiple matches.
fn try_resolve_any(
    resolver: &ProfileResolver,
    query: &str,
    type_hint: Option<&str>,
) -> Result<ResolvedProfile, anyhow::Error> {
    if let Some(t) = type_hint {
        return resolver
            .resolve(query, t)
            .map_err(|e| anyhow::anyhow!("{e}"));
    }

    let mut matches: Vec<ResolvedProfile> = Vec::new();

    for profile_type in &["machine", "filament", "process"] {
        match resolver.resolve(query, profile_type) {
            Ok(resolved) => matches.push(resolved),
            Err(ProfileError::NotFound { .. } | ProfileError::TypeMismatch { .. }) => continue,
            Err(e) => return Err(e.into()),
        }
    }

    match matches.len() {
        0 => anyhow::bail!(
            "Profile '{}' not found in any type (machine, filament, process)",
            query
        ),
        1 => Ok(matches.remove(0)),
        _ => {
            let types_list: Vec<&str> = matches.iter().map(|m| m.profile_type.as_str()).collect();
            anyhow::bail!(
                "Ambiguous profile '{}': found in types {}. Use --type to disambiguate.",
                query,
                types_list.join(", ")
            )
        }
    }
}

/// Returns the base directory for user profiles (`~/.slicecore/profiles/`).
fn user_profiles_base_dir() -> Result<PathBuf, anyhow::Error> {
    let home =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    Ok(home.join(".slicecore/profiles"))
}

/// Implements the `profile clone` command.
///
/// Resolves the source profile, copies its content to the user profiles
/// directory, and injects a `[metadata]` section recording clone lineage.
fn cmd_clone(
    source: &str,
    new_name: &str,
    force: bool,
    type_hint: Option<&str>,
    profiles_dir: Option<&Path>,
) -> Result<(), anyhow::Error> {
    // Validate new name
    if !is_valid_profile_name(new_name) {
        anyhow::bail!(
            "Invalid profile name '{}'. Use only letters, numbers, hyphens, and \
             underscores (must not start with hyphen).",
            new_name
        );
    }

    // Resolve source profile
    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, source, type_hint)?;

    // Load and re-serialize to TOML
    let config = PrintConfig::from_file(&resolved.path)?;
    let toml_body = toml::to_string_pretty(&config)?;

    // Build metadata header
    let metadata = format!(
        "# Custom profile cloned from {source}\n\
         [metadata]\n\
         name = \"{new_name}\"\n\
         is_custom = true\n\
         inherits = \"{source}\"\n\
         clone_source = \"{}\"",
        resolved.path.display()
    );

    // Determine destination path
    let base = match profiles_dir {
        Some(d) => d.to_path_buf(),
        None => user_profiles_base_dir()?,
    };
    let dest = base
        .join(&resolved.profile_type)
        .join(format!("{new_name}.toml"));

    // Create parent directories
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Check for conflicts
    if dest.exists() && !force {
        anyhow::bail!(
            "Profile '{}' already exists at {}. Use --force to overwrite or choose a different name.",
            new_name,
            dest.display()
        );
    }

    // Write the cloned profile
    std::fs::write(&dest, format!("{metadata}\n\n{toml_body}"))?;

    // Success output
    println!(
        "Created custom profile '{}' at {}",
        new_name,
        dest.display()
    );
    println!("\nNext steps:");
    println!("  slicecore profile show {new_name}");
    println!("  slicecore profile set {new_name} <key> <value>");
    println!("  slicecore profile edit {new_name}");

    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: navigate a dotted key path in a TOML value tree (immutable)
// ---------------------------------------------------------------------------

/// Navigates a dotted key path (e.g., `"speed.perimeter"`) in a `toml::Value`.
///
/// Returns `None` if any intermediate key is missing or not a table.
fn navigate_toml_path<'a>(doc: &'a toml::Value, key: &str) -> Option<&'a toml::Value> {
    let mut current = doc;
    for part in key.split('.') {
        current = current.as_table()?.get(part)?;
    }
    Some(current)
}

// ---------------------------------------------------------------------------
// Helper: navigate a dotted key path in a TOML value tree (mutable)
// ---------------------------------------------------------------------------

/// Navigates a dotted key path, creating intermediate tables as needed.
///
/// Returns a mutable reference to the leaf value's parent slot. The caller
/// should then insert or update the final key.
fn navigate_toml_path_mut<'a>(doc: &'a mut toml::Value, key: &str) -> &'a mut toml::Value {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = doc;
    // Navigate/create all parts including the leaf -- the caller sets the value
    for part in &parts[..parts.len().saturating_sub(1)] {
        // Ensure current is a table
        if !current.is_table() {
            *current = toml::Value::Table(toml::map::Map::new());
        }
        let table = current.as_table_mut().expect("just ensured table");
        if !table.contains_key(*part) {
            table.insert(
                (*part).to_string(),
                toml::Value::Table(toml::map::Map::new()),
            );
        }
        current = table.get_mut(*part).expect("just inserted");
    }
    current
}

// ---------------------------------------------------------------------------
// Helper: parse a string value into a toml::Value with type inference
// ---------------------------------------------------------------------------

/// Parses a string value into a `toml::Value`, inferring the type.
///
/// Tries integer, float, boolean in order; falls back to string.
fn parse_toml_value(value: &str) -> toml::Value {
    if let Ok(i) = value.parse::<i64>() {
        return toml::Value::Integer(i);
    }
    if let Ok(f) = value.parse::<f64>() {
        return toml::Value::Float(f);
    }
    if let Ok(b) = value.parse::<bool>() {
        return toml::Value::Boolean(b);
    }
    toml::Value::String(value.to_string())
}

// ---------------------------------------------------------------------------
// Helper: require user-source profile (reject library/builtin)
// ---------------------------------------------------------------------------

/// Returns an error if the resolved profile is not a user profile.
fn require_user_profile(resolved: &ResolvedProfile) -> Result<(), anyhow::Error> {
    if resolved.source != ProfileSource::User {
        anyhow::bail!(
            "Cannot modify {} profile '{}'. Clone it first:\n  slicecore profile clone {} my-{}",
            resolved.source,
            resolved.name,
            resolved.name,
            resolved.name,
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// cmd_set: Set a single setting value in a custom profile
// ---------------------------------------------------------------------------

/// Implements the `profile set` command.
fn cmd_set(
    name: &str,
    key: &str,
    value: &str,
    profiles_dir: Option<&Path>,
) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, name, None)?;
    require_user_profile(&resolved)?;

    // Validate key against setting registry
    let registry = slicecore_engine::setting_registry();
    if registry.get(&SettingKey::new(key)).is_none() {
        let suggestions = registry.search(key);
        if suggestions.is_empty() {
            anyhow::bail!("Unknown setting key '{key}'");
        }
        let top: Vec<&str> = suggestions
            .iter()
            .take(3)
            .map(|d| d.key.0.as_str())
            .collect();
        anyhow::bail!(
            "Unknown setting key '{key}'. Did you mean: {}?",
            top.join(", ")
        );
    }

    // Parse and update TOML
    let contents = std::fs::read_to_string(&resolved.path)?;
    let mut doc: toml::Value = toml::from_str(&contents)?;

    let parts: Vec<&str> = key.split('.').collect();
    let leaf_key = parts.last().expect("key is non-empty");
    let parent = navigate_toml_path_mut(&mut doc, key);
    let table = parent
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("Cannot set key '{key}': parent is not a table"))?;
    table.insert((*leaf_key).to_string(), parse_toml_value(value));

    std::fs::write(&resolved.path, toml::to_string_pretty(&doc)?)?;
    println!("Set {key} = {value} in profile '{name}'");
    Ok(())
}

// ---------------------------------------------------------------------------
// Profile set management handlers
// ---------------------------------------------------------------------------

/// Implements the `profile set create` command.
fn cmd_set_create(
    name: &str,
    machine: &str,
    filament: &str,
    process: &str,
    profiles_dir: Option<&Path>,
) -> Result<(), anyhow::Error> {
    let path = enabled_profiles_path(profiles_dir)?;
    let mut enabled = EnabledProfiles::load(&path)?.unwrap_or_default();

    let set = ProfileSet {
        machine: machine.to_string(),
        filament: filament.to_string(),
        process: process.to_string(),
    };

    enabled.add_set(name.to_string(), set);
    enabled.save(&path)?;

    println!(
        "Created profile set '{name}': machine={machine}, filament={filament}, process={process}"
    );
    Ok(())
}

/// Implements the `profile set delete` command.
fn cmd_set_delete(name: &str, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let path = enabled_profiles_path(profiles_dir)?;
    let mut enabled = EnabledProfiles::load(&path)?
        .ok_or_else(|| anyhow::anyhow!("No enabled-profiles.toml found. No sets to delete."))?;

    if enabled.get_set(name).is_none() {
        anyhow::bail!("Profile set '{name}' not found.");
    }

    enabled.remove_set(name);
    enabled.save(&path)?;

    println!("Deleted profile set '{name}'");
    Ok(())
}

/// Implements the `profile set list` command.
fn cmd_set_list(json_output: bool, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let path = enabled_profiles_path(profiles_dir)?;
    let enabled = EnabledProfiles::load(&path)?.unwrap_or_default();

    if enabled.sets.is_empty() {
        eprintln!("No saved profile sets.");
        return Ok(());
    }

    if json_output {
        println!("{}", serde_json::to_string_pretty(&enabled.sets)?);
        return Ok(());
    }

    let default_name = enabled.defaults.set.as_deref();

    // Table header
    let header = format!(
        "{:<20} {:<30} {:<30} {:<30} DEFAULT",
        "NAME", "MACHINE", "FILAMENT", "PROCESS"
    );
    println!("{header}");
    println!("{}", "-".repeat(120));

    let mut names: Vec<&String> = enabled.sets.keys().collect();
    names.sort();

    for name in names {
        let set = &enabled.sets[name];
        let is_default = default_name == Some(name.as_str());
        let marker = if is_default { "[default]" } else { "" };
        println!(
            "{:<20} {:<30} {:<30} {:<30} {}",
            name, set.machine, set.filament, set.process, marker
        );
    }

    Ok(())
}

/// Implements the `profile set show` command.
fn cmd_set_show(name: &str, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let path = enabled_profiles_path(profiles_dir)?;
    let enabled = EnabledProfiles::load(&path)?
        .ok_or_else(|| anyhow::anyhow!("No enabled-profiles.toml found."))?;

    let set = enabled
        .get_set(name)
        .ok_or_else(|| anyhow::anyhow!("Profile set '{name}' not found."))?;

    let is_default = enabled.defaults.set.as_deref() == Some(name);

    println!("Profile Set: {name}");
    println!("  Machine:  {}", set.machine);
    println!("  Filament: {}", set.filament);
    println!("  Process:  {}", set.process);
    println!("  Default:  {is_default}");

    Ok(())
}

/// Implements the `profile set default` command.
fn cmd_set_default(name: Option<&str>, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let path = enabled_profiles_path(profiles_dir)?;
    let mut enabled = EnabledProfiles::load(&path)?.unwrap_or_default();

    match name {
        None => {
            match enabled.default_set() {
                Some((n, set)) => {
                    println!("Default profile set: {n}");
                    println!("  Machine:  {}", set.machine);
                    println!("  Filament: {}", set.filament);
                    println!("  Process:  {}", set.process);
                }
                None => {
                    println!("No default set configured.");
                }
            }
            Ok(())
        }
        Some(n) => {
            if enabled.get_set(n).is_none() {
                anyhow::bail!("Profile set '{n}' not found. Create it first with: slicecore profile set create {n} --machine <M> --filament <F> --process <P>");
            }
            enabled.set_default(Some(n.to_string()));
            enabled.save(&path)?;
            println!("Set '{n}' as default profile set.");
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// cmd_get: Get a single setting value from a profile
// ---------------------------------------------------------------------------

/// Implements the `profile get` command.
fn cmd_get(name: &str, key: &str, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, name, None)?;

    let contents = std::fs::read_to_string(&resolved.path)?;
    let doc: toml::Value = toml::from_str(&contents)?;

    match navigate_toml_path(&doc, key) {
        Some(val) => {
            println!("{val}");
            Ok(())
        }
        None => anyhow::bail!("Key '{key}' not found in profile '{name}'"),
    }
}

// ---------------------------------------------------------------------------
// cmd_reset: Reset a setting to its inherited source value
// ---------------------------------------------------------------------------

/// Implements the `profile reset` command.
fn cmd_reset(name: &str, key: &str, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, name, None)?;
    require_user_profile(&resolved)?;

    // Read target profile
    let contents = std::fs::read_to_string(&resolved.path)?;
    let mut doc: toml::Value = toml::from_str(&contents)?;

    // Find inherits field
    let inherits = doc
        .get("metadata")
        .and_then(|m| m.get("inherits"))
        .and_then(toml::Value::as_str)
        .map(String::from);

    let inherits = match inherits {
        Some(s) => s,
        None => anyhow::bail!(
            "Profile '{name}' has no inherited source. Cannot reset to original value."
        ),
    };

    // Resolve the source profile
    let source_resolved = try_resolve_any(&resolver, &inherits, None)?;
    let source_contents = std::fs::read_to_string(&source_resolved.path)?;
    let source_doc: toml::Value = toml::from_str(&source_contents)?;

    let source_val = navigate_toml_path(&source_doc, key)
        .ok_or_else(|| anyhow::anyhow!("Key '{key}' not found in source profile '{inherits}'"))?
        .clone();

    // Set key in target to source value
    let parts: Vec<&str> = key.split('.').collect();
    let leaf_key = parts.last().expect("key is non-empty");
    let parent = navigate_toml_path_mut(&mut doc, key);
    let table = parent
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("Cannot reset key '{key}': parent is not a table"))?;
    table.insert((*leaf_key).to_string(), source_val);

    std::fs::write(&resolved.path, toml::to_string_pretty(&doc)?)?;
    println!("Reset {key} in profile '{name}' to inherited value from '{inherits}'");
    Ok(())
}

// ---------------------------------------------------------------------------
// cmd_validate: Validate a profile against the setting schema
// ---------------------------------------------------------------------------

/// Implements the `profile validate` command.
fn cmd_validate(
    name: &str,
    json_output: bool,
    profiles_dir: Option<&Path>,
) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, name, None)?;

    let config = PrintConfig::from_file(&resolved.path)?;
    let config_json = serde_json::to_value(&config)?;
    let issues = slicecore_engine::setting_registry().validate_config(&config_json);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&issues)?);
        return Ok(());
    }

    if issues.is_empty() {
        println!("Profile '{name}' is valid");
        return Ok(());
    }

    let mut errors = 0usize;
    let mut warnings = 0usize;

    for issue in &issues {
        let prefix = match issue.severity {
            slicecore_config_schema::ValidationSeverity::Error => {
                errors += 1;
                "ERROR"
            }
            slicecore_config_schema::ValidationSeverity::Warning => {
                warnings += 1;
                "WARNING"
            }
            slicecore_config_schema::ValidationSeverity::Info => "INFO",
        };
        println!("{prefix}: [{}] {}", issue.key, issue.message);
    }

    eprintln!("{errors} error(s), {warnings} warning(s)");
    Ok(())
}

// ---------------------------------------------------------------------------
// cmd_edit: Open profile in $EDITOR for manual editing
// ---------------------------------------------------------------------------

/// Implements the `profile edit` command.
fn cmd_edit(name: &str, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, name, None)?;
    require_user_profile(&resolved)?;

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "nano".to_string());

    let status = std::process::Command::new(&editor)
        .arg(&resolved.path)
        .status()?;

    if !status.success() {
        eprintln!("Editor exited with non-zero status");
    }

    if !resolved.path.exists() {
        anyhow::bail!("Profile file was deleted during editing");
    }

    // Validate TOML syntax
    let contents = std::fs::read_to_string(&resolved.path)?;
    match toml::from_str::<toml::Value>(&contents) {
        Ok(_doc) => {
            // Run schema validation and print any issues
            if let Ok(config) = PrintConfig::from_file(&resolved.path) {
                if let Ok(config_json) = serde_json::to_value(&config) {
                    let issues = slicecore_engine::setting_registry().validate_config(&config_json);
                    for issue in &issues {
                        let prefix = match issue.severity {
                            slicecore_config_schema::ValidationSeverity::Error => "ERROR",
                            slicecore_config_schema::ValidationSeverity::Warning => "WARNING",
                            slicecore_config_schema::ValidationSeverity::Info => "INFO",
                        };
                        eprintln!("{prefix}: [{}] {}", issue.key, issue.message);
                    }
                }
            }
        }
        Err(err) => {
            eprintln!("Warning: TOML syntax error: {err}. File saved but may need fixing.");
        }
    }

    println!("Profile '{name}' updated at {}", resolved.path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// cmd_delete: Delete a custom profile
// ---------------------------------------------------------------------------

/// Implements the `profile delete` command.
fn cmd_delete(name: &str, yes: bool, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, name, None)?;

    if resolved.source != ProfileSource::User {
        anyhow::bail!(
            "Cannot delete {} profile '{}'. Only user profiles can be deleted.",
            resolved.source,
            resolved.name,
        );
    }

    println!("Will delete: {}", resolved.path.display());

    if !yes {
        anyhow::bail!("Use --yes to confirm deletion, or abort");
    }

    std::fs::remove_file(&resolved.path)?;
    println!("Deleted profile '{name}'");
    Ok(())
}

// ---------------------------------------------------------------------------
// cmd_rename: Rename a custom profile
// ---------------------------------------------------------------------------

/// Implements the `profile rename` command.
fn cmd_rename(
    old_name: &str,
    new_name: &str,
    profiles_dir: Option<&Path>,
) -> Result<(), anyhow::Error> {
    if !is_valid_profile_name(new_name) {
        anyhow::bail!(
            "Invalid profile name '{}'. Use only letters, numbers, hyphens, and \
             underscores (must not start with hyphen).",
            new_name,
        );
    }

    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, old_name, None)?;
    require_user_profile(&resolved)?;

    let new_path = resolved
        .path
        .parent()
        .expect("profile file has parent dir")
        .join(format!("{new_name}.toml"));

    if new_path.exists() {
        anyhow::bail!(
            "Profile '{new_name}' already exists at {}",
            new_path.display()
        );
    }

    // Read, update metadata.name, write to new path
    let contents = std::fs::read_to_string(&resolved.path)?;
    let mut doc: toml::Value = toml::from_str(&contents)?;

    if let Some(meta) = doc.get_mut("metadata").and_then(toml::Value::as_table_mut) {
        meta.insert(
            "name".to_string(),
            toml::Value::String(new_name.to_string()),
        );
    }

    std::fs::write(&new_path, toml::to_string_pretty(&doc)?)?;
    std::fs::remove_file(&resolved.path)?;

    println!(
        "Renamed profile '{old_name}' to '{new_name}' at {}",
        new_path.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Alias commands: list, show, search (thin wrappers using ProfileResolver)
// ---------------------------------------------------------------------------

/// Implements the `profile list` alias command.
// ---------------------------------------------------------------------------
// Enable / Disable / Status commands
// ---------------------------------------------------------------------------
/// Returns the path for `enabled-profiles.toml`, respecting `--profiles-dir`.
///
/// When `profiles_dir` is given, uses `<profiles_dir>/enabled-profiles.toml`.
/// Otherwise falls back to `EnabledProfiles::default_path()`.
fn enabled_profiles_path(profiles_dir: Option<&Path>) -> Result<PathBuf, anyhow::Error> {
    if let Some(dir) = profiles_dir {
        Ok(dir.join("enabled-profiles.toml"))
    } else {
        EnabledProfiles::default_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))
    }
}

/// Implements the `profile enable` command.
fn cmd_enable(
    ids: &[String],
    type_hint: Option<&str>,
    json_output: bool,
    profiles_dir: Option<&Path>,
) -> Result<(), anyhow::Error> {
    let path = enabled_profiles_path(profiles_dir)?;
    let mut enabled = EnabledProfiles::load(&path)?.unwrap_or_default();

    let effective_ids: Vec<String> = if ids.is_empty() {
        crate::profile_wizard::run_enable_picker(type_hint, profiles_dir)?
    } else {
        ids.to_vec()
    };

    if effective_ids.is_empty() {
        return Ok(());
    }

    let resolver = ProfileResolver::new(profiles_dir);
    let mut results: Vec<serde_json::Value> = Vec::new();

    for id in &effective_ids {
        let resolved = try_resolve_any(&resolver, id, type_hint)?;
        enabled.enable(&resolved.profile_type, &resolved.name);
        eprintln!(
            "Enabled {} profile: {}",
            resolved.profile_type, resolved.name
        );
        if json_output {
            results.push(serde_json::json!({
                "id": resolved.name,
                "type": resolved.profile_type,
            }));
        }
    }

    enabled.save(&path)?;

    if json_output {
        let all: Vec<&str> = enabled.all_enabled().iter().map(|(_, id)| *id).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "enabled": all }))?
        );
    }

    Ok(())
}

/// Implements the `profile disable` command.
fn cmd_disable(
    ids: &[String],
    type_hint: Option<&str>,
    json_output: bool,
    profiles_dir: Option<&Path>,
) -> Result<(), anyhow::Error> {
    let path = enabled_profiles_path(profiles_dir)?;
    let loaded = EnabledProfiles::load(&path)?;
    let Some(mut enabled) = loaded else {
        eprintln!("No profiles are enabled.");
        return Ok(());
    };

    let effective_ids: Vec<String> = if ids.is_empty() {
        crate::profile_wizard::run_disable_picker(type_hint, profiles_dir)?
    } else {
        ids.to_vec()
    };

    if effective_ids.is_empty() {
        return Ok(());
    }

    for id in &effective_ids {
        if let Some(t) = type_hint {
            enabled.disable(t, id);
        } else {
            enabled.disable("machine", id);
            enabled.disable("filament", id);
            enabled.disable("process", id);
        }
        eprintln!("Disabled profile: {id}");
    }

    enabled.save(&path)?;

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "disabled": &effective_ids }))?
        );
    }

    Ok(())
}

/// Implements the `profile status` command.
fn cmd_status(json_output: bool, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let path = enabled_profiles_path(profiles_dir)?;
    let loaded = EnabledProfiles::load(&path)?;

    let Some(enabled) = loaded else {
        eprintln!("No profiles enabled. Run 'slicecore profile setup' to get started.");
        return Ok(());
    };

    let (mc, fc, pc) = enabled.counts();

    if json_output {
        let machine_list: &[String] = &enabled.machine.enabled;
        let filament_list: &[String] = &enabled.filament.enabled;
        let process_list: &[String] = &enabled.process.enabled;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "machine_count": mc,
                "filament_count": fc,
                "process_count": pc,
                "machine": machine_list,
                "filament": filament_list,
                "process": process_list,
            }))?
        );
    } else {
        println!("Profile activation status:");
        println!("  Machines:  {mc} enabled");
        println!("  Filaments: {fc} enabled");
        println!("  Process:   {pc} enabled");
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)] // activation filter flags added alongside existing params
fn cmd_list(
    cli_filters: &CliProfileFilters,
    vendors_only: bool,
    profiles_dir: Option<&Path>,
    json_output: bool,
    filter_enabled: bool,
    filter_disabled: bool,
    filter_all: bool,
    show_compat: bool,
) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let all_profiles =
        resolver.search("", cli_filters.profile_type.as_deref(), usize::MAX);

    // Determine activation filter mode
    let ep_path = enabled_profiles_path(profiles_dir).ok();
    let enabled_profiles = ep_path
        .as_ref()
        .and_then(|p| EnabledProfiles::load(p).ok().flatten());

    let show_all =
        filter_all || (!filter_enabled && !filter_disabled && enabled_profiles.is_none());
    let show_enabled =
        filter_enabled || (!filter_all && !filter_disabled && enabled_profiles.is_some());
    let show_disabled = filter_disabled;

    // Apply activation filtering
    let activation_filtered: Vec<&ResolvedProfile> = if show_all {
        all_profiles.iter().collect()
    } else if show_disabled {
        if let Some(ref ep) = enabled_profiles {
            all_profiles
                .iter()
                .filter(|p| !ep.is_enabled(&p.profile_type, &p.name))
                .collect()
        } else {
            all_profiles.iter().collect()
        }
    } else if show_enabled {
        if let Some(ref ep) = enabled_profiles {
            let result: Vec<&ResolvedProfile> = all_profiles
                .iter()
                .filter(|p| ep.is_enabled(&p.profile_type, &p.name))
                .collect();
            if result.is_empty() {
                eprintln!(
                    "No enabled profiles. Run 'slicecore profile setup' or use --all to see everything."
                );
            }
            result
        } else {
            all_profiles.iter().collect()
        }
    } else {
        all_profiles.iter().collect()
    };

    // Apply ProfileFilters from CLI (vendor, material, nozzle, type)
    let engine_filters: ProfileFilters = cli_filters.into();
    let index = resolver.index();
    let filtered: Vec<&&ResolvedProfile> = activation_filtered
        .iter()
        .filter(|p| {
            // If we have an index, use matches_filters for precise filtering
            if let Some(idx) = index {
                if let Some(entry) = idx.profiles.iter().find(|e| e.name == p.name) {
                    return matches_filters(entry, &engine_filters);
                }
            }
            // Fallback: filter by vendor from ProfileSource
            if let Some(ref v) = cli_filters.vendor {
                match &p.source {
                    ProfileSource::Library { vendor: pv } => {
                        if !pv.to_lowercase().contains(&v.to_lowercase()) {
                            return false;
                        }
                    }
                    _ => return false,
                }
            }
            true
        })
        .collect();

    // Build compatibility info for --compat column
    let compat_info = if show_compat {
        index.and_then(|idx| {
            enabled_profiles.as_ref().map(|ep| {
                let machine_ids: Vec<String> =
                    ep.machine.enabled.clone();
                CompatibilityInfo::from_index_entries(&machine_ids, idx)
            })
        })
    } else {
        None
    };

    // Get machine entries for compat_report
    let machine_entries_data: Vec<_> = if show_compat {
        if let Some(idx) = index {
            if let Some(ref ep) = enabled_profiles {
                idx.profiles
                    .iter()
                    .filter(|e| {
                        e.profile_type == "machine"
                            && ep.machine.enabled.contains(&e.id)
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    let machine_entry_refs: Vec<&_> = machine_entries_data.to_vec();

    if vendors_only {
        let mut vendors: Vec<String> = filtered
            .iter()
            .filter_map(|p| match &p.source {
                ProfileSource::Library { vendor: v } => Some(v.clone()),
                _ => None,
            })
            .collect();
        vendors.sort();
        vendors.dedup();

        if json_output {
            println!("{}", serde_json::to_string_pretty(&vendors)?);
        } else {
            for v in &vendors {
                println!("{v}");
            }
            eprintln!("{} vendor(s) found", vendors.len());
        }
        return Ok(());
    }

    if json_output {
        let entries: Vec<serde_json::Value> = filtered
            .iter()
            .map(|p| {
                let mut obj = serde_json::json!({
                    "name": p.name,
                    "profile_type": p.profile_type,
                    "source": p.source.to_string(),
                    "path": p.path.display().to_string(),
                });
                if show_compat {
                    if let Some(ref ci) = compat_info {
                        if let Some(idx) = index {
                            if let Some(entry) =
                                idx.profiles.iter().find(|e| e.name == p.name)
                            {
                                let report = CompatibilityInfo::compat_report(
                                    entry,
                                    &machine_entry_refs,
                                    300.0,
                                    None,
                                );
                                obj["compatible"] =
                                    serde_json::json!(ci.is_compatible(entry));
                                obj["compat_report"] =
                                    serde_json::to_value(&report).unwrap_or_default();
                            }
                        }
                    }
                }
                obj
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        if show_compat {
            println!(
                "{:<10} {:<12} {:<50} {:<10} {:<15}",
                "TYPE", "VENDOR", "NAME", "COMPAT", "SOURCE"
            );
            println!("{}", "-".repeat(101));
        } else {
            println!(
                "{:<10} {:<12} {:<50} {:<15}",
                "TYPE", "VENDOR", "NAME", "SOURCE"
            );
            println!("{}", "-".repeat(91));
        }
        for p in &filtered {
            let vendor_name = match &p.source {
                ProfileSource::Library { vendor: v } => v.as_str(),
                _ => "-",
            };
            if show_compat {
                let compat_marker = if let Some(ref ci) = compat_info {
                    if let Some(idx) = index {
                        if let Some(entry) =
                            idx.profiles.iter().find(|e| e.name == p.name)
                        {
                            if ci.is_compatible(entry) {
                                "OK"
                            } else {
                                "WARN"
                            }
                        } else {
                            "?"
                        }
                    } else {
                        "?"
                    }
                } else {
                    "-"
                };
                println!(
                    "{:<10} {:<12} {:<50} {:<10} {:<15}",
                    p.profile_type, vendor_name, p.name, compat_marker, p.source,
                );
            } else {
                println!(
                    "{:<10} {:<12} {:<50} {:<15}",
                    p.profile_type, vendor_name, p.name, p.source,
                );
            }
        }
        eprintln!("{} profile(s) found", filtered.len());
    }
    Ok(())
}

/// Implements the `profile show` alias command.
fn cmd_show(id: &str, raw: bool, profiles_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let resolved = try_resolve_any(&resolver, id, None)?;

    if raw {
        let contents = std::fs::read_to_string(&resolved.path)?;
        print!("{contents}");
    } else {
        println!("Name:   {}", resolved.name);
        println!("Type:   {}", resolved.profile_type);
        println!("Source: {}", resolved.source);
        println!("Path:   {}", resolved.path.display());
        println!();

        let contents = std::fs::read_to_string(&resolved.path)?;
        let doc: toml::Value = toml::from_str(&contents)?;
        if let Some(table) = doc.as_table() {
            for (section, val) in table {
                if section == "metadata" {
                    continue;
                }
                if let Some(inner) = val.as_table() {
                    println!("[{section}]");
                    for (k, v) in inner {
                        println!("  {k} = {v}");
                    }
                    println!();
                } else {
                    println!("{section} = {val}");
                }
            }
        }
    }
    Ok(())
}

/// Implements the `profile search` command with filter flags and compatibility filtering.
#[allow(clippy::too_many_arguments)]
fn cmd_search(
    query: &str,
    limit: usize,
    cli_filters: &CliProfileFilters,
    include_incompatible: bool,
    enable: bool,
    show_all: bool,
    profiles_dir: Option<&Path>,
    json_output: bool,
) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let matching = resolver.search(query, cli_filters.profile_type.as_deref(), limit);

    if matching.is_empty() {
        eprintln!("No profiles found matching '{query}'.");
        return Ok(());
    }

    // Load index for filter application
    let index = resolver.index();
    let engine_filters: ProfileFilters = cli_filters.into();

    // Apply ProfileFilters
    let filtered: Vec<&ResolvedProfile> = matching
        .iter()
        .filter(|p| {
            if let Some(idx) = index {
                if let Some(entry) = idx.profiles.iter().find(|e| e.name == p.name) {
                    return matches_filters(entry, &engine_filters);
                }
            }
            // Fallback: vendor filter from source
            if let Some(ref v) = cli_filters.vendor {
                match &p.source {
                    ProfileSource::Library { vendor: pv } => {
                        if !pv.to_lowercase().contains(&v.to_lowercase()) {
                            return false;
                        }
                    }
                    _ => return false,
                }
            }
            true
        })
        .collect();

    // Load enabled profiles for activation and compatibility filtering
    let ep_path = enabled_profiles_path(profiles_dir).ok();
    let enabled_profiles = ep_path
        .as_ref()
        .and_then(|p| EnabledProfiles::load(p).ok().flatten());

    // Apply activation filter (show only enabled unless --all)
    let activation_filtered: Vec<&ResolvedProfile> = if show_all || enabled_profiles.is_none() {
        filtered
    } else if let Some(ref ep) = enabled_profiles {
        filtered
            .into_iter()
            .filter(|p| ep.is_enabled(&p.profile_type, &p.name))
            .collect()
    } else {
        filtered
    };

    // Build compat info for compatibility filtering
    let compat_info = index.and_then(|idx| {
        enabled_profiles.as_ref().map(|ep| {
            let machine_ids: Vec<String> = ep.machine.enabled.clone();
            CompatibilityInfo::from_index_entries(&machine_ids, idx)
        })
    });

    // Get machine entries for nozzle checks
    let machine_entries_data: Vec<_> = if let Some(idx) = index {
        if let Some(ref ep) = enabled_profiles {
            idx.profiles
                .iter()
                .filter(|e| {
                    e.profile_type == "machine" && ep.machine.enabled.contains(&e.id)
                })
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    let machine_entry_refs: Vec<&_> = machine_entries_data.to_vec();

    // Apply compatibility filtering unless --include-incompatible
    let compat_filtered: Vec<(&ResolvedProfile, Option<String>)> = activation_filtered
        .into_iter()
        .filter_map(|p| {
            if include_incompatible {
                // Show all, but annotate with warning markers
                let warning = get_compat_warning(p, index, &compat_info, &machine_entry_refs);
                Some((p, warning))
            } else if let Some(ref ci) = compat_info {
                if let Some(idx) = index {
                    if let Some(entry) = idx.profiles.iter().find(|e| e.name == p.name) {
                        if ci.is_compatible(entry) {
                            return Some((p, None));
                        }
                        // Also check nozzle
                        if CompatibilityInfo::check_nozzle(entry, &machine_entry_refs).is_some() {
                            return None; // Nozzle mismatch, filter out
                        }
                        return Some((p, None));
                    }
                }
                Some((p, None))
            } else {
                Some((p, None))
            }
        })
        .collect();

    if compat_filtered.is_empty() {
        eprintln!("No profiles found matching '{query}' with current filters.");
        return Ok(());
    }

    if json_output {
        let entries: Vec<serde_json::Value> = compat_filtered
            .iter()
            .map(|(p, warning)| {
                let mut obj = serde_json::json!({
                    "name": p.name,
                    "profile_type": p.profile_type,
                    "source": p.source.to_string(),
                    "path": p.path.display().to_string(),
                });
                if let Some(w) = warning {
                    obj["warning"] = serde_json::json!(w);
                }
                obj
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        println!(
            "{:<10} {:<12} {:<50} {:<15}",
            "TYPE", "VENDOR", "NAME", "SOURCE"
        );
        println!("{}", "-".repeat(91));
        for (p, warning) in &compat_filtered {
            let vendor_name = match &p.source {
                ProfileSource::Library { vendor: v } => v.as_str(),
                _ => "-",
            };
            let name_display = if let Some(w) = warning {
                format!("{} [{}]", p.name, w)
            } else {
                p.name.clone()
            };
            println!(
                "{:<10} {:<12} {:<50} {:<15}",
                p.profile_type, vendor_name, name_display, p.source,
            );
        }
        eprintln!(
            "{} result(s) (showing up to {limit})",
            compat_filtered.len()
        );
    }

    // If --enable flag, enable matched profiles
    if enable {
        let path = enabled_profiles_path(profiles_dir)?;
        let mut ep = enabled_profiles
            .clone()
            .unwrap_or_default();
        for (p, _) in &compat_filtered {
            ep.enable(&p.profile_type, &p.name);
            eprintln!("Enabled {} profile: {}", p.profile_type, p.name);
        }
        ep.save(&path)?;
    }

    Ok(())
}

/// Returns a compatibility warning string for a profile, or `None` if compatible.
fn get_compat_warning(
    p: &ResolvedProfile,
    index: Option<&ProfileIndex>,
    compat_info: &Option<CompatibilityInfo>,
    machine_entries: &[&slicecore_engine::profile_library::ProfileIndexEntry],
) -> Option<String> {
    let idx = index?;
    let entry = idx.profiles.iter().find(|e| e.name == p.name)?;

    let mut warnings = Vec::new();

    if let Some(ref ci) = compat_info {
        if !ci.is_compatible(entry) {
            warnings.push("TYPE");
        }
    }

    if let Some(slicecore_engine::enabled_profiles::CompatCheck::NozzleMismatch { .. }) =
        CompatibilityInfo::check_nozzle(entry, machine_entries)
    {
        warnings.push("NOZZLE");
    }

    if let Some(slicecore_engine::enabled_profiles::CompatCheck::TemperatureWarning { .. }) =
        CompatibilityInfo::check_temperature(None, 300.0)
    {
        warnings.push("TEMP");
    }

    if warnings.is_empty() {
        None
    } else {
        Some(warnings.join(","))
    }
}

/// Implements the `profile compat` command.
///
/// Shows detailed compatibility breakdown for a profile against enabled printers.
fn cmd_compat(
    id: &str,
    profiles_dir: Option<&Path>,
    json_output: bool,
) -> Result<(), anyhow::Error> {
    let resolver = ProfileResolver::new(profiles_dir);
    let index = resolver
        .index()
        .ok_or_else(|| anyhow::anyhow!("No profile index found. Run profile conversion first."))?;

    let entry = index
        .profiles
        .iter()
        .find(|e| e.id == id || e.name == id)
        .ok_or_else(|| anyhow::anyhow!("Profile '{id}' not found in index"))?;

    // Load enabled profiles to get machine entries
    let ep_path = enabled_profiles_path(profiles_dir)?;
    let enabled = EnabledProfiles::load(&ep_path)?
        .ok_or_else(|| anyhow::anyhow!("No enabled profiles. Run 'slicecore profile setup' first."))?;

    let machine_ids: Vec<String> = enabled.machine.enabled.clone();
    if machine_ids.is_empty() {
        anyhow::bail!("No machine profiles enabled. Enable a printer first with 'slicecore profile enable'.");
    }

    // Get machine entries from index
    let machine_entries: Vec<&_> = index
        .profiles
        .iter()
        .filter(|e| e.profile_type == "machine" && machine_ids.contains(&e.id))
        .collect();

    // Build compat info for type/vendor/ID checking
    let compat_info = CompatibilityInfo::from_index_entries(&machine_ids, index);

    // Build full compat report (nozzle + temperature checks)
    let report = CompatibilityInfo::compat_report(entry, &machine_entries, 300.0, None);

    // Check type compatibility via CompatibilityInfo
    let type_compatible = compat_info.is_compatible(entry);

    if json_output {
        let output = serde_json::json!({
            "profile_id": entry.id,
            "profile_name": entry.name,
            "profile_type": entry.profile_type,
            "type_compatible": type_compatible,
            "overall_compatible": type_compatible && report.is_compatible(),
            "compat_report": serde_json::to_value(&report)?,
            "machines_checked": machine_ids,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Compatibility report for: {}", entry.name);
        println!("  Profile ID:   {}", entry.id);
        println!("  Profile type: {}", entry.profile_type);
        if let Some(ref mat) = entry.material {
            println!("  Material:     {mat}");
        }
        if let Some(nozzle) = entry.nozzle_size {
            println!("  Nozzle size:  {nozzle}mm");
        }
        println!();
        println!("Checked against {} enabled machine(s):", machine_entries.len());
        for m in &machine_entries {
            println!("  - {} ({})", m.name, m.id);
        }
        println!();

        // Type compatibility
        let type_status = if type_compatible { "OK" } else { "MISMATCH" };
        println!("Type compatibility: {type_status}");

        // Individual checks from report
        for check in &report.checks {
            match check {
                slicecore_engine::enabled_profiles::CompatCheck::Compatible => {
                    println!("  Check: OK");
                }
                slicecore_engine::enabled_profiles::CompatCheck::NozzleMismatch {
                    profile_nozzle,
                    printer_nozzles,
                } => {
                    let nozzles_str: Vec<String> =
                        printer_nozzles.iter().map(|n| format!("{n}mm")).collect();
                    println!(
                        "  Nozzle: MISMATCH (profile: {profile_nozzle}mm, printers: {})",
                        nozzles_str.join(", ")
                    );
                }
                slicecore_engine::enabled_profiles::CompatCheck::TemperatureWarning {
                    filament_min,
                    printer_max,
                } => {
                    println!(
                        "  Temperature: WARNING (filament min: {filament_min}C, printer max: {printer_max}C)"
                    );
                }
            }
        }

        println!();
        let overall = type_compatible && report.is_compatible();
        if overall {
            println!("Overall: COMPATIBLE");
        } else {
            println!("Overall: INCOMPATIBLE");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_profile_names() {
        assert!(is_valid_profile_name("my-pla"));
        assert!(is_valid_profile_name("PLA_Basic_v2"));
        assert!(is_valid_profile_name("a"));
    }

    #[test]
    fn test_invalid_profile_names() {
        assert!(!is_valid_profile_name(""));
        assert!(!is_valid_profile_name("-starts-dash"));
        assert!(!is_valid_profile_name("has spaces"));
        assert!(!is_valid_profile_name("path/../traversal"));
        assert!(!is_valid_profile_name("has.dots"));
    }

    #[test]
    fn test_valid_name_length_boundary() {
        let name_128 = "a".repeat(128);
        assert!(is_valid_profile_name(&name_128));

        let name_129 = "a".repeat(129);
        assert!(!is_valid_profile_name(&name_129));
    }

    #[test]
    fn test_parse_toml_value_integer() {
        assert_eq!(parse_toml_value("42"), toml::Value::Integer(42));
    }

    #[test]
    fn test_parse_toml_value_float() {
        assert_eq!(parse_toml_value("1.5"), toml::Value::Float(1.5));
    }

    #[test]
    fn test_parse_toml_value_bool() {
        assert_eq!(parse_toml_value("true"), toml::Value::Boolean(true));
        assert_eq!(parse_toml_value("false"), toml::Value::Boolean(false));
    }

    #[test]
    fn test_parse_toml_value_string() {
        assert_eq!(
            parse_toml_value("hello"),
            toml::Value::String("hello".to_string())
        );
    }

    #[test]
    fn test_navigate_toml_path() {
        let doc = toml::Value::Table({
            let mut root = toml::map::Map::new();
            let mut speed = toml::map::Map::new();
            speed.insert("perimeter".to_string(), toml::Value::Integer(50));
            root.insert("speed".to_string(), toml::Value::Table(speed));
            root
        });

        let result = navigate_toml_path(&doc, "speed.perimeter");
        assert_eq!(result, Some(&toml::Value::Integer(50)));
    }

    #[test]
    fn test_navigate_toml_path_missing() {
        let doc = toml::Value::Table({
            let mut root = toml::map::Map::new();
            let mut speed = toml::map::Map::new();
            speed.insert("perimeter".to_string(), toml::Value::Integer(50));
            root.insert("speed".to_string(), toml::Value::Table(speed));
            root
        });

        let result = navigate_toml_path(&doc, "speed.nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_navigate_toml_path_mut_creates_intermediate() {
        let mut doc = toml::Value::Table(toml::map::Map::new());
        let parent = navigate_toml_path_mut(&mut doc, "a.b.c");
        let table = parent.as_table_mut().expect("should be a table");
        table.insert("c".to_string(), toml::Value::Integer(99));

        // Verify the nested structure was created
        let result = navigate_toml_path(&doc, "a.b.c");
        assert_eq!(result, Some(&toml::Value::Integer(99)));
    }
}
