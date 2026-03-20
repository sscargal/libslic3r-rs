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
//! - `set`: Set a single setting value
//! - `get`: Get a single setting value
//! - `reset`: Reset a setting to its inherited value
//! - `edit`: Open profile in `$EDITOR`
//! - `validate`: Validate profile against schema
//! - `delete`: Delete a custom profile
//! - `rename`: Rename a custom profile

use std::path::{Path, PathBuf};

use clap::Subcommand;
use slicecore_engine::config::PrintConfig;
use slicecore_engine::profile_resolve::{ProfileError, ProfileResolver, ResolvedProfile};

/// Profile management subcommands.
#[derive(Subcommand)]
pub enum ProfileCommand {
    /// Create a custom profile by cloning an existing preset.
    ///
    /// Copies the source profile to ~/.slicecore/profiles/{type}/ with a
    /// [metadata] section recording the clone lineage.
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
    Set {
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

    /// List profiles from the profile library.
    ///
    /// Alias for the top-level `list-profiles` command, available under the
    /// `profile` command group for convenience.
    List {
        /// Filter by vendor name (e.g., BBL, Creality, Prusa)
        #[arg(long)]
        vendor: Option<String>,

        /// Filter by profile type (filament, process, machine)
        #[arg(long, value_name = "TYPE")]
        profile_type: Option<String>,

        /// Filter by material type (PLA, ABS, PETG, TPU, etc.)
        #[arg(long)]
        material: Option<String>,

        /// List available vendors only (no individual profiles)
        #[arg(long)]
        vendors: bool,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
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

    /// Search profiles by keyword.
    ///
    /// Alias for the top-level `search-profiles` command.
    Search {
        /// Search query
        query: String,

        /// Maximum number of results
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Override profiles directory
        #[arg(long)]
        profiles_dir: Option<PathBuf>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Compare two print profiles side by side.
    ///
    /// Alias for the top-level `diff-profiles` command.
    Diff(crate::diff_profiles_command::DiffProfilesArgs),
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
        ProfileCommand::Set { .. } => {
            anyhow::bail!("profile set not yet implemented")
        }
        ProfileCommand::Get { .. } => {
            anyhow::bail!("profile get not yet implemented")
        }
        ProfileCommand::Reset { .. } => {
            anyhow::bail!("profile reset not yet implemented")
        }
        ProfileCommand::Edit { .. } => {
            anyhow::bail!("profile edit not yet implemented")
        }
        ProfileCommand::Validate { .. } => {
            anyhow::bail!("profile validate not yet implemented")
        }
        ProfileCommand::Delete { .. } => {
            anyhow::bail!("profile delete not yet implemented")
        }
        ProfileCommand::Rename { .. } => {
            anyhow::bail!("profile rename not yet implemented")
        }
        ProfileCommand::List { .. } => {
            anyhow::bail!("profile list not yet implemented (use top-level list-profiles)")
        }
        ProfileCommand::Show { .. } => {
            anyhow::bail!("profile show not yet implemented (use top-level show-profile)")
        }
        ProfileCommand::Search { .. } => {
            anyhow::bail!("profile search not yet implemented (use top-level search-profiles)")
        }
        ProfileCommand::Diff(_) => {
            anyhow::bail!("profile diff not yet implemented (use top-level diff-profiles)")
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
    let home = home::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
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
    println!("Created custom profile '{}' at {}", new_name, dest.display());
    println!("\nNext steps:");
    println!("  slicecore profile show {new_name}");
    println!("  slicecore profile set {new_name} <key> <value>");
    println!("  slicecore profile edit {new_name}");

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
}
