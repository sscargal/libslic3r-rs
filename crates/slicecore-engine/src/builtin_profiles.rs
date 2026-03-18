//! Built-in profiles for first-time users.
//!
//! Provides compiled-in TOML profile strings for generic materials and a
//! generic printer, so the binary is self-contained for users who haven't
//! imported any slicer profiles yet.

/// A built-in profile compiled into the binary.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinProfile {
    /// Short identifier (e.g., `"generic_pla"`).
    pub name: &'static str,
    /// Profile category: `"filament"`, `"machine"`, or `"process"`.
    pub profile_type: &'static str,
    /// Human-readable display name.
    pub display_name: &'static str,
    /// Raw TOML content.
    pub toml_content: &'static str,
}

const GENERIC_PLA_TOML: &str = r#"
profile_type = "filament"
name = "Generic PLA"
filament_type = "PLA"

[filament]
diameter = 1.75
density = 1.24
nozzle_temperatures = [210.0]
bed_temperatures = [60.0]
first_layer_nozzle_temperatures = [215.0]
first_layer_bed_temperatures = [65.0]
max_volumetric_speed = 15.0
nozzle_temperature_range_low = 190.0
nozzle_temperature_range_high = 230.0

[cooling]
fan_speed = 100
fan_below_layer_time = 100.0
disable_fan_first_layers = 1
"#;

const GENERIC_PETG_TOML: &str = r#"
profile_type = "filament"
name = "Generic PETG"
filament_type = "PETG"

[filament]
diameter = 1.75
density = 1.27
nozzle_temperatures = [240.0]
bed_temperatures = [80.0]
first_layer_nozzle_temperatures = [245.0]
first_layer_bed_temperatures = [85.0]
max_volumetric_speed = 12.0
nozzle_temperature_range_low = 220.0
nozzle_temperature_range_high = 260.0

[cooling]
fan_speed = 50
fan_below_layer_time = 60.0
disable_fan_first_layers = 3
"#;

const GENERIC_ABS_TOML: &str = r#"
profile_type = "filament"
name = "Generic ABS"
filament_type = "ABS"

[filament]
diameter = 1.75
density = 1.04
nozzle_temperatures = [250.0]
bed_temperatures = [100.0]
first_layer_nozzle_temperatures = [255.0]
first_layer_bed_temperatures = [105.0]
max_volumetric_speed = 12.0
nozzle_temperature_range_low = 230.0
nozzle_temperature_range_high = 270.0

[cooling]
fan_speed = 0
fan_below_layer_time = 20.0
disable_fan_first_layers = 4
"#;

const GENERIC_PRINTER_TOML: &str = r#"
profile_type = "machine"
name = "Generic FDM Printer"

[machine]
bed_x = 220.0
bed_y = 220.0
printable_height = 250.0
nozzle_diameters = [0.4]
max_speed_x = 250.0
max_speed_y = 250.0
max_speed_z = 12.0
max_speed_e = 80.0
max_acceleration_x = 3000.0
max_acceleration_y = 3000.0
max_acceleration_z = 100.0
max_acceleration_e = 5000.0
max_acceleration_extruding = 3000.0
max_acceleration_retracting = 3000.0
max_acceleration_travel = 3000.0
min_layer_height = 0.07
max_layer_height = 0.32
extruder_count = 1

gcode_dialect = "marlin"
"#;

const STANDARD_PROCESS_TOML: &str = r#"
profile_type = "process"
name = "Standard Quality"

layer_height = 0.2
first_layer_height = 0.2
wall_count = 3
infill_density = 0.2
infill_pattern = "grid"
top_solid_layers = 4
bottom_solid_layers = 4

[speeds]
perimeter = 50.0
infill = 80.0
travel = 150.0
first_layer = 25.0
bridge = 25.0
top_surface = 30.0
support = 50.0

[retraction]
length = 0.8
speed = 35.0
z_hop = 0.2
"#;

static BUILTIN_PROFILES: [BuiltinProfile; 5] = [
    BuiltinProfile {
        name: "generic_pla",
        profile_type: "filament",
        display_name: "Generic PLA",
        toml_content: GENERIC_PLA_TOML,
    },
    BuiltinProfile {
        name: "generic_petg",
        profile_type: "filament",
        display_name: "Generic PETG",
        toml_content: GENERIC_PETG_TOML,
    },
    BuiltinProfile {
        name: "generic_abs",
        profile_type: "filament",
        display_name: "Generic ABS",
        toml_content: GENERIC_ABS_TOML,
    },
    BuiltinProfile {
        name: "generic_printer",
        profile_type: "machine",
        display_name: "Generic FDM Printer",
        toml_content: GENERIC_PRINTER_TOML,
    },
    BuiltinProfile {
        name: "standard",
        profile_type: "process",
        display_name: "Standard Quality",
        toml_content: STANDARD_PROCESS_TOML,
    },
];

/// Returns the built-in profile matching `name`, if any.
///
/// # Examples
///
/// ```
/// use slicecore_engine::builtin_profiles::get_builtin_profile;
///
/// let pla = get_builtin_profile("generic_pla").unwrap();
/// assert_eq!(pla.profile_type, "filament");
/// ```
pub fn get_builtin_profile(name: &str) -> Option<&'static BuiltinProfile> {
    BUILTIN_PROFILES.iter().find(|p| p.name == name)
}

/// Returns all available built-in profiles.
///
/// # Examples
///
/// ```
/// use slicecore_engine::builtin_profiles::list_builtin_profiles;
///
/// let profiles = list_builtin_profiles();
/// assert!(profiles.len() >= 5);
/// ```
pub fn list_builtin_profiles() -> &'static [BuiltinProfile] {
    &BUILTIN_PROFILES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_pla_profile_exists() {
        let p = get_builtin_profile("generic_pla").expect("generic_pla should exist");
        assert_eq!(p.profile_type, "filament");
        let table: toml::Value = toml::from_str(p.toml_content).expect("valid TOML");
        assert!(table.as_table().is_some());
        assert_eq!(
            table.get("profile_type").and_then(|v| v.as_str()),
            Some("filament")
        );
    }

    #[test]
    fn generic_petg_profile_exists() {
        let p = get_builtin_profile("generic_petg").expect("generic_petg should exist");
        assert_eq!(p.profile_type, "filament");
        let table: toml::Value = toml::from_str(p.toml_content).expect("valid TOML");
        assert!(table.as_table().is_some());
        assert_eq!(
            table.get("profile_type").and_then(|v| v.as_str()),
            Some("filament")
        );
    }

    #[test]
    fn generic_printer_profile_exists() {
        let p = get_builtin_profile("generic_printer").expect("generic_printer should exist");
        assert_eq!(p.profile_type, "machine");
        let table: toml::Value = toml::from_str(p.toml_content).expect("valid TOML");
        assert!(table.as_table().is_some());
        assert_eq!(
            table.get("profile_type").and_then(|v| v.as_str()),
            Some("machine")
        );
    }

    #[test]
    fn standard_process_profile_exists() {
        let p = get_builtin_profile("standard").expect("standard should exist");
        assert_eq!(p.profile_type, "process");
        let table: toml::Value = toml::from_str(p.toml_content).expect("valid TOML");
        assert!(table.as_table().is_some());
        assert_eq!(
            table.get("profile_type").and_then(|v| v.as_str()),
            Some("process")
        );
        // Check 0.20mm layer height and 20% infill
        let lh = table
            .get("layer_height")
            .and_then(|v| v.as_float())
            .expect("layer_height");
        assert!(
            (lh - 0.2).abs() < f64::EPSILON,
            "expected 0.20mm layer height"
        );
        let infill = table
            .get("infill_density")
            .and_then(|v| v.as_float())
            .expect("infill_density");
        assert!((infill - 0.2).abs() < f64::EPSILON, "expected 20% infill");
    }

    #[test]
    fn list_returns_all_profiles() {
        let profiles = list_builtin_profiles();
        assert!(profiles.len() >= 5, "expected at least 5 built-in profiles");
        let names: Vec<&str> = profiles.iter().map(|p| p.name).collect();
        assert!(names.contains(&"generic_pla"));
        assert!(names.contains(&"generic_petg"));
        assert!(names.contains(&"generic_abs"));
        assert!(names.contains(&"generic_printer"));
        assert!(names.contains(&"standard"));
    }

    #[test]
    fn all_builtin_toml_strings_parse() {
        for p in list_builtin_profiles() {
            let result: Result<toml::Value, _> = toml::from_str(p.toml_content);
            assert!(
                result.is_ok(),
                "Profile '{}' has invalid TOML: {:?}",
                p.name,
                result.err()
            );
        }
    }
}
