//! JSON Schema 2020-12 generation from setting definitions.

use serde_json::{json, Map, Value};

use crate::registry::SettingRegistry;
use crate::types::{Constraint, ValueType};

/// Inserts a property schema at a nested path within a JSON Schema object.
///
/// Creates intermediate `{ "type": "object", "properties": {} }` nodes as needed.
fn insert_nested(root: &mut Value, key_parts: &[&str], property_schema: Value) {
    if key_parts.is_empty() {
        return;
    }

    if key_parts.len() == 1 {
        // Leaf: insert directly into properties
        if let Some(props) = root
            .as_object_mut()
            .and_then(|o| o.get_mut("properties"))
            .and_then(Value::as_object_mut)
        {
            props.insert(key_parts[0].to_owned(), property_schema);
        }
        return;
    }

    // Intermediate node: ensure it exists with type=object and properties
    let props = root
        .as_object_mut()
        .and_then(|o| o.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .expect("root must have a properties object");

    if !props.contains_key(key_parts[0]) {
        props.insert(
            key_parts[0].to_owned(),
            json!({
                "type": "object",
                "properties": {}
            }),
        );
    }

    let child = props.get_mut(key_parts[0]).expect("just inserted");
    insert_nested(child, &key_parts[1..], property_schema);
}

/// Converts a `ValueType` to its JSON Schema representation.
fn value_type_to_schema(vt: &ValueType) -> Value {
    match vt {
        ValueType::Float => json!({ "type": "number" }),
        ValueType::Int => json!({ "type": "integer" }),
        ValueType::Bool => json!({ "type": "boolean" }),
        ValueType::String => json!({ "type": "string" }),
        ValueType::Percent => json!({ "type": "number", "minimum": 0, "maximum": 100 }),
        ValueType::FloatVec => json!({ "type": "array", "items": { "type": "number" } }),
        ValueType::Enum { variants } => {
            let names: Vec<&str> = variants.iter().map(|v| v.value.as_str()).collect();
            json!({ "type": "string", "enum": names })
        }
    }
}

impl SettingRegistry {
    /// Generates a JSON Schema 2020-12 document describing all registered settings.
    ///
    /// The schema uses nested `properties` matching the dotted-key hierarchy
    /// (e.g., `speed.perimeter` becomes `properties.speed.properties.perimeter`).
    ///
    /// Each leaf property includes:
    /// - Standard JSON Schema fields: `type`, `minimum`, `maximum`, `default`, `description`, `enum`
    /// - Custom `x-` extensions: `x-tier`, `x-category`, `x-units`, `x-display-name`,
    ///   `x-affects`, `x-affected-by`, `x-tags`, `x-since-version`, `x-deprecated`,
    ///   `x-description`, `x-depends-on`
    #[must_use]
    pub fn to_json_schema(&self) -> Value {
        let mut root = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://slicecore.dev/config-schema.json",
            "title": "SliceCore Print Configuration",
            "type": "object",
            "properties": {}
        });

        for def in self.all() {
            let mut prop = value_type_to_schema(&def.value_type);
            let obj = prop.as_object_mut().expect("schema is an object");

            // Standard JSON Schema fields
            obj.insert("description".to_owned(), Value::String(def.description.clone()));

            if !def.default_value.is_null() {
                obj.insert("default".to_owned(), def.default_value.clone());
            }

            // Apply range constraints
            for constraint in &def.constraints {
                if let Constraint::Range { min, max } = constraint {
                    // Only set if not already set by ValueType (e.g., Percent)
                    if !obj.contains_key("minimum") {
                        obj.insert("minimum".to_owned(), json!(min));
                    }
                    if !obj.contains_key("maximum") {
                        obj.insert("maximum".to_owned(), json!(max));
                    }
                }
            }

            // x- extensions
            obj.insert("x-tier".to_owned(), json!(def.tier as u8));
            obj.insert("x-category".to_owned(), Value::String(def.category.as_str().to_owned()));

            if let Some(ref units) = def.units {
                obj.insert("x-units".to_owned(), Value::String(units.clone()));
            }

            obj.insert("x-display-name".to_owned(), Value::String(def.display_name.clone()));

            if !def.affects.is_empty() {
                let keys: Vec<&str> = def.affects.iter().map(|k| k.0.as_str()).collect();
                obj.insert("x-affects".to_owned(), json!(keys));
            }

            if !def.affected_by.is_empty() {
                let keys: Vec<&str> = def.affected_by.iter().map(|k| k.0.as_str()).collect();
                obj.insert("x-affected-by".to_owned(), json!(keys));
            }

            if !def.tags.is_empty() {
                obj.insert("x-tags".to_owned(), json!(def.tags));
            }

            obj.insert("x-since-version".to_owned(), Value::String(def.since_version.clone()));

            if let Some(ref dep) = def.deprecated {
                obj.insert("x-deprecated".to_owned(), Value::String(dep.clone()));
            }

            obj.insert("x-description".to_owned(), Value::String(def.description.clone()));

            // x-depends-on from DependsOn constraints
            for constraint in &def.constraints {
                if let Constraint::DependsOn { key, condition } = constraint {
                    obj.insert(
                        "x-depends-on".to_owned(),
                        json!({ "key": key.0, "condition": condition }),
                    );
                }
            }

            // Insert into nested structure
            let parts: Vec<&str> = def.key.0.split('.').collect();
            insert_nested(&mut root, &parts, prop);
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Constraint, EnumVariant, SettingCategory, SettingDefinition, SettingKey, Tier, ValueType,
    };

    fn make_def(key: &str, vt: ValueType) -> SettingDefinition {
        SettingDefinition {
            key: SettingKey::new(key),
            display_name: key.to_owned(),
            description: format!("Description for {key}"),
            tier: Tier::Simple,
            category: SettingCategory::Speed,
            value_type: vt,
            default_value: serde_json::Value::from(42.0),
            constraints: Vec::new(),
            affects: Vec::new(),
            affected_by: Vec::new(),
            units: Some("mm/s".to_owned()),
            tags: vec!["speed".to_owned()],
            since_version: "0.1.0".to_owned(),
            deprecated: None,
        }
    }

    #[test]
    fn schema_contains_meta_fields() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("speed.perimeter", ValueType::Float));

        let schema = reg.to_json_schema();
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(schema["title"], "SliceCore Print Configuration");
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn nested_properties_from_dotted_keys() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("speed.perimeter", ValueType::Float));
        reg.register(make_def("speed.infill", ValueType::Float));

        let schema = reg.to_json_schema();
        let speed = &schema["properties"]["speed"];
        assert_eq!(speed["type"], "object");
        assert_eq!(speed["properties"]["perimeter"]["type"], "number");
        assert_eq!(speed["properties"]["infill"]["type"], "number");
    }

    #[test]
    fn value_type_mapping() {
        let mut reg = SettingRegistry::new();
        reg.register(make_def("a", ValueType::Int));
        reg.register(make_def("b", ValueType::Bool));
        reg.register(make_def("c", ValueType::String));
        reg.register(make_def("d", ValueType::Percent));
        reg.register(make_def("e", ValueType::FloatVec));
        reg.register(make_def(
            "f",
            ValueType::Enum {
                variants: vec![
                    EnumVariant {
                        value: "fast".to_owned(),
                        display: "Fast".to_owned(),
                        description: String::new(),
                    },
                    EnumVariant {
                        value: "slow".to_owned(),
                        display: "Slow".to_owned(),
                        description: String::new(),
                    },
                ],
            },
        ));

        let schema = reg.to_json_schema();
        assert_eq!(schema["properties"]["a"]["type"], "integer");
        assert_eq!(schema["properties"]["b"]["type"], "boolean");
        assert_eq!(schema["properties"]["c"]["type"], "string");
        assert_eq!(schema["properties"]["d"]["type"], "number");
        assert_eq!(schema["properties"]["d"]["minimum"], 0);
        assert_eq!(schema["properties"]["d"]["maximum"], 100);
        assert_eq!(schema["properties"]["e"]["type"], "array");
        assert_eq!(schema["properties"]["f"]["type"], "string");
        assert_eq!(schema["properties"]["f"]["enum"][0], "fast");
        assert_eq!(schema["properties"]["f"]["enum"][1], "slow");
    }

    #[test]
    fn x_extensions_present() {
        let mut reg = SettingRegistry::new();
        let mut def = make_def("speed.perimeter", ValueType::Float);
        def.affects = vec![SettingKey::new("speed.infill")];
        def.tags = vec!["speed".to_owned(), "perimeter".to_owned()];
        reg.register(def);

        let schema = reg.to_json_schema();
        let prop = &schema["properties"]["speed"]["properties"]["perimeter"];
        assert_eq!(prop["x-tier"], 1);
        assert_eq!(prop["x-category"], "speed");
        assert_eq!(prop["x-units"], "mm/s");
        assert_eq!(prop["x-display-name"], "speed.perimeter");
        assert_eq!(prop["x-affects"][0], "speed.infill");
        assert_eq!(prop["x-tags"][0], "speed");
        assert_eq!(prop["x-tags"][1], "perimeter");
        assert_eq!(prop["x-since-version"], "0.1.0");
    }

    #[test]
    fn range_constraint_adds_min_max() {
        let mut reg = SettingRegistry::new();
        let mut def = make_def("layer_height", ValueType::Float);
        def.constraints = vec![Constraint::Range {
            min: 0.05,
            max: 0.6,
        }];
        reg.register(def);

        let schema = reg.to_json_schema();
        let prop = &schema["properties"]["layer_height"];
        assert_eq!(prop["minimum"], 0.05);
        assert_eq!(prop["maximum"], 0.6);
    }

    #[test]
    fn depends_on_constraint_adds_x_depends_on() {
        let mut reg = SettingRegistry::new();
        let mut def = make_def("retract.length", ValueType::Float);
        def.constraints = vec![Constraint::DependsOn {
            key: SettingKey::new("retract.enable"),
            condition: "== true".to_owned(),
        }];
        reg.register(def);

        let schema = reg.to_json_schema();
        let prop = &schema["properties"]["retract"]["properties"]["length"];
        assert_eq!(prop["x-depends-on"]["key"], "retract.enable");
        assert_eq!(prop["x-depends-on"]["condition"], "== true");
    }
}
