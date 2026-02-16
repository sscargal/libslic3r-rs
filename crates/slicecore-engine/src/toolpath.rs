//! Toolpath types and layer toolpath assembly.
//!
//! Toolpaths are the intermediate representation between geometry (contours,
//! perimeter shells, infill lines) and G-code. Each [`ToolpathSegment`]
//! represents a single linear move with associated metadata (feature type,
//! E-axis value, feedrate, Z height).
//!
//! [`LayerToolpath`] collects all segments for a single layer in print order.
//! The assembly function converts perimeters and infill into ordered segments
//! with travel moves inserted between disconnected paths.

use slicecore_math::Point2;

use crate::config::PrintConfig;
use crate::extrusion::compute_e_value;
use crate::infill::LayerInfill;
use crate::perimeter::ContourPerimeters;

/// The type of feature being printed (affects speed and extrusion settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureType {
    /// Outermost visible perimeter wall.
    OuterPerimeter,
    /// Inner perimeter walls.
    InnerPerimeter,
    /// Solid infill (100% density, top/bottom surfaces).
    SolidInfill,
    /// Sparse infill (configured density).
    SparseInfill,
    /// Skirt outline.
    Skirt,
    /// Brim adhesion aid.
    Brim,
    /// Non-extrusion travel move.
    Travel,
}

/// A single extrusion or travel segment in the toolpath.
#[derive(Debug, Clone)]
pub struct ToolpathSegment {
    /// Start position in mm.
    pub start: Point2,
    /// End position in mm.
    pub end: Point2,
    /// What feature is being printed.
    pub feature: FeatureType,
    /// E-axis value in mm (0.0 for travel moves).
    pub e_value: f64,
    /// Feedrate in mm/min.
    pub feedrate: f64,
    /// Z height in mm.
    pub z: f64,
}

impl ToolpathSegment {
    /// Computes the length of this segment in mm.
    pub fn length(&self) -> f64 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// All toolpath segments for a single layer, in print order.
#[derive(Debug, Clone)]
pub struct LayerToolpath {
    /// Index of this layer in the layer stack.
    pub layer_index: usize,
    /// Z height of this layer in mm.
    pub z: f64,
    /// Height of this layer in mm.
    pub layer_height: f64,
    /// Ordered toolpath segments (perimeters, infill, travel).
    pub segments: Vec<ToolpathSegment>,
}

impl LayerToolpath {
    /// Estimates the time to print this layer in seconds.
    ///
    /// Sums `segment_length / feedrate_mm_per_sec` for all segments.
    /// Feedrate is stored in mm/min, so it is divided by 60.
    pub fn estimated_time_seconds(&self) -> f64 {
        self.segments
            .iter()
            .map(|seg| {
                let feedrate_mm_per_sec = seg.feedrate / 60.0;
                if feedrate_mm_per_sec > 0.0 {
                    seg.length() / feedrate_mm_per_sec
                } else {
                    0.0
                }
            })
            .sum()
    }
}

/// Assembles perimeters and infill into an ordered layer toolpath.
///
/// The assembly order is:
/// 1. Perimeters (in wall order per config): outer/inner shells converted to
///    sequential line segments with Travel moves between disconnected paths.
/// 2. Infill lines: nearest-neighbor ordered with Travel moves between lines.
///
/// # Parameters
/// - `layer_index`: Index of this layer.
/// - `z`: Z height of this layer in mm.
/// - `layer_height`: Height of this layer in mm.
/// - `perimeters`: Perimeter shells from [`generate_perimeters`](crate::perimeter::generate_perimeters).
/// - `infill`: Infill lines from [`generate_rectilinear_infill`](crate::infill::generate_rectilinear_infill).
/// - `config`: Print configuration for speeds and extrusion parameters.
///
/// # Returns
/// A [`LayerToolpath`] with all segments in print order.
pub fn assemble_layer_toolpath(
    layer_index: usize,
    z: f64,
    layer_height: f64,
    perimeters: &[ContourPerimeters],
    infill: &LayerInfill,
    config: &PrintConfig,
) -> LayerToolpath {
    let mut segments = Vec::new();

    let extrusion_width = config.extrusion_width();
    let is_first_layer = layer_index == 0;

    // Speeds in mm/min (config stores mm/s).
    let perimeter_speed = if is_first_layer {
        config.first_layer_speed * 60.0
    } else {
        config.perimeter_speed * 60.0
    };
    let infill_speed = if is_first_layer {
        config.first_layer_speed * 60.0
    } else {
        config.infill_speed * 60.0
    };
    let travel_speed = config.travel_speed * 60.0;

    // Track the current nozzle position for inserting travel moves.
    let mut current_pos: Option<Point2> = None;

    // --- Perimeters ---
    for contour_perims in perimeters {
        for shell in &contour_perims.shells {
            let feature = if shell.is_outer {
                FeatureType::OuterPerimeter
            } else {
                FeatureType::InnerPerimeter
            };

            for polygon in &shell.polygons {
                let pts = polygon.points();
                if pts.len() < 2 {
                    continue;
                }

                // Convert first point to mm.
                let (first_x, first_y) = pts[0].to_mm();
                let first_pt = Point2::new(first_x, first_y);

                // Insert travel to the start of this polygon if needed.
                if let Some(pos) = current_pos {
                    let dist = distance(&pos, &first_pt);
                    if dist > 0.001 {
                        // Non-trivial distance -- insert travel.
                        segments.push(ToolpathSegment {
                            start: pos,
                            end: first_pt,
                            feature: FeatureType::Travel,
                            e_value: 0.0,
                            feedrate: travel_speed,
                            z,
                        });
                    }
                }

                // Emit extrusion segments for each edge of the polygon.
                let mut prev = first_pt;
                for ipt in pts.iter().skip(1) {
                    let (px, py) = ipt.to_mm();
                    let pt = Point2::new(px, py);
                    let seg_len = distance(&prev, &pt);

                    if seg_len > 0.0001 {
                        let e = compute_e_value(
                            seg_len,
                            extrusion_width,
                            layer_height,
                            config.filament_diameter,
                            config.extrusion_multiplier,
                        );

                        segments.push(ToolpathSegment {
                            start: prev,
                            end: pt,
                            feature,
                            e_value: e,
                            feedrate: perimeter_speed,
                            z,
                        });
                    }

                    prev = pt;
                }

                // Close the polygon: last point back to first.
                let close_len = distance(&prev, &first_pt);
                if close_len > 0.0001 {
                    let e = compute_e_value(
                        close_len,
                        extrusion_width,
                        layer_height,
                        config.filament_diameter,
                        config.extrusion_multiplier,
                    );

                    segments.push(ToolpathSegment {
                        start: prev,
                        end: first_pt,
                        feature,
                        e_value: e,
                        feedrate: perimeter_speed,
                        z,
                    });
                    current_pos = Some(first_pt);
                } else {
                    current_pos = Some(prev);
                }
            }
        }
    }

    // --- Infill ---
    if !infill.lines.is_empty() {
        let infill_feature = if infill.is_solid {
            FeatureType::SolidInfill
        } else {
            FeatureType::SparseInfill
        };

        // Order infill lines by nearest-neighbor heuristic.
        let ordered_lines = nearest_neighbor_order(&infill.lines, current_pos);

        for line in &ordered_lines {
            let (sx, sy) = line.start.to_mm();
            let (ex, ey) = line.end.to_mm();
            let start_pt = Point2::new(sx, sy);
            let end_pt = Point2::new(ex, ey);

            // Insert travel to infill line start if needed.
            if let Some(pos) = current_pos {
                let dist = distance(&pos, &start_pt);
                if dist > 0.001 {
                    segments.push(ToolpathSegment {
                        start: pos,
                        end: start_pt,
                        feature: FeatureType::Travel,
                        e_value: 0.0,
                        feedrate: travel_speed,
                        z,
                    });
                }
            }

            // Emit the infill extrusion.
            let seg_len = distance(&start_pt, &end_pt);
            if seg_len > 0.0001 {
                let e = compute_e_value(
                    seg_len,
                    extrusion_width,
                    layer_height,
                    config.filament_diameter,
                    config.extrusion_multiplier,
                );

                segments.push(ToolpathSegment {
                    start: start_pt,
                    end: end_pt,
                    feature: infill_feature,
                    e_value: e,
                    feedrate: infill_speed,
                    z,
                });

                current_pos = Some(end_pt);
            }
        }
    }

    LayerToolpath {
        layer_index,
        z,
        layer_height,
        segments,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Euclidean distance between two points in mm.
fn distance(a: &Point2, b: &Point2) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

/// Orders infill lines by nearest-neighbor heuristic.
///
/// Starting from `start_pos` (or the first line if no position given),
/// greedily picks the next line whose start or end is closest to the
/// current position. If the closest endpoint is the line's end, the
/// line is reversed.
fn nearest_neighbor_order(
    lines: &[crate::infill::InfillLine],
    start_pos: Option<Point2>,
) -> Vec<crate::infill::InfillLine> {
    if lines.is_empty() {
        return Vec::new();
    }

    let mut remaining: Vec<(usize, bool)> = (0..lines.len()).map(|i| (i, false)).collect();
    let mut result = Vec::with_capacity(lines.len());

    let mut current = if let Some(pos) = start_pos {
        pos
    } else {
        let (sx, sy) = lines[0].start.to_mm();
        Point2::new(sx, sy)
    };

    for _ in 0..lines.len() {
        let mut best_idx = None;
        let mut best_dist = f64::MAX;
        let mut best_reversed = false;

        for (slot_idx, &(line_idx, used)) in remaining.iter().enumerate() {
            if used {
                continue;
            }

            let (sx, sy) = lines[line_idx].start.to_mm();
            let start_pt = Point2::new(sx, sy);
            let dist_to_start = distance(&current, &start_pt);

            let (ex, ey) = lines[line_idx].end.to_mm();
            let end_pt = Point2::new(ex, ey);
            let dist_to_end = distance(&current, &end_pt);

            let (dist, reversed) = if dist_to_start <= dist_to_end {
                (dist_to_start, false)
            } else {
                (dist_to_end, true)
            };

            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(slot_idx);
                best_reversed = reversed;
            }
        }

        if let Some(idx) = best_idx {
            let (line_idx, _) = remaining[idx];
            remaining[idx].1 = true;

            let line = &lines[line_idx];
            if best_reversed {
                // Reverse the line direction.
                result.push(crate::infill::InfillLine {
                    start: line.end,
                    end: line.start,
                });
                let (ex, ey) = line.start.to_mm();
                current = Point2::new(ex, ey);
            } else {
                result.push(line.clone());
                let (ex, ey) = line.end.to_mm();
                current = Point2::new(ex, ey);
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infill::{generate_rectilinear_infill, InfillLine, LayerInfill};
    use crate::perimeter::generate_perimeters;
    use slicecore_geo::polygon::Polygon;
    use slicecore_math::IPoint2;

    /// Helper to create a validated CCW square.
    fn make_square(size: f64) -> slicecore_geo::polygon::ValidPolygon {
        Polygon::from_mm(&[
            (0.0, 0.0),
            (size, 0.0),
            (size, size),
            (0.0, size),
        ])
        .validate()
        .unwrap()
    }

    fn default_config() -> PrintConfig {
        PrintConfig::default()
    }

    #[test]
    fn toolpath_assembly_perimeters_before_infill() {
        let square = make_square(20.0);
        let config = default_config();

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let inner_contour = &perimeters[0].inner_contour;

        let infill_lines = generate_rectilinear_infill(inner_contour, 0.2, 0.0, config.extrusion_width());
        let infill = LayerInfill {
            lines: infill_lines,
            is_solid: false,
        };

        let toolpath = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &infill, &config);

        // Should have segments.
        assert!(
            !toolpath.segments.is_empty(),
            "Toolpath should have segments"
        );

        // Find the first perimeter and first infill segment indices.
        let first_perim_idx = toolpath
            .segments
            .iter()
            .position(|s| {
                s.feature == FeatureType::OuterPerimeter
                    || s.feature == FeatureType::InnerPerimeter
            });
        let first_infill_idx = toolpath
            .segments
            .iter()
            .position(|s| {
                s.feature == FeatureType::SparseInfill || s.feature == FeatureType::SolidInfill
            });

        if let (Some(perim), Some(infill)) = (first_perim_idx, first_infill_idx) {
            assert!(
                perim < infill,
                "Perimeters (idx {}) should come before infill (idx {})",
                perim,
                infill
            );
        }
    }

    #[test]
    fn toolpath_travel_between_disconnected_paths() {
        let square = make_square(20.0);
        let config = default_config();

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let inner_contour = &perimeters[0].inner_contour;

        let infill_lines = generate_rectilinear_infill(inner_contour, 0.2, 0.0, config.extrusion_width());
        let infill = LayerInfill {
            lines: infill_lines,
            is_solid: false,
        };

        let toolpath = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &infill, &config);

        let travel_count = toolpath
            .segments
            .iter()
            .filter(|s| s.feature == FeatureType::Travel)
            .count();

        assert!(
            travel_count > 0,
            "Should have travel segments between disconnected paths"
        );
    }

    #[test]
    fn toolpath_e_values_correct() {
        let square = make_square(20.0);
        let config = default_config();

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let inner_contour = &perimeters[0].inner_contour;

        let infill_lines = generate_rectilinear_infill(inner_contour, 0.2, 0.0, config.extrusion_width());
        let infill = LayerInfill {
            lines: infill_lines,
            is_solid: false,
        };

        let toolpath = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &infill, &config);

        for seg in &toolpath.segments {
            match seg.feature {
                FeatureType::Travel => {
                    assert!(
                        seg.e_value.abs() < 1e-15,
                        "Travel segments should have zero E, got {}",
                        seg.e_value
                    );
                }
                _ => {
                    assert!(
                        seg.e_value > 0.0,
                        "Extrusion segments should have positive E, got {} for {:?}",
                        seg.e_value,
                        seg.feature
                    );
                }
            }
        }
    }

    #[test]
    fn toolpath_first_layer_uses_first_layer_speed() {
        let square = make_square(20.0);
        let config = PrintConfig {
            first_layer_speed: 20.0, // mm/s
            perimeter_speed: 45.0,   // mm/s
            infill_speed: 80.0,      // mm/s
            ..Default::default()
        };

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let infill = LayerInfill {
            lines: Vec::new(),
            is_solid: false,
        };

        // Layer 0 (first layer).
        let toolpath = assemble_layer_toolpath(0, 0.3, 0.3, &perimeters, &infill, &config);
        let first_layer_speed_mmmin = 20.0 * 60.0;

        for seg in &toolpath.segments {
            if seg.feature != FeatureType::Travel {
                assert!(
                    (seg.feedrate - first_layer_speed_mmmin).abs() < 0.1,
                    "First layer extrusion should use first_layer_speed ({} mm/min), got {} mm/min",
                    first_layer_speed_mmmin,
                    seg.feedrate
                );
            }
        }
    }

    #[test]
    fn toolpath_subsequent_layers_use_feature_speeds() {
        let square = make_square(20.0);
        let config = PrintConfig {
            perimeter_speed: 45.0, // mm/s
            infill_speed: 80.0,    // mm/s
            travel_speed: 150.0,   // mm/s
            ..Default::default()
        };

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let inner_contour = &perimeters[0].inner_contour;

        let infill_lines = generate_rectilinear_infill(inner_contour, 0.2, 0.0, config.extrusion_width());
        let infill = LayerInfill {
            lines: infill_lines,
            is_solid: false,
        };

        // Layer 2 (not first layer).
        let toolpath = assemble_layer_toolpath(2, 0.7, 0.2, &perimeters, &infill, &config);

        let perim_speed_mmmin = 45.0 * 60.0;
        let infill_speed_mmmin = 80.0 * 60.0;
        let travel_speed_mmmin = 150.0 * 60.0;

        for seg in &toolpath.segments {
            match seg.feature {
                FeatureType::OuterPerimeter | FeatureType::InnerPerimeter => {
                    assert!(
                        (seg.feedrate - perim_speed_mmmin).abs() < 0.1,
                        "Perimeter speed should be {} mm/min, got {}",
                        perim_speed_mmmin,
                        seg.feedrate
                    );
                }
                FeatureType::SparseInfill | FeatureType::SolidInfill => {
                    assert!(
                        (seg.feedrate - infill_speed_mmmin).abs() < 0.1,
                        "Infill speed should be {} mm/min, got {}",
                        infill_speed_mmmin,
                        seg.feedrate
                    );
                }
                FeatureType::Travel => {
                    assert!(
                        (seg.feedrate - travel_speed_mmmin).abs() < 0.1,
                        "Travel speed should be {} mm/min, got {}",
                        travel_speed_mmmin,
                        seg.feedrate
                    );
                }
                _ => {}
            }
        }
    }

    #[test]
    fn toolpath_estimated_time_positive() {
        let square = make_square(20.0);
        let config = default_config();

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let inner_contour = &perimeters[0].inner_contour;

        let infill_lines = generate_rectilinear_infill(inner_contour, 0.2, 0.0, config.extrusion_width());
        let infill = LayerInfill {
            lines: infill_lines,
            is_solid: false,
        };

        let toolpath = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &infill, &config);

        let time = toolpath.estimated_time_seconds();
        assert!(
            time > 0.0,
            "Estimated time should be positive for a layer with segments, got {}",
            time
        );
    }

    #[test]
    fn toolpath_empty_layer() {
        let config = default_config();
        let infill = LayerInfill {
            lines: Vec::new(),
            is_solid: false,
        };

        let toolpath = assemble_layer_toolpath(0, 0.2, 0.2, &[], &infill, &config);
        assert!(
            toolpath.segments.is_empty(),
            "Empty perimeters and infill should produce empty toolpath"
        );
        assert!(
            (toolpath.estimated_time_seconds() - 0.0).abs() < 1e-15,
            "Empty toolpath should have 0 estimated time"
        );
    }

    #[test]
    fn toolpath_segment_length() {
        let seg = ToolpathSegment {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(3.0, 4.0),
            feature: FeatureType::Travel,
            e_value: 0.0,
            feedrate: 9000.0,
            z: 0.2,
        };
        assert!(
            (seg.length() - 5.0).abs() < 1e-9,
            "Segment length should be 5.0, got {}",
            seg.length()
        );
    }

    #[test]
    fn toolpath_solid_infill_uses_correct_feature() {
        let infill = LayerInfill {
            lines: vec![InfillLine {
                start: IPoint2::from_mm(1.0, 1.0),
                end: IPoint2::from_mm(10.0, 1.0),
            }],
            is_solid: true,
        };

        let config = default_config();
        let toolpath = assemble_layer_toolpath(1, 0.4, 0.2, &[], &infill, &config);

        let has_solid = toolpath
            .segments
            .iter()
            .any(|s| s.feature == FeatureType::SolidInfill);
        assert!(
            has_solid,
            "Solid infill should use SolidInfill feature type"
        );
    }

    #[test]
    fn nearest_neighbor_ordering_reduces_travel() {
        // Create infill lines that would benefit from nearest-neighbor ordering.
        // Lines at y=1, y=2, y=3 -- already in order, so nearest-neighbor should
        // keep them in order or reverse adjacent ones.
        let lines = vec![
            InfillLine {
                start: IPoint2::from_mm(0.0, 1.0),
                end: IPoint2::from_mm(10.0, 1.0),
            },
            InfillLine {
                start: IPoint2::from_mm(0.0, 2.0),
                end: IPoint2::from_mm(10.0, 2.0),
            },
            InfillLine {
                start: IPoint2::from_mm(0.0, 3.0),
                end: IPoint2::from_mm(10.0, 3.0),
            },
        ];

        let ordered = nearest_neighbor_order(&lines, Some(Point2::new(0.0, 0.0)));
        assert_eq!(ordered.len(), 3, "Should have 3 lines");

        // The first line should be the one closest to (0,0), which is line at y=1.
        let (_, first_y) = ordered[0].start.to_mm();
        assert!(
            (first_y - 1.0).abs() < 0.001,
            "First line should be at y~1.0, got y={}",
            first_y
        );
    }
}
