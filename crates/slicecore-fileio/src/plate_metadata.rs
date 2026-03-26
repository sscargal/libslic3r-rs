//! Plate metadata types for 3MF project export.
//!
//! Each plate in a multi-plate 3MF project has associated metadata serialized
//! as JSON: object positions, bounding boxes, print statistics, and filament
//! slot mapping.

use serde::Serialize;

/// Per-plate metadata for a 3MF project file.
#[derive(Debug, Clone, Serialize)]
pub struct PlateMetadata {
    /// 1-indexed plate number.
    pub plate_index: u32,
    /// Objects placed on this plate.
    pub objects: Vec<PlateObject>,
    /// Plate build area size `[x, y]` in mm.
    pub plate_size: [f64; 2],
    /// Print statistics for this plate.
    pub statistics: PlateStatistics,
    /// Filament slot assignments (AMS mapping), omitted if `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filament_mapping: Option<Vec<FilamentSlot>>,
}

/// An object placed on a plate.
#[derive(Debug, Clone, Serialize)]
pub struct PlateObject {
    /// Object name.
    pub name: String,
    /// Object position `[x, y, z]` in mm.
    pub position: [f64; 3],
    /// Axis-aligned bounding box `[min_x, min_y, min_z, max_x, max_y, max_z]`.
    pub bounding_box: [f64; 6],
    /// Number of triangles in the mesh.
    pub triangle_count: usize,
}

/// Print statistics for a single plate.
#[derive(Debug, Clone, Serialize)]
pub struct PlateStatistics {
    /// Total filament length in mm.
    pub filament_length_mm: f64,
    /// Total filament weight in grams.
    pub filament_weight_g: f64,
    /// Estimated filament cost.
    pub filament_cost: f64,
    /// Estimated print time in seconds.
    pub estimated_time_seconds: f64,
    /// Total number of layers.
    pub layer_count: usize,
}

/// A filament slot assignment for AMS.
#[derive(Debug, Clone, Serialize)]
pub struct FilamentSlot {
    /// AMS tray identifier: "0"-"3" or "external".
    pub slot: String,
    /// Filament type (e.g., "PLA", "PETG").
    pub filament_type: String,
    /// Optional filament color as hex (e.g., "#FFFFFF").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plate_metadata() -> PlateMetadata {
        PlateMetadata {
            plate_index: 1,
            objects: vec![PlateObject {
                name: "Cube".to_string(),
                position: [100.0, 100.0, 0.0],
                bounding_box: [90.0, 90.0, 0.0, 110.0, 110.0, 20.0],
                triangle_count: 12,
            }],
            plate_size: [256.0, 256.0],
            statistics: PlateStatistics {
                filament_length_mm: 5000.0,
                filament_weight_g: 15.0,
                filament_cost: 0.45,
                estimated_time_seconds: 3600.0,
                layer_count: 100,
            },
            filament_mapping: Some(vec![FilamentSlot {
                slot: "0".to_string(),
                filament_type: "PLA".to_string(),
                color: Some("#FF0000".to_string()),
            }]),
        }
    }

    #[test]
    fn test_plate_metadata_serializes_to_json() {
        let meta = sample_plate_metadata();
        let json = serde_json::to_string_pretty(&meta).unwrap();
        assert!(json.contains("\"plate_index\""));
        assert!(json.contains("\"statistics\""));
        assert!(json.contains("\"filament_length_mm\""));
    }

    #[test]
    fn test_plate_metadata_no_filament_mapping_omits_field() {
        let mut meta = sample_plate_metadata();
        meta.filament_mapping = None;
        let json = serde_json::to_string_pretty(&meta).unwrap();
        assert!(
            !json.contains("\"filament_mapping\""),
            "filament_mapping should be omitted when None"
        );
    }

    #[test]
    fn test_filament_slot_no_color_omits_field() {
        let slot = FilamentSlot {
            slot: "external".to_string(),
            filament_type: "PETG".to_string(),
            color: None,
        };
        let json = serde_json::to_string(&slot).unwrap();
        assert!(
            !json.contains("\"color\""),
            "color should be omitted when None"
        );
    }
}
