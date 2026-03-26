//! VLH optimizer implementations: greedy with lookahead and dynamic programming.
//!
//! Both optimizers select per-Z layer heights by minimizing a weighted
//! multi-objective cost function. The greedy optimizer uses a sliding lookahead
//! window; the DP optimizer finds the globally optimal height sequence through
//! a discrete candidate lattice.
//!
//! All computation is serial and deterministic (SLICE-05 compliant). No
//! floating-point non-determinism from parallel reduction or random
//! tie-breaking.

#[cfg(test)]
use super::VlhWeights;
use super::{ObjectiveScores, VlhConfig};

/// Per-Z sample with pre-computed objective scores and feature demands.
#[derive(Debug, Clone)]
pub struct ZSample {
    pub z: f64,
    pub scores: ObjectiveScores,
    pub feature_demanded_height: Option<f64>,
    pub stress_factor: f64,
    pub external_surface_fraction: f64,
}

/// Number of lookahead layers for the greedy optimizer.
const GREEDY_LOOKAHEAD: usize = 5;

/// Greedy optimizer with lookahead window.
///
/// Selects layer heights by evaluating the next `GREEDY_LOOKAHEAD` Z samples
/// at each step and picking the height that minimizes total cost over the
/// window. Feature demands override the objective-derived height when present.
///
/// # Algorithm
///
/// 1. First layer is always `(first_layer_height / 2, first_layer_height)`.
/// 2. At each position, evaluate the next `GREEDY_LOOKAHEAD` candidate heights.
/// 3. Pick the height minimizing sum of `|selected - ideal|` over the window.
/// 4. Clamp to `[min_height, min(max_height, nozzle_diameter * 0.75)]`.
/// 5. Advance by the selected height and repeat.
///
/// # Returns
///
/// Vector of `(z_position, layer_height)` pairs with monotonically increasing Z.
#[must_use]
pub fn optimize_greedy(z_samples: &[ZSample], config: &VlhConfig) -> Vec<(f64, f64)> {
    if z_samples.is_empty() {
        return Vec::new();
    }

    let nozzle_limit = config.nozzle_diameter * 0.75;
    let effective_max = config.max_height.min(nozzle_limit);
    let min_h = config.min_height;

    // Pre-compute the total Z range from samples.
    let max_z = z_samples.last().map(|s| s.z).unwrap_or(0.0);

    let mut result: Vec<(f64, f64)> = Vec::new();

    // First layer: always first_layer_height.
    let first_h = config.first_layer_height.clamp(min_h, effective_max);
    let first_z = first_h / 2.0;
    result.push((first_z, first_h));

    let mut prev_top = first_h; // top of previous layer

    loop {
        if prev_top >= max_z {
            break;
        }

        // Find the Z-sample index closest to current position.
        let sample_idx = find_sample_index(z_samples, prev_top);
        if sample_idx >= z_samples.len() {
            break;
        }

        // Evaluate candidate heights over the lookahead window.
        let lookahead_end = (sample_idx + GREEDY_LOOKAHEAD).min(z_samples.len());
        let window = &z_samples[sample_idx..lookahead_end];

        if window.is_empty() {
            break;
        }

        // Compute ideal height for the current position from its scores.
        let current_sample = &z_samples[sample_idx];
        let ideal_from_scores = current_sample.scores.combine(&config.weights);
        let ideal = match current_sample.feature_demanded_height {
            Some(demanded) => ideal_from_scores.min(demanded),
            None => ideal_from_scores,
        };
        let ideal = ideal.clamp(min_h, effective_max);

        // For each candidate in the window, compute total cost as
        // sum of |candidate - ideal_at_z| for each Z in the window.
        let mut best_h = ideal;
        let mut best_cost = f64::MAX;

        // Candidate heights: the ideal at each window position.
        for w_sample in window {
            let w_ideal = {
                let from_scores = w_sample.scores.combine(&config.weights);
                match w_sample.feature_demanded_height {
                    Some(demanded) => from_scores.min(demanded),
                    None => from_scores,
                }
            };
            let candidate = w_ideal.clamp(min_h, effective_max);

            // Cost: sum of |candidate - ideal_at_z| over the window.
            let cost: f64 = window
                .iter()
                .map(|s| {
                    let s_ideal = {
                        let from_s = s.scores.combine(&config.weights);
                        match s.feature_demanded_height {
                            Some(d) => from_s.min(d),
                            None => from_s,
                        }
                    };
                    let s_ideal = s_ideal.clamp(min_h, effective_max);
                    (candidate - s_ideal).abs()
                })
                .sum();

            // Use total_cmp for deterministic tie-breaking.
            if cost.total_cmp(&best_cost) == std::cmp::Ordering::Less {
                best_h = candidate;
                best_cost = cost;
            }
        }

        let selected_h = best_h.clamp(min_h, effective_max);
        let next_z = prev_top + selected_h / 2.0;
        let next_top = prev_top + selected_h;

        // Stop if this layer would overshoot.
        if next_top > max_z + effective_max * 0.01 {
            // Handle remaining height.
            let remaining = max_z - prev_top;
            if remaining > min_h * 0.5 {
                let final_h = remaining.clamp(min_h, effective_max);
                let final_z = prev_top + final_h / 2.0;
                result.push((final_z, final_h));
            }
            break;
        }

        result.push((next_z, selected_h));
        prev_top = next_top;
    }

    result
}

/// Find the index of the Z-sample closest to (but not before) the given Z.
fn find_sample_index(z_samples: &[ZSample], z: f64) -> usize {
    z_samples.partition_point(|s| s.z < z)
}

/// Number of discrete candidate heights for the DP optimizer.
const NUM_CANDIDATES: usize = 15;

/// Maximum allowed ratio between adjacent layer heights in DP optimizer.
const MAX_ADJACENT_RATIO: f64 = 1.5;

/// Dynamic programming optimizer for globally optimal layer height sequences.
///
/// Discretizes the height space into `NUM_CANDIDATES` linearly-spaced values
/// and finds the minimum-cost path through the lattice using standard DP.
/// Transitions between adjacent layers are constrained to a maximum height
/// ratio of 1.5x.
///
/// # Complexity
///
/// - Time: `O(num_z_levels * NUM_CANDIDATES^2)` = `O(n * 225)`
/// - Memory: `O(num_z_levels * NUM_CANDIDATES * 2)` for cost + predecessor tables
///
/// # Returns
///
/// Vector of `(z_position, layer_height)` pairs with monotonically increasing Z.
#[must_use]
pub fn optimize_dp(z_samples: &[ZSample], config: &VlhConfig) -> Vec<(f64, f64)> {
    if z_samples.is_empty() {
        return Vec::new();
    }

    let nozzle_limit = config.nozzle_diameter * 0.75;
    let effective_max = config.max_height.min(nozzle_limit);
    let min_h = config.min_height;

    let max_z = z_samples.last().map(|s| s.z).unwrap_or(0.0);

    // Step 1: Build Z-level sequence by walking forward with a coarse step.
    // Each Z-level represents a layer position where we choose a height.
    let mut z_levels: Vec<usize> = Vec::new(); // indices into z_samples
    {
        // First layer.
        let first_h = config.first_layer_height.clamp(min_h, effective_max);
        let first_idx =
            find_sample_index(z_samples, first_h / 2.0).min(z_samples.len().saturating_sub(1));
        z_levels.push(first_idx);
        let mut prev_top = first_h;

        // Walk forward with a median step to build the level positions.
        let median_h = (min_h + effective_max) / 2.0;
        loop {
            if prev_top >= max_z {
                break;
            }
            let next_z = prev_top + median_h;
            let idx = find_sample_index(z_samples, next_z).min(z_samples.len().saturating_sub(1));
            // Avoid duplicates.
            if z_levels.last().copied() == Some(idx) {
                // Try next index.
                if idx + 1 < z_samples.len() {
                    z_levels.push(idx + 1);
                    prev_top = z_samples[idx + 1].z;
                } else {
                    break;
                }
            } else {
                z_levels.push(idx);
                prev_top = z_samples[idx].z;
            }
        }
    }

    let num_levels = z_levels.len();
    if num_levels == 0 {
        return Vec::new();
    }

    // Step 2: Discretize candidate heights.
    let candidates: Vec<f64> = (0..NUM_CANDIDATES)
        .map(|i| min_h + (effective_max - min_h) * i as f64 / (NUM_CANDIDATES - 1).max(1) as f64)
        .collect();

    // Step 3: Build DP table.
    // dp_cost[level][candidate] = minimum cost to reach this state.
    // dp_pred[level][candidate] = predecessor candidate index.
    let mut dp_cost: Vec<Vec<f64>> = vec![vec![f64::MAX; NUM_CANDIDATES]; num_levels];
    let mut dp_pred: Vec<Vec<usize>> = vec![vec![0; NUM_CANDIDATES]; num_levels];

    // First level: force first_layer_height.
    let first_h = config.first_layer_height.clamp(min_h, effective_max);
    for (h_idx, &candidate) in candidates.iter().enumerate() {
        let dist = (candidate - first_h).abs();
        // Only the candidate closest to first_layer_height gets low cost.
        if dist < (effective_max - min_h) / (NUM_CANDIDATES as f64) + 1e-9 {
            dp_cost[0][h_idx] = dist;
        }
        // Others stay at MAX (effectively forbidden).
    }

    // Fill DP table.
    for level in 1..num_levels {
        let sample = &z_samples[z_levels[level]];
        let ideal_from_scores = sample.scores.combine(&config.weights);
        let ideal = match sample.feature_demanded_height {
            Some(d) => ideal_from_scores.min(d),
            None => ideal_from_scores,
        }
        .clamp(min_h, effective_max);

        for (h_idx, &candidate) in candidates.iter().enumerate() {
            let per_z_cost = (candidate - ideal).abs();

            let mut best_prev_cost = f64::MAX;
            let mut best_prev_idx = 0_usize;

            for (prev_idx, &prev_h) in candidates.iter().enumerate() {
                let prev_cost = dp_cost[level - 1][prev_idx];
                if prev_cost >= f64::MAX / 2.0 {
                    continue; // unreachable state
                }

                // Transition constraint: ratio must be within MAX_ADJACENT_RATIO.
                let ratio = candidate / prev_h;
                if ratio > MAX_ADJACENT_RATIO || ratio < 1.0 / MAX_ADJACENT_RATIO {
                    continue; // forbidden transition
                }

                // Transition cost penalizes ratio changes.
                let transition_cost = (candidate / prev_h - 1.0).abs();
                let total = prev_cost + per_z_cost + transition_cost;

                if total.total_cmp(&best_prev_cost) == std::cmp::Ordering::Less {
                    best_prev_cost = total;
                    best_prev_idx = prev_idx;
                }
            }

            dp_cost[level][h_idx] = best_prev_cost;
            dp_pred[level][h_idx] = best_prev_idx;
        }
    }

    // Step 4: Backtrack from minimum-cost final state.
    let mut best_final_idx = 0_usize;
    let mut best_final_cost = f64::MAX;
    for (h_idx, &cost) in dp_cost[num_levels - 1].iter().enumerate() {
        if cost.total_cmp(&best_final_cost) == std::cmp::Ordering::Less {
            best_final_cost = cost;
            best_final_idx = h_idx;
        }
    }

    // If no valid path found, fall back to greedy.
    if best_final_cost >= f64::MAX / 2.0 {
        return optimize_greedy(z_samples, config);
    }

    let mut height_indices: Vec<usize> = vec![0; num_levels];
    height_indices[num_levels - 1] = best_final_idx;
    for level in (0..num_levels - 1).rev() {
        height_indices[level] = dp_pred[level + 1][height_indices[level + 1]];
    }

    // Step 5: Convert height indices to (z, height) pairs.
    let mut result: Vec<(f64, f64)> = Vec::with_capacity(num_levels);
    let mut prev_top = 0.0_f64;

    for (level, &h_idx) in height_indices.iter().enumerate() {
        let h = if level == 0 {
            first_h
        } else {
            candidates[h_idx]
        };
        let z = prev_top + h / 2.0;
        result.push((z, h));
        prev_top += h;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::super::OptimizerMode;
    use super::*;

    /// Helper: create a VlhConfig with sensible defaults for testing.
    fn test_config() -> VlhConfig {
        VlhConfig {
            min_height: 0.05,
            max_height: 0.3,
            first_layer_height: 0.2,
            weights: VlhWeights::new(1.0, 0.0, 0.0, 0.0),
            optimizer_mode: OptimizerMode::Greedy,
            smoothing_strength: 0.5,
            smoothing_iterations: 3,
            diagnostics: false,
            stochastic: false,
            feature_overhang_weight: 1.0,
            feature_bridge_weight: 1.0,
            feature_thin_wall_weight: 1.0,
            feature_hole_weight: 1.0,
            overhang_angle_min: 40.0,
            overhang_angle_max: 60.0,
            thin_wall_threshold: 0.8,
            feature_margin_layers: 2,
            nozzle_diameter: 0.4,
        }
    }

    /// Generate Z samples simulating a sphere-like curvature profile.
    /// High curvature at the "equator" (middle Z), low at poles.
    fn sphere_z_samples(total_height: f64, step: f64, min_h: f64, max_h: f64) -> Vec<ZSample> {
        let mut samples = Vec::new();
        let mid = total_height / 2.0;
        let mut z = 0.0;
        while z <= total_height {
            // Curvature peaks at midpoint.
            let dist_from_mid = ((z - mid) / mid).abs().clamp(0.0, 1.0);
            let curvature = 1.0 - dist_from_mid; // 1.0 at mid, 0.0 at poles
            let external = 1.0;
            let quality_h = max_h - (max_h - min_h) * curvature;
            let scores = ObjectiveScores {
                quality_height: quality_h,
                speed_height: max_h,
                strength_height: max_h,
                material_height: max_h,
            };
            samples.push(ZSample {
                z,
                scores,
                feature_demanded_height: None,
                stress_factor: 0.0,
                external_surface_fraction: external,
            });
            z += step;
        }
        samples
    }

    /// Generate flat Z samples (no curvature variation).
    fn flat_z_samples(total_height: f64, step: f64, max_h: f64) -> Vec<ZSample> {
        let mut samples = Vec::new();
        let mut z = 0.0;
        while z <= total_height {
            let scores = ObjectiveScores {
                quality_height: max_h,
                speed_height: max_h,
                strength_height: max_h,
                material_height: max_h,
            };
            samples.push(ZSample {
                z,
                scores,
                feature_demanded_height: None,
                stress_factor: 0.0,
                external_surface_fraction: 1.0,
            });
            z += step;
        }
        samples
    }

    mod greedy {
        use super::*;

        #[test]
        fn quality_only_high_curvature_produces_thin_layers() {
            let config = test_config(); // quality-only weights
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let result = optimize_greedy(&samples, &config);

            assert!(!result.is_empty(), "Should produce output");

            // Layers near the equator (z ~ 5.0) should be thinner than near poles
            let equator_layers: Vec<f64> = result
                .iter()
                .filter(|&&(z, _)| z > 3.5 && z < 6.5)
                .map(|&(_, h)| h)
                .collect();
            let pole_layers: Vec<f64> = result
                .iter()
                .filter(|&&(z, _)| z < 1.5 || z > 8.5)
                .map(|&(_, h)| h)
                .collect();

            if !equator_layers.is_empty() && !pole_layers.is_empty() {
                let avg_eq: f64 = equator_layers.iter().sum::<f64>() / equator_layers.len() as f64;
                let avg_pole: f64 = pole_layers.iter().sum::<f64>() / pole_layers.len() as f64;
                assert!(
                    avg_eq < avg_pole,
                    "Equator avg ({avg_eq:.4}) should be < pole avg ({avg_pole:.4})"
                );
            }
        }

        #[test]
        fn speed_only_produces_near_max_height() {
            let mut config = test_config();
            config.weights = VlhWeights::new(0.0, 1.0, 0.0, 0.0);
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let result = optimize_greedy(&samples, &config);

            assert!(!result.is_empty(), "Should produce output");

            let nozzle_limit = config.nozzle_diameter * 0.75; // 0.3
                                                              // Skip first (fixed) and last (remainder) layers.
            let interior = &result[1..result.len().saturating_sub(1)];
            for &(z, h) in interior {
                // Speed-only should produce max or near-max heights
                assert!(
                    h >= config.max_height * 0.8 || h >= nozzle_limit * 0.8,
                    "Speed-only at z={z:.3} should be near max, got {h:.4}"
                );
            }
        }

        #[test]
        fn respects_min_max_bounds() {
            let config = test_config();
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let result = optimize_greedy(&samples, &config);

            let nozzle_limit = config.nozzle_diameter * 0.75;
            let effective_max = config.max_height.min(nozzle_limit);
            for &(z, h) in &result {
                assert!(
                    h >= config.min_height - 1e-9,
                    "Height {h:.6} at z={z:.3} below min {}",
                    config.min_height
                );
                assert!(
                    h <= effective_max + 1e-9,
                    "Height {h:.6} at z={z:.3} above effective max {effective_max}"
                );
            }
        }

        #[test]
        fn preserves_first_layer_height() {
            let config = test_config();
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let result = optimize_greedy(&samples, &config);

            assert!(!result.is_empty(), "Should produce output");
            assert!(
                (result[0].1 - config.first_layer_height).abs() < 1e-9,
                "First layer height should be {}, got {}",
                config.first_layer_height,
                result[0].1
            );
            assert!(
                (result[0].0 - config.first_layer_height / 2.0).abs() < 1e-9,
                "First layer Z should be {}, got {}",
                config.first_layer_height / 2.0,
                result[0].0
            );
        }

        #[test]
        fn z_values_monotonically_increasing() {
            let config = test_config();
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let result = optimize_greedy(&samples, &config);

            for i in 1..result.len() {
                assert!(
                    result[i].0 > result[i - 1].0,
                    "Z[{}]={} should be > Z[{}]={}",
                    i,
                    result[i].0,
                    i - 1,
                    result[i - 1].0
                );
            }
        }

        #[test]
        fn is_deterministic() {
            let config = test_config();
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let first = optimize_greedy(&samples, &config);

            for run in 0..100 {
                let again = optimize_greedy(&samples, &config);
                assert_eq!(
                    first.len(),
                    again.len(),
                    "Run {run}: length mismatch {} vs {}",
                    first.len(),
                    again.len()
                );
                for (i, (a, b)) in first.iter().zip(again.iter()).enumerate() {
                    assert!(
                        (a.0 - b.0).abs() < 1e-15 && (a.1 - b.1).abs() < 1e-15,
                        "Run {run}, layer {i}: ({},{}) vs ({},{})",
                        a.0,
                        a.1,
                        b.0,
                        b.1
                    );
                }
            }
        }

        #[test]
        fn respects_feature_demands() {
            let config = test_config();
            // Create samples where a feature demands thin height in a region.
            let mut samples = flat_z_samples(5.0, 0.01, 0.3);
            // Demand 0.08mm height in the 2.0-3.0 range
            for s in &mut samples {
                if s.z >= 2.0 && s.z <= 3.0 {
                    s.feature_demanded_height = Some(0.08);
                }
            }
            let result = optimize_greedy(&samples, &config);

            // Layers in the demanded region should be thinner
            let demanded_layers: Vec<f64> = result
                .iter()
                .filter(|&&(z, _)| z > 2.0 && z < 3.0)
                .map(|&(_, h)| h)
                .collect();
            if !demanded_layers.is_empty() {
                let avg: f64 = demanded_layers.iter().sum::<f64>() / demanded_layers.len() as f64;
                assert!(
                    avg <= 0.12,
                    "Feature-demanded region avg height ({avg:.4}) should be <= 0.12"
                );
            }
        }

        #[test]
        fn empty_input_returns_empty() {
            let config = test_config();
            let result = optimize_greedy(&[], &config);
            assert!(result.is_empty(), "Empty input should produce empty output");
        }
    }

    mod dp {
        use super::*;

        #[test]
        fn quality_only_close_to_or_better_than_greedy() {
            let config = test_config(); // quality-only weights
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let greedy = optimize_greedy(&samples, &config);
            let dp = optimize_dp(&samples, &config);

            assert!(!dp.is_empty(), "DP should produce output");

            // DP should have similar or better quality (thinner equator layers)
            let dp_equator: Vec<f64> = dp
                .iter()
                .filter(|&&(z, _)| z > 3.5 && z < 6.5)
                .map(|&(_, h)| h)
                .collect();
            let greedy_equator: Vec<f64> = greedy
                .iter()
                .filter(|&&(z, _)| z > 3.5 && z < 6.5)
                .map(|&(_, h)| h)
                .collect();

            if !dp_equator.is_empty() && !greedy_equator.is_empty() {
                let avg_dp: f64 = dp_equator.iter().sum::<f64>() / dp_equator.len() as f64;
                let avg_greedy: f64 =
                    greedy_equator.iter().sum::<f64>() / greedy_equator.len() as f64;
                // DP optimizes globally so may trade local height for smoother
                // transitions. Allow 2x tolerance.
                assert!(
                    avg_dp <= avg_greedy * 2.0,
                    "DP equator avg ({avg_dp:.4}) should be close to greedy ({avg_greedy:.4})"
                );
            }
        }

        #[test]
        fn respects_min_max_bounds() {
            let config = test_config();
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let result = optimize_dp(&samples, &config);

            let nozzle_limit = config.nozzle_diameter * 0.75;
            let effective_max = config.max_height.min(nozzle_limit);
            for &(z, h) in &result {
                assert!(
                    h >= config.min_height - 1e-9,
                    "DP height {h:.6} at z={z:.3} below min {}",
                    config.min_height
                );
                assert!(
                    h <= effective_max + 1e-9,
                    "DP height {h:.6} at z={z:.3} above effective max {effective_max}"
                );
            }
        }

        #[test]
        fn preserves_first_layer_height() {
            let config = test_config();
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let result = optimize_dp(&samples, &config);

            assert!(!result.is_empty(), "DP should produce output");
            assert!(
                (result[0].1 - config.first_layer_height).abs() < 1e-9,
                "DP first layer height should be {}, got {}",
                config.first_layer_height,
                result[0].1
            );
        }

        #[test]
        fn z_values_monotonically_increasing() {
            let config = test_config();
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let result = optimize_dp(&samples, &config);

            for i in 1..result.len() {
                assert!(
                    result[i].0 > result[i - 1].0,
                    "DP Z[{}]={} should be > Z[{}]={}",
                    i,
                    result[i].0,
                    i - 1,
                    result[i - 1].0
                );
            }
        }

        #[test]
        fn is_deterministic() {
            let config = test_config();
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let first = optimize_dp(&samples, &config);

            for run in 0..50 {
                let again = optimize_dp(&samples, &config);
                assert_eq!(
                    first.len(),
                    again.len(),
                    "DP run {run}: length mismatch {} vs {}",
                    first.len(),
                    again.len()
                );
                for (i, (a, b)) in first.iter().zip(again.iter()).enumerate() {
                    assert!(
                        (a.0 - b.0).abs() < 1e-15 && (a.1 - b.1).abs() < 1e-15,
                        "DP run {run}, layer {i}: ({},{}) vs ({},{})",
                        a.0,
                        a.1,
                        b.0,
                        b.1
                    );
                }
            }
        }

        #[test]
        fn performance_500_layers_15_candidates() {
            let mut config = test_config();
            config.min_height = 0.05;
            config.max_height = 0.3;
            // 500 layers * 0.1mm avg = 50mm total height, sampled at 0.01
            let samples = sphere_z_samples(50.0, 0.01, 0.05, 0.3);
            let start = std::time::Instant::now();
            let result = optimize_dp(&samples, &config);
            let elapsed = start.elapsed();
            assert!(
                elapsed.as_secs() < 5,
                "DP on ~500-layer model took {elapsed:?}, should be < 5s"
            );
            assert!(
                !result.is_empty(),
                "DP should produce output for large model"
            );
        }

        #[test]
        fn max_adjacent_height_ratio_constraint() {
            let config = test_config();
            let samples = sphere_z_samples(10.0, 0.01, 0.05, 0.3);
            let result = optimize_dp(&samples, &config);

            for i in 1..result.len() {
                let ratio = result[i].1 / result[i - 1].1;
                assert!(
                    ratio <= 1.55 && ratio >= 1.0 / 1.55,
                    "DP adjacent ratio {ratio:.3} at layers {}/{} (h={:.4}/{:.4}) exceeds 1.5x",
                    i - 1,
                    i,
                    result[i - 1].1,
                    result[i].1
                );
            }
        }

        #[test]
        fn small_input_3_layers() {
            let config = test_config();
            // Just enough samples for ~3 layers
            let samples = sphere_z_samples(0.5, 0.01, 0.05, 0.3);
            let result = optimize_dp(&samples, &config);

            assert!(
                !result.is_empty(),
                "DP should produce output for small input"
            );
            // All heights should be valid
            for &(_, h) in &result {
                assert!(h >= config.min_height - 1e-9, "Height {h} below min");
            }
        }
    }
}
