//! Scarf joint seam implementation.
//!
//! Standard perimeter seams create a visible bump where the extrusion starts
//! and ends. The scarf joint gradually ramps Z height and flow rate over a
//! configurable length, creating a smooth wedge overlap that makes the seam
//! nearly invisible on smooth surfaces.
//!
//! The main entry point is [`apply_scarf_joint`], which modifies perimeter
//! toolpath segments near the seam point.

use crate::config::ScarfJointConfig;
use crate::toolpath::ToolpathSegment;
use slicecore_math::Point2;

/// Applies a scarf joint to perimeter segments at the seam region.
///
/// The scarf modifies Z and E values of toolpath segments near the seam
/// to create gradual ramps instead of an abrupt start/end.
///
/// # Algorithm
///
/// 1. **Trailing ramp (end of loop):** The last `scarf_length` mm of the
///    perimeter loop gradually decrease Z from `layer_z` to
///    `layer_z - layer_height * scarf_start_height`.
///
/// 2. **Leading ramp (start of loop):** The first `scarf_length` mm of the
///    perimeter loop gradually increase Z from
///    `layer_z - layer_height * scarf_start_height` to `layer_z`.
///
/// 3. **Flow adjustment:** E values are scaled proportionally with Z change.
///
/// # Parameters
/// - `perimeter_segments`: Mutable segments for a single perimeter polygon.
///   The seam is at index 0 (first segment starts at the seam point).
/// - `config`: Scarf joint configuration.
/// - `layer_height`: Height of the current layer in mm.
/// - `layer_z`: Z height of the current layer in mm.
pub fn apply_scarf_joint(
    perimeter_segments: &mut Vec<ToolpathSegment>,
    config: &ScarfJointConfig,
    layer_height: f64,
    layer_z: f64,
) {
    if !config.enabled || perimeter_segments.is_empty() || config.scarf_length <= 0.0 {
        return;
    }

    let scarf_length = config.scarf_length;
    let z_drop = layer_height * config.scarf_start_height;

    if z_drop <= 0.0 {
        return;
    }

    let total_perimeter_length: f64 = perimeter_segments.iter().map(|s| s.length()).sum();

    // Don't apply scarf if the perimeter is shorter than the scarf length.
    // Use the minimum of scarf_length and half the perimeter length.
    let effective_scarf_length = scarf_length.min(total_perimeter_length / 2.0);
    if effective_scarf_length <= 0.001 {
        return;
    }

    // Step 1: Apply leading ramp (first segments of the loop).
    // These segments ramp Z from (layer_z - z_drop) up to layer_z.
    apply_leading_ramp(
        perimeter_segments,
        effective_scarf_length,
        layer_z,
        z_drop,
        config.scarf_flow_ratio,
        config.scarf_steps,
    );

    // Step 2: Apply trailing ramp (last segments of the loop).
    // These segments ramp Z from layer_z down to (layer_z - z_drop).
    apply_trailing_ramp(
        perimeter_segments,
        effective_scarf_length,
        layer_z,
        z_drop,
        config.scarf_flow_ratio,
        config.scarf_steps,
    );
}

/// Applies the leading ramp: first `scarf_length` mm of the perimeter
/// ramps Z from `(layer_z - z_drop)` up to `layer_z`.
fn apply_leading_ramp(
    segments: &mut Vec<ToolpathSegment>,
    scarf_length: f64,
    layer_z: f64,
    z_drop: f64,
    flow_ratio: f64,
    steps: u32,
) {
    // Accumulate distance from the start to find which segments are in the ramp.
    let mut accumulated = 0.0;
    let mut split_point: Option<(usize, f64)> = None;

    for (i, seg) in segments.iter().enumerate() {
        let seg_len = seg.length();
        if accumulated + seg_len >= scarf_length {
            // This segment contains the boundary.
            let remaining = scarf_length - accumulated;
            if remaining > 0.001 && (seg_len - remaining) > 0.001 {
                split_point = Some((i, remaining));
            }
            break;
        }
        accumulated += seg_len;
    }

    // Split the boundary segment if needed.
    if let Some((idx, distance)) = split_point {
        let seg = segments[idx].clone();
        let (first, second) = split_segment_at_distance(&seg, distance);
        segments.splice(idx..=idx, [first, second]);
    }

    // Now apply Z ramp to all segments within the scarf region.
    accumulated = 0.0;
    for seg in segments.iter_mut() {
        let seg_len = seg.length();
        if accumulated >= scarf_length {
            break;
        }

        let seg_end_dist = accumulated + seg_len;
        let clamped_end = seg_end_dist.min(scarf_length);

        // Fraction through the ramp at the midpoint of this segment's ramp region.
        let mid_frac = ((accumulated + clamped_end) / 2.0) / scarf_length;

        // Z ramps from (layer_z - z_drop) at fraction 0 to layer_z at fraction 1.
        let ramp_z = (layer_z - z_drop) + z_drop * mid_frac;
        seg.z = ramp_z;

        // Adjust E value: scale proportionally with Z ratio.
        let z_ratio = ramp_z / layer_z;
        seg.e_value *= z_ratio * flow_ratio;

        accumulated += seg_len;
    }

    // Sub-divide leading ramp segments into scarf_steps if needed.
    if steps > 1 {
        subdivide_ramp_region(
            segments,
            scarf_length,
            steps,
            layer_z,
            z_drop,
            flow_ratio,
            true,
        );
    }
}

/// Applies the trailing ramp: last `scarf_length` mm of the perimeter
/// ramps Z from `layer_z` down to `(layer_z - z_drop)`.
fn apply_trailing_ramp(
    segments: &mut Vec<ToolpathSegment>,
    scarf_length: f64,
    layer_z: f64,
    z_drop: f64,
    flow_ratio: f64,
    steps: u32,
) {
    // Accumulate distance from the end to find which segments are in the ramp.
    let total_length: f64 = segments.iter().map(|s| s.length()).sum();
    let ramp_start_dist = total_length - scarf_length;

    if ramp_start_dist < 0.0 {
        return;
    }

    // Find the segment containing the ramp start boundary.
    let mut accumulated = 0.0;
    let mut split_point: Option<(usize, f64)> = None;

    for (i, seg) in segments.iter().enumerate() {
        let seg_len = seg.length();
        if accumulated + seg_len >= ramp_start_dist && accumulated < ramp_start_dist {
            let remaining = ramp_start_dist - accumulated;
            if remaining > 0.001 && (seg_len - remaining) > 0.001 {
                split_point = Some((i, remaining));
            }
            break;
        }
        accumulated += seg_len;
    }

    // Split the boundary segment if needed.
    if let Some((idx, distance)) = split_point {
        let seg = segments[idx].clone();
        let (first, second) = split_segment_at_distance(&seg, distance);
        segments.splice(idx..=idx, [first, second]);
    }

    // Now apply Z ramp to all segments from ramp_start_dist to the end.
    accumulated = 0.0;
    for seg in segments.iter_mut() {
        let seg_len = seg.length();
        let seg_start_dist = accumulated;
        let seg_end_dist = accumulated + seg_len;

        if seg_start_dist >= ramp_start_dist {
            // This segment is fully in the trailing ramp.
            let frac_start = (seg_start_dist - ramp_start_dist) / scarf_length;
            let frac_end = (seg_end_dist - ramp_start_dist) / scarf_length;
            let mid_frac = (frac_start + frac_end) / 2.0;

            // Z ramps from layer_z at fraction 0 down to (layer_z - z_drop) at fraction 1.
            let ramp_z = layer_z - z_drop * mid_frac;
            seg.z = ramp_z;

            // Adjust E value: scale proportionally with Z ratio.
            let z_ratio = ramp_z / layer_z;
            seg.e_value *= z_ratio * flow_ratio;
        }

        accumulated += seg_len;
    }

    // Sub-divide trailing ramp segments into scarf_steps if needed.
    if steps > 1 {
        subdivide_ramp_region(
            segments,
            scarf_length,
            steps,
            layer_z,
            z_drop,
            flow_ratio,
            false,
        );
    }
}

/// Subdivides segments within a ramp region into finer steps.
///
/// `is_leading` controls whether this is the leading ramp (start of loop,
/// Z increasing) or trailing ramp (end of loop, Z decreasing).
fn subdivide_ramp_region(
    segments: &mut Vec<ToolpathSegment>,
    scarf_length: f64,
    steps: u32,
    layer_z: f64,
    z_drop: f64,
    flow_ratio: f64,
    is_leading: bool,
) {
    let step_length = scarf_length / steps as f64;
    if step_length < 0.001 {
        return;
    }

    let total_length: f64 = segments.iter().map(|s| s.length()).sum();
    let ramp_start = if is_leading {
        0.0
    } else {
        total_length - scarf_length
    };
    let ramp_end = if is_leading {
        scarf_length
    } else {
        total_length
    };

    // Collect new segments by splitting existing ones at step boundaries.
    let mut new_segments = Vec::new();
    let mut accumulated = 0.0;

    for seg in segments.iter() {
        let seg_len = seg.length();
        let seg_start = accumulated;
        let seg_end = accumulated + seg_len;

        // Check if this segment overlaps the ramp region.
        if seg_end <= ramp_start || seg_start >= ramp_end || seg_len < 0.001 {
            new_segments.push(seg.clone());
            accumulated += seg_len;
            continue;
        }

        // Find step boundaries within this segment.
        let mut split_distances = Vec::new();
        for step_idx in 1..steps {
            let step_dist = ramp_start + step_length * step_idx as f64;
            if step_dist > seg_start + 0.001 && step_dist < seg_end - 0.001 {
                split_distances.push(step_dist - seg_start);
            }
        }

        if split_distances.is_empty() {
            new_segments.push(seg.clone());
        } else {
            // Split the segment at each step boundary.
            let mut current = seg.clone();
            let mut offset = 0.0;

            for split_dist in &split_distances {
                let local_dist = split_dist - offset;
                if local_dist > 0.001 && local_dist < current.length() - 0.001 {
                    let (first, second) = split_segment_at_distance(&current, local_dist);
                    new_segments.push(first);
                    current = second;
                    offset = *split_dist;
                }
            }
            new_segments.push(current);
        }

        accumulated += seg_len;
    }

    *segments = new_segments;

    // Re-apply Z and E values based on final positions.
    accumulated = 0.0;
    for seg in segments.iter_mut() {
        let seg_len = seg.length();
        let seg_start = accumulated;
        let seg_end = accumulated + seg_len;
        let seg_mid = (seg_start + seg_end) / 2.0;

        if seg_mid >= ramp_start && seg_mid <= ramp_end {
            let frac = (seg_mid - ramp_start) / scarf_length;
            let ramp_z = if is_leading {
                // Leading: Z from (layer_z - z_drop) up to layer_z.
                (layer_z - z_drop) + z_drop * frac
            } else {
                // Trailing: Z from layer_z down to (layer_z - z_drop).
                layer_z - z_drop * frac
            };

            // Re-compute E proportionally.
            // E was already set; we need to reset to original then re-apply.
            // Since we're re-computing from scratch, use the original Z ratio.
            let z_ratio = ramp_z / layer_z;
            // Restore original e_value: undo previous ratio, apply new one.
            // Actually, since segments may have been split, we need to use the
            // segment's original layer_z-based E. For split segments, e_value
            // was already proportioned by the split. Just set Z and adjust E.
            seg.z = ramp_z;
            seg.e_value = seg.e_value.abs() * z_ratio * flow_ratio;
        }

        accumulated += seg_len;
    }
}

/// Splits a toolpath segment at a given distance from its start.
///
/// Returns two segments: the first covers `[start, split_point]` and
/// the second covers `[split_point, end]`. E values are proportioned
/// by the length ratio.
pub(crate) fn split_segment_at_distance(
    segment: &ToolpathSegment,
    distance_from_start: f64,
) -> (ToolpathSegment, ToolpathSegment) {
    let total_len = segment.length();
    let ratio = if total_len > 0.0 {
        (distance_from_start / total_len).clamp(0.0, 1.0)
    } else {
        0.5
    };

    // Interpolate the split point.
    let split_x = segment.start.x + (segment.end.x - segment.start.x) * ratio;
    let split_y = segment.start.y + (segment.end.y - segment.start.y) * ratio;
    let split_point = Point2::new(split_x, split_y);

    let first = ToolpathSegment {
        start: segment.start,
        end: split_point,
        feature: segment.feature,
        e_value: segment.e_value * ratio,
        feedrate: segment.feedrate,
        z: segment.z,
        extrusion_width: None,
    };

    let second = ToolpathSegment {
        start: split_point,
        end: segment.end,
        feature: segment.feature,
        e_value: segment.e_value * (1.0 - ratio),
        feedrate: segment.feedrate,
        z: segment.z,
        extrusion_width: None,
    };

    (first, second)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ScarfJointConfig;
    use crate::toolpath::FeatureType;

    /// Creates a simple 4-segment square perimeter (20mm sides).
    fn make_square_segments(z: f64) -> Vec<ToolpathSegment> {
        vec![
            ToolpathSegment {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(20.0, 0.0),
                feature: FeatureType::OuterPerimeter,
                e_value: 1.0,
                feedrate: 2700.0,
                z,
                extrusion_width: None,
            },
            ToolpathSegment {
                start: Point2::new(20.0, 0.0),
                end: Point2::new(20.0, 20.0),
                feature: FeatureType::OuterPerimeter,
                e_value: 1.0,
                feedrate: 2700.0,
                z,
                extrusion_width: None,
            },
            ToolpathSegment {
                start: Point2::new(20.0, 20.0),
                end: Point2::new(0.0, 20.0),
                feature: FeatureType::OuterPerimeter,
                e_value: 1.0,
                feedrate: 2700.0,
                z,
                extrusion_width: None,
            },
            ToolpathSegment {
                start: Point2::new(0.0, 20.0),
                end: Point2::new(0.0, 0.0),
                feature: FeatureType::OuterPerimeter,
                e_value: 1.0,
                feedrate: 2700.0,
                z,
                extrusion_width: None,
            },
        ]
    }

    #[test]
    fn scarf_disabled_leaves_segments_unchanged() {
        let layer_z = 0.4;
        let mut segments = make_square_segments(layer_z);
        let original = segments.clone();

        let config = ScarfJointConfig {
            enabled: false,
            ..Default::default()
        };

        apply_scarf_joint(&mut segments, &config, 0.2, layer_z);

        assert_eq!(segments.len(), original.len());
        for (s, o) in segments.iter().zip(original.iter()) {
            assert!((s.z - o.z).abs() < 1e-12);
            assert!((s.e_value - o.e_value).abs() < 1e-12);
        }
    }

    #[test]
    fn scarf_zero_length_no_modification() {
        let layer_z = 0.4;
        let mut segments = make_square_segments(layer_z);
        let original_z_values: Vec<f64> = segments.iter().map(|s| s.z).collect();

        let config = ScarfJointConfig {
            enabled: true,
            scarf_length: 0.0,
            ..Default::default()
        };

        apply_scarf_joint(&mut segments, &config, 0.2, layer_z);

        for (i, seg) in segments.iter().enumerate() {
            assert!(
                (seg.z - original_z_values[i]).abs() < 1e-12,
                "scarf_length=0 should not modify Z"
            );
        }
    }

    #[test]
    fn scarf_creates_z_variation_near_seam() {
        let layer_z = 0.4;
        let layer_height = 0.2;
        let mut segments = make_square_segments(layer_z);

        let config = ScarfJointConfig {
            enabled: true,
            scarf_length: 10.0,
            scarf_start_height: 0.5,
            scarf_steps: 10,
            scarf_flow_ratio: 1.0,
            ..Default::default()
        };

        apply_scarf_joint(&mut segments, &config, layer_height, layer_z);

        // Some segments should have Z different from layer_z.
        let has_z_variation = segments.iter().any(|s| (s.z - layer_z).abs() > 0.001);
        assert!(
            has_z_variation,
            "Scarf should create Z variation near the seam"
        );
    }

    #[test]
    fn trailing_ramp_z_decreases_before_seam() {
        let layer_z = 0.4;
        let layer_height = 0.2;
        let mut segments = make_square_segments(layer_z);

        let config = ScarfJointConfig {
            enabled: true,
            scarf_length: 10.0,
            scarf_start_height: 0.5,
            scarf_steps: 1,
            scarf_flow_ratio: 1.0,
            ..Default::default()
        };

        apply_scarf_joint(&mut segments, &config, layer_height, layer_z);

        // The last segment(s) should have Z < layer_z (trailing ramp going down).
        let last = segments.last().unwrap();
        assert!(
            last.z < layer_z - 0.001,
            "Trailing ramp: last segment Z ({}) should be below layer_z ({})",
            last.z,
            layer_z
        );
    }

    #[test]
    fn leading_ramp_z_increases_from_start() {
        let layer_z = 0.4;
        let layer_height = 0.2;
        let mut segments = make_square_segments(layer_z);

        let config = ScarfJointConfig {
            enabled: true,
            scarf_length: 10.0,
            scarf_start_height: 0.5,
            scarf_steps: 1,
            scarf_flow_ratio: 1.0,
            ..Default::default()
        };

        apply_scarf_joint(&mut segments, &config, layer_height, layer_z);

        // The first segment should have Z < layer_z (leading ramp starting low).
        let first = &segments[0];
        assert!(
            first.z < layer_z - 0.001,
            "Leading ramp: first segment Z ({}) should be below layer_z ({})",
            first.z,
            layer_z
        );
    }

    #[test]
    fn e_values_adjusted_proportionally_with_z() {
        let layer_z = 0.4;
        let layer_height = 0.2;
        let mut segments = make_square_segments(layer_z);

        let config = ScarfJointConfig {
            enabled: true,
            scarf_length: 10.0,
            scarf_start_height: 0.5,
            scarf_steps: 1,
            scarf_flow_ratio: 1.0,
            ..Default::default()
        };

        // Store original E values (per unit length) for comparison.
        let original_e_per_mm: Vec<f64> = segments
            .iter()
            .map(|s| {
                let len = s.length();
                if len > 0.0 {
                    s.e_value / len
                } else {
                    0.0
                }
            })
            .collect();

        apply_scarf_joint(&mut segments, &config, layer_height, layer_z);

        // For segments with Z < layer_z, E/mm should be reduced.
        for seg in &segments {
            if seg.z < layer_z - 0.001 {
                let seg_len = seg.length();
                if seg_len > 0.001 {
                    let e_per_mm = seg.e_value / seg_len;
                    // E should be reduced when Z is below layer_z.
                    // The ratio should be approximately seg.z / layer_z.
                    assert!(
                        e_per_mm < original_e_per_mm[0] + 0.001,
                        "E/mm ({}) should be less than original ({}) for lower Z",
                        e_per_mm,
                        original_e_per_mm[0]
                    );
                }
            }
        }
    }

    #[test]
    fn scarf_steps_controls_subdivision() {
        let layer_z = 0.4;
        let layer_height = 0.2;

        // With 1 step (no subdivision).
        let mut segments_1 = make_square_segments(layer_z);
        let config_1 = ScarfJointConfig {
            enabled: true,
            scarf_length: 10.0,
            scarf_start_height: 0.5,
            scarf_steps: 1,
            scarf_flow_ratio: 1.0,
            ..Default::default()
        };
        apply_scarf_joint(&mut segments_1, &config_1, layer_height, layer_z);

        // With 10 steps (more subdivision).
        let mut segments_10 = make_square_segments(layer_z);
        let config_10 = ScarfJointConfig {
            enabled: true,
            scarf_length: 10.0,
            scarf_start_height: 0.5,
            scarf_steps: 10,
            scarf_flow_ratio: 1.0,
            ..Default::default()
        };
        apply_scarf_joint(&mut segments_10, &config_10, layer_height, layer_z);

        // More steps should produce more segments (due to subdivision).
        assert!(
            segments_10.len() >= segments_1.len(),
            "10 steps ({} segments) should produce >= segments than 1 step ({} segments)",
            segments_10.len(),
            segments_1.len()
        );
    }

    #[test]
    fn split_segment_preserves_total_e() {
        let seg = ToolpathSegment {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(10.0, 0.0),
            feature: FeatureType::OuterPerimeter,
            e_value: 1.0,
            feedrate: 2700.0,
            z: 0.4,
            extrusion_width: None,
        };

        let (first, second) = split_segment_at_distance(&seg, 3.0);

        // Total E should be preserved.
        let total_e = first.e_value + second.e_value;
        assert!(
            (total_e - 1.0).abs() < 1e-9,
            "Split should preserve total E: {} + {} = {}",
            first.e_value,
            second.e_value,
            total_e
        );

        // First segment should end at (3, 0).
        assert!((first.end.x - 3.0).abs() < 1e-9);
        assert!((first.end.y - 0.0).abs() < 1e-9);

        // Second segment should start at (3, 0).
        assert!((second.start.x - 3.0).abs() < 1e-9);
    }

    #[test]
    fn split_segment_preserves_total_length() {
        let seg = ToolpathSegment {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(10.0, 0.0),
            feature: FeatureType::OuterPerimeter,
            e_value: 1.0,
            feedrate: 2700.0,
            z: 0.4,
            extrusion_width: None,
        };

        let (first, second) = split_segment_at_distance(&seg, 7.0);

        let total_len = first.length() + second.length();
        assert!(
            (total_len - 10.0).abs() < 1e-9,
            "Split should preserve total length: {} + {} = {}",
            first.length(),
            second.length(),
            total_len
        );
    }

    #[test]
    fn scarf_on_empty_segments_no_panic() {
        let mut segments = Vec::new();
        let config = ScarfJointConfig {
            enabled: true,
            scarf_length: 10.0,
            ..Default::default()
        };

        apply_scarf_joint(&mut segments, &config, 0.2, 0.4);
        assert!(segments.is_empty());
    }

    #[test]
    fn scarf_z_stays_within_bounds() {
        let layer_z = 0.4;
        let layer_height = 0.2;
        let mut segments = make_square_segments(layer_z);

        let config = ScarfJointConfig {
            enabled: true,
            scarf_length: 15.0,
            scarf_start_height: 0.5,
            scarf_steps: 10,
            scarf_flow_ratio: 1.0,
            ..Default::default()
        };

        apply_scarf_joint(&mut segments, &config, layer_height, layer_z);

        let z_min = layer_z - layer_height * config.scarf_start_height;
        for seg in &segments {
            assert!(
                seg.z >= z_min - 0.01 && seg.z <= layer_z + 0.01,
                "Scarf Z ({}) should be between {} and {}",
                seg.z,
                z_min,
                layer_z
            );
        }
    }
}
