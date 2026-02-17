//! Slicing preview data for visualization.
//!
//! The [`SlicePreview`] struct contains per-layer visualization data including
//! contour boundaries, perimeter paths, infill lines, and travel moves.
//! All data is JSON-serializable via serde for easy consumption by
//! visualization frontends.
//!
//! Use [`generate_preview`] to build a `SlicePreview` from layer toolpaths
//! and contour data produced by the slicing pipeline.

use serde::{Deserialize, Serialize};

use crate::toolpath::{FeatureType, LayerToolpath};
use slicecore_geo::polygon::ValidPolygon;

/// Complete slicing preview for visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlicePreview {
    /// Per-layer preview data.
    pub layers: Vec<LayerPreview>,
    /// Model bounding box [min_x, min_y, min_z, max_x, max_y, max_z] in mm.
    pub bounding_box: [f64; 6],
    /// Total number of layers.
    pub total_layers: usize,
    /// Total estimated print time in seconds.
    pub estimated_time_seconds: f64,
}

/// Preview data for a single layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerPreview {
    /// Z height in mm.
    pub z: f64,
    /// Layer height in mm.
    pub layer_height: f64,
    /// Contour boundaries as polylines (each is Vec of [x, y] in mm).
    pub contours: Vec<Vec<[f64; 2]>>,
    /// Perimeter paths as polylines.
    pub perimeters: Vec<Vec<[f64; 2]>>,
    /// Infill lines as line segment pairs [[start_x, start_y], [end_x, end_y]].
    pub infill_lines: Vec<[[f64; 2]; 2]>,
    /// Travel moves as line segment pairs.
    pub travel_moves: Vec<[[f64; 2]; 2]>,
    /// Feature type label for each path segment.
    pub feature_types: Vec<String>,
}

/// Generates a [`SlicePreview`] from layer toolpaths and contour data.
///
/// # Parameters
/// - `layer_toolpaths`: Ordered layer toolpaths from the slicing pipeline.
/// - `contours_per_layer`: Per-layer contour polygons from the slicer.
/// - `bounding_box`: Model bounding box as [min_x, min_y, min_z, max_x, max_y, max_z].
///
/// # Returns
/// A `SlicePreview` containing visualization data for all layers.
pub fn generate_preview(
    layer_toolpaths: &[LayerToolpath],
    contours_per_layer: &[Vec<ValidPolygon>],
    bounding_box: [f64; 6],
) -> SlicePreview {
    let total_layers = layer_toolpaths.len();
    let estimated_time_seconds: f64 = layer_toolpaths
        .iter()
        .map(|lt| lt.estimated_time_seconds())
        .sum();

    let mut layers = Vec::with_capacity(total_layers);

    for (i, lt) in layer_toolpaths.iter().enumerate() {
        // Convert contour polygons to polylines.
        let contours = if i < contours_per_layer.len() {
            contours_per_layer[i]
                .iter()
                .map(|polygon| {
                    polygon
                        .points()
                        .iter()
                        .map(|pt| {
                            let (x, y) = pt.to_mm();
                            [x, y]
                        })
                        .collect::<Vec<[f64; 2]>>()
                })
                .collect()
        } else {
            Vec::new()
        };

        // Group toolpath segments by feature type.
        let mut perimeters: Vec<Vec<[f64; 2]>> = Vec::new();
        let mut infill_lines: Vec<[[f64; 2]; 2]> = Vec::new();
        let mut travel_moves: Vec<[[f64; 2]; 2]> = Vec::new();
        let mut feature_types: Vec<String> = Vec::new();

        // Build perimeter polylines: consecutive perimeter segments form a polyline.
        let mut current_perim_polyline: Vec<[f64; 2]> = Vec::new();

        for seg in &lt.segments {
            let start = [seg.start.x, seg.start.y];
            let end = [seg.end.x, seg.end.y];

            let feature_label = feature_type_label(seg.feature);
            feature_types.push(feature_label);

            match seg.feature {
                FeatureType::OuterPerimeter
                | FeatureType::InnerPerimeter
                | FeatureType::VariableWidthPerimeter
                | FeatureType::GapFill => {
                    // Extend or start a perimeter polyline.
                    if current_perim_polyline.is_empty() {
                        current_perim_polyline.push(start);
                    } else {
                        // Check if this segment is contiguous with the previous.
                        let last = current_perim_polyline.last().unwrap();
                        let dx = start[0] - last[0];
                        let dy = start[1] - last[1];
                        if (dx * dx + dy * dy).sqrt() > 0.01 {
                            // Gap detected: finish current polyline, start new one.
                            perimeters.push(std::mem::take(&mut current_perim_polyline));
                            current_perim_polyline.push(start);
                        }
                    }
                    current_perim_polyline.push(end);
                }
                FeatureType::SolidInfill | FeatureType::SparseInfill | FeatureType::Support => {
                    // Flush any in-progress perimeter polyline.
                    if !current_perim_polyline.is_empty() {
                        perimeters.push(std::mem::take(&mut current_perim_polyline));
                    }
                    infill_lines.push([start, end]);
                }
                FeatureType::Travel => {
                    // Flush any in-progress perimeter polyline.
                    if !current_perim_polyline.is_empty() {
                        perimeters.push(std::mem::take(&mut current_perim_polyline));
                    }
                    travel_moves.push([start, end]);
                }
                FeatureType::Skirt | FeatureType::Brim => {
                    // Treat skirt/brim like perimeters for visualization.
                    if current_perim_polyline.is_empty() {
                        current_perim_polyline.push(start);
                    } else {
                        let last = current_perim_polyline.last().unwrap();
                        let dx = start[0] - last[0];
                        let dy = start[1] - last[1];
                        if (dx * dx + dy * dy).sqrt() > 0.01 {
                            perimeters.push(std::mem::take(&mut current_perim_polyline));
                            current_perim_polyline.push(start);
                        }
                    }
                    current_perim_polyline.push(end);
                }
            }
        }

        // Flush remaining perimeter polyline.
        if !current_perim_polyline.is_empty() {
            perimeters.push(current_perim_polyline);
        }

        layers.push(LayerPreview {
            z: lt.z,
            layer_height: lt.layer_height,
            contours,
            perimeters,
            infill_lines,
            travel_moves,
            feature_types,
        });
    }

    SlicePreview {
        layers,
        bounding_box,
        total_layers,
        estimated_time_seconds,
    }
}

/// Returns a human-readable label for a feature type.
fn feature_type_label(feature: FeatureType) -> String {
    match feature {
        FeatureType::OuterPerimeter => "outer_perimeter".to_string(),
        FeatureType::InnerPerimeter => "inner_perimeter".to_string(),
        FeatureType::SolidInfill => "solid_infill".to_string(),
        FeatureType::SparseInfill => "sparse_infill".to_string(),
        FeatureType::Skirt => "skirt".to_string(),
        FeatureType::Brim => "brim".to_string(),
        FeatureType::GapFill => "gap_fill".to_string(),
        FeatureType::VariableWidthPerimeter => "variable_width_perimeter".to_string(),
        FeatureType::Support => "support".to_string(),
        FeatureType::Travel => "travel".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::ToolpathSegment;
    use slicecore_math::Point2;

    /// Creates a simple layer toolpath with a perimeter and some infill.
    fn sample_layer_toolpath() -> LayerToolpath {
        LayerToolpath {
            layer_index: 0,
            z: 0.3,
            layer_height: 0.3,
            segments: vec![
                // Perimeter: 4 sides of a square
                ToolpathSegment {
                    start: Point2::new(10.0, 10.0),
                    end: Point2::new(20.0, 10.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.5,
                    feedrate: 2700.0,
                    z: 0.3,
                    extrusion_width: None,
                },
                ToolpathSegment {
                    start: Point2::new(20.0, 10.0),
                    end: Point2::new(20.0, 20.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.5,
                    feedrate: 2700.0,
                    z: 0.3,
                    extrusion_width: None,
                },
                ToolpathSegment {
                    start: Point2::new(20.0, 20.0),
                    end: Point2::new(10.0, 20.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.5,
                    feedrate: 2700.0,
                    z: 0.3,
                    extrusion_width: None,
                },
                ToolpathSegment {
                    start: Point2::new(10.0, 20.0),
                    end: Point2::new(10.0, 10.0),
                    feature: FeatureType::OuterPerimeter,
                    e_value: 0.5,
                    feedrate: 2700.0,
                    z: 0.3,
                    extrusion_width: None,
                },
                // Travel to infill
                ToolpathSegment {
                    start: Point2::new(10.0, 10.0),
                    end: Point2::new(12.0, 12.0),
                    feature: FeatureType::Travel,
                    e_value: 0.0,
                    feedrate: 9000.0,
                    z: 0.3,
                    extrusion_width: None,
                },
                // Infill lines
                ToolpathSegment {
                    start: Point2::new(12.0, 12.0),
                    end: Point2::new(18.0, 12.0),
                    feature: FeatureType::SparseInfill,
                    e_value: 0.3,
                    feedrate: 4800.0,
                    z: 0.3,
                    extrusion_width: None,
                },
                ToolpathSegment {
                    start: Point2::new(18.0, 14.0),
                    end: Point2::new(12.0, 14.0),
                    feature: FeatureType::SparseInfill,
                    e_value: 0.3,
                    feedrate: 4800.0,
                    z: 0.3,
                    extrusion_width: None,
                },
            ],
        }
    }

    fn sample_contours() -> Vec<ValidPolygon> {
        use slicecore_geo::polygon::Polygon;
        vec![Polygon::from_mm(&[
            (10.0, 10.0),
            (20.0, 10.0),
            (20.0, 20.0),
            (10.0, 20.0),
        ])
        .validate()
        .unwrap()]
    }

    #[test]
    fn preview_correct_layer_count() {
        let lt = sample_layer_toolpath();
        let contours = vec![sample_contours()];
        let bbox = [10.0, 10.0, 0.0, 20.0, 20.0, 0.3];

        let preview = generate_preview(&[lt], &contours, bbox);

        assert_eq!(preview.total_layers, 1);
        assert_eq!(preview.layers.len(), 1);
    }

    #[test]
    fn preview_layer_has_nonempty_contours() {
        let lt = sample_layer_toolpath();
        let contours = vec![sample_contours()];
        let bbox = [10.0, 10.0, 0.0, 20.0, 20.0, 0.3];

        let preview = generate_preview(&[lt], &contours, bbox);
        let layer = &preview.layers[0];

        assert!(
            !layer.contours.is_empty(),
            "Layer should have non-empty contours"
        );
        assert!(
            layer.contours[0].len() >= 4,
            "Contour should have at least 4 points for a square"
        );
    }

    #[test]
    fn preview_layer_has_nonempty_infill() {
        let lt = sample_layer_toolpath();
        let contours = vec![sample_contours()];
        let bbox = [10.0, 10.0, 0.0, 20.0, 20.0, 0.3];

        let preview = generate_preview(&[lt], &contours, bbox);
        let layer = &preview.layers[0];

        assert!(
            !layer.infill_lines.is_empty(),
            "Layer with infill segments should have non-empty infill_lines"
        );
        assert_eq!(layer.infill_lines.len(), 2, "Should have 2 infill lines");
    }

    #[test]
    fn preview_serializes_to_valid_json() {
        let lt = sample_layer_toolpath();
        let contours = vec![sample_contours()];
        let bbox = [10.0, 10.0, 0.0, 20.0, 20.0, 0.3];

        let preview = generate_preview(&[lt], &contours, bbox);

        let json = serde_json::to_string(&preview);
        assert!(
            json.is_ok(),
            "Preview should serialize to JSON: {:?}",
            json.err()
        );
        let json_str = json.unwrap();
        assert!(
            !json_str.is_empty(),
            "JSON output should be non-empty"
        );
    }

    #[test]
    fn preview_json_round_trip() {
        let lt = sample_layer_toolpath();
        let contours = vec![sample_contours()];
        let bbox = [10.0, 10.0, 0.0, 20.0, 20.0, 0.3];

        let preview = generate_preview(&[lt], &contours, bbox);

        let json = serde_json::to_string(&preview).unwrap();
        let deserialized: SlicePreview = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.total_layers, preview.total_layers);
        assert_eq!(deserialized.bounding_box, preview.bounding_box);
        assert_eq!(deserialized.layers.len(), preview.layers.len());
        assert!(
            (deserialized.estimated_time_seconds - preview.estimated_time_seconds).abs() < 1e-9
        );

        // Verify layer data round-trips.
        let orig_layer = &preview.layers[0];
        let rt_layer = &deserialized.layers[0];
        assert_eq!(orig_layer.contours.len(), rt_layer.contours.len());
        assert_eq!(orig_layer.perimeters.len(), rt_layer.perimeters.len());
        assert_eq!(orig_layer.infill_lines.len(), rt_layer.infill_lines.len());
        assert_eq!(orig_layer.travel_moves.len(), rt_layer.travel_moves.len());
        assert_eq!(orig_layer.feature_types.len(), rt_layer.feature_types.len());
    }

    #[test]
    fn preview_bounding_box_matches() {
        let lt = sample_layer_toolpath();
        let contours = vec![sample_contours()];
        let bbox = [10.0, 10.0, 0.0, 20.0, 20.0, 0.3];

        let preview = generate_preview(&[lt], &contours, bbox);

        assert_eq!(preview.bounding_box, bbox);
        // Check X range.
        assert!((preview.bounding_box[0] - 10.0).abs() < 1e-9);
        assert!((preview.bounding_box[3] - 20.0).abs() < 1e-9);
    }

    #[test]
    fn preview_has_travel_moves() {
        let lt = sample_layer_toolpath();
        let contours = vec![sample_contours()];
        let bbox = [10.0, 10.0, 0.0, 20.0, 20.0, 0.3];

        let preview = generate_preview(&[lt], &contours, bbox);
        let layer = &preview.layers[0];

        assert!(
            !layer.travel_moves.is_empty(),
            "Layer with travel segments should have non-empty travel_moves"
        );
    }

    #[test]
    fn preview_has_perimeter_polylines() {
        let lt = sample_layer_toolpath();
        let contours = vec![sample_contours()];
        let bbox = [10.0, 10.0, 0.0, 20.0, 20.0, 0.3];

        let preview = generate_preview(&[lt], &contours, bbox);
        let layer = &preview.layers[0];

        assert!(
            !layer.perimeters.is_empty(),
            "Layer with perimeter segments should have perimeter polylines"
        );
        // The 4 contiguous perimeter segments should form 1 polyline with 5 points.
        assert_eq!(layer.perimeters.len(), 1);
        assert_eq!(layer.perimeters[0].len(), 5); // start + 4 ends
    }

    #[test]
    fn preview_feature_types_match_segment_count() {
        let lt = sample_layer_toolpath();
        let contours = vec![sample_contours()];
        let bbox = [10.0, 10.0, 0.0, 20.0, 20.0, 0.3];

        let preview = generate_preview(&[lt], &contours, bbox);
        let layer = &preview.layers[0];

        // feature_types should have one entry per toolpath segment.
        assert_eq!(
            layer.feature_types.len(),
            7,
            "Should have one feature type label per segment"
        );
    }

    #[test]
    fn preview_empty_layers() {
        let lt = LayerToolpath {
            layer_index: 0,
            z: 0.3,
            layer_height: 0.3,
            segments: Vec::new(),
        };
        let bbox = [0.0, 0.0, 0.0, 10.0, 10.0, 0.3];

        let preview = generate_preview(&[lt], &[], bbox);

        assert_eq!(preview.total_layers, 1);
        let layer = &preview.layers[0];
        assert!(layer.contours.is_empty());
        assert!(layer.perimeters.is_empty());
        assert!(layer.infill_lines.is_empty());
        assert!(layer.travel_moves.is_empty());
        assert!(layer.feature_types.is_empty());
    }
}
