//! Flat metadata JSON output for setting definitions.

use serde_json::Value;

use crate::registry::SettingRegistry;
use crate::types::{SettingCategory, Tier};

impl SettingRegistry {
    /// Produces a flat JSON array of all setting definitions.
    ///
    /// Each element is the full serialized `SettingDefinition`, suitable for
    /// consumption by UI generators, AI tooling, and documentation systems.
    #[must_use]
    pub fn to_metadata_json(&self) -> Value {
        Value::Array(
            self.all()
                .map(|def| serde_json::to_value(def).unwrap_or_default())
                .collect(),
        )
    }

    /// Produces a filtered flat JSON array of setting definitions.
    ///
    /// Settings are included only if they match both the tier and category
    /// filters (when provided). Passing `None` for a filter skips that check.
    #[must_use]
    pub fn to_filtered_metadata_json(
        &self,
        max_tier: Option<Tier>,
        category: Option<SettingCategory>,
    ) -> Value {
        Value::Array(
            self.all()
                .filter(|def| {
                    max_tier.map_or(true, |t| def.tier <= t)
                        && category.map_or(true, |c| def.category == c)
                })
                .map(|def| serde_json::to_value(def).unwrap_or_default())
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{OverrideSafety, SettingDefinition, SettingKey, ValueType};

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
            override_safety: OverrideSafety::default(),
        }
    }

    #[test]
    fn metadata_json_is_array() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("a", Tier::Simple, SettingCategory::Quality));
        reg.register(make_def("b", Tier::Advanced, SettingCategory::Speed));

        let meta = reg.to_metadata_json();
        let arr = meta.as_array().expect("should be array");
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn each_element_has_key_field() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def(
            "layer_height",
            Tier::Simple,
            SettingCategory::Quality,
        ));

        let meta = reg.to_metadata_json();
        let arr = meta.as_array().unwrap();
        assert_eq!(arr[0]["key"], "layer_height");
    }

    #[test]
    fn filtered_by_tier() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("s", Tier::Simple, SettingCategory::Quality));
        reg.register(make_def("a", Tier::Advanced, SettingCategory::Quality));
        reg.register(make_def("d", Tier::Developer, SettingCategory::Quality));

        let filtered = reg.to_filtered_metadata_json(Some(Tier::Simple), None);
        assert_eq!(filtered.as_array().unwrap().len(), 1);

        let filtered = reg.to_filtered_metadata_json(Some(Tier::Advanced), None);
        assert_eq!(filtered.as_array().unwrap().len(), 2);
    }

    #[test]
    fn filtered_by_category() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("q1", Tier::Simple, SettingCategory::Quality));
        reg.register(make_def("s1", Tier::Simple, SettingCategory::Speed));

        let filtered = reg.to_filtered_metadata_json(None, Some(SettingCategory::Speed));
        assert_eq!(filtered.as_array().unwrap().len(), 1);
        assert_eq!(filtered[0]["key"], "s1");
    }

    #[test]
    fn filtered_by_both() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("q1", Tier::Simple, SettingCategory::Quality));
        reg.register(make_def("q2", Tier::Advanced, SettingCategory::Quality));
        reg.register(make_def("s1", Tier::Simple, SettingCategory::Speed));

        let filtered =
            reg.to_filtered_metadata_json(Some(Tier::Simple), Some(SettingCategory::Quality));
        assert_eq!(filtered.as_array().unwrap().len(), 1);
        assert_eq!(filtered[0]["key"], "q1");
    }

    #[test]
    fn no_filters_returns_all() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("a", Tier::Simple, SettingCategory::Quality));
        reg.register(make_def("b", Tier::Developer, SettingCategory::Speed));

        let filtered = reg.to_filtered_metadata_json(None, None);
        assert_eq!(filtered.as_array().unwrap().len(), 2);
    }
}
