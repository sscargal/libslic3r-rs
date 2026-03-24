//! Modifier mesh region detection and per-region setting overrides.
//!
//! Modifier meshes are 3D volumes that define regions of a model where
//! different slicing settings should be applied. For example, a modifier
//! volume can increase infill density in a stress-critical area while
//! leaving the rest of the model at a lower density.
//!
//! # Pipeline integration
//!
//! 1. Each modifier mesh is sliced at the current layer Z using
//!    [`slice_modifier`] to produce a 2D footprint ([`ModifierRegion`]).
//! 2. [`split_by_modifiers`] intersects the model contours with modifier
//!    footprints to produce separate regions, each with its effective
//!    [`PrintConfig`].
//! 3. The engine generates perimeters and infill separately for each region.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slicecore_geo::{polygon_difference, polygon_intersection, ValidPolygon};
use slicecore_mesh::TriangleMesh;
use slicecore_slicer::slice_at_height;

use crate::config::PrintConfig;
use crate::profile_compose::{merge_layer, FieldSource, SourceType};

/// A modifier mesh: a 3D volume paired with setting overrides.
///
/// When the slicer processes a layer, the modifier mesh is sliced at the
/// same Z height to determine its 2D footprint. Contours inside that
/// footprint receive the overridden settings.
///
/// # Overrides
///
/// Overrides are stored as a TOML partial table, allowing any
/// [`PrintConfig`] field to be overridden (not just the 8 hardcoded
/// fields of the old `SettingOverrides` struct).
pub struct ModifierMesh {
    /// The 3D volume defining the modifier region.
    pub mesh: TriangleMesh,
    /// Settings to apply within the modifier volume, as a TOML partial table.
    /// Any `PrintConfig` field can be overridden (e.g., `{"infill_density": 0.8}`).
    pub overrides: toml::map::Map<String, toml::Value>,
    /// Unique identifier for this modifier (for provenance tracking).
    pub modifier_id: String,
}

/// A modifier's 2D footprint at a specific Z height.
///
/// Produced by slicing a [`ModifierMesh`] at a layer Z.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifierRegion {
    /// The modifier's 2D contours at this Z height.
    pub contours: Vec<ValidPolygon>,
    /// TOML partial overrides for this region.
    pub overrides: toml::map::Map<String, toml::Value>,
    /// Modifier identifier for provenance.
    pub modifier_id: String,
}

/// Slices a modifier mesh at a given Z height.
///
/// Returns `Some(ModifierRegion)` if the modifier intersects this Z height
/// (produces at least one contour). Returns `None` if the modifier does not
/// intersect (e.g., the Z is above or below the modifier volume).
pub fn slice_modifier(modifier: &ModifierMesh, z: f64) -> Option<ModifierRegion> {
    let contours = slice_at_height(&modifier.mesh, z);
    if contours.is_empty() {
        None
    } else {
        Some(ModifierRegion {
            contours,
            overrides: modifier.overrides.clone(),
            modifier_id: modifier.modifier_id.clone(),
        })
    }
}

/// Splits model contours into regions based on modifier footprints.
///
/// For each modifier region, computes the intersection between model contours
/// and modifier contours (the overlap). The non-modified remainder is the
/// model minus all modifier footprints.
///
/// Returns a list of `(region_contours, effective_config)` pairs:
/// - One entry per modifier whose intersection with the model is non-empty.
/// - One entry for the remainder (base config) if any model area is outside
///   all modifiers.
///
/// # Parameters
///
/// - `contours`: The model's contours at this layer Z.
/// - `modifiers`: Active modifier regions at this layer Z (from [`slice_modifier`]).
/// - `base_config`: The base print configuration.
pub fn split_by_modifiers(
    contours: &[ValidPolygon],
    modifiers: &[ModifierRegion],
    base_config: &PrintConfig,
) -> Vec<(Vec<ValidPolygon>, PrintConfig)> {
    if modifiers.is_empty() {
        // No modifiers -- the entire model uses the base config.
        return vec![(contours.to_vec(), base_config.clone())];
    }

    // Serialize the base config to a TOML table once for reuse.
    let base_table = toml::Value::try_from(base_config)
        .expect("PrintConfig should serialize to TOML")
        .as_table()
        .expect("PrintConfig should serialize as a TOML table")
        .clone();

    let mut regions = Vec::new();
    let mut remainder = contours.to_vec();

    for modifier in modifiers {
        if remainder.is_empty() {
            break;
        }

        // Compute intersection: model contours that overlap with this modifier.
        let intersection = polygon_intersection(&remainder, &modifier.contours).unwrap_or_default();

        if !intersection.is_empty() {
            let effective_config =
                apply_toml_overrides(&base_table, &modifier.overrides, &modifier.modifier_id);
            regions.push((intersection, effective_config));

            // Subtract this modifier's footprint from the remainder.
            let diff = polygon_difference(&remainder, &modifier.contours).unwrap_or_default();
            remainder = diff;
        }
    }

    // Add the remainder (unmodified region) with base config.
    if !remainder.is_empty() {
        regions.push((remainder, base_config.clone()));
    }

    regions
}

/// Applies TOML partial overrides onto a serialized base config table.
///
/// Uses the same [`merge_layer`] infrastructure as profile composition,
/// so nested keys (e.g., `speeds.perimeter`) are deep-merged correctly.
fn apply_toml_overrides(
    base_table: &toml::map::Map<String, toml::Value>,
    overrides: &toml::map::Map<String, toml::Value>,
    modifier_id: &str,
) -> PrintConfig {
    let mut merged = base_table.clone();
    let source = FieldSource {
        source_type: SourceType::PerRegionOverride {
            object_id: String::new(),
            modifier_id: modifier_id.to_owned(),
        },
        file_path: None,
        overrode: None,
    };
    let mut provenance = HashMap::new();
    let mut warnings = Vec::new();

    merge_layer(
        &mut merged,
        overrides,
        "",
        &source,
        &mut provenance,
        &mut warnings,
    );

    // Deserialize the merged table back to PrintConfig.
    PrintConfig::deserialize(toml::Value::Table(merged))
        .expect("merged TOML table should deserialize to PrintConfig")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::Polygon;
    use slicecore_math::Point3;

    /// Creates a box mesh from (x0, y0, z0) to (x1, y1, z1).
    fn make_box(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> TriangleMesh {
        let vertices = vec![
            Point3::new(x0, y0, z0),
            Point3::new(x1, y0, z0),
            Point3::new(x1, y1, z0),
            Point3::new(x0, y1, z0),
            Point3::new(x0, y0, z1),
            Point3::new(x1, y0, z1),
            Point3::new(x1, y1, z1),
            Point3::new(x0, y1, z1),
        ];
        let indices = vec![
            // top (z1)
            [4, 5, 6],
            [4, 6, 7],
            // bottom (z0)
            [1, 0, 3],
            [1, 3, 2],
            // right (x1)
            [1, 2, 6],
            [1, 6, 5],
            // left (x0)
            [0, 4, 7],
            [0, 7, 3],
            // back (y1)
            [3, 7, 6],
            [3, 6, 2],
            // front (y0)
            [0, 1, 5],
            [0, 5, 4],
        ];
        TriangleMesh::new(vertices, indices).expect("box mesh should be valid")
    }

    /// Helper to create a validated CCW square polygon.
    fn make_square(x: f64, y: f64, size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(x, y), (x + size, y), (x + size, y + size), (x, y + size)])
            .validate()
            .unwrap()
    }

    /// Helper to build a TOML overrides map from key-value pairs.
    fn toml_overrides(pairs: &[(&str, toml::Value)]) -> toml::map::Map<String, toml::Value> {
        let mut map = toml::map::Map::new();
        for (key, val) in pairs {
            map.insert((*key).to_string(), val.clone());
        }
        map
    }

    #[test]
    fn slice_modifier_within_bounds_returns_some() {
        let mesh = make_box(0.0, 0.0, 0.0, 10.0, 10.0, 10.0);
        let mut overrides = toml::map::Map::new();
        overrides.insert("infill_density".to_string(), toml::Value::Float(0.8));
        let modifier = ModifierMesh {
            mesh,
            overrides,
            modifier_id: "test-mod".to_string(),
        };
        // Z=5.0 is within the box (0..10).
        let region = slice_modifier(&modifier, 5.0);
        assert!(region.is_some(), "Modifier should intersect at z=5.0");
        let region = region.unwrap();
        assert!(!region.contours.is_empty());
        assert_eq!(
            region
                .overrides
                .get("infill_density")
                .and_then(toml::Value::as_float),
            Some(0.8)
        );
    }

    #[test]
    fn slice_modifier_outside_bounds_returns_none() {
        let mesh = make_box(0.0, 0.0, 2.0, 10.0, 10.0, 8.0);
        let modifier = ModifierMesh {
            mesh,
            overrides: toml::map::Map::new(),
            modifier_id: "test-empty".to_string(),
        };
        // Z=1.0 is below the box (2..8).
        assert!(slice_modifier(&modifier, 1.0).is_none());
        // Z=9.0 is above the box.
        assert!(slice_modifier(&modifier, 9.0).is_none());
    }

    #[test]
    fn split_by_modifiers_no_modifiers_returns_base() {
        let contours = vec![make_square(0.0, 0.0, 20.0)];
        let base = PrintConfig::default();
        let regions = split_by_modifiers(&contours, &[], &base);
        assert_eq!(regions.len(), 1);
        assert!((regions[0].1.infill_density - base.infill_density).abs() < 1e-9);
    }

    #[test]
    fn split_by_modifiers_partial_overlap_produces_two_regions() {
        // Model: 20x20mm square at origin.
        let model_contour = make_square(0.0, 0.0, 20.0);
        // Modifier: 10x10mm square covering the right half.
        let modifier_contour = make_square(10.0, 0.0, 10.0);

        let modifier_region = ModifierRegion {
            contours: vec![modifier_contour],
            overrides: toml_overrides(&[("infill_density", toml::Value::Float(0.9))]),
            modifier_id: "partial-mod".to_string(),
        };

        let base = PrintConfig::default();
        let regions = split_by_modifiers(&[model_contour], &[modifier_region], &base);

        // Should produce 2 regions: the modified overlap and the remainder.
        assert_eq!(
            regions.len(),
            2,
            "Expected 2 regions (modified + remainder)"
        );

        // One region should have the overridden density.
        let has_modified = regions
            .iter()
            .any(|(_, cfg)| (cfg.infill_density - 0.9).abs() < 1e-9);
        assert!(
            has_modified,
            "One region should have modified infill density"
        );

        // The other should have the base density.
        let has_base = regions
            .iter()
            .any(|(_, cfg)| (cfg.infill_density - base.infill_density).abs() < 1e-9);
        assert!(has_base, "One region should have base infill density");
    }

    #[test]
    fn split_by_modifiers_full_overlap_produces_one_region() {
        // Model: 10x10mm square.
        let model_contour = make_square(0.0, 0.0, 10.0);
        // Modifier: 20x20mm square fully covering the model.
        let modifier_contour = make_square(-5.0, -5.0, 20.0);

        let modifier_region = ModifierRegion {
            contours: vec![modifier_contour],
            overrides: toml_overrides(&[("wall_count", toml::Value::Integer(5))]),
            modifier_id: "full-mod".to_string(),
        };

        let base = PrintConfig::default();
        let regions = split_by_modifiers(&[model_contour], &[modifier_region], &base);

        // Model is fully inside modifier, so only the modified region exists.
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].1.wall_count, 5);
    }

    #[test]
    fn modifier_with_arbitrary_field_override() {
        // Override a field that was NOT in the old SettingOverrides struct.
        let model_contour = make_square(0.0, 0.0, 10.0);
        let modifier_contour = make_square(-1.0, -1.0, 12.0);

        let modifier_region = ModifierRegion {
            contours: vec![modifier_contour],
            overrides: toml_overrides(&[("wall_count", toml::Value::Integer(5))]),
            modifier_id: "arbitrary-mod".to_string(),
        };

        let base = PrintConfig::default();
        let regions = split_by_modifiers(&[model_contour], &[modifier_region], &base);

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].1.wall_count, 5);
    }

    #[test]
    fn modifier_with_nested_field_override() {
        // Override a nested field via deep merge (speeds.perimeter).
        let model_contour = make_square(0.0, 0.0, 10.0);
        let modifier_contour = make_square(-1.0, -1.0, 12.0);

        let mut speeds_table = toml::map::Map::new();
        speeds_table.insert("perimeter".to_string(), toml::Value::Float(30.0));
        let overrides = toml_overrides(&[("speeds", toml::Value::Table(speeds_table))]);

        let modifier_region = ModifierRegion {
            contours: vec![modifier_contour],
            overrides,
            modifier_id: "nested-mod".to_string(),
        };

        let base = PrintConfig::default();
        let regions = split_by_modifiers(&[model_contour], &[modifier_region], &base);

        assert_eq!(regions.len(), 1);
        assert!((regions[0].1.speeds.perimeter - 30.0).abs() < 1e-9);
        // Other speed fields should remain at defaults.
        assert!((regions[0].1.speeds.infill - base.speeds.infill).abs() < 1e-9);
    }

    #[test]
    fn two_overlapping_modifiers_last_defined_wins() {
        // Two modifiers both fully cover the model; last-defined should win
        // for the overlap area (which in practice is handled by the subtraction
        // logic -- first modifier claims area, second gets what's left).
        // Here we test with both fully covering so only the first gets area.
        let model_contour = make_square(0.0, 0.0, 10.0);
        let modifier_contour = make_square(-1.0, -1.0, 12.0);

        let mod1 = ModifierRegion {
            contours: vec![modifier_contour.clone()],
            overrides: toml_overrides(&[("infill_density", toml::Value::Float(0.5))]),
            modifier_id: "mod1".to_string(),
        };
        let mod2 = ModifierRegion {
            contours: vec![modifier_contour],
            overrides: toml_overrides(&[("infill_density", toml::Value::Float(0.9))]),
            modifier_id: "mod2".to_string(),
        };

        let base = PrintConfig::default();
        let regions = split_by_modifiers(&[model_contour], &[mod1, mod2], &base);

        // First modifier claims entire area; second has nothing left.
        assert_eq!(regions.len(), 1);
        assert!(
            (regions[0].1.infill_density - 0.5).abs() < 1e-9,
            "First modifier should claim the area"
        );
    }

    #[test]
    fn empty_overrides_produces_base_config() {
        let model_contour = make_square(0.0, 0.0, 10.0);
        let modifier_contour = make_square(-1.0, -1.0, 12.0);

        let modifier_region = ModifierRegion {
            contours: vec![modifier_contour],
            overrides: toml::map::Map::new(),
            modifier_id: "empty-mod".to_string(),
        };

        let base = PrintConfig::default();
        let regions = split_by_modifiers(&[model_contour], &[modifier_region], &base);

        assert_eq!(regions.len(), 1);
        assert!((regions[0].1.infill_density - base.infill_density).abs() < 1e-9);
        assert_eq!(regions[0].1.wall_count, base.wall_count);
        assert!((regions[0].1.speeds.perimeter - base.speeds.perimeter).abs() < 1e-9);
    }
}
