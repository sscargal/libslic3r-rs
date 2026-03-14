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

/// Returns the built-in profile matching `name`, if any.
pub fn get_builtin_profile(_name: &str) -> Option<&'static BuiltinProfile> {
    todo!()
}

/// Returns all available built-in profiles.
pub fn list_builtin_profiles() -> &'static [BuiltinProfile] {
    todo!()
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
        assert!((lh - 0.2).abs() < f64::EPSILON, "expected 0.20mm layer height");
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
