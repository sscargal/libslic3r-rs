//! Cascade resolution engine for per-object config composition.
//!
//! Resolves the 10-layer cascade for each object in a [`PlateConfig`]:
//! - Layers 1-6 are composed by [`ProfileComposer`] (base config).
//! - Layer 7: default object overrides (applied to all objects).
//! - Layer 8: per-object overrides (named override set + inline).
//! - Layers 9-10: deferred to slicing (layer-range and per-region).
//!
//! Objects with no overrides share an [`Arc<PrintConfig>`] for memory efficiency.

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::PrintConfig;
use crate::error::EngineError;
use crate::plate_config::{LayerRangeOverride, ObjectConfig, PlateConfig};
use crate::profile_compose::{ComposedConfig, FieldSource, ProfileComposer, SourceType};

/// A fully resolved object with its effective [`PrintConfig`] and provenance.
#[derive(Debug)]
pub struct ResolvedObject {
    /// Index of this object in `PlateConfig.objects` (0-based).
    pub index: usize,
    /// Object name (from config or defaulted to `"object_N"`).
    pub name: String,
    /// The fully resolved config for this object.
    pub config: Arc<PrintConfig>,
    /// Per-field provenance showing which cascade layer set each value.
    pub provenance: HashMap<String, FieldSource>,
    /// Number of copies of this object.
    pub copies: u32,
}

/// Resolves all objects in a [`PlateConfig`] through the 10-layer cascade.
///
/// The cascade resolver takes a base [`ComposedConfig`] (layers 1-6) and applies
/// per-object overrides (layers 7-8) to produce a [`ResolvedObject`] per object.
#[derive(Debug)]
pub struct CascadeResolver;

impl CascadeResolver {
    /// Resolves a single object's config through the cascade.
    ///
    /// Takes the base composer result (layers 1-6 already composed) and applies
    /// layers 7-8 (default object overrides + per-object overrides).
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] if:
    /// - The object references an unknown override set name.
    /// - Profile composition fails after applying overrides.
    pub fn resolve_object_config(
        base_composed: &ComposedConfig,
        plate: &PlateConfig,
        object: &ObjectConfig,
        object_index: usize,
    ) -> Result<ComposedConfig, EngineError> {
        let object_id = object
            .name
            .clone()
            .unwrap_or_else(|| format!("object_{object_index}"));

        // Create a fresh composer and replay the base config as a table
        let mut composer = ProfileComposer::new();

        // Serialize the base config to a TOML table to use as starting point
        let base_value = toml::Value::try_from(&base_composed.config).map_err(|e| {
            EngineError::ConfigError(format!("failed to serialize base config: {e}"))
        })?;
        let base_table = match base_value {
            toml::Value::Table(t) => t,
            _ => {
                return Err(EngineError::ConfigError(
                    "base config did not serialize to a table".to_string(),
                ));
            }
        };

        // Add base config as the Default layer
        composer.add_table_layer(SourceType::Default, base_table);

        // Layer 7: Default object overrides (applied to ALL objects)
        if let Some(ref defaults) = plate.default_object_overrides {
            composer.add_table_layer(SourceType::DefaultObjectOverride, defaults.clone());
        }

        // Layer 8: Per-object overrides
        // First: named override set
        if let Some(ref set_name) = object.override_set {
            let set_table = plate.override_sets.get(set_name).ok_or_else(|| {
                let available: Vec<&String> = plate.override_sets.keys().collect();
                let suggestion = find_closest_match(set_name, &available);
                let hint = suggestion.map_or_else(
                    || format!("available sets: {available:?}"),
                    |s| format!("did you mean '{s}'?"),
                );
                EngineError::ConfigError(format!(
                    "unknown override set '{set_name}' for object '{object_id}'; {hint}"
                ))
            })?;
            composer.add_table_layer(
                SourceType::PerObjectOverride {
                    object_id: object_id.clone(),
                },
                set_table.clone(),
            );
        }

        // Second: inline overrides (after named set, so inline wins)
        if let Some(ref inline) = object.inline_overrides {
            composer.add_table_layer(
                SourceType::PerObjectOverride {
                    object_id: object_id.clone(),
                },
                inline.clone(),
            );
        }

        composer.compose()
    }

    /// Resolves all objects in a [`PlateConfig`].
    ///
    /// Returns one [`ResolvedObject`] per object. Objects with no overrides
    /// (no `default_object_overrides`, no `override_set`, no `inline_overrides`)
    /// share an [`Arc<PrintConfig>`] for memory efficiency.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] if any object's resolution fails.
    pub fn resolve_all(
        plate: &PlateConfig,
        base_composed: &ComposedConfig,
    ) -> Result<Vec<ResolvedObject>, EngineError> {
        let has_defaults = plate.default_object_overrides.is_some();

        // Pre-compute shared base config for objects with no overrides
        let shared_config: Option<Arc<PrintConfig>> = if !has_defaults {
            Some(Arc::new(base_composed.config.clone()))
        } else {
            None
        };

        let mut results = Vec::with_capacity(plate.objects.len());

        for (i, object) in plate.objects.iter().enumerate() {
            let name = object.name.clone().unwrap_or_else(|| format!("object_{i}"));

            let needs_resolution =
                has_defaults || object.override_set.is_some() || object.inline_overrides.is_some();

            if needs_resolution {
                let composed = Self::resolve_object_config(base_composed, plate, object, i)?;
                results.push(ResolvedObject {
                    index: i,
                    name,
                    config: Arc::new(composed.config),
                    provenance: composed.provenance,
                    copies: object.copies,
                });
            } else if let Some(ref shared) = shared_config {
                results.push(ResolvedObject {
                    index: i,
                    name,
                    config: Arc::clone(shared),
                    provenance: base_composed.provenance.clone(),
                    copies: object.copies,
                });
            }
        }

        Ok(results)
    }

    /// Resolves layer-range overrides (cascade layer 9) for a specific Z height.
    ///
    /// Takes the base per-object config (layers 1-8 already resolved) and applies
    /// any matching [`LayerRangeOverride`]s from the [`ObjectConfig`]. Returns the
    /// original [`Arc`] if no overrides match, or a new [`Arc<PrintConfig>`] with
    /// applied overrides when they do.
    ///
    /// Multiple matching ranges are applied in definition order (last wins for
    /// conflicting fields, matching the "last-defined wins" rule).
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] if profile composition fails after applying overrides.
    pub fn resolve_for_z(
        resolved: &ResolvedObject,
        object_config: &ObjectConfig,
        z: f64,
        layer_number: u32,
    ) -> Result<Arc<PrintConfig>, EngineError> {
        let matching: Vec<&LayerRangeOverride> = object_config
            .layer_overrides
            .iter()
            .filter(|lr| Self::layer_range_matches(lr, z, layer_number))
            .collect();

        if matching.is_empty() {
            return Ok(Arc::clone(&resolved.config));
        }

        // Serialize the resolved base config to a TOML table
        let base_value = toml::Value::try_from(&*resolved.config).map_err(|e| {
            EngineError::ConfigError(format!(
                "failed to serialize base config for resolve_for_z: {e}"
            ))
        })?;
        let base_table = match base_value {
            toml::Value::Table(t) => t,
            _ => {
                return Err(EngineError::ConfigError(
                    "base config did not serialize to a table".to_string(),
                ));
            }
        };

        let mut composer = ProfileComposer::new();
        composer.add_table_layer(SourceType::Default, base_table);

        for (i, lr) in matching.iter().enumerate() {
            let range_desc = if let Some((z_min, z_max)) = lr.z_range {
                format!("z:{z_min:.2}-{z_max:.2}")
            } else if let Some((start, end)) = lr.layer_range {
                format!("layers:{start}-{end}")
            } else {
                format!("range-{i}")
            };
            composer.add_table_layer(
                SourceType::LayerRangeOverride {
                    object_id: resolved.name.clone(),
                    range_desc,
                },
                lr.overrides.clone(),
            );
        }

        let composed = composer.compose()?;
        Ok(Arc::new(composed.config))
    }

    /// Checks whether a [`LayerRangeOverride`] matches a given Z height and layer number.
    fn layer_range_matches(lr: &LayerRangeOverride, z: f64, layer_number: u32) -> bool {
        if let Some((z_min, z_max)) = lr.z_range {
            z >= z_min - 1e-6 && z <= z_max + 1e-6
        } else if let Some((layer_start, layer_end)) = lr.layer_range {
            layer_number >= layer_start && layer_number <= layer_end
        } else {
            false
        }
    }
}

/// Find the closest matching string using Jaro-Winkler similarity.
fn find_closest_match<'a>(target: &str, candidates: &[&'a String]) -> Option<&'a str> {
    candidates
        .iter()
        .map(|c| (c, strsim::jaro_winkler(target, c)))
        .filter(|(_, score)| *score > 0.7)
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(c, _)| c.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a base composed config from defaults.
    fn base_composed() -> ComposedConfig {
        let composer = ProfileComposer::new();
        composer.compose().expect("default compose should work")
    }

    /// Helper: create a simple plate with one object and no overrides.
    fn simple_plate() -> PlateConfig {
        PlateConfig {
            machine_profile: None,
            filament_profile: None,
            process_profile: None,
            user_override_file: None,
            cli_set_overrides: Vec::new(),
            default_object_overrides: None,
            override_sets: HashMap::new(),
            objects: vec![ObjectConfig::default()],
        }
    }

    #[test]
    fn resolve_single_object_no_overrides() {
        let base = base_composed();
        let plate = simple_plate();
        let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
        assert_eq!(results.len(), 1);
        // Config should match the base
        assert_eq!(results[0].config.layer_height, base.config.layer_height,);
        assert_eq!(results[0].name, "object_0");
        assert_eq!(results[0].copies, 1);
    }

    #[test]
    fn resolve_object_with_default_object_overrides() {
        let base = base_composed();
        let mut plate = simple_plate();

        let mut overrides = toml::map::Map::new();
        overrides.insert("infill_density".to_string(), toml::Value::Float(0.75));
        plate.default_object_overrides = Some(overrides);

        let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            (results[0].config.infill_density - 0.75).abs() < f64::EPSILON,
            "infill_density should be 0.75 from default object overrides"
        );
    }

    #[test]
    fn resolve_object_with_named_override_set() {
        let base = base_composed();
        let mut plate = simple_plate();

        let mut set_table = toml::map::Map::new();
        set_table.insert("wall_count".to_string(), toml::Value::Integer(6));

        plate
            .override_sets
            .insert("thick_walls".to_string(), set_table);
        plate.objects[0].override_set = Some("thick_walls".to_string());

        let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
        assert_eq!(results[0].config.wall_count, 6);
    }

    #[test]
    fn resolve_object_with_inline_overrides() {
        let base = base_composed();
        let mut plate = simple_plate();

        let mut inline = toml::map::Map::new();
        inline.insert("layer_height".to_string(), toml::Value::Float(0.1));
        plate.objects[0].inline_overrides = Some(inline);

        let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
        assert!(
            (results[0].config.layer_height - 0.1).abs() < f64::EPSILON,
            "layer_height should be 0.1 from inline overrides"
        );
    }

    #[test]
    fn inline_overrides_override_named_set() {
        let base = base_composed();
        let mut plate = simple_plate();

        // Named set says wall_count = 6
        let mut set_table = toml::map::Map::new();
        set_table.insert("wall_count".to_string(), toml::Value::Integer(6));
        plate.override_sets.insert("thick".to_string(), set_table);
        plate.objects[0].override_set = Some("thick".to_string());

        // Inline says wall_count = 8 (should win)
        let mut inline = toml::map::Map::new();
        inline.insert("wall_count".to_string(), toml::Value::Integer(8));
        plate.objects[0].inline_overrides = Some(inline);

        let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
        assert_eq!(
            results[0].config.wall_count, 8,
            "inline should override named set"
        );
    }

    #[test]
    fn provenance_tracks_per_object_override() {
        let base = base_composed();
        let mut plate = simple_plate();

        let mut inline = toml::map::Map::new();
        inline.insert("infill_density".to_string(), toml::Value::Float(0.5));
        plate.objects[0].inline_overrides = Some(inline);

        let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
        let prov = results[0].provenance.get("infill_density");
        assert!(prov.is_some(), "infill_density should have provenance");
        match &prov.unwrap().source_type {
            SourceType::PerObjectOverride { object_id } => {
                assert_eq!(object_id, "object_0");
            }
            other => panic!("expected PerObjectOverride, got {other:?}"),
        }
    }

    #[test]
    fn two_objects_no_overrides_share_arc() {
        let base = base_composed();
        let mut plate = simple_plate();
        plate.objects.push(ObjectConfig::default());

        let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
        assert_eq!(results.len(), 2);
        // Both should point to the same Arc
        assert!(
            Arc::ptr_eq(&results[0].config, &results[1].config),
            "objects with no overrides should share Arc<PrintConfig>"
        );
    }

    #[test]
    fn per_region_inherits_from_per_object_not_global() {
        // Per-region overrides (layer 10) are deferred, but we verify that
        // per-object config (layer 8) is the base for further composition.
        // An object with inline overrides should have those in its base config.
        let base = base_composed();
        let mut plate = simple_plate();

        let mut inline = toml::map::Map::new();
        inline.insert("infill_density".to_string(), toml::Value::Float(0.9));
        plate.objects[0].inline_overrides = Some(inline);

        let composed =
            CascadeResolver::resolve_object_config(&base, &plate, &plate.objects[0], 0).unwrap();
        // The resolved config has infill_density = 0.9 (per-object)
        assert!((composed.config.infill_density - 0.9).abs() < f64::EPSILON);

        // If we now add a per-region override on top of this, it would start from 0.9
        // (This is the architectural guarantee -- layers 9-10 use this resolved config as base)
        let mut region_composer = ProfileComposer::new();
        let obj_table = toml::Value::try_from(&composed.config)
            .unwrap()
            .as_table()
            .unwrap()
            .clone();
        region_composer.add_table_layer(SourceType::Default, obj_table);

        let mut region_override = toml::map::Map::new();
        region_override.insert("wall_count".to_string(), toml::Value::Integer(10));
        region_composer.add_table_layer(
            SourceType::PerRegionOverride {
                object_id: "object_0".to_string(),
                modifier_id: "mod_1".to_string(),
            },
            region_override,
        );

        let region_result = region_composer.compose().unwrap();
        // infill_density should still be 0.9 (inherited from per-object, not reset to global)
        assert!((region_result.config.infill_density - 0.9).abs() < f64::EPSILON);
        assert_eq!(region_result.config.wall_count, 10);
    }

    #[test]
    fn last_modifier_wins_when_overlapping() {
        // When two modifiers overlap, the last one in the list wins.
        // This test verifies the composition order behavior.
        let base = base_composed();
        let mut composer = ProfileComposer::new();
        let base_table = toml::Value::try_from(&base.config)
            .unwrap()
            .as_table()
            .unwrap()
            .clone();
        composer.add_table_layer(SourceType::Default, base_table);

        // First modifier sets wall_count = 4
        let mut mod1 = toml::map::Map::new();
        mod1.insert("wall_count".to_string(), toml::Value::Integer(4));
        composer.add_table_layer(
            SourceType::PerRegionOverride {
                object_id: "obj".to_string(),
                modifier_id: "mod_1".to_string(),
            },
            mod1,
        );

        // Second modifier sets wall_count = 8 (should win)
        let mut mod2 = toml::map::Map::new();
        mod2.insert("wall_count".to_string(), toml::Value::Integer(8));
        composer.add_table_layer(
            SourceType::PerRegionOverride {
                object_id: "obj".to_string(),
                modifier_id: "mod_2".to_string(),
            },
            mod2,
        );

        let result = composer.compose().unwrap();
        assert_eq!(result.config.wall_count, 8, "last modifier should win");
    }

    #[test]
    fn unknown_override_set_error_with_suggestion() {
        let base = base_composed();
        let mut plate = simple_plate();

        let mut set_table = toml::map::Map::new();
        set_table.insert("wall_count".to_string(), toml::Value::Integer(6));
        plate
            .override_sets
            .insert("thick_walls".to_string(), set_table);
        plate.objects[0].override_set = Some("thik_walls".to_string()); // typo

        let err = CascadeResolver::resolve_all(&plate, &base).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unknown override set") && msg.contains("did you mean"),
            "error should mention unknown override set and suggest: {msg}"
        );
    }

    #[test]
    fn default_object_override_provenance_is_layer_7() {
        let base = base_composed();
        let mut plate = simple_plate();

        let mut overrides = toml::map::Map::new();
        overrides.insert("wall_count".to_string(), toml::Value::Integer(5));
        plate.default_object_overrides = Some(overrides);

        let results = CascadeResolver::resolve_all(&plate, &base).unwrap();
        let prov = results[0].provenance.get("wall_count");
        assert!(prov.is_some());
        assert_eq!(
            prov.unwrap().source_type,
            SourceType::DefaultObjectOverride,
            "wall_count should come from DefaultObjectOverride (layer 7)"
        );
    }

    // ---- resolve_for_z tests ----

    /// Helper: create a `ResolvedObject` from a base composed config.
    fn resolved_from_base(base: &ComposedConfig) -> ResolvedObject {
        ResolvedObject {
            index: 0,
            name: "test_object".to_string(),
            config: Arc::new(base.config.clone()),
            provenance: base.provenance.clone(),
            copies: 1,
        }
    }

    #[test]
    fn resolve_for_z_no_overrides_returns_same_arc() {
        let base = base_composed();
        let resolved = resolved_from_base(&base);
        let obj_config = ObjectConfig::default(); // no layer_overrides

        let result = CascadeResolver::resolve_for_z(&resolved, &obj_config, 1.0, 5).unwrap();
        assert!(
            Arc::ptr_eq(&result, &resolved.config),
            "should return same Arc when no layer-range overrides match"
        );
    }

    #[test]
    fn resolve_for_z_z_range_match_applies_override() {
        let base = base_composed();
        let resolved = resolved_from_base(&base);
        let original_wall_count = resolved.config.wall_count;

        let mut overrides = toml::map::Map::new();
        overrides.insert("wall_count".to_string(), toml::Value::Integer(10));

        let mut obj_config = ObjectConfig::default();
        obj_config
            .layer_overrides
            .push(crate::plate_config::LayerRangeOverride {
                z_range: Some((0.5, 2.0)),
                layer_range: None,
                overrides,
            });

        // z=1.0 is within [0.5, 2.0]
        let result = CascadeResolver::resolve_for_z(&resolved, &obj_config, 1.0, 5).unwrap();
        assert!(
            !Arc::ptr_eq(&result, &resolved.config),
            "should return new Arc when overrides match"
        );
        assert_eq!(
            result.wall_count, 10,
            "wall_count should be overridden to 10"
        );
        assert_ne!(original_wall_count, 10, "sanity: base wall_count is not 10");
    }

    #[test]
    fn resolve_for_z_z_range_no_match_returns_original() {
        let base = base_composed();
        let resolved = resolved_from_base(&base);

        let mut overrides = toml::map::Map::new();
        overrides.insert("wall_count".to_string(), toml::Value::Integer(10));

        let mut obj_config = ObjectConfig::default();
        obj_config
            .layer_overrides
            .push(crate::plate_config::LayerRangeOverride {
                z_range: Some((5.0, 10.0)),
                layer_range: None,
                overrides,
            });

        // z=1.0 is outside [5.0, 10.0]
        let result = CascadeResolver::resolve_for_z(&resolved, &obj_config, 1.0, 5).unwrap();
        assert!(
            Arc::ptr_eq(&result, &resolved.config),
            "should return original Arc when z_range does not match"
        );
    }

    #[test]
    fn resolve_for_z_layer_range_match() {
        let base = base_composed();
        let resolved = resolved_from_base(&base);

        let mut overrides = toml::map::Map::new();
        overrides.insert("infill_density".to_string(), toml::Value::Float(0.99));

        let mut obj_config = ObjectConfig::default();
        obj_config
            .layer_overrides
            .push(crate::plate_config::LayerRangeOverride {
                z_range: None,
                layer_range: Some((3, 7)),
                overrides,
            });

        // layer_number=5 is within [3, 7]
        let result = CascadeResolver::resolve_for_z(&resolved, &obj_config, 1.0, 5).unwrap();
        assert!(
            (result.infill_density - 0.99).abs() < f64::EPSILON,
            "infill_density should be 0.99"
        );
    }

    #[test]
    fn resolve_for_z_multiple_overlapping_last_wins() {
        let base = base_composed();
        let resolved = resolved_from_base(&base);

        let mut overrides1 = toml::map::Map::new();
        overrides1.insert("wall_count".to_string(), toml::Value::Integer(4));
        overrides1.insert("infill_density".to_string(), toml::Value::Float(0.5));

        let mut overrides2 = toml::map::Map::new();
        overrides2.insert("wall_count".to_string(), toml::Value::Integer(8));

        let mut obj_config = ObjectConfig::default();
        obj_config
            .layer_overrides
            .push(crate::plate_config::LayerRangeOverride {
                z_range: Some((0.0, 5.0)),
                layer_range: None,
                overrides: overrides1,
            });
        obj_config
            .layer_overrides
            .push(crate::plate_config::LayerRangeOverride {
                z_range: Some((0.0, 3.0)),
                layer_range: None,
                overrides: overrides2,
            });

        let result = CascadeResolver::resolve_for_z(&resolved, &obj_config, 1.0, 5).unwrap();
        assert_eq!(
            result.wall_count, 8,
            "last-defined override should win for wall_count"
        );
        assert!(
            (result.infill_density - 0.5).abs() < f64::EPSILON,
            "infill_density from first override should persist (no conflict)"
        );
    }

    #[test]
    fn resolve_for_z_boundary_match() {
        let base = base_composed();
        let resolved = resolved_from_base(&base);

        let mut overrides = toml::map::Map::new();
        overrides.insert("wall_count".to_string(), toml::Value::Integer(7));

        let mut obj_config = ObjectConfig::default();
        obj_config
            .layer_overrides
            .push(crate::plate_config::LayerRangeOverride {
                z_range: Some((1.0, 2.0)),
                layer_range: None,
                overrides,
            });

        // Exactly at z_min boundary
        let result_min = CascadeResolver::resolve_for_z(&resolved, &obj_config, 1.0, 0).unwrap();
        assert_eq!(result_min.wall_count, 7, "should match at z_min boundary");

        // Exactly at z_max boundary
        let result_max = CascadeResolver::resolve_for_z(&resolved, &obj_config, 2.0, 0).unwrap();
        assert_eq!(result_max.wall_count, 7, "should match at z_max boundary");

        // Just outside z_max
        let result_outside =
            CascadeResolver::resolve_for_z(&resolved, &obj_config, 2.001, 0).unwrap();
        assert!(
            Arc::ptr_eq(&result_outside, &resolved.config),
            "should not match outside z_max + epsilon"
        );
    }
}
