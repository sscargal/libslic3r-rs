//! Seam placement strategies for perimeter loops.
//!
//! Seam placement controls where each perimeter polygon starts and ends.
//! Different strategies produce visually different seam lines:
//!
//! - **Aligned** creates a vertical seam line across layers
//! - **Random** scatters seam points for a less visible seam
//! - **Rear** hides the seam at the back of the model
//! - **NearestCorner** hides the seam in concave corners
//!
//! The main entry point is [`select_seam_point`], which returns the index of
//! the vertex where printing should start for a given polygon.

use serde::{Deserialize, Serialize};
use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::IPoint2;

/// Seam placement strategy.
///
/// Controls where each perimeter loop starts and ends, affecting the
/// visual appearance of the printed object's seam line.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeamPosition {
    /// Align seam points vertically across layers.
    ///
    /// If a previous seam point is available, finds the vertex closest to it.
    /// Otherwise, starts at the rear (maximum Y). This creates a consistent
    /// vertical seam line across all layers.
    #[default]
    Aligned,
    /// Deterministic pseudo-random placement.
    ///
    /// Uses the layer index as a seed for deterministic vertex selection.
    /// Same layer index always produces the same seam index.
    Random,
    /// Place seam at the rear (maximum Y) of the model.
    ///
    /// Finds the vertex with maximum Y coordinate. Breaks ties by choosing
    /// the vertex closest to the previous seam (if available), or the one
    /// with the smallest X coordinate.
    Rear,
    /// Smart hiding: prefer concave corners, then convex, with alignment bias.
    ///
    /// Scores each vertex by its corner angle, preferring concave corners
    /// (which hide the seam better). Adds an alignment bonus if a previous
    /// seam point is available. Falls back to Aligned strategy on smooth
    /// curves with no sharp corners.
    NearestCorner,
}

/// Selects the starting vertex index for a perimeter polygon.
///
/// Returns the index into `polygon.points()` where printing should start.
///
/// # Parameters
/// - `polygon`: The perimeter polygon to find a seam point for.
/// - `strategy`: The seam placement strategy to use.
/// - `previous_seam`: The seam point from a previous polygon or layer (for alignment).
/// - `layer_index`: Current layer index (used by Random strategy as seed).
///
/// # Returns
/// An index `i` where `0 <= i < polygon.points().len()`.
pub fn select_seam_point(
    polygon: &ValidPolygon,
    strategy: SeamPosition,
    previous_seam: Option<IPoint2>,
    layer_index: usize,
) -> usize {
    let pts = polygon.points();
    let n = pts.len();
    debug_assert!(n >= 3, "ValidPolygon should have at least 3 points");

    match strategy {
        SeamPosition::Aligned => select_aligned(pts, previous_seam),
        SeamPosition::Random => select_random(n, layer_index),
        SeamPosition::Rear => select_rear(pts, previous_seam),
        SeamPosition::NearestCorner => select_nearest_corner(pts, previous_seam),
    }
}

/// Aligned: find the vertex closest to `previous_seam`, or max-Y if None.
fn select_aligned(pts: &[IPoint2], previous_seam: Option<IPoint2>) -> usize {
    match previous_seam {
        Some(prev) => closest_vertex(pts, prev),
        None => max_y_vertex(pts, None),
    }
}

/// Random: deterministic pseudo-random vertex selection based on layer_index.
fn select_random(n: usize, layer_index: usize) -> usize {
    // Knuth multiplicative hash constant (golden ratio * 2^32).
    let hash = (layer_index as u64).wrapping_mul(2654435761) >> 16;
    (hash as usize) % n
}

/// Rear: find the vertex with maximum Y. Break ties by proximity to previous
/// seam or by smallest X.
fn select_rear(pts: &[IPoint2], previous_seam: Option<IPoint2>) -> usize {
    max_y_vertex(pts, previous_seam)
}

/// NearestCorner (Smart Hiding): score vertices by corner angle and alignment.
///
/// Concave corners score highest, convex corners score by their angle,
/// and an alignment bonus is added for proximity to previous seam.
/// Falls back to Aligned if all vertices have nearly equal angles.
fn select_nearest_corner(pts: &[IPoint2], previous_seam: Option<IPoint2>) -> usize {
    let n = pts.len();

    // Compute the interior angle at each vertex using the sequential edge
    // cross product to determine concavity.
    //
    // For a CCW polygon:
    // - cross(edge_in, edge_out) > 0 -> convex (left turn), interior angle < PI
    // - cross(edge_in, edge_out) < 0 -> concave/reflex (right turn), interior angle > PI
    //
    // We compute the angle between edges at each vertex, and mark whether
    // the vertex is concave based on the cross product sign.
    let mut angles: Vec<f64> = Vec::with_capacity(n);
    let mut is_concave: Vec<bool> = Vec::with_capacity(n);

    for i in 0..n {
        let prev = pts[(i + n - 1) % n];
        let curr = pts[i];
        let next = pts[(i + 1) % n];

        // Sequential edge vectors.
        let edge_in_x = (curr.x - prev.x) as i128;
        let edge_in_y = (curr.y - prev.y) as i128;
        let edge_out_x = (next.x - curr.x) as i128;
        let edge_out_y = (next.y - curr.y) as i128;

        // Cross product of sequential edges: positive = left turn (convex in CCW).
        let cross = edge_in_x * edge_out_y - edge_in_y * edge_out_x;

        // Compute the angle between the two edges (always positive).
        let angle = compute_corner_angle(prev, curr, next);
        angles.push(angle);

        // For CCW polygon: negative cross = concave (reflex) corner.
        is_concave.push(cross < 0);
    }

    // Check if all angles are nearly equal (smooth curve / regular polygon).
    // If so, fall back to Aligned.
    let mean_angle: f64 = angles.iter().sum::<f64>() / n as f64;
    let max_deviation = angles
        .iter()
        .map(|a| (a - mean_angle).abs())
        .fold(0.0_f64, f64::max);

    // Threshold: if max deviation from mean is less than 5 degrees, it's "smooth".
    if max_deviation < 5.0_f64.to_radians() {
        return select_aligned(pts, previous_seam);
    }

    // Score each vertex.
    let mut best_idx = 0;
    let mut best_score = f64::NEG_INFINITY;

    for i in 0..n {
        let angle = angles[i];

        // Concave (reflex) corners score highest for seam hiding.
        // Convex corners score by their sharpness (smaller angle = sharper corner).
        let base_score = if is_concave[i] {
            200.0 + angle
        } else {
            // For convex corners, prefer sharper corners (smaller angle means
            // more deviation from straight, better for hiding seam).
            // Angle is in [0, PI] for convex; smaller = sharper.
            std::f64::consts::PI - angle
        };

        // Alignment bonus: closer to previous seam = higher bonus.
        let alignment_bonus = if let Some(prev) = previous_seam {
            let dist_sq = distance_squared_i64(pts[i], prev);
            // Convert to mm for scoring. COORD_SCALE = 1e6, so dist_sq in coord^2.
            // dist_mm = sqrt(dist_sq) / COORD_SCALE
            let dist_mm = (dist_sq as f64).sqrt() / slicecore_math::COORD_SCALE;
            50.0 / (1.0 + dist_mm)
        } else {
            0.0
        };

        let score = base_score + alignment_bonus;

        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    best_idx
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Computes the interior angle at vertex `curr` between edges
/// `prev->curr` and `curr->next`, in radians [0, 2*PI).
///
/// Uses atan2 of the cross product and dot product of the edge vectors.
pub(crate) fn compute_corner_angle(prev: IPoint2, curr: IPoint2, next: IPoint2) -> f64 {
    let ax = prev.x - curr.x;
    let ay = prev.y - curr.y;
    let bx = next.x - curr.x;
    let by = next.y - curr.y;

    // Cross product (gives sin of angle) and dot product (gives cos of angle).
    let cross = (ax as i128) * (by as i128) - (ay as i128) * (bx as i128);
    let dot = (ax as i128) * (bx as i128) + (ay as i128) * (by as i128);

    let angle = (cross as f64).atan2(dot as f64);

    // Normalize to [0, 2*PI).
    if angle < 0.0 {
        angle + 2.0 * std::f64::consts::PI
    } else {
        angle
    }
}

/// Squared distance between two integer points, returned as i128 to avoid overflow.
pub(crate) fn distance_squared_i64(a: IPoint2, b: IPoint2) -> i128 {
    let dx = (a.x - b.x) as i128;
    let dy = (a.y - b.y) as i128;
    dx * dx + dy * dy
}

/// Finds the vertex index closest to a given point.
fn closest_vertex(pts: &[IPoint2], target: IPoint2) -> usize {
    let mut best_idx = 0;
    let mut best_dist = i128::MAX;

    for (i, &pt) in pts.iter().enumerate() {
        let dist = distance_squared_i64(pt, target);
        if dist < best_dist {
            best_dist = dist;
            best_idx = i;
        }
    }

    best_idx
}

/// Finds the vertex index with maximum Y coordinate.
/// Breaks ties by proximity to `tiebreak_point` (if Some), otherwise by smallest X.
fn max_y_vertex(pts: &[IPoint2], tiebreak_point: Option<IPoint2>) -> usize {
    let mut best_idx = 0;
    let mut best_y = i64::MIN;
    let mut best_tiebreak: i128 = i128::MAX;

    for (i, &pt) in pts.iter().enumerate() {
        if pt.y > best_y
            || (pt.y == best_y && {
                let tb = match tiebreak_point {
                    Some(prev) => distance_squared_i64(pt, prev),
                    None => pt.x as i128, // smallest X as tiebreak
                };
                tb < best_tiebreak
            })
        {
            best_y = pt.y;
            best_tiebreak = match tiebreak_point {
                Some(prev) => distance_squared_i64(pt, prev),
                None => pt.x as i128,
            };
            best_idx = i;
        }
    }

    best_idx
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;

    /// Helper to create a validated CCW square at the origin with given size (mm).
    fn make_square(size: f64) -> ValidPolygon {
        Polygon::from_mm(&[
            (0.0, 0.0),
            (size, 0.0),
            (size, size),
            (0.0, size),
        ])
        .validate()
        .unwrap()
    }

    /// Helper to create an L-shaped polygon with one obvious concave corner.
    fn make_l_shape() -> ValidPolygon {
        // L-shape (CCW):
        //   (0,0) -> (10,0) -> (10,5) -> (5,5) -> (5,10) -> (0,10)
        // The concave corner is at (5,5).
        Polygon::from_mm(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 5.0),
            (5.0, 5.0),
            (5.0, 10.0),
            (0.0, 10.0),
        ])
        .validate()
        .unwrap()
    }

    #[test]
    fn aligned_with_previous_seam_returns_nearby_vertex() {
        let square = make_square(20.0);
        let pts = square.points();

        // Place previous seam near vertex 2 = (20, 20).
        let prev_seam = IPoint2::from_mm(20.1, 20.1);

        let idx1 = select_seam_point(&square, SeamPosition::Aligned, Some(prev_seam), 0);
        // Should pick vertex closest to (20.1, 20.1), which is vertex 2 = (20, 20).
        let (sel_x, sel_y) = pts[idx1].to_mm();
        assert!(
            (sel_x - 20.0).abs() < 0.5 && (sel_y - 20.0).abs() < 0.5,
            "Aligned should select vertex near (20, 20), got ({}, {})",
            sel_x,
            sel_y
        );

        // A second call with the same previous seam should return the same vertex.
        let idx2 = select_seam_point(&square, SeamPosition::Aligned, Some(prev_seam), 1);
        assert_eq!(
            idx1, idx2,
            "Aligned with same previous_seam should return same index across layers"
        );
    }

    #[test]
    fn aligned_without_previous_seam_selects_rear() {
        let square = make_square(20.0);
        let pts = square.points();

        // No previous seam -- should select max-Y vertex.
        let idx = select_seam_point(&square, SeamPosition::Aligned, None, 0);
        let max_y = pts.iter().map(|p| p.y).max().unwrap();
        assert_eq!(
            pts[idx].y, max_y,
            "Aligned without previous_seam should select max-Y vertex"
        );
    }

    #[test]
    fn random_different_layers_produce_different_indices() {
        let square = make_square(20.0);
        let n = square.points().len();

        // Collect indices for many layer indices and ensure not all the same.
        let indices: Vec<usize> = (0..20)
            .map(|li| select_seam_point(&square, SeamPosition::Random, None, li))
            .collect();

        // All should be valid.
        for &idx in &indices {
            assert!(
                idx < n,
                "Random seam index {} should be < polygon size {}",
                idx,
                n
            );
        }

        // Not all identical (with 20 samples on a 4-vertex polygon, very unlikely).
        let all_same = indices.iter().all(|&i| i == indices[0]);
        assert!(
            !all_same,
            "Random seam should produce different indices for different layers"
        );
    }

    #[test]
    fn random_same_layer_produces_same_index() {
        let square = make_square(20.0);

        let idx1 = select_seam_point(&square, SeamPosition::Random, None, 42);
        let idx2 = select_seam_point(&square, SeamPosition::Random, None, 42);
        assert_eq!(
            idx1, idx2,
            "Random seam should be deterministic: same layer_index = same result"
        );
    }

    #[test]
    fn rear_returns_max_y_vertex() {
        let square = make_square(20.0);
        let pts = square.points();

        let idx = select_seam_point(&square, SeamPosition::Rear, None, 0);
        let max_y = pts.iter().map(|p| p.y).max().unwrap();
        assert_eq!(
            pts[idx].y, max_y,
            "Rear seam should select the vertex with maximum Y"
        );
    }

    #[test]
    fn nearest_corner_selects_concave_corner_on_l_shape() {
        let l_shape = make_l_shape();
        let pts = l_shape.points();

        let idx = select_seam_point(&l_shape, SeamPosition::NearestCorner, None, 0);

        // The concave corner is at (5, 5) in mm.
        let (sel_x, sel_y) = pts[idx].to_mm();
        assert!(
            (sel_x - 5.0).abs() < 0.5 && (sel_y - 5.0).abs() < 0.5,
            "NearestCorner should select concave corner at (5, 5), got ({}, {})",
            sel_x,
            sel_y
        );
    }

    #[test]
    fn nearest_corner_on_regular_polygon_falls_back_to_aligned() {
        // A square has all equal angles (90 degrees each), so NearestCorner
        // should fall back to Aligned behavior.
        let square = make_square(20.0);

        let prev_seam = IPoint2::from_mm(0.1, 0.1);

        let corner_idx =
            select_seam_point(&square, SeamPosition::NearestCorner, Some(prev_seam), 0);
        let aligned_idx =
            select_seam_point(&square, SeamPosition::Aligned, Some(prev_seam), 0);

        assert_eq!(
            corner_idx, aligned_idx,
            "NearestCorner on regular polygon should fall back to Aligned"
        );
    }

    #[test]
    fn all_strategies_return_valid_index() {
        let square = make_square(20.0);
        let n = square.points().len();

        let strategies = [
            SeamPosition::Aligned,
            SeamPosition::Random,
            SeamPosition::Rear,
            SeamPosition::NearestCorner,
        ];
        let prev = Some(IPoint2::from_mm(10.0, 10.0));

        for strategy in &strategies {
            for layer in 0..10 {
                let idx = select_seam_point(&square, *strategy, prev, layer);
                assert!(
                    idx < n,
                    "Strategy {:?} layer {} returned index {} >= polygon size {}",
                    strategy,
                    layer,
                    idx,
                    n
                );
            }
        }
    }

    #[test]
    fn seam_position_serde_round_trip() {
        let positions = [
            SeamPosition::Aligned,
            SeamPosition::Random,
            SeamPosition::Rear,
            SeamPosition::NearestCorner,
        ];

        for pos in &positions {
            let json = serde_json::to_string(pos).unwrap();
            let deserialized: SeamPosition = serde_json::from_str(&json).unwrap();
            assert_eq!(
                *pos, deserialized,
                "Serde round-trip failed for {:?}",
                pos
            );
        }
    }

    #[test]
    fn compute_corner_angle_right_angle() {
        // A right angle at the origin with edges along +X and +Y.
        let prev = IPoint2::from_mm(1.0, 0.0);
        let curr = IPoint2::from_mm(0.0, 0.0);
        let next = IPoint2::from_mm(0.0, 1.0);

        let angle = compute_corner_angle(prev, curr, next);
        // Should be ~PI/2 (90 degrees).
        assert!(
            (angle - std::f64::consts::FRAC_PI_2).abs() < 0.01,
            "Right angle should be ~PI/2, got {}",
            angle
        );
    }

    #[test]
    fn distance_squared_basic() {
        let a = IPoint2::from_mm(0.0, 0.0);
        let b = IPoint2::from_mm(3.0, 4.0);

        let dist_sq = distance_squared_i64(a, b);
        // 3^2 + 4^2 = 25 in mm^2, in coord units: (3e6)^2 + (4e6)^2 = 25e12
        let expected = 25_000_000_000_000i128;
        // Allow small tolerance due to rounding.
        let diff = (dist_sq - expected).abs();
        assert!(
            diff < 1000, // tiny tolerance in coord^2
            "distance_squared should be ~{}, got {} (diff={})",
            expected,
            dist_sq,
            diff
        );
    }
}
