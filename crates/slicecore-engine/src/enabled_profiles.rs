//! Profile activation state management.
//!
//! Manages which printer, filament, and process profiles are enabled (active)
//! for the current workspace. Persists state to `enabled-profiles.toml` with
//! typed `[machine]`, `[filament]`, and `[process]` sections.
//!
//! # File Format
//!
//! ```toml
//! [machine]
//! enabled = ["BBL/Bambu_X1C", "BBL/Bambu_A1"]
//!
//! [filament]
//! enabled = ["Bambu_PLA_Basic", "Bambu_PETG_Basic"]
//!
//! [process]
//! enabled = ["0.20mm_Standard"]
//! ```
//!
//! # First-Run Detection
//!
//! [`EnabledProfiles::load`] returns `Ok(None)` when the file does not exist,
//! distinguishing a first-run scenario (no file yet) from a corrupt file
//! (parse error).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::profile_library::{ProfileIndex, ProfileIndexEntry};

/// A section of enabled profile IDs for a single profile type.
///
/// Each section holds a flat list of profile identifiers that the user
/// has activated.
///
/// # Examples
///
/// ```
/// use slicecore_engine::enabled_profiles::ProfileSection;
///
/// let section = ProfileSection::default();
/// assert!(section.enabled.is_empty());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProfileSection {
    /// List of enabled profile identifiers.
    #[serde(default)]
    pub enabled: Vec<String>,
}

/// A named profile set: machine + filament + process triple.
///
/// Represents a complete configuration for slicing, combining one printer,
/// one filament, and one process profile.
///
/// # Examples
///
/// ```
/// use slicecore_engine::enabled_profiles::ProfileSet;
///
/// let set = ProfileSet {
///     machine: "BBL/Bambu_X1C".to_string(),
///     filament: "Bambu_PLA_Basic".to_string(),
///     process: "0.20mm_Standard".to_string(),
/// };
/// assert_eq!(set.machine, "BBL/Bambu_X1C");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProfileSet {
    /// Machine (printer) profile identifier.
    pub machine: String,
    /// Filament profile identifier.
    pub filament: String,
    /// Process (print settings) profile identifier.
    pub process: String,
}

/// Defaults section for enabled-profiles.toml.
///
/// Stores which profile set (if any) should be used as the default
/// for slicing operations.
///
/// # Examples
///
/// ```
/// use slicecore_engine::enabled_profiles::DefaultsSection;
///
/// let defaults = DefaultsSection { set: Some("my-set".to_string()) };
/// assert_eq!(defaults.set, Some("my-set".to_string()));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DefaultsSection {
    /// Name of the default profile set, if configured.
    #[serde(default)]
    pub set: Option<String>,
}

/// Tracks which profiles are enabled across machine, filament, and process types.
///
/// This is the primary data structure for profile activation. It serializes
/// to/from TOML with `[machine]`, `[filament]`, and `[process]` sections.
///
/// # Examples
///
/// ```
/// use slicecore_engine::enabled_profiles::EnabledProfiles;
///
/// let mut ep = EnabledProfiles::default();
/// ep.enable("machine", "BBL/Bambu_X1C");
/// assert!(ep.is_enabled("machine", "BBL/Bambu_X1C"));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct EnabledProfiles {
    /// Enabled machine (printer) profiles.
    #[serde(default)]
    pub machine: ProfileSection,
    /// Enabled filament profiles.
    #[serde(default)]
    pub filament: ProfileSection,
    /// Enabled process (print settings) profiles.
    #[serde(default)]
    pub process: ProfileSection,
    /// Named profile sets (machine + filament + process triples).
    #[serde(default)]
    pub sets: HashMap<String, ProfileSet>,
    /// Default profile set configuration.
    #[serde(default)]
    pub defaults: DefaultsSection,
}

impl EnabledProfiles {
    /// Returns the default path for the enabled-profiles configuration file.
    ///
    /// The file is stored at `~/.slicecore/enabled-profiles.toml`.
    /// Returns `None` on platforms where no home directory is available
    /// (e.g., WASM).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use slicecore_engine::enabled_profiles::EnabledProfiles;
    ///
    /// if let Some(path) = EnabledProfiles::default_path() {
    ///     println!("Config at: {}", path.display());
    /// }
    /// ```
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            home_dir().map(|h| h.join(".slicecore").join("enabled-profiles.toml"))
        }
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }

    /// Loads enabled profiles from a TOML file.
    ///
    /// Returns `Ok(None)` if the file does not exist (first-run scenario).
    /// Returns `Ok(Some(..))` on successful parse.
    /// Returns `Err(..)` if the file exists but cannot be parsed.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ConfigError`] if the file exists but contains
    /// invalid TOML.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use slicecore_engine::enabled_profiles::EnabledProfiles;
    /// use std::path::Path;
    ///
    /// let result = EnabledProfiles::load(Path::new("/tmp/enabled-profiles.toml"));
    /// ```
    pub fn load(path: &Path) -> Result<Option<Self>, EngineError> {
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path).map_err(|e| {
            EngineError::ConfigError(format!(
                "failed to read enabled-profiles from '{}': {e}",
                path.display()
            ))
        })?;
        let profiles: Self = toml::from_str(&content).map_err(|e| {
            EngineError::ConfigError(format!(
                "failed to parse enabled-profiles from '{}': {e}",
                path.display()
            ))
        })?;
        Ok(Some(profiles))
    }

    /// Saves enabled profiles to a TOML file.
    ///
    /// Creates parent directories if they do not exist.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::ConfigError`] if serialization or I/O fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use slicecore_engine::enabled_profiles::EnabledProfiles;
    /// use std::path::Path;
    ///
    /// let ep = EnabledProfiles::default();
    /// ep.save(Path::new("/tmp/enabled-profiles.toml")).unwrap();
    /// ```
    pub fn save(&self, path: &Path) -> Result<(), EngineError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                EngineError::ConfigError(format!(
                    "failed to create directory '{}': {e}",
                    parent.display()
                ))
            })?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| {
            EngineError::ConfigError(format!("failed to serialize enabled-profiles: {e}"))
        })?;
        std::fs::write(path, content).map_err(|e| {
            EngineError::ConfigError(format!(
                "failed to write enabled-profiles to '{}': {e}",
                path.display()
            ))
        })?;
        Ok(())
    }

    /// Checks whether a profile is enabled for the given type.
    ///
    /// Returns `false` for unknown profile types.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::enabled_profiles::EnabledProfiles;
    ///
    /// let mut ep = EnabledProfiles::default();
    /// ep.enable("machine", "BBL/Bambu_X1C");
    /// assert!(ep.is_enabled("machine", "BBL/Bambu_X1C"));
    /// assert!(!ep.is_enabled("machine", "Other"));
    /// assert!(!ep.is_enabled("unknown_type", "any"));
    /// ```
    #[must_use]
    pub fn is_enabled(&self, profile_type: &str, id: &str) -> bool {
        let section = match profile_type {
            "machine" => &self.machine,
            "filament" => &self.filament,
            "process" => &self.process,
            _ => return false,
        };
        section.enabled.iter().any(|e| e == id)
    }

    /// Enables a profile by adding its ID to the appropriate section.
    ///
    /// No-op if the profile is already enabled (prevents duplicates).
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::enabled_profiles::EnabledProfiles;
    ///
    /// let mut ep = EnabledProfiles::default();
    /// ep.enable("filament", "PLA_Basic");
    /// ep.enable("filament", "PLA_Basic"); // no duplicate
    /// assert_eq!(ep.filament.enabled.len(), 1);
    /// ```
    pub fn enable(&mut self, profile_type: &str, id: &str) {
        let section = match profile_type {
            "machine" => &mut self.machine,
            "filament" => &mut self.filament,
            "process" => &mut self.process,
            _ => return,
        };
        if !section.enabled.iter().any(|e| e == id) {
            section.enabled.push(id.to_string());
        }
    }

    /// Disables a profile by removing its ID from the appropriate section.
    ///
    /// No-op if the profile is not currently enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::enabled_profiles::EnabledProfiles;
    ///
    /// let mut ep = EnabledProfiles::default();
    /// ep.enable("filament", "PLA_Basic");
    /// ep.disable("filament", "PLA_Basic");
    /// assert!(!ep.is_enabled("filament", "PLA_Basic"));
    /// ```
    pub fn disable(&mut self, profile_type: &str, id: &str) {
        let section = match profile_type {
            "machine" => &mut self.machine,
            "filament" => &mut self.filament,
            "process" => &mut self.process,
            _ => return,
        };
        section.enabled.retain(|e| e != id);
    }

    /// Returns all enabled profile IDs as `(type, id)` pairs.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::enabled_profiles::EnabledProfiles;
    ///
    /// let mut ep = EnabledProfiles::default();
    /// ep.enable("machine", "X1C");
    /// ep.enable("filament", "PLA");
    /// let all = ep.all_enabled();
    /// assert_eq!(all.len(), 2);
    /// ```
    #[must_use]
    pub fn all_enabled(&self) -> Vec<(&str, &str)> {
        let mut result = Vec::new();
        for id in &self.machine.enabled {
            result.push(("machine", id.as_str()));
        }
        for id in &self.filament.enabled {
            result.push(("filament", id.as_str()));
        }
        for id in &self.process.enabled {
            result.push(("process", id.as_str()));
        }
        result
    }

    /// Returns the count of enabled profiles per type.
    ///
    /// Returns `(machine_count, filament_count, process_count)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::enabled_profiles::EnabledProfiles;
    ///
    /// let mut ep = EnabledProfiles::default();
    /// ep.enable("machine", "X1C");
    /// ep.enable("filament", "PLA");
    /// ep.enable("filament", "PETG");
    /// assert_eq!(ep.counts(), (1, 2, 0));
    /// ```
    #[must_use]
    pub fn counts(&self) -> (usize, usize, usize) {
        (
            self.machine.enabled.len(),
            self.filament.enabled.len(),
            self.process.enabled.len(),
        )
    }

    /// Adds a named profile set.
    ///
    /// Inserts the set into the `sets` map, replacing any existing set with
    /// the same name.
    pub fn add_set(&mut self, name: String, set: ProfileSet) {
        self.sets.insert(name, set);
    }

    /// Removes a named profile set.
    ///
    /// If the removed set is also the default, the default is cleared.
    /// Returns the removed set if it existed.
    pub fn remove_set(&mut self, name: &str) -> Option<ProfileSet> {
        let removed = self.sets.remove(name);
        if self.defaults.set.as_deref() == Some(name) {
            self.defaults.set = None;
        }
        removed
    }

    /// Returns a reference to the named profile set, if it exists.
    #[must_use]
    pub fn get_set(&self, name: &str) -> Option<&ProfileSet> {
        self.sets.get(name)
    }

    /// Sets the default profile set name.
    ///
    /// If `name` is `Some`, validates that the set exists in the `sets` map.
    /// If the set does not exist, the default is not changed.
    pub fn set_default(&mut self, name: Option<String>) {
        match &name {
            Some(n) if !self.sets.contains_key(n.as_str()) => {}
            _ => self.defaults.set = name,
        }
    }

    /// Returns the default profile set name and value, if configured.
    #[must_use]
    pub fn default_set(&self) -> Option<(&str, &ProfileSet)> {
        let name = self.defaults.set.as_deref()?;
        let set = self.sets.get(name)?;
        Some((name, set))
    }

    /// Returns `true` when no profiles are enabled in any section.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::enabled_profiles::EnabledProfiles;
    ///
    /// let ep = EnabledProfiles::default();
    /// assert!(ep.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.machine.enabled.is_empty()
            && self.filament.enabled.is_empty()
            && self.process.enabled.is_empty()
    }
}

/// Result of a single compatibility check between a profile and printer capabilities.
///
/// Each variant represents a specific type of compatibility issue that may be
/// detected when evaluating whether a filament/process profile works with the
/// user's enabled printers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum CompatCheck {
    /// Profile is fully compatible.
    Compatible,
    /// Profile nozzle size does not match any printer nozzle.
    NozzleMismatch {
        /// Nozzle size specified by the profile.
        profile_nozzle: f64,
        /// Nozzle sizes available on the printer(s).
        printer_nozzles: Vec<f64>,
    },
    /// Filament minimum temperature exceeds printer maximum.
    TemperatureWarning {
        /// Minimum temperature the filament requires.
        filament_min: f64,
        /// Maximum temperature the printer supports.
        printer_max: f64,
    },
}

/// Aggregated compatibility report for a profile against printer capabilities.
///
/// Collects the results of multiple [`CompatCheck`] evaluations and provides
/// convenience methods to determine overall compatibility.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CompatReport {
    /// Individual check results.
    pub checks: Vec<CompatCheck>,
}

impl CompatReport {
    /// Returns `true` when all checks passed (all are [`CompatCheck::Compatible`])
    /// or when no checks were performed.
    #[must_use]
    pub fn is_compatible(&self) -> bool {
        self.checks.iter().all(|c| *c == CompatCheck::Compatible)
    }

    /// Returns references to checks that are not [`CompatCheck::Compatible`].
    #[must_use]
    pub fn warnings(&self) -> Vec<&CompatCheck> {
        self.checks
            .iter()
            .filter(|c| **c != CompatCheck::Compatible)
            .collect()
    }
}

/// Compatibility information derived from enabled machine profiles.
///
/// Used to filter filament/process profiles to only those compatible with
/// the user's enabled printers. When no compatibility data is available
/// (all fields are `None`), all profiles are considered compatible.
///
/// # Examples
///
/// ```
/// use slicecore_engine::enabled_profiles::CompatibilityInfo;
/// use slicecore_engine::profile_library::ProfileIndexEntry;
///
/// let compat = CompatibilityInfo::default();
/// // Default (no data) means everything is compatible
/// let entry = ProfileIndexEntry {
///     id: "test".to_string(),
///     name: "Test".to_string(),
///     source: "test".to_string(),
///     vendor: "Test".to_string(),
///     profile_type: "filament".to_string(),
///     material: Some("PLA".to_string()),
///     nozzle_size: None,
///     printer_model: None,
///     path: "test.toml".to_string(),
///     layer_height: None,
///     quality: None,
/// };
/// assert!(compat.is_compatible(&entry));
/// ```
#[derive(Debug, Clone, Default)]
pub struct CompatibilityInfo {
    /// Compatible filament material types (e.g., `["PLA", "PETG"]`).
    pub filament_types: Option<Vec<String>>,
    /// Compatible filament vendor names.
    pub filament_vendors: Option<Vec<String>>,
    /// Compatible filament profile IDs.
    pub filament_ids: Option<Vec<String>>,
}

impl CompatibilityInfo {
    /// Checks whether a profile index entry is compatible.
    ///
    /// Returns `true` when no compatibility data is present (all fields `None`),
    /// treating the absence of constraints as "all compatible".
    ///
    /// When `filament_types` is `Some`, checks `entry.material` against the list.
    /// When `filament_ids` is `Some`, checks `entry.id` against the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::enabled_profiles::CompatibilityInfo;
    /// use slicecore_engine::profile_library::ProfileIndexEntry;
    ///
    /// let compat = CompatibilityInfo {
    ///     filament_types: Some(vec!["PLA".to_string()]),
    ///     filament_vendors: None,
    ///     filament_ids: None,
    /// };
    /// let entry = ProfileIndexEntry {
    ///     id: "test".to_string(),
    ///     name: "Test PLA".to_string(),
    ///     source: "test".to_string(),
    ///     vendor: "Test".to_string(),
    ///     profile_type: "filament".to_string(),
    ///     material: Some("PLA".to_string()),
    ///     nozzle_size: None,
    ///     printer_model: None,
    ///     path: "test.toml".to_string(),
    ///     layer_height: None,
    ///     quality: None,
    /// };
    /// assert!(compat.is_compatible(&entry));
    /// ```
    #[must_use]
    pub fn is_compatible(&self, entry: &ProfileIndexEntry) -> bool {
        // No constraints = all compatible
        if self.filament_types.is_none()
            && self.filament_vendors.is_none()
            && self.filament_ids.is_none()
        {
            return true;
        }

        // Check filament_ids first (most specific)
        if let Some(ref ids) = self.filament_ids {
            if ids.iter().any(|id| id == &entry.id) {
                return true;
            }
        }

        // Check filament_types
        if let Some(ref types) = self.filament_types {
            if let Some(ref material) = entry.material {
                let material_lower = material.to_lowercase();
                if types.iter().any(|t| t.to_lowercase() == material_lower) {
                    return true;
                }
            }
        }

        // Check filament_vendors
        if let Some(ref vendors) = self.filament_vendors {
            let vendor_lower = entry.vendor.to_lowercase();
            if vendors.iter().any(|v| v.to_lowercase() == vendor_lower) {
                return true;
            }
        }

        false
    }

    /// Checks nozzle diameter compatibility between a profile entry and printer entries.
    ///
    /// Returns `None` if compatible (matching nozzle found within epsilon tolerance,
    /// or entry has no nozzle size, or no printer nozzle data).
    /// Returns `Some(NozzleMismatch)` when the profile nozzle does not match any
    /// printer nozzle within an epsilon of 0.001.
    #[must_use]
    pub fn check_nozzle(
        entry: &ProfileIndexEntry,
        machine_entries: &[&ProfileIndexEntry],
    ) -> Option<CompatCheck> {
        let filament_nozzle = entry.nozzle_size?;

        let printer_nozzles: Vec<f64> = machine_entries
            .iter()
            .filter_map(|m| m.nozzle_size)
            .collect();

        if printer_nozzles.is_empty() {
            return None;
        }

        let matches = printer_nozzles
            .iter()
            .any(|n| (n - filament_nozzle).abs() < 0.001);

        if matches {
            None
        } else {
            Some(CompatCheck::NozzleMismatch {
                profile_nozzle: filament_nozzle,
                printer_nozzles,
            })
        }
    }

    /// Checks temperature compatibility between filament requirements and printer capabilities.
    ///
    /// Uses a conservative 300C default threshold because `MachineConfig` does not yet
    /// expose per-printer max nozzle temperature. When per-printer temps become available
    /// in profile metadata, this default should be replaced with actual printer capabilities.
    ///
    /// Returns `None` if compatible or if no filament temperature data is available.
    /// Returns `Some(TemperatureWarning)` when `filament_min_temp` exceeds `printer_max_temp`.
    #[must_use]
    pub fn check_temperature(
        filament_min_temp: Option<f64>,
        printer_max_temp: f64,
    ) -> Option<CompatCheck> {
        let min_temp = filament_min_temp?;

        if min_temp > printer_max_temp {
            Some(CompatCheck::TemperatureWarning {
                filament_min: min_temp,
                printer_max: printer_max_temp,
            })
        } else {
            None
        }
    }

    /// Builds a compatibility report by running nozzle and temperature checks.
    ///
    /// Collects all check results into a [`CompatReport`]. Checks that return `None`
    /// (compatible) are added as [`CompatCheck::Compatible`].
    #[must_use]
    pub fn compat_report(
        entry: &ProfileIndexEntry,
        machine_entries: &[&ProfileIndexEntry],
        printer_max_temp: f64,
        filament_min_temp: Option<f64>,
    ) -> CompatReport {
        let mut checks = Vec::new();

        match Self::check_nozzle(entry, machine_entries) {
            Some(check) => checks.push(check),
            None => checks.push(CompatCheck::Compatible),
        }

        match Self::check_temperature(filament_min_temp, printer_max_temp) {
            Some(check) => checks.push(check),
            None => checks.push(CompatCheck::Compatible),
        }

        CompatReport { checks }
    }

    /// Builds compatibility info from enabled machine IDs and a profile index.
    ///
    /// For each enabled machine, finds filament entries whose `printer_model`
    /// field contains the machine's model name, then collects their materials
    /// as compatible types.
    ///
    /// This implements the implicit compatibility from upstream slicers: a
    /// filament named `Bambu PLA @BBL X1C` is compatible with a machine
    /// whose model matches `BBL X1C`.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::enabled_profiles::CompatibilityInfo;
    /// use slicecore_engine::profile_library::{ProfileIndex, ProfileIndexEntry};
    ///
    /// let index = ProfileIndex {
    ///     version: 1,
    ///     generated: String::new(),
    ///     profiles: vec![
    ///         ProfileIndexEntry {
    ///             id: "machine/BBL/Bambu_X1C".to_string(),
    ///             name: "Bambu X1 Carbon".to_string(),
    ///             source: "orcaslicer".to_string(),
    ///             vendor: "BBL".to_string(),
    ///             profile_type: "machine".to_string(),
    ///             material: None,
    ///             nozzle_size: None,
    ///             printer_model: Some("Bambu X1 Carbon".to_string()),
    ///             path: "machine.toml".to_string(),
    ///             layer_height: None,
    ///             quality: None,
    ///         },
    ///         ProfileIndexEntry {
    ///             id: "filament/PLA_X1C".to_string(),
    ///             name: "PLA @BBL X1C".to_string(),
    ///             source: "orcaslicer".to_string(),
    ///             vendor: "BBL".to_string(),
    ///             profile_type: "filament".to_string(),
    ///             material: Some("PLA".to_string()),
    ///             nozzle_size: None,
    ///             printer_model: Some("Bambu X1 Carbon".to_string()),
    ///             path: "filament.toml".to_string(),
    ///             layer_height: None,
    ///             quality: None,
    ///         },
    ///     ],
    /// };
    ///
    /// let compat = CompatibilityInfo::from_index_entries(
    ///     &["machine/BBL/Bambu_X1C".to_string()],
    ///     &index,
    /// );
    /// assert!(compat.filament_types.is_some());
    /// ```
    #[must_use]
    pub fn from_index_entries(machine_ids: &[String], index: &ProfileIndex) -> Self {
        if machine_ids.is_empty() {
            return Self::default();
        }

        // Find the printer_model values for enabled machines
        let machine_models: Vec<&str> = index
            .profiles
            .iter()
            .filter(|e| e.profile_type == "machine" && machine_ids.contains(&e.id))
            .filter_map(|e| e.printer_model.as_deref())
            .collect();

        if machine_models.is_empty() {
            return Self::default();
        }

        // Find filament entries whose printer_model matches any enabled machine model
        let mut filament_types = Vec::new();
        let mut filament_ids = Vec::new();

        for entry in &index.profiles {
            if entry.profile_type != "filament" {
                continue;
            }
            let Some(ref entry_model) = entry.printer_model else {
                continue;
            };

            // Check if this filament's printer_model matches any enabled machine
            let matches = machine_models.iter().any(|model| entry_model == model);

            if matches {
                filament_ids.push(entry.id.clone());
                if let Some(ref material) = entry.material {
                    if !filament_types.contains(material) {
                        filament_types.push(material.clone());
                    }
                }
            }
        }

        Self {
            filament_types: if filament_types.is_empty() {
                None
            } else {
                Some(filament_types)
            },
            filament_vendors: None,
            filament_ids: if filament_ids.is_empty() {
                None
            } else {
                Some(filament_ids)
            },
        }
    }
}

/// Platform-aware home directory lookup.
#[cfg(not(target_arch = "wasm32"))]
fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper to create a test `ProfileIndexEntry` with minimal required fields.
    fn make_test_entry(nozzle_size: Option<f64>, profile_type: &str) -> ProfileIndexEntry {
        ProfileIndexEntry {
            id: "test/entry".to_string(),
            name: "Test Entry".to_string(),
            source: "test".to_string(),
            vendor: "TestVendor".to_string(),
            profile_type: profile_type.to_string(),
            material: Some("PLA".to_string()),
            nozzle_size,
            printer_model: None,
            path: "test.toml".to_string(),
            layer_height: None,
            quality: None,
        }
    }

    #[test]
    fn test_check_nozzle_matching() {
        let entry = make_test_entry(Some(0.4), "filament");
        let machine = make_test_entry(Some(0.4), "machine");
        let result = CompatibilityInfo::check_nozzle(&entry, &[&machine]);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_nozzle_mismatch() {
        let entry = make_test_entry(Some(0.4), "filament");
        let machine = make_test_entry(Some(0.6), "machine");
        let result = CompatibilityInfo::check_nozzle(&entry, &[&machine]);
        assert!(matches!(result, Some(CompatCheck::NozzleMismatch { .. })));
    }

    #[test]
    fn test_check_nozzle_no_filament_size() {
        let entry = make_test_entry(None, "filament");
        let machine = make_test_entry(Some(0.4), "machine");
        let result = CompatibilityInfo::check_nozzle(&entry, &[&machine]);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_nozzle_no_printer_data() {
        let entry = make_test_entry(Some(0.4), "filament");
        let result = CompatibilityInfo::check_nozzle(&entry, &[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_nozzle_epsilon() {
        let entry = make_test_entry(Some(0.4), "filament");
        let machine = make_test_entry(Some(0.400_000_01), "machine");
        let result = CompatibilityInfo::check_nozzle(&entry, &[&machine]);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_temperature_ok() {
        let result = CompatibilityInfo::check_temperature(Some(190.0), 300.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_temperature_warning() {
        let result = CompatibilityInfo::check_temperature(Some(350.0), 300.0);
        assert!(matches!(
            result,
            Some(CompatCheck::TemperatureWarning { .. })
        ));
    }

    #[test]
    fn test_check_temperature_no_data() {
        let result = CompatibilityInfo::check_temperature(None, 300.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_compat_report_all_compatible() {
        let entry = make_test_entry(Some(0.4), "filament");
        let machine = make_test_entry(Some(0.4), "machine");
        let report =
            CompatibilityInfo::compat_report(&entry, &[&machine], 300.0, None);
        assert!(report.is_compatible());
        assert!(report.warnings().is_empty());
    }

    #[test]
    fn test_compat_report_with_warnings() {
        let entry = make_test_entry(Some(0.4), "filament");
        let machine = make_test_entry(Some(0.6), "machine");
        let report =
            CompatibilityInfo::compat_report(&entry, &[&machine], 300.0, None);
        assert!(!report.is_compatible());
        assert_eq!(report.warnings().len(), 1);
    }

    #[test]
    fn default_has_empty_sections() {
        let ep = EnabledProfiles::default();
        assert!(ep.machine.enabled.is_empty());
        assert!(ep.filament.enabled.is_empty());
        assert!(ep.process.enabled.is_empty());
        assert!(ep.is_empty());
    }

    #[test]
    fn round_trip_toml_serialization() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("enabled-profiles.toml");

        let mut original = EnabledProfiles::default();
        original.enable("machine", "BBL/Bambu_X1C");
        original.enable("filament", "PLA_Basic");
        original.enable("filament", "PETG_Basic");
        original.enable("process", "0.20mm_Standard");

        original.save(&path).unwrap();
        let loaded = EnabledProfiles::load(&path).unwrap().unwrap();
        assert_eq!(original, loaded);
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("does-not-exist.toml");
        let result = EnabledProfiles::load(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn load_valid_toml_returns_some() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("enabled-profiles.toml");
        let content = r#"
[machine]
enabled = ["BBL/Bambu_X1C"]

[filament]
enabled = ["PLA_Basic", "PETG_Basic"]

[process]
enabled = []
"#;
        std::fs::write(&path, content).unwrap();
        let loaded = EnabledProfiles::load(&path).unwrap().unwrap();
        assert_eq!(loaded.machine.enabled, vec!["BBL/Bambu_X1C"]);
        assert_eq!(loaded.filament.enabled, vec!["PLA_Basic", "PETG_Basic"]);
        assert!(loaded.process.enabled.is_empty());
    }

    #[test]
    fn is_enabled_returns_true_when_present() {
        let mut ep = EnabledProfiles::default();
        ep.enable("machine", "BBL/Bambu_X1C");
        assert!(ep.is_enabled("machine", "BBL/Bambu_X1C"));
    }

    #[test]
    fn is_enabled_returns_false_when_absent() {
        let ep = EnabledProfiles::default();
        assert!(!ep.is_enabled("machine", "BBL/Bambu_X1C"));
    }

    #[test]
    fn is_enabled_returns_false_for_unknown_type() {
        let ep = EnabledProfiles::default();
        assert!(!ep.is_enabled("unknown_type", "any"));
    }

    #[test]
    fn enable_adds_to_section_no_duplicates() {
        let mut ep = EnabledProfiles::default();
        ep.enable("machine", "BBL/Bambu_X1C");
        ep.enable("machine", "BBL/Bambu_X1C");
        assert_eq!(ep.machine.enabled.len(), 1);
        assert!(ep.is_enabled("machine", "BBL/Bambu_X1C"));
    }

    #[test]
    fn disable_removes_from_section_no_error_on_double() {
        let mut ep = EnabledProfiles::default();
        ep.enable("machine", "BBL/Bambu_X1C");
        ep.disable("machine", "BBL/Bambu_X1C");
        assert!(!ep.is_enabled("machine", "BBL/Bambu_X1C"));
        // Double-disable should not error
        ep.disable("machine", "BBL/Bambu_X1C");
        assert!(!ep.is_enabled("machine", "BBL/Bambu_X1C"));
    }

    #[test]
    fn all_enabled_returns_flat_list() {
        let mut ep = EnabledProfiles::default();
        ep.enable("machine", "X1C");
        ep.enable("filament", "PLA");
        ep.enable("process", "Standard");

        let all = ep.all_enabled();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&("machine", "X1C")));
        assert!(all.contains(&("filament", "PLA")));
        assert!(all.contains(&("process", "Standard")));
    }

    #[test]
    fn counts_returns_per_type_counts() {
        let mut ep = EnabledProfiles::default();
        ep.enable("machine", "X1C");
        ep.enable("filament", "PLA");
        ep.enable("filament", "PETG");
        ep.enable("process", "Standard");
        assert_eq!(ep.counts(), (1, 2, 1));
    }

    #[test]
    fn compatibility_info_default_is_all_compatible() {
        let compat = CompatibilityInfo::default();
        let entry = ProfileIndexEntry {
            id: "test".to_string(),
            name: "Test".to_string(),
            source: "test".to_string(),
            vendor: "Test".to_string(),
            profile_type: "filament".to_string(),
            material: Some("PLA".to_string()),
            nozzle_size: None,
            printer_model: None,
            path: "test.toml".to_string(),
            layer_height: None,
            quality: None,
        };
        assert!(compat.is_compatible(&entry));
    }

    #[test]
    fn compatibility_info_from_index_entries_extracts_types() {
        let index = ProfileIndex {
            version: 1,
            generated: String::new(),
            profiles: vec![
                ProfileIndexEntry {
                    id: "machine/BBL/Bambu_X1C".to_string(),
                    name: "Bambu X1 Carbon".to_string(),
                    source: "orcaslicer".to_string(),
                    vendor: "BBL".to_string(),
                    profile_type: "machine".to_string(),
                    material: None,
                    nozzle_size: None,
                    printer_model: Some("Bambu X1 Carbon".to_string()),
                    path: "machine.toml".to_string(),
                    layer_height: None,
                    quality: None,
                },
                ProfileIndexEntry {
                    id: "filament/BBL/PLA_X1C".to_string(),
                    name: "PLA @BBL X1C".to_string(),
                    source: "orcaslicer".to_string(),
                    vendor: "BBL".to_string(),
                    profile_type: "filament".to_string(),
                    material: Some("PLA".to_string()),
                    nozzle_size: None,
                    printer_model: Some("Bambu X1 Carbon".to_string()),
                    path: "filament.toml".to_string(),
                    layer_height: None,
                    quality: None,
                },
                ProfileIndexEntry {
                    id: "filament/BBL/PETG_X1C".to_string(),
                    name: "PETG @BBL X1C".to_string(),
                    source: "orcaslicer".to_string(),
                    vendor: "BBL".to_string(),
                    profile_type: "filament".to_string(),
                    material: Some("PETG".to_string()),
                    nozzle_size: None,
                    printer_model: Some("Bambu X1 Carbon".to_string()),
                    path: "filament2.toml".to_string(),
                    layer_height: None,
                    quality: None,
                },
                // This filament is for a different printer
                ProfileIndexEntry {
                    id: "filament/Creality/PLA_Ender".to_string(),
                    name: "PLA @Ender 3".to_string(),
                    source: "orcaslicer".to_string(),
                    vendor: "Creality".to_string(),
                    profile_type: "filament".to_string(),
                    material: Some("PLA".to_string()),
                    nozzle_size: None,
                    printer_model: Some("Ender 3".to_string()),
                    path: "filament3.toml".to_string(),
                    layer_height: None,
                    quality: None,
                },
            ],
        };

        let compat =
            CompatibilityInfo::from_index_entries(&["machine/BBL/Bambu_X1C".to_string()], &index);

        // Should have found PLA and PETG as compatible types
        let types = compat.filament_types.as_ref().unwrap();
        assert!(types.contains(&"PLA".to_string()));
        assert!(types.contains(&"PETG".to_string()));
        assert_eq!(types.len(), 2);

        // Should have the BBL filament IDs but not Creality
        let ids = compat.filament_ids.as_ref().unwrap();
        assert!(ids.contains(&"filament/BBL/PLA_X1C".to_string()));
        assert!(ids.contains(&"filament/BBL/PETG_X1C".to_string()));
        assert!(!ids.contains(&"filament/Creality/PLA_Ender".to_string()));
    }

    // -----------------------------------------------------------------------
    // ProfileSet / DefaultsSection / EnabledProfiles extension tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_profile_set_toml_roundtrip() {
        let mut ep = EnabledProfiles::default();
        ep.enable("machine", "X1C");
        ep.add_set(
            "my-set".to_string(),
            ProfileSet {
                machine: "X1C".to_string(),
                filament: "PLA".to_string(),
                process: "Standard".to_string(),
            },
        );
        ep.defaults.set = Some("my-set".to_string());

        let toml_str = toml::to_string_pretty(&ep).unwrap();
        let loaded: EnabledProfiles = toml::from_str(&toml_str).unwrap();

        assert_eq!(ep, loaded);
        assert_eq!(loaded.sets.len(), 1);
        assert_eq!(loaded.defaults.set, Some("my-set".to_string()));
    }

    #[test]
    fn test_enabled_profiles_backward_compat() {
        // TOML without [sets] or [defaults] should deserialize with empty defaults
        let content = r#"
[machine]
enabled = ["X1C"]

[filament]
enabled = ["PLA"]

[process]
enabled = []
"#;
        let loaded: EnabledProfiles = toml::from_str(content).unwrap();
        assert_eq!(loaded.machine.enabled, vec!["X1C"]);
        assert!(loaded.sets.is_empty());
        assert_eq!(loaded.defaults.set, None);
    }

    #[test]
    fn test_add_remove_set() {
        let mut ep = EnabledProfiles::default();
        let set = ProfileSet {
            machine: "X1C".to_string(),
            filament: "PLA".to_string(),
            process: "Standard".to_string(),
        };
        ep.add_set("my-set".to_string(), set.clone());
        assert!(ep.get_set("my-set").is_some());

        let removed = ep.remove_set("my-set");
        assert_eq!(removed, Some(set));
        assert!(ep.get_set("my-set").is_none());
    }

    #[test]
    fn test_set_default() {
        let mut ep = EnabledProfiles::default();
        let set = ProfileSet {
            machine: "X1C".to_string(),
            filament: "PLA".to_string(),
            process: "Standard".to_string(),
        };
        ep.add_set("my-set".to_string(), set);
        ep.set_default(Some("my-set".to_string()));

        let (name, ps) = ep.default_set().unwrap();
        assert_eq!(name, "my-set");
        assert_eq!(ps.machine, "X1C");

        // Setting default to nonexistent set should be a no-op
        ep.set_default(Some("nonexistent".to_string()));
        assert_eq!(ep.defaults.set, Some("my-set".to_string()));

        // Setting to None clears default
        ep.set_default(None);
        assert!(ep.default_set().is_none());
    }

    #[test]
    fn test_remove_set_clears_default() {
        let mut ep = EnabledProfiles::default();
        ep.add_set(
            "my-set".to_string(),
            ProfileSet {
                machine: "X1C".to_string(),
                filament: "PLA".to_string(),
                process: "Standard".to_string(),
            },
        );
        ep.set_default(Some("my-set".to_string()));
        assert!(ep.default_set().is_some());

        ep.remove_set("my-set");
        assert_eq!(ep.defaults.set, None);
        assert!(ep.default_set().is_none());
    }
}
