//! Interactive profile setup wizard and pickers.
//!
//! Provides the first-run wizard (vendor -> printer -> filament flow),
//! interactive enable/disable pickers, and slicer detection for import suggestion.

use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use slicecore_engine::enabled_profiles::{CompatibilityInfo, EnabledProfiles};
use slicecore_engine::profile_library::ProfileIndex;
use slicecore_engine::profile_resolve::ProfileResolver;

/// Runs the interactive setup wizard.
///
/// Guides the user through vendor -> printer model -> filament selection.
/// If `reset` is `true`, starts with a fresh (empty) `EnabledProfiles`.
/// Otherwise loads any existing state so the user can add to it.
///
/// # Errors
///
/// Returns an error if profile loading/saving fails or the terminal is
/// not interactive.
pub fn run_setup_wizard(
    profiles_dir: Option<&Path>,
    reset: bool,
) -> Result<(), anyhow::Error> {
    require_tty()?;

    let mut enabled = if reset {
        EnabledProfiles::default()
    } else {
        load_or_default(profiles_dir)?
    };

    let resolver = ProfileResolver::new(profiles_dir);
    let Some(index) = resolver.index() else {
        suggest_import()?;
        return Ok(());
    };

    if index.profiles.is_empty() {
        suggest_import()?;
        return Ok(());
    }

    wizard_select_printers(index, &mut enabled)?;
    enabled = wizard_select_filaments(index, &enabled)?;
    wizard_auto_enable_process(index, &mut enabled)?;

    save_enabled(&enabled, profiles_dir)?;

    let (m, f, p) = enabled.counts();
    eprintln!(
        "Setup complete! {} machines, {} filaments, {} process profiles enabled.",
        m, f, p
    );

    Ok(())
}

/// Non-interactive setup path for CI and scripting.
///
/// Enables the specified machine, filament, and process profile IDs.
/// If `processes` is empty but `machines` is non-empty, process profiles
/// are auto-enabled for the selected machines.
///
/// # Errors
///
/// Returns an error if any profile ID cannot be resolved or if saving fails.
pub fn run_setup_noninteractive(
    machines: &[String],
    filaments: &[String],
    processes: &[String],
    profiles_dir: Option<&Path>,
    reset: bool,
) -> Result<(), anyhow::Error> {
    let mut enabled = if reset {
        EnabledProfiles::default()
    } else {
        load_or_default(profiles_dir)?
    };

    let resolver = ProfileResolver::new(profiles_dir);

    for id in machines {
        let resolved = resolver
            .resolve(id, "machine")
            .map_err(|e| anyhow::anyhow!("Machine '{id}' not found: {e}"))?;
        enabled.enable("machine", &resolved.name);
        eprintln!("Enabled machine: {}", resolved.name);
    }

    for id in filaments {
        let resolved = resolver
            .resolve(id, "filament")
            .map_err(|e| anyhow::anyhow!("Filament '{id}' not found: {e}"))?;
        enabled.enable("filament", &resolved.name);
        eprintln!("Enabled filament: {}", resolved.name);
    }

    for id in processes {
        let resolved = resolver
            .resolve(id, "process")
            .map_err(|e| anyhow::anyhow!("Process '{id}' not found: {e}"))?;
        enabled.enable("process", &resolved.name);
        eprintln!("Enabled process: {}", resolved.name);
    }

    // Auto-enable process profiles for selected machines if none specified
    if processes.is_empty() && !machines.is_empty() {
        if let Some(index) = resolver.index() {
            wizard_auto_enable_process(index, &mut enabled)?;
        }
    }

    save_enabled(&enabled, profiles_dir)?;

    let (m, f, p) = enabled.counts();
    eprintln!(
        "Setup complete! {} machines, {} filaments, {} process profiles enabled.",
        m, f, p
    );

    Ok(())
}

/// Runs an interactive picker for enabling profiles.
///
/// Shows disabled profiles and lets the user select which to enable.
/// Returns the list of profile IDs the user selected.
///
/// # Errors
///
/// Returns an error if the terminal is not interactive, or if loading fails.
pub fn run_enable_picker(
    profile_type: Option<&str>,
    profiles_dir: Option<&Path>,
) -> Result<Vec<String>, anyhow::Error> {
    require_tty()?;

    let resolver = ProfileResolver::new(profiles_dir);
    let Some(index) = resolver.index() else {
        anyhow::bail!("No profile library found. Run 'slicecore import-profiles' first.");
    };

    let enabled = load_or_default(profiles_dir)?;

    // Collect profiles not yet enabled, optionally filtered by type
    let candidates: Vec<&slicecore_engine::profile_library::ProfileIndexEntry> = index
        .profiles
        .iter()
        .filter(|e| {
            if let Some(t) = profile_type {
                if e.profile_type != t {
                    return false;
                }
            }
            !enabled.is_enabled(&e.profile_type, &e.name)
        })
        .collect();

    if candidates.is_empty() {
        eprintln!("All profiles are already enabled (or no profiles available).");
        return Ok(Vec::new());
    }

    let labels: Vec<String> = candidates
        .iter()
        .map(|e| format!("[{}] {} ({})", e.profile_type, e.name, e.vendor))
        .collect();

    let selections = dialoguer::MultiSelect::new()
        .with_prompt("Select profiles to enable (Space to toggle, Enter to confirm)")
        .items(&labels)
        .interact()?;

    let selected: Vec<String> = selections
        .into_iter()
        .map(|i| candidates[i].name.clone())
        .collect();

    Ok(selected)
}

/// Runs an interactive picker for disabling profiles.
///
/// Shows currently enabled profiles and lets the user select which to disable.
/// Returns the list of profile IDs the user selected.
///
/// # Errors
///
/// Returns an error if the terminal is not interactive, or if loading fails.
pub fn run_disable_picker(
    profile_type: Option<&str>,
    profiles_dir: Option<&Path>,
) -> Result<Vec<String>, anyhow::Error> {
    require_tty()?;

    let enabled = load_or_default(profiles_dir)?;
    let all = enabled.all_enabled();

    let filtered: Vec<(&str, &str)> = all
        .into_iter()
        .filter(|(t, _)| {
            if let Some(filter) = profile_type {
                *t == filter
            } else {
                true
            }
        })
        .collect();

    if filtered.is_empty() {
        eprintln!("No profiles are currently enabled.");
        return Ok(Vec::new());
    }

    let labels: Vec<String> = filtered
        .iter()
        .map(|(t, id)| format!("[{}] {}", t, id))
        .collect();

    let selections = dialoguer::MultiSelect::new()
        .with_prompt("Select profiles to disable (Space to toggle, Enter to confirm)")
        .items(&labels)
        .interact()?;

    let selected: Vec<String> = selections
        .into_iter()
        .map(|i| filtered[i].1.to_string())
        .collect();

    Ok(selected)
}

// ---------------------------------------------------------------------------
// Internal wizard steps
// ---------------------------------------------------------------------------

/// Guides the user through selecting printer vendors and models.
fn wizard_select_printers(
    index: &ProfileIndex,
    enabled: &mut EnabledProfiles,
) -> Result<(), anyhow::Error> {
    let machines: Vec<&slicecore_engine::profile_library::ProfileIndexEntry> = index
        .profiles
        .iter()
        .filter(|e| e.profile_type == "machine")
        .collect();

    if machines.is_empty() {
        eprintln!("No machine profiles found in the library.");
        return Ok(());
    }

    // Extract unique vendors
    let mut vendors: Vec<String> = machines
        .iter()
        .map(|e| e.vendor.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    vendors.sort();

    let vendor_selections = dialoguer::MultiSelect::new()
        .with_prompt("Select printer vendor(s)")
        .items(&vendors)
        .interact()?;

    if vendor_selections.is_empty() {
        eprintln!("No vendors selected. Skipping printer selection.");
        return Ok(());
    }

    let selected_vendors: Vec<&str> = vendor_selections
        .iter()
        .map(|&i| vendors[i].as_str())
        .collect();

    // For each selected vendor, choose printers
    for vendor in &selected_vendors {
        let vendor_machines: Vec<&slicecore_engine::profile_library::ProfileIndexEntry> = machines
            .iter()
            .filter(|e| e.vendor == *vendor)
            .copied()
            .collect();

        let machine_labels: Vec<&str> = vendor_machines.iter().map(|e| e.name.as_str()).collect();

        // Pre-select already-enabled machines
        let defaults: Vec<bool> = vendor_machines
            .iter()
            .map(|e| enabled.is_enabled("machine", &e.name))
            .collect();

        let machine_selections = dialoguer::MultiSelect::new()
            .with_prompt(format!("Select {} printer(s)", vendor))
            .items(&machine_labels)
            .defaults(&defaults)
            .interact()?;

        for &i in &machine_selections {
            enabled.enable("machine", &vendor_machines[i].name);
        }
    }

    Ok(())
}

/// Guides the user through selecting filaments compatible with enabled printers.
fn wizard_select_filaments(
    index: &ProfileIndex,
    enabled: &EnabledProfiles,
) -> Result<EnabledProfiles, anyhow::Error> {
    let mut enabled = enabled.clone();
    let machine_ids: Vec<String> = enabled.machine.enabled.clone();

    let compat = CompatibilityInfo::from_index_entries(&machine_ids, index);

    let filaments: Vec<&slicecore_engine::profile_library::ProfileIndexEntry> = index
        .profiles
        .iter()
        .filter(|e| e.profile_type == "filament")
        .collect();

    if filaments.is_empty() {
        eprintln!("No filament profiles found in the library.");
        return Ok(enabled);
    }

    // Filter to compatible filaments
    let compatible: Vec<&slicecore_engine::profile_library::ProfileIndexEntry> = filaments
        .iter()
        .filter(|e| compat.is_compatible(e))
        .copied()
        .collect();

    let display_filaments = if compatible.is_empty() {
        eprintln!("No compatible filaments found. Showing all filaments.");
        &filaments
    } else {
        &compatible
    };

    let labels: Vec<String> = display_filaments
        .iter()
        .map(|e| {
            let material = e
                .material
                .as_deref()
                .unwrap_or("unknown");
            format!("{} ({}, {})", e.name, material, e.vendor)
        })
        .collect();

    // Pre-select all compatible filaments (user can deselect)
    let defaults: Vec<bool> = display_filaments
        .iter()
        .map(|e| {
            enabled.is_enabled("filament", &e.name) || compat.is_compatible(e)
        })
        .collect();

    let selections = dialoguer::MultiSelect::new()
        .with_prompt("Select filaments (Enter to accept all compatible)")
        .items(&labels)
        .defaults(&defaults)
        .interact()?;

    // Clear previously auto-selected filaments, then add user's choices
    for e in display_filaments {
        enabled.disable("filament", &e.name);
    }
    for &i in &selections {
        enabled.enable("filament", &display_filaments[i].name);
    }

    Ok(enabled)
}

/// Auto-enables process profiles matching selected printers.
fn wizard_auto_enable_process(
    index: &ProfileIndex,
    enabled: &mut EnabledProfiles,
) -> Result<(), anyhow::Error> {
    let machine_names: HashSet<String> = enabled
        .machine
        .enabled
        .iter()
        .cloned()
        .collect();

    // Collect vendor names from enabled machines in the index
    let machine_vendors: HashSet<&str> = index
        .profiles
        .iter()
        .filter(|e| e.profile_type == "machine" && machine_names.contains(&e.name))
        .map(|e| e.vendor.as_str())
        .collect();

    let process_entries: Vec<&slicecore_engine::profile_library::ProfileIndexEntry> = index
        .profiles
        .iter()
        .filter(|e| e.profile_type == "process")
        .collect();

    let mut count = 0usize;

    for entry in &process_entries {
        // Match by printer_model containing a machine name or by vendor
        let matches = if let Some(ref model) = entry.printer_model {
            machine_names.iter().any(|m| model.contains(m.as_str()))
        } else {
            machine_vendors.contains(entry.vendor.as_str())
        };

        if matches {
            enabled.enable("process", &entry.name);
            count += 1;
        }
    }

    // If no matches, enable generic/vendor-neutral process profiles
    if count == 0 {
        for entry in &process_entries {
            if entry.vendor.to_lowercase() == "generic"
                || entry.printer_model.is_none()
            {
                enabled.enable("process", &entry.name);
                count += 1;
            }
        }
    }

    eprintln!("Auto-enabled {count} process profiles for your printers.");

    Ok(())
}

/// Suggests running `import-profiles` when no profile library is found.
fn suggest_import() -> Result<(), anyhow::Error> {
    let slicers = detect_installed_slicers();

    if let Some((name, path)) = slicers.first() {
        eprintln!(
            "Found {} at {}. Run:\n  slicecore import-profiles --source-dir {}",
            name,
            path.display(),
            path.display()
        );
    } else {
        eprintln!(
            "No profile library found. Import profiles first:\n  \
             slicecore import-profiles --source-dir /path/to/OrcaSlicer/resources/profiles"
        );
    }

    Ok(())
}

/// Detects installed slicer profile directories.
fn detect_installed_slicers() -> Vec<(&'static str, PathBuf)> {
    let mut found = Vec::new();

    let home = match home::home_dir() {
        Some(h) => h,
        None => return found,
    };

    // Common paths per platform
    let candidates: Vec<(&str, PathBuf)> = if cfg!(target_os = "macos") {
        vec![
            (
                "OrcaSlicer",
                home.join("Library/Application Support/OrcaSlicer/system"),
            ),
            (
                "PrusaSlicer",
                home.join("Library/Application Support/PrusaSlicer/vendor"),
            ),
            (
                "BambuStudio",
                home.join("Library/Application Support/BambuStudio/system"),
            ),
        ]
    } else {
        // Linux
        vec![
            ("OrcaSlicer", home.join(".config/OrcaSlicer/system")),
            ("PrusaSlicer", home.join(".config/PrusaSlicer/vendor")),
            ("BambuStudio", home.join(".config/BambuStudio/system")),
        ]
    };

    for (name, path) in candidates {
        if path.exists() {
            found.push((name, path));
        }
    }

    found
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Ensures stdin is a terminal; returns an error if not.
fn require_tty() -> Result<(), anyhow::Error> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "Interactive mode requires a terminal. \
             Use --machine and --filament flags for non-interactive setup."
        );
    }
    Ok(())
}

/// Loads existing `EnabledProfiles` or returns a default.
fn load_or_default(profiles_dir: Option<&Path>) -> Result<EnabledProfiles, anyhow::Error> {
    let path = enabled_profiles_path(profiles_dir)?;
    Ok(EnabledProfiles::load(&path)?.unwrap_or_default())
}

/// Resolves the path for the enabled-profiles file.
fn enabled_profiles_path(profiles_dir: Option<&Path>) -> Result<PathBuf, anyhow::Error> {
    if let Some(dir) = profiles_dir {
        Ok(dir.join("enabled-profiles.toml"))
    } else {
        EnabledProfiles::default_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))
    }
}

/// Saves `EnabledProfiles` to the appropriate path.
fn save_enabled(
    enabled: &EnabledProfiles,
    profiles_dir: Option<&Path>,
) -> Result<(), anyhow::Error> {
    let path = enabled_profiles_path(profiles_dir)?;
    enabled.save(&path)?;
    Ok(())
}
