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

use serde::{Deserialize, Serialize};
use slicecore_math::{IPoint2, Point2};

use crate::config::{PrintConfig, ScarfJointType};
use crate::extrusion::compute_e_value;
use crate::gap_fill::GapFillPath;
use crate::infill::LayerInfill;
use crate::perimeter::ContourPerimeters;
use crate::scarf::apply_scarf_joint;
use crate::seam::select_seam_point;

/// The type of feature being printed (affects speed and extrusion settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Gap fill between perimeters.
    GapFill,
    /// Variable-width perimeter (Arachne).
    VariableWidthPerimeter,
    /// Support structure extrusion.
    Support,
    /// Support interface layer extrusion (dense contact surface).
    SupportInterface,
    /// Bridge extrusion (unsupported horizontal span).
    Bridge,
    /// Ironing pass over top surfaces (very low flow, tight spacing).
    Ironing,
    /// Purge tower extrusion (multi-material waste management).
    PurgeTower,
    /// Non-extrusion travel move.
    Travel,
}

/// A single extrusion or travel segment in the toolpath.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Extrusion width override in mm (None = use config default).
    /// Used by Arachne variable-width perimeters.
    pub extrusion_width: Option<f64>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Assembles perimeters, gap fills, and infill into an ordered layer toolpath.
///
/// The assembly order is:
/// 1. Perimeters (in wall order per config): outer/inner shells converted to
///    sequential line segments with Travel moves between disconnected paths.
///    Each perimeter polygon starts at the seam-selected vertex.
///    If scarf joint is enabled, it is applied to each perimeter polygon
///    after segment generation.
/// 2. Gap fill paths: thin extrusions filling narrow gaps between perimeters.
/// 3. Infill lines: nearest-neighbor ordered with Travel moves between lines.
///
/// # Parameters
/// - `layer_index`: Index of this layer.
/// - `z`: Z height of this layer in mm.
/// - `layer_height`: Height of this layer in mm.
/// - `perimeters`: Perimeter shells from [`generate_perimeters`](crate::perimeter::generate_perimeters).
/// - `gap_fills`: Gap fill paths from [`detect_and_fill_gaps`](crate::gap_fill::detect_and_fill_gaps).
/// - `infill`: Infill lines from [`generate_rectilinear_infill`](crate::infill::generate_rectilinear_infill).
/// - `config`: Print configuration for speeds and extrusion parameters.
/// - `previous_seam`: Seam point from the previous layer for cross-layer alignment.
///
/// # Returns
/// A tuple of `(LayerToolpath, Option<IPoint2>)` where the second element is
/// the last seam point used on this layer (for cross-layer seam tracking).
#[allow(clippy::too_many_arguments)]
pub fn assemble_layer_toolpath(
    layer_index: usize,
    z: f64,
    layer_height: f64,
    perimeters: &[ContourPerimeters],
    gap_fills: &[GapFillPath],
    infill: &LayerInfill,
    config: &PrintConfig,
    previous_seam: Option<IPoint2>,
) -> (LayerToolpath, Option<IPoint2>) {
    let mut segments = Vec::new();

    let extrusion_width = config.extrusion_width();
    let is_first_layer = layer_index == 0;

    // Speeds in mm/min (config stores mm/s).
    let perimeter_speed = if is_first_layer {
        config.speeds.first_layer * 60.0
    } else {
        config.speeds.perimeter * 60.0
    };
    let infill_speed = if is_first_layer {
        config.speeds.first_layer * 60.0
    } else {
        config.speeds.infill * 60.0
    };
    let travel_speed = config.speeds.travel * 60.0;

    // Track the current nozzle position for inserting travel moves.
    let mut current_pos: Option<Point2> = None;

    // Track seam point for cross-layer alignment.
    let mut last_seam: Option<IPoint2> = previous_seam;

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
                let n = pts.len();

                // Select the seam point (starting vertex) for this polygon.
                let seam_idx = select_seam_point(
                    polygon,
                    config.seam_position,
                    last_seam,
                    layer_index,
                );

                // Update the seam tracking point.
                last_seam = Some(pts[seam_idx]);

                // Convert seam point to mm.
                let (seam_x, seam_y) = pts[seam_idx].to_mm();
                let seam_pt = Point2::new(seam_x, seam_y);

                // Insert travel to the seam point of this polygon if needed.
                if let Some(pos) = current_pos {
                    let dist = distance(&pos, &seam_pt);
                    if dist > 0.001 {
                        segments.push(ToolpathSegment {
                            start: pos,
                            end: seam_pt,
                            feature: FeatureType::Travel,
                            e_value: 0.0,
                            feedrate: travel_speed,
                            z,
                            extrusion_width: None,
                        });
                    }
                }

                // Emit extrusion segments starting from the seam point,
                // wrapping around the polygon.
                let mut polygon_segments: Vec<ToolpathSegment> = Vec::new();
                let mut prev = seam_pt;
                for offset in 1..=n {
                    let idx = (seam_idx + offset) % n;
                    let (px, py) = pts[idx].to_mm();
                    let pt = Point2::new(px, py);
                    let seg_len = distance(&prev, &pt);

                    if seg_len > 0.0001 {
                        let e = compute_e_value(
                            seg_len,
                            extrusion_width,
                            layer_height,
                            config.filament.diameter,
                            config.extrusion_multiplier,
                        );

                        polygon_segments.push(ToolpathSegment {
                            start: prev,
                            end: pt,
                            feature,
                            e_value: e,
                            feedrate: perimeter_speed,
                            z,
                            extrusion_width: None,
                        });
                    }

                    prev = pt;
                }

                // Apply scarf joint if enabled and applicable to this polygon type.
                if config.scarf_joint.enabled {
                    let should_apply = if shell.is_outer {
                        true
                    } else {
                        config.scarf_joint.scarf_inner_walls
                    };
                    // Skip holes unless ContourAndHole is set.
                    let skip_hole = !shell.is_outer
                        && config.scarf_joint.scarf_joint_type == ScarfJointType::Contour;
                    if should_apply && !skip_hole {
                        apply_scarf_joint(
                            &mut polygon_segments,
                            &config.scarf_joint,
                            layer_height,
                            z,
                        );
                    }
                }

                segments.extend(polygon_segments);

                // After wrapping around, prev should be back at seam_pt.
                // No explicit close needed since we iterate n edges (seam_idx -> ... -> seam_idx).
                current_pos = Some(seam_pt);
            }
        }
    }

    // --- Gap Fill ---
    if !gap_fills.is_empty() {
        for gap_path in gap_fills {
            if gap_path.points.len() < 2 {
                continue;
            }

            // Convert first point to mm for travel.
            let (fx, fy) = gap_path.points[0].to_mm();
            let first_pt = Point2::new(fx, fy);

            // Insert travel to gap fill path start if needed.
            if let Some(pos) = current_pos {
                let dist = distance(&pos, &first_pt);
                if dist > 0.001 {
                    segments.push(ToolpathSegment {
                        start: pos,
                        end: first_pt,
                        feature: FeatureType::Travel,
                        e_value: 0.0,
                        feedrate: travel_speed,
                        z,
                        extrusion_width: None,
                    });
                }
            }

            // Emit extrusion segments along the gap fill path.
            let mut prev = first_pt;
            for i in 1..gap_path.points.len() {
                let (px, py) = gap_path.points[i].to_mm();
                let pt = Point2::new(px, py);
                let seg_len = distance(&prev, &pt);

                if seg_len > 0.0001 {
                    // Use the gap fill's width for E-value computation.
                    let e = compute_e_value(
                        seg_len,
                        gap_path.width,
                        layer_height,
                        config.filament.diameter,
                        config.extrusion_multiplier,
                    );

                    segments.push(ToolpathSegment {
                        start: prev,
                        end: pt,
                        feature: FeatureType::GapFill,
                        e_value: e,
                        feedrate: perimeter_speed,
                        z,
                        extrusion_width: None,
                    });
                }

                prev = pt;
            }

            current_pos = Some(prev);
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
                        extrusion_width: None,
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
                    config.filament.diameter,
                    config.extrusion_multiplier,
                );

                segments.push(ToolpathSegment {
                    start: start_pt,
                    end: end_pt,
                    feature: infill_feature,
                    e_value: e,
                    feedrate: infill_speed,
                    z,
                    extrusion_width: None,
                });

                current_pos = Some(end_pt);
            }
        }
    }

    (
        LayerToolpath {
            layer_index,
            z,
            layer_height,
            segments,
        },
        last_seam,
    )
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

        let (toolpath, _) = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &[], &infill, &config, None);

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

        let (toolpath, _) = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &[], &infill, &config, None);

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

        let (toolpath, _) = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &[], &infill, &config, None);

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
        let config = PrintConfig::default(); // speeds.first_layer=20, speeds.perimeter=45, speeds.infill=80

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let infill = LayerInfill {
            lines: Vec::new(),
            is_solid: false,
        };

        // Layer 0 (first layer).
        let (toolpath, _) = assemble_layer_toolpath(0, 0.3, 0.3, &perimeters, &[], &infill, &config, None);
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
        let config = PrintConfig::default(); // speeds.perimeter=45, speeds.infill=80, speeds.travel=150

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let inner_contour = &perimeters[0].inner_contour;

        let infill_lines = generate_rectilinear_infill(inner_contour, 0.2, 0.0, config.extrusion_width());
        let infill = LayerInfill {
            lines: infill_lines,
            is_solid: false,
        };

        // Layer 2 (not first layer).
        let (toolpath, _) = assemble_layer_toolpath(2, 0.7, 0.2, &perimeters, &[], &infill, &config, None);

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

        let (toolpath, _) = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &[], &infill, &config, None);

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

        let (toolpath, _) = assemble_layer_toolpath(0, 0.2, 0.2, &[], &[], &infill, &config, None);
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
        extrusion_width: None,
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
        let (toolpath, _) = assemble_layer_toolpath(1, 0.4, 0.2, &[], &[], &infill, &config, None);

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
    fn toolpath_with_aligned_seam_consecutive_layers_nearby() {
        let square = make_square(20.0);
        let config = PrintConfig {
            seam_position: crate::seam::SeamPosition::Aligned,
            ..Default::default()
        };

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let infill = LayerInfill {
            lines: Vec::new(),
            is_solid: false,
        };

        // Layer 0 -- no previous seam.
        let (tp0, seam0) = assemble_layer_toolpath(0, 0.3, 0.3, &perimeters, &[], &infill, &config, None);
        assert!(!tp0.segments.is_empty(), "Layer 0 should have segments");
        assert!(seam0.is_some(), "Layer 0 should have a seam point");

        // Layer 1 -- pass previous seam from layer 0.
        let (tp1, seam1) = assemble_layer_toolpath(1, 0.5, 0.2, &perimeters, &[], &infill, &config, seam0);
        assert!(!tp1.segments.is_empty(), "Layer 1 should have segments");
        assert!(seam1.is_some(), "Layer 1 should have a seam point");

        // Seam points should be at the same vertex (or very close) across layers.
        let s0 = seam0.unwrap();
        let s1 = seam1.unwrap();
        let dist_sq = crate::seam::distance_squared_i64(s0, s1);
        // Same polygon, same vertex should be selected -- distance should be 0.
        assert_eq!(
            dist_sq, 0,
            "Aligned seam should select the same vertex across layers with the same polygon"
        );
    }

    #[test]
    fn toolpath_with_rear_seam_starts_near_max_y() {
        let square = make_square(20.0);
        let config = PrintConfig {
            seam_position: crate::seam::SeamPosition::Rear,
            ..Default::default()
        };

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let infill = LayerInfill {
            lines: Vec::new(),
            is_solid: false,
        };

        let (toolpath, _) = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &[], &infill, &config, None);

        // Find the first perimeter extrusion segment.
        let first_perim = toolpath.segments.iter().find(|s| {
            s.feature == FeatureType::OuterPerimeter || s.feature == FeatureType::InnerPerimeter
        });

        assert!(first_perim.is_some(), "Should have perimeter segments");
        let seg = first_perim.unwrap();

        // The first perimeter segment should start near maximum Y (20mm).
        assert!(
            seg.start.y > 15.0,
            "Rear seam: first perimeter should start near max Y (20mm), got y={}",
            seg.start.y
        );
    }

    #[test]
    fn toolpath_seam_rotation_forms_complete_perimeter() {
        let square = make_square(20.0);
        let config = default_config();

        let perimeters = generate_perimeters(&[square.clone()], &config);
        let infill = LayerInfill {
            lines: Vec::new(),
            is_solid: false,
        };

        let (toolpath, _) = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &[], &infill, &config, None);

        // Count total perimeter extrusion length.
        let total_perim_len: f64 = toolpath
            .segments
            .iter()
            .filter(|s| {
                s.feature == FeatureType::OuterPerimeter
                    || s.feature == FeatureType::InnerPerimeter
            })
            .map(|s| s.length())
            .sum();

        // A 20mm square perimeter is ~80mm (4 sides of 20mm).
        // With 2 shells, the outer is ~80mm and inner is smaller.
        // Total should be significantly positive.
        assert!(
            total_perim_len > 50.0,
            "Total perimeter length should be substantial (>50mm), got {}",
            total_perim_len
        );

        // Verify E-values are still correct after seam rotation.
        for seg in &toolpath.segments {
            match seg.feature {
                FeatureType::Travel => {
                    assert!(
                        seg.e_value.abs() < 1e-15,
                        "Travel segments should have zero E after seam rotation"
                    );
                }
                FeatureType::OuterPerimeter | FeatureType::InnerPerimeter => {
                    assert!(
                        seg.e_value > 0.0,
                        "Perimeter segments should have positive E after seam rotation"
                    );
                }
                _ => {}
            }
        }
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

    #[test]
    fn toolpath_scarf_enabled_creates_z_variation() {
        let square = make_square(20.0);
        let config = PrintConfig {
            scarf_joint: crate::config::ScarfJointConfig {
                enabled: true,
                scarf_length: 10.0,
                scarf_start_height: 0.5,
                scarf_steps: 5,
                scarf_flow_ratio: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let perimeters = generate_perimeters(&[square], &config);
        let infill = LayerInfill {
            lines: Vec::new(),
            is_solid: false,
        };

        let (toolpath, _) = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &[], &infill, &config, None);

        // With scarf enabled, some perimeter segments should have Z != 0.4.
        let has_z_variation = toolpath
            .segments
            .iter()
            .filter(|s| s.feature != FeatureType::Travel)
            .any(|s| (s.z - 0.4).abs() > 0.001);

        assert!(
            has_z_variation,
            "Scarf-enabled toolpath should have Z variation in perimeter segments"
        );
    }

    #[test]
    fn toolpath_scarf_disabled_no_z_variation() {
        let square = make_square(20.0);
        let config = PrintConfig {
            scarf_joint: crate::config::ScarfJointConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let perimeters = generate_perimeters(&[square], &config);
        let infill = LayerInfill {
            lines: Vec::new(),
            is_solid: false,
        };

        let (toolpath, _) = assemble_layer_toolpath(1, 0.4, 0.2, &perimeters, &[], &infill, &config, None);

        // With scarf disabled, all perimeter segments should have Z == 0.4.
        for seg in &toolpath.segments {
            assert!(
                (seg.z - 0.4).abs() < 1e-9,
                "Scarf-disabled: all segments should have Z=0.4, got Z={}",
                seg.z
            );
        }
    }

    #[test]
    fn toolpath_segments_default_extrusion_width_is_none() {
        // All segments from assemble_layer_toolpath should have extrusion_width: None.
        let square = make_square(20.0);
        let config = default_config();
        let perimeters = generate_perimeters(&[square], &config);
        let infill = LayerInfill {
            lines: Vec::new(),
            is_solid: false,
        };

        let (toolpath, _) = assemble_layer_toolpath(
            1, 0.4, 0.2, &perimeters, &[], &infill, &config, None,
        );

        for seg in &toolpath.segments {
            assert!(
                seg.extrusion_width.is_none(),
                "Classic perimeter segments should have extrusion_width: None, got {:?}",
                seg.extrusion_width
            );
        }
    }

    #[test]
    fn variable_width_perimeter_feature_type_exists() {
        // Verify the VariableWidthPerimeter feature type can be used in a segment.
        let seg = ToolpathSegment {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(5.0, 0.0),
            feature: FeatureType::VariableWidthPerimeter,
            e_value: 0.1,
            feedrate: 2700.0,
            z: 0.4,
            extrusion_width: Some(0.35),
        };

        assert_eq!(seg.feature, FeatureType::VariableWidthPerimeter);
        assert_eq!(seg.extrusion_width, Some(0.35));
        assert!(seg.length() > 0.0);
    }
}
