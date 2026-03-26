//! Laplacian smoothing and ratio clamping for VLH layer heights.
//!
//! Provides smooth height transitions between layers while preserving anchor
//! points (first layer, feature-demanded heights) and enforcing maximum
//! adjacent height change ratios as a safety net.

/// 1D Laplacian smoothing on layer heights.
///
/// `heights`: mutable slice of `(z, height)` pairs.
/// `pinned`: boolean slice, `true` = anchor point (not moved during smoothing).
/// `lambda`: smoothing strength `0.0` (no change) to `1.0` (full move toward
///   neighbor average).
/// `iterations`: number of smoothing passes (recommended 3-5).
///
/// Boundary elements (first and last) are never moved.
/// Uses uniform kernel (equal weight for left and right neighbors).
pub fn laplacian_smooth(
    heights: &mut [(f64, f64)],
    pinned: &[bool],
    lambda: f64,
    iterations: usize,
) {
    let len = heights.len();
    if len < 3 {
        return;
    }

    let lambda = lambda.clamp(0.0, 1.0);
    if lambda < 1e-15 {
        return;
    }

    for _ in 0..iterations {
        // Snapshot current heights for the iteration.
        let snapshot: Vec<f64> = heights.iter().map(|h| h.1).collect();

        for i in 1..len - 1 {
            if pinned.get(i).copied().unwrap_or(false) {
                continue;
            }
            let avg = (snapshot[i - 1] + snapshot[i + 1]) / 2.0;
            heights[i].1 = snapshot[i] + lambda * (avg - snapshot[i]);
        }
    }
}

/// Forward-backward ratio clamping (safety net after Laplacian smoothing).
///
/// Ensures no adjacent layers differ by more than `max_ratio` (default 1.5 = 50%).
/// This is the same algorithm as `smooth_heights` in `adaptive.rs`, extracted
/// for reuse.
pub fn ratio_clamp(heights: &mut [(f64, f64)], max_ratio: f64) {
    if heights.len() < 2 {
        return;
    }

    // Forward pass: clamp each height relative to the previous.
    for i in 1..heights.len() {
        let prev_h = heights[i - 1].1;
        let max_h = prev_h * max_ratio;
        let min_h = prev_h / max_ratio;
        heights[i].1 = heights[i].1.clamp(min_h, max_h);
    }

    // Backward pass: clamp each height relative to the next.
    for i in (0..heights.len() - 1).rev() {
        let next_h = heights[i + 1].1;
        let max_h = next_h * max_ratio;
        let min_h = next_h / max_ratio;
        heights[i].1 = heights[i].1.clamp(min_h, max_h);
    }
}

/// Full smoothing pipeline: Laplacian first, then ratio clamp as safety net.
///
/// Recomputes Z positions after smoothing to maintain consistent layer stacking.
///
/// # Arguments
///
/// * `heights` - Mutable `(z, height)` pairs
/// * `pinned` - Boolean slice, `true` = anchor point
/// * `lambda` - Smoothing strength `[0.0, 1.0]`
/// * `iterations` - Number of Laplacian passes
/// * `max_ratio` - Maximum adjacent height ratio (e.g., 1.5 = 50%)
/// * `min_height` - Minimum allowed layer height
/// * `max_height` - Maximum allowed layer height
pub fn smooth_vlh_heights(
    heights: &mut [(f64, f64)],
    pinned: &[bool],
    lambda: f64,
    iterations: usize,
    max_ratio: f64,
    min_height: f64,
    max_height: f64,
) {
    if heights.len() < 2 {
        return;
    }

    // Step 1: Laplacian smoothing.
    laplacian_smooth(heights, pinned, lambda, iterations);

    // Step 2: Ratio clamping safety net.
    ratio_clamp(heights, max_ratio);

    // Step 3: Clamp all non-first heights to valid range.
    for entry in heights.iter_mut().skip(1) {
        entry.1 = entry.1.clamp(min_height, max_height);
    }

    // Step 4: Recompute Z positions for consistent stacking.
    recompute_z_positions(heights);
}

/// Recomputes Z positions after smoothing to maintain consistent layer stacking.
///
/// Each layer's Z center = previous layer's top + current layer's height / 2.
fn recompute_z_positions(heights: &mut [(f64, f64)]) {
    if heights.len() < 2 {
        return;
    }
    for i in 1..heights.len() {
        let prev_top = heights[i - 1].0 + heights[i - 1].1 / 2.0;
        heights[i].0 = prev_top + heights[i].1 / 2.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laplacian_smooth_lambda_zero_unchanged() {
        let mut heights = vec![(0.1, 0.1), (0.25, 0.3), (0.4, 0.1)];
        let original = heights.clone();
        let pinned = vec![false, false, false];
        laplacian_smooth(&mut heights, &pinned, 0.0, 5);
        for (h, o) in heights.iter().zip(original.iter()) {
            assert!(
                (h.1 - o.1).abs() < 1e-12,
                "Lambda=0 should leave heights unchanged: got {}, expected {}",
                h.1,
                o.1,
            );
        }
    }

    #[test]
    fn laplacian_smooth_lambda_one_moves_middle() {
        let mut heights = vec![(0.05, 0.1), (0.25, 0.3), (0.4, 0.1)];
        let pinned = vec![false, false, false];
        laplacian_smooth(&mut heights, &pinned, 1.0, 1);
        // Middle element should move toward average of neighbors: (0.1 + 0.1) / 2 = 0.1
        // With lambda=1.0: new = 0.3 + 1.0 * (0.1 - 0.3) = 0.1
        assert!(
            (heights[1].1 - 0.1).abs() < 1e-9,
            "Middle should move to neighbor average with lambda=1.0, got {}",
            heights[1].1,
        );
    }

    #[test]
    fn laplacian_smooth_preserves_pinned() {
        let mut heights = vec![
            (0.05, 0.1),
            (0.25, 0.3),
            (0.4, 0.1),
            (0.5, 0.2),
            (0.65, 0.1),
        ];
        let pinned = vec![false, false, true, false, false];
        laplacian_smooth(&mut heights, &pinned, 1.0, 10);
        assert!(
            (heights[2].1 - 0.1).abs() < 1e-12,
            "Pinned element should not move: got {}",
            heights[2].1,
        );
    }

    #[test]
    fn laplacian_smooth_no_shrinkage_to_mean() {
        let mut heights = vec![
            (0.05, 0.05),
            (0.15, 0.15),
            (0.35, 0.25),
            (0.55, 0.15),
            (0.65, 0.05),
        ];
        let pinned = vec![false, false, false, false, false];
        laplacian_smooth(&mut heights, &pinned, 0.5, 5);
        // Boundary elements are not moved.
        let first = heights[0].1;
        let last = heights[heights.len() - 1].1;
        assert!(
            (first - 0.05).abs() < 1e-12,
            "First boundary should not move"
        );
        assert!((last - 0.05).abs() < 1e-12, "Last boundary should not move");
        // Interior heights should vary (not all equal to mean).
        let all_same = heights.windows(2).all(|w| (w[0].1 - w[1].1).abs() < 1e-6);
        assert!(
            !all_same,
            "Heights should NOT all converge to the same value after 5 iterations"
        );
    }

    #[test]
    fn laplacian_smooth_boundary_not_moved() {
        let mut heights = vec![(0.05, 0.1), (0.25, 0.3), (0.4, 0.1)];
        let pinned = vec![false, false, false];
        laplacian_smooth(&mut heights, &pinned, 1.0, 10);
        assert!(
            (heights[0].1 - 0.1).abs() < 1e-12,
            "First element should not move: got {}",
            heights[0].1,
        );
        assert!(
            (heights[2].1 - 0.1).abs() < 1e-12,
            "Last element should not move: got {}",
            heights[2].1,
        );
    }

    #[test]
    fn ratio_clamp_enforces_max_ratio() {
        let mut heights = vec![
            (0.05, 0.1),
            (0.15, 0.3), // 3x jump
            (0.4, 0.05), // 6x drop
            (0.45, 0.3), // 6x jump
        ];
        ratio_clamp(&mut heights, 1.5);
        for i in 1..heights.len() {
            let ratio = heights[i].1 / heights[i - 1].1;
            assert!(
                ratio <= 1.51 && ratio >= 1.0 / 1.51,
                "Ratio at index {} is {:.3} (heights: {:.4}, {:.4}), should be within [1/1.5, 1.5]",
                i,
                ratio,
                heights[i - 1].1,
                heights[i].1,
            );
        }
    }

    #[test]
    fn ratio_clamp_preserves_first_layer() {
        let mut heights = vec![(0.1, 0.2), (0.3, 0.1), (0.4, 0.3)];
        ratio_clamp(&mut heights, 1.5);
        for i in 1..heights.len() {
            let ratio = heights[i].1 / heights[i - 1].1;
            assert!(
                ratio <= 1.51 && ratio >= 1.0 / 1.51,
                "Ratio violated at index {}",
                i,
            );
        }
    }

    #[test]
    fn smooth_vlh_heights_monotonic_z() {
        let mut heights = vec![
            (0.1, 0.2),
            (0.3, 0.15),
            (0.5, 0.25),
            (0.7, 0.1),
            (0.85, 0.2),
        ];
        let pinned = vec![true, false, false, false, false];
        smooth_vlh_heights(&mut heights, &pinned, 0.5, 3, 1.5, 0.05, 0.3);
        for i in 1..heights.len() {
            assert!(
                heights[i].0 > heights[i - 1].0,
                "Z positions should be monotonically increasing: z[{}]={} <= z[{}]={}",
                i,
                heights[i].0,
                i - 1,
                heights[i - 1].0,
            );
        }
    }

    #[test]
    fn smoothing_is_deterministic() {
        let original = vec![
            (0.1, 0.2),
            (0.3, 0.15),
            (0.5, 0.25),
            (0.7, 0.1),
            (0.85, 0.2),
        ];
        let pinned = vec![true, false, false, false, false];
        let mut first = original.clone();
        smooth_vlh_heights(&mut first, &pinned, 0.5, 3, 1.5, 0.05, 0.3);
        for _ in 0..100 {
            let mut other = original.clone();
            smooth_vlh_heights(&mut other, &pinned, 0.5, 3, 1.5, 0.05, 0.3);
            for (a, b) in first.iter().zip(other.iter()) {
                assert!(
                    (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12,
                    "Smoothing should be deterministic"
                );
            }
        }
    }
}
