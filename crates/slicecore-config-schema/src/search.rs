//! Full-text search across setting definitions.

use crate::registry::SettingRegistry;
use crate::types::SettingDefinition;

impl SettingRegistry {
    /// Searches settings by case-insensitive substring match across key,
    /// display name, description, and tags.
    ///
    /// Results are ranked by match quality:
    /// - Key match: +4 points
    /// - Display name match: +3 points
    /// - Tag match: +2 points
    /// - Description match: +1 point
    ///
    /// Within the same score, results are sorted alphabetically by key.
    /// Returns an empty vector for empty queries.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&SettingDefinition> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut results: Vec<(&SettingDefinition, u8)> = self
            .all()
            .filter_map(|def| {
                let mut score = 0u8;

                // Exact key segment match (highest priority)
                if def.key.0.to_lowercase().contains(&query_lower) {
                    score += 4;
                }
                // Display name match
                if def.display_name.to_lowercase().contains(&query_lower) {
                    score += 3;
                }
                // Tag match
                if def
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&query_lower))
                {
                    score += 2;
                }
                // Description match (lowest priority)
                if def.description.to_lowercase().contains(&query_lower) {
                    score += 1;
                }

                if score > 0 {
                    Some((def, score))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.key.0.cmp(&b.0.key.0)));
        results.into_iter().map(|(def, _)| def).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SettingCategory, SettingKey, Tier, ValueType};

    fn make_def(
        key: &str,
        display: &str,
        desc: &str,
        tags: Vec<&str>,
    ) -> SettingDefinition {
        SettingDefinition {
            key: SettingKey::new(key),
            display_name: display.to_owned(),
            description: desc.to_owned(),
            tier: Tier::Simple,
            category: SettingCategory::Speed,
            value_type: ValueType::Float,
            default_value: serde_json::Value::from(0.0),
            constraints: Vec::new(),
            affects: Vec::new(),
            affected_by: Vec::new(),
            units: None,
            tags: tags.into_iter().map(String::from).collect(),
            since_version: "0.1.0".to_owned(),
            deprecated: None,
        }
    }

    #[test]
    fn search_by_key_returns_top_result() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def(
            "speed.perimeter",
            "Perimeter Speed",
            "Speed for perimeters",
            vec!["speed"],
        ));
        reg.register(make_def(
            "speed.infill",
            "Infill Speed",
            "Speed for infill",
            vec!["speed"],
        ));

        let results = reg.search("perimeter");
        assert!(!results.is_empty());
        assert_eq!(results[0].key.0, "speed.perimeter");
    }

    #[test]
    fn search_retraction_fields() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def(
            "retract.length",
            "Retraction Length",
            "How far to retract",
            vec!["retraction"],
        ));
        reg.register(make_def(
            "retract.speed",
            "Retraction Speed",
            "Speed of retraction",
            vec!["retraction"],
        ));
        reg.register(make_def(
            "speed.perimeter",
            "Perimeter Speed",
            "Speed for perimeters",
            vec!["speed"],
        ));

        let results = reg.search("retract");
        assert_eq!(results.len(), 2);
        // Both retraction fields should appear before speed.perimeter
        let keys: Vec<&str> = results.iter().map(|d| d.key.0.as_str()).collect();
        assert!(keys.contains(&"retract.length"));
        assert!(keys.contains(&"retract.speed"));
    }

    #[test]
    fn empty_query_returns_empty() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("a", "A", "desc", vec![]));

        let results = reg.search("");
        assert!(results.is_empty());
    }

    #[test]
    fn case_insensitive_matching() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def(
            "speed.perimeter",
            "Perimeter Speed",
            "Controls perimeter speed",
            vec![],
        ));

        let results_lower = reg.search("perimeter");
        let results_upper = reg.search("PERIMETER");
        let results_mixed = reg.search("Perimeter");

        assert_eq!(results_lower.len(), results_upper.len());
        assert_eq!(results_lower.len(), results_mixed.len());
        assert!(!results_lower.is_empty());
    }

    #[test]
    fn tag_match_contributes_to_score() {
        let mut reg = SettingRegistry::new();
        // Has "quality" only in tags
        reg.register(make_def("layer_height", "Layer Height", "height of layer", vec!["quality"]));
        // Has "quality" nowhere
        reg.register(make_def("speed.travel", "Travel Speed", "travel speed", vec!["speed"]));

        let results = reg.search("quality");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key.0, "layer_height");
    }

    #[test]
    fn no_match_returns_empty() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("speed.perimeter", "Perimeter Speed", "speed", vec![]));

        let results = reg.search("zzzznonexistent");
        assert!(results.is_empty());
    }
}
