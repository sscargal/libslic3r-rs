//! Central registry of all setting definitions.

use std::collections::BTreeMap;

use crate::types::{Constraint, SettingCategory, SettingDefinition, SettingKey, Tier};

/// Central registry that stores and indexes all setting definitions.
///
/// Supports registration, lookup, filtering by tier/category, inverse
/// dependency graph computation, and integrity validation.
///
/// # Examples
///
/// ```
/// use slicecore_config_schema::{SettingRegistry, SettingDefinition, SettingKey, Tier, SettingCategory, ValueType};
///
/// let mut registry = SettingRegistry::new();
/// assert!(registry.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct SettingRegistry {
    definitions: BTreeMap<SettingKey, SettingDefinition>,
}

impl SettingRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a setting definition.
    ///
    /// # Panics
    ///
    /// Panics if a definition with the same key is already registered.
    pub fn register(&mut self, def: SettingDefinition) {
        let key = def.key.clone();
        assert!(
            !self.definitions.contains_key(&key),
            "Duplicate setting key: {key}"
        );
        self.definitions.insert(key, def);
    }

    /// Looks up a setting definition by key.
    #[must_use]
    pub fn get(&self, key: &SettingKey) -> Option<&SettingDefinition> {
        self.definitions.get(key)
    }

    /// Convenience lookup by string key.
    #[must_use]
    pub fn get_by_str(&self, key: &str) -> Option<&SettingDefinition> {
        self.definitions.get(&SettingKey::new(key))
    }

    /// Returns an iterator over all definitions, sorted by key.
    pub fn all(&self) -> impl Iterator<Item = &SettingDefinition> {
        self.definitions.values()
    }

    /// Returns the number of registered definitions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    /// Returns true if no definitions are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    /// Computes the inverse dependency graph by populating each definition's
    /// `affected_by` field from other definitions' `affects` lists.
    ///
    /// For each definition A whose `affects` list contains key B, this adds
    /// A's key to B's `affected_by` list.
    pub fn compute_affected_by(&mut self) {
        // Collect forward edges: (affected_key, affecting_key)
        let edges: Vec<(SettingKey, SettingKey)> = self
            .definitions
            .values()
            .flat_map(|def| {
                def.affects
                    .iter()
                    .map(move |affected| (affected.clone(), def.key.clone()))
            })
            .collect();

        // Clear existing affected_by and rebuild
        for def in self.definitions.values_mut() {
            def.affected_by.clear();
        }

        for (affected_key, affecting_key) in edges {
            if let Some(def) = self.definitions.get_mut(&affected_key) {
                def.affected_by.push(affecting_key);
            }
        }

        // Sort affected_by lists for deterministic output
        for def in self.definitions.values_mut() {
            def.affected_by.sort();
        }
    }

    /// Returns definitions with tier at or below the given maximum.
    #[must_use]
    pub fn filter_by_tier(&self, max_tier: Tier) -> Vec<&SettingDefinition> {
        self.definitions
            .values()
            .filter(|def| def.tier <= max_tier)
            .collect()
    }

    /// Returns definitions matching the given category.
    #[must_use]
    pub fn filter_by_category(&self, category: SettingCategory) -> Vec<&SettingDefinition> {
        self.definitions
            .values()
            .filter(|def| def.category == category)
            .collect()
    }

    /// Validates referential integrity of the registry.
    ///
    /// Checks that all keys referenced in `affects` lists and `DependsOn`
    /// constraints actually exist in the registry. Returns a list of error
    /// messages for any dangling references found.
    #[must_use]
    pub fn validate_integrity(&self) -> Vec<String> {
        let mut errors = Vec::new();

        for def in self.definitions.values() {
            for affected_key in &def.affects {
                if !self.definitions.contains_key(affected_key) {
                    errors.push(format!(
                        "Setting '{}' affects non-existent key '{affected_key}'",
                        def.key,
                    ));
                }
            }

            for constraint in &def.constraints {
                if let Constraint::DependsOn { key, .. } = constraint {
                    if !self.definitions.contains_key(key) {
                        errors.push(format!(
                            "Setting '{}' has DependsOn constraint referencing non-existent key '{key}'",
                            def.key,
                        ));
                    }
                }
            }
        }

        errors
    }

    /// Populates default values from a serialized config JSON object.
    ///
    /// Traverses nested JSON paths matching each setting's dotted key
    /// (e.g., `speed.perimeter` maps to `json["speed"]["perimeter"]`).
    /// Only overwrites `default_value` when a matching path is found.
    pub fn populate_defaults(&mut self, defaults_json: &serde_json::Value) {
        for def in self.definitions.values_mut() {
            let parts: Vec<&str> = def.key.0.split('.').collect();
            let mut current = defaults_json;
            let mut found = true;
            for part in &parts {
                match current.get(part) {
                    Some(next) => current = next,
                    None => {
                        found = false;
                        break;
                    }
                }
            }
            if found {
                def.default_value = current.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ValueType;

    /// Helper to create a minimal setting definition for testing.
    fn make_def(key: &str, tier: Tier, category: SettingCategory) -> SettingDefinition {
        SettingDefinition {
            key: SettingKey::new(key),
            display_name: key.to_owned(),
            description: String::new(),
            tier,
            category,
            value_type: ValueType::Float,
            default_value: serde_json::Value::from(0.0),
            constraints: Vec::new(),
            affects: Vec::new(),
            affected_by: Vec::new(),
            units: None,
            tags: Vec::new(),
            since_version: "0.1.0".to_owned(),
            deprecated: None,
        }
    }

    #[test]
    fn register_and_get_round_trip() {
        let mut reg = SettingRegistry::new();
        let def = make_def("print.layer_height", Tier::Simple, SettingCategory::Quality);
        reg.register(def);

        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());

        let found = reg.get(&SettingKey::new("print.layer_height"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().display_name, "print.layer_height");

        let found_str = reg.get_by_str("print.layer_height");
        assert!(found_str.is_some());
    }

    #[test]
    fn compute_affected_by_produces_correct_inverse() {
        let mut reg = SettingRegistry::new();

        let mut a = make_def("a", Tier::Simple, SettingCategory::Quality);
        a.affects = vec![SettingKey::new("b"), SettingKey::new("c")];
        reg.register(a);

        let b = make_def("b", Tier::Simple, SettingCategory::Quality);
        reg.register(b);

        let mut c = make_def("c", Tier::Simple, SettingCategory::Quality);
        c.affects = vec![SettingKey::new("b")];
        reg.register(c);

        reg.compute_affected_by();

        let b_def = reg.get_by_str("b").unwrap();
        assert_eq!(b_def.affected_by.len(), 2);
        assert!(b_def.affected_by.contains(&SettingKey::new("a")));
        assert!(b_def.affected_by.contains(&SettingKey::new("c")));

        let c_def = reg.get_by_str("c").unwrap();
        assert_eq!(c_def.affected_by.len(), 1);
        assert!(c_def.affected_by.contains(&SettingKey::new("a")));

        // a is not affected by anything
        let a_def = reg.get_by_str("a").unwrap();
        assert!(a_def.affected_by.is_empty());
    }

    #[test]
    fn filter_by_tier() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("simple", Tier::Simple, SettingCategory::Quality));
        reg.register(make_def("advanced", Tier::Advanced, SettingCategory::Quality));
        reg.register(make_def("dev", Tier::Developer, SettingCategory::Quality));

        let simple = reg.filter_by_tier(Tier::Simple);
        assert_eq!(simple.len(), 1);

        let up_to_advanced = reg.filter_by_tier(Tier::Advanced);
        assert_eq!(up_to_advanced.len(), 2);

        let all = reg.filter_by_tier(Tier::Developer);
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn filter_by_category() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("q1", Tier::Simple, SettingCategory::Quality));
        reg.register(make_def("q2", Tier::Advanced, SettingCategory::Quality));
        reg.register(make_def("s1", Tier::Simple, SettingCategory::Speed));

        let quality = reg.filter_by_category(SettingCategory::Quality);
        assert_eq!(quality.len(), 2);

        let speed = reg.filter_by_category(SettingCategory::Speed);
        assert_eq!(speed.len(), 1);
    }

    #[test]
    fn validate_integrity_catches_dangling_references() {
        let mut reg = SettingRegistry::new();

        let mut a = make_def("a", Tier::Simple, SettingCategory::Quality);
        a.affects = vec![SettingKey::new("nonexistent")];
        a.constraints = vec![Constraint::DependsOn {
            key: SettingKey::new("also_missing"),
            condition: "== true".to_owned(),
        }];
        reg.register(a);

        let errors = reg.validate_integrity();
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("nonexistent"));
        assert!(errors[1].contains("also_missing"));
    }

    #[test]
    #[should_panic(expected = "Duplicate setting key")]
    fn duplicate_key_registration_panics() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("dup", Tier::Simple, SettingCategory::Quality));
        reg.register(make_def("dup", Tier::Advanced, SettingCategory::Speed));
    }
}
