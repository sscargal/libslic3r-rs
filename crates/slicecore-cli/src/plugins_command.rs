//! Plugin management CLI subcommands.
//!
//! Provides the `slicecore plugins` command with subcommands for listing,
//! enabling, disabling, inspecting, and validating installed plugins.

use std::path::Path;

use clap::Subcommand;
use comfy_table::presets::UTF8_FULL;
use comfy_table::Table;

use slicecore_plugin::discovery::{self, DiscoveredPlugin};
use slicecore_plugin::status::{self, PluginStatus};
use slicecore_plugin::PluginRegistry;
use slicecore_plugin_api::{PluginCapability, PluginType};

/// Plugin management subcommands.
#[derive(Subcommand)]
pub enum PluginsCommand {
    /// List installed plugins
    List {
        /// Output as JSON instead of a table
        #[arg(long)]
        json: bool,

        /// Filter by category ("infill" or "postprocessor")
        #[arg(long)]
        category: Option<String>,

        /// Filter by status ("enabled", "disabled", or "error")
        #[arg(long)]
        status: Option<String>,
    },

    /// Enable a plugin
    Enable {
        /// Plugin name
        name: String,
    },

    /// Disable a plugin
    Disable {
        /// Plugin name
        name: String,
    },

    /// Show detailed plugin information
    Info {
        /// Plugin name
        name: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Validate a plugin (health check)
    Validate {
        /// Plugin name
        name: String,
    },
}

/// Serializable entry for JSON list output.
#[derive(serde::Serialize)]
struct PluginListEntry {
    name: String,
    version: String,
    plugin_type: String,
    category: String,
    status: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Runs a plugin management subcommand.
///
/// # Errors
///
/// Returns an error if plugin discovery, status updates, or validation fails.
pub fn run_plugins(cmd: PluginsCommand, plugin_dir: &Path) -> Result<(), anyhow::Error> {
    match cmd {
        PluginsCommand::List {
            json,
            category,
            status,
        } => cmd_list(plugin_dir, json, category.as_deref(), status.as_deref()),
        PluginsCommand::Enable { name } => cmd_enable(plugin_dir, &name),
        PluginsCommand::Disable { name } => cmd_disable(plugin_dir, &name),
        PluginsCommand::Info { name, json } => cmd_info(plugin_dir, &name, json),
        PluginsCommand::Validate { name } => cmd_validate(plugin_dir, &name),
    }
}

/// Lists installed plugins with optional filtering.
fn cmd_list(
    plugin_dir: &Path,
    json: bool,
    category: Option<&str>,
    status_filter: Option<&str>,
) -> Result<(), anyhow::Error> {
    let plugins = discovery::discover_all_with_status(plugin_dir)?;

    let entries: Vec<PluginListEntry> = plugins
        .iter()
        .map(to_list_entry)
        .filter(|e| match category {
            Some("infill") => e.category == "infill_pattern",
            Some("postprocessor") => e.category == "gcode_post_processor",
            Some(_) => false,
            None => true,
        })
        .filter(|e| match status_filter {
            Some("enabled") => e.status == "Enabled",
            Some("disabled") => e.status == "Disabled",
            Some("error") => e.status.starts_with("Error"),
            Some(_) => false,
            None => true,
        })
        .collect();

    if json {
        let output = serde_json::to_string_pretty(&entries)?;
        println!("{output}");
    } else {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec!["Name", "Version", "Type", "Category", "Status"]);
        for entry in &entries {
            table.add_row(vec![
                &entry.name,
                &entry.version,
                &entry.plugin_type,
                &entry.category,
                &entry.status,
            ]);
        }
        println!("{table}");
    }

    Ok(())
}

/// Converts a `DiscoveredPlugin` to a `PluginListEntry` for display.
fn to_list_entry(plugin: &DiscoveredPlugin) -> PluginListEntry {
    let (name, version, plugin_type, category) = match &plugin.manifest {
        Some(m) => {
            let name = m.metadata.name.clone();
            let version = m.metadata.version.clone();
            let ptype = match m.plugin_type {
                PluginType::Native => "native".to_string(),
                PluginType::Wasm => "wasm".to_string(),
            };
            let cat = m
                .capabilities
                .first()
                .map(|c| match c {
                    PluginCapability::InfillPattern => "infill_pattern".to_string(),
                    PluginCapability::GcodePostProcessor => "gcode_post_processor".to_string(),
                })
                .unwrap_or_else(|| "???".to_string());
            (name, version, ptype, cat)
        }
        None => {
            let dir_name = plugin
                .dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "???".to_string());
            (
                dir_name,
                "???".to_string(),
                "???".to_string(),
                "???".to_string(),
            )
        }
    };

    let status_str = if plugin.error.is_some() {
        let brief = plugin
            .error
            .as_deref()
            .unwrap_or("unknown")
            .chars()
            .take(60)
            .collect::<String>();
        format!("Error: {brief}")
    } else if plugin.status.enabled {
        "Enabled".to_string()
    } else {
        "Disabled".to_string()
    };

    PluginListEntry {
        name,
        version,
        plugin_type,
        category,
        status: status_str,
        path: plugin.dir.display().to_string(),
        error: plugin.error.clone(),
    }
}

/// Enables a plugin after validation.
fn cmd_enable(plugin_dir: &Path, name: &str) -> Result<(), anyhow::Error> {
    let plugins = discovery::discover_all_with_status(plugin_dir)?;
    let plugin = find_plugin_by_name(&plugins, name)
        .ok_or_else(|| anyhow::anyhow!("Plugin '{name}' not found in {}", plugin_dir.display()))?;

    if plugin.status.enabled {
        println!("Plugin '{name}' is already enabled");
        return Ok(());
    }

    // Validate: check that manifest is parseable and version is compatible.
    // discover_plugins performs both parse + version validation and returns
    // only valid plugins (errors propagate immediately).
    match discovery::discover_plugins(plugin_dir) {
        Ok(valid_plugins) => {
            let found = valid_plugins.iter().any(|(dir, _)| *dir == plugin.dir);
            if !found {
                // Plugin exists in discover_all_with_status but not in
                // discover_plugins -- means it has a validation error.
                let err_msg = plugin
                    .error
                    .as_deref()
                    .unwrap_or("unknown validation error");
                anyhow::bail!("Plugin '{name}' failed validation: {err_msg}. Remains disabled.");
            }
        }
        Err(e) => {
            anyhow::bail!("Plugin '{name}' failed validation: {e}. Remains disabled.");
        }
    }

    status::write_status(&plugin.dir, &PluginStatus { enabled: true })?;
    println!("Plugin '{name}' enabled successfully");
    Ok(())
}

/// Disables a plugin after verifying its manifest is parseable.
fn cmd_disable(plugin_dir: &Path, name: &str) -> Result<(), anyhow::Error> {
    let plugins = discovery::discover_all_with_status(plugin_dir)?;
    let plugin = find_plugin_by_name(&plugins, name)
        .ok_or_else(|| anyhow::anyhow!("Plugin '{name}' not found in {}", plugin_dir.display()))?;

    // Validate plugin identity: manifest must be parseable
    if plugin.manifest.is_none() {
        anyhow::bail!(
            "Plugin '{name}' has an invalid manifest and cannot be managed. \
             Run `slicecore plugins validate {name}` for details."
        );
    }

    if !plugin.status.enabled {
        println!("Plugin '{name}' is already disabled");
        return Ok(());
    }

    status::write_status(&plugin.dir, &PluginStatus { enabled: false })?;
    println!("Plugin '{name}' disabled");
    Ok(())
}

/// Shows detailed information about a plugin.
fn cmd_info(plugin_dir: &Path, name: &str, json: bool) -> Result<(), anyhow::Error> {
    let plugins = discovery::discover_all_with_status(plugin_dir)?;
    let plugin = find_plugin_by_name(&plugins, name)
        .ok_or_else(|| anyhow::anyhow!("Plugin '{name}' not found in {}", plugin_dir.display()))?;

    if json {
        let info = build_info_json(plugin);
        let output = serde_json::to_string_pretty(&info)?;
        println!("{output}");
    } else {
        print_info_text(plugin);
    }

    Ok(())
}

/// Validates a plugin by checking manifest, version, and attempting to load.
fn cmd_validate(plugin_dir: &Path, name: &str) -> Result<(), anyhow::Error> {
    let plugins = discovery::discover_all_with_status(plugin_dir)?;
    let plugin = find_plugin_by_name(&plugins, name)
        .ok_or_else(|| anyhow::anyhow!("Plugin '{name}' not found in {}", plugin_dir.display()))?;

    let mut all_ok = true;

    // 1. Check manifest parse
    match &plugin.manifest {
        Some(m) => {
            println!("Manifest:    OK");

            // 2. Check version (if there was a version error, it's in error field)
            if let Some(err) = &plugin.error {
                println!("API Version: FAIL ({err})");
                all_ok = false;
            } else {
                println!(
                    "API Version: OK ({} - {})",
                    m.metadata.min_api_version, m.metadata.max_api_version
                );
            }
        }
        None => {
            let err_msg = plugin.error.as_deref().unwrap_or("unknown parse error");
            println!("Manifest:    FAIL ({err_msg})");
            all_ok = false;
        }
    }

    // 3. Attempt load test via discover_and_load on the parent plugin_dir.
    // This validates that the plugin can be fully loaded (manifest + library).
    let mut registry = PluginRegistry::new();
    match registry.discover_and_load(plugin_dir) {
        Ok(loaded) => {
            let found = loaded.iter().any(|info| info.name == name);
            if found {
                println!("Load Test:   OK");
            } else {
                println!("Load Test:   FAIL (plugin not in loaded set)");
                all_ok = false;
            }
        }
        Err(e) => {
            println!("Load Test:   FAIL ({e})");
            all_ok = false;
        }
    }

    if all_ok {
        Ok(())
    } else {
        anyhow::bail!("Validation failed for plugin '{name}'")
    }
}

/// Finds a plugin by name (matching manifest name or directory name).
fn find_plugin_by_name<'a>(
    plugins: &'a [DiscoveredPlugin],
    name: &str,
) -> Option<&'a DiscoveredPlugin> {
    plugins.iter().find(|p| {
        if let Some(ref m) = p.manifest {
            if m.metadata.name == name {
                return true;
            }
        }
        // Fall back to directory name
        p.dir
            .file_name()
            .map(|n| n.to_string_lossy() == name)
            .unwrap_or(false)
    })
}

/// Builds a JSON-serializable info object for a plugin.
fn build_info_json(plugin: &DiscoveredPlugin) -> serde_json::Value {
    match &plugin.manifest {
        Some(m) => {
            let caps: Vec<String> = m
                .capabilities
                .iter()
                .map(|c| match c {
                    PluginCapability::InfillPattern => "infill_pattern".to_string(),
                    PluginCapability::GcodePostProcessor => "gcode_post_processor".to_string(),
                })
                .collect();
            let ptype = match m.plugin_type {
                PluginType::Native => "native",
                PluginType::Wasm => "wasm",
            };
            let status_str = if plugin.status.enabled {
                "Enabled"
            } else {
                "Disabled"
            };
            serde_json::json!({
                "name": m.metadata.name,
                "version": m.metadata.version,
                "description": m.metadata.description,
                "author": m.metadata.author,
                "license": m.metadata.license,
                "plugin_type": ptype,
                "capabilities": caps,
                "status": status_str,
                "path": plugin.dir.display().to_string(),
                "min_api_version": m.metadata.min_api_version,
                "max_api_version": m.metadata.max_api_version,
                "library_filename": m.library_filename,
                "error": plugin.error,
            })
        }
        None => {
            let dir_name = plugin
                .dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "???".to_string());
            serde_json::json!({
                "name": dir_name,
                "status": "Error",
                "path": plugin.dir.display().to_string(),
                "error": plugin.error,
            })
        }
    }
}

/// Prints plugin info as labeled text fields.
fn print_info_text(plugin: &DiscoveredPlugin) {
    match &plugin.manifest {
        Some(m) => {
            let ptype = match m.plugin_type {
                PluginType::Native => "native",
                PluginType::Wasm => "wasm",
            };
            let caps: Vec<&str> = m
                .capabilities
                .iter()
                .map(|c| match c {
                    PluginCapability::InfillPattern => "infill_pattern",
                    PluginCapability::GcodePostProcessor => "gcode_post_processor",
                })
                .collect();
            let status_str = if plugin.status.enabled {
                "Enabled"
            } else {
                "Disabled"
            };
            println!("Name:           {}", m.metadata.name);
            println!("Version:        {}", m.metadata.version);
            println!("Description:    {}", m.metadata.description);
            println!("Author:         {}", m.metadata.author);
            println!("License:        {}", m.metadata.license);
            println!("Type:           {ptype}");
            println!("Capabilities:   {}", caps.join(", "));
            println!("Status:         {status_str}");
            println!("Path:           {}", plugin.dir.display());
            println!(
                "API Version:    {} - {}",
                m.metadata.min_api_version, m.metadata.max_api_version
            );
            println!("Library:        {}", m.library_filename);
            if let Some(err) = &plugin.error {
                println!("Error:          {err}");
            }
        }
        None => {
            let dir_name = plugin
                .dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "???".to_string());
            println!("Name:           {dir_name}");
            println!("Status:         Error");
            println!("Path:           {}", plugin.dir.display());
            if let Some(err) = &plugin.error {
                println!("Error:          {err}");
            }
        }
    }
}
