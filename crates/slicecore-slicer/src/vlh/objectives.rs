//! Objective scoring functions for multi-objective VLH optimization.
//!
//! Each objective maps mesh geometry information at a given Z height to a
//! desired layer height. The four objectives (quality, speed, strength,
//! material) are combined via weighted sum to produce a single target height.
//!
//! All functions are **pure** (no state, no randomness) ensuring deterministic
//! output for SLICE-05 compliance. Computation is serial with no
//! floating-point non-determinism from parallel reduction.

use super::ObjectiveScores;

/// Compute quality-driven desired layer height from surface curvature.
///
/// High curvature on external surfaces demands thinner layers for visual
/// quality. Internal surfaces (low `external_surface_fraction`) get no
/// quality penalty since they are invisible.
///
/// # Arguments
///
/// * `curvature` - Surface curvature at this Z (0.0 = flat, 1.0 = max curve)
/// * `external_surface_fraction` - Fraction of external (visible) surface (0.0-1.0)
/// * `min_height` - Minimum layer height in mm
/// * `max_height` - Maximum layer height in mm
///
/// # Returns
///
/// Desired layer height in mm. High effective curvature -> near `min_height`,
/// zero effective curvature -> `max_height`.
#[must_use]
pub fn compute_quality_height(
    curvature: f64,
    external_surface_fraction: f64,
    min_height: f64,
    max_height: f64,
) -> f64 {
    let effective_curvature = curvature * external_surface_fraction;
    let clamped = effective_curvature.clamp(0.0, 1.0);
    max_height - (max_height - min_height) * clamped
}

/// Compute speed-driven desired layer height.
///
/// Always returns `max_height` because thicker layers are faster to print.
/// This trivial function exists for API symmetry and future extension
/// (e.g., could account for acceleration limits at different heights).
///
/// # Arguments
///
/// * `_min_height` - Minimum layer height (unused)
/// * `max_height` - Maximum layer height in mm
#[must_use]
pub fn compute_speed_height(_min_height: f64, max_height: f64) -> f64 {
    max_height
}

/// Compute strength-driven desired layer height from stress factors.
///
/// Regions with high stress (near holes, thin walls, sharp overhangs)
/// benefit from thinner layers for better inter-layer adhesion and
/// dimensional accuracy.
///
/// # Arguments
///
/// * `stress_factor` - Stress level at this Z (0.0 = no stress, 1.0 = max stress)
/// * `min_height` - Minimum layer height in mm
/// * `max_height` - Maximum layer height in mm
///
/// # Returns
///
/// Desired layer height. High stress -> near `min_height`, zero stress ->
/// `max_height`.
#[must_use]
pub fn compute_strength_height(stress_factor: f64, min_height: f64, max_height: f64) -> f64 {
    let clamped = stress_factor.clamp(0.0, 1.0);
    max_height - (max_height - min_height) * clamped
}

/// Compute material-saving desired layer height.
///
/// Always returns `max_height` because thicker layers use less material per
/// unit height (fewer layers = less inter-layer overlap). Like speed, this
/// is trivial but exists for API symmetry and future extension.
///
/// # Arguments
///
/// * `_min_height` - Minimum layer height (unused)
/// * `max_height` - Maximum layer height in mm
#[must_use]
pub fn compute_material_height(_min_height: f64, max_height: f64) -> f64 {
    max_height
}

/// Compute all four objective scores in a single call.
///
/// This is the primary entry point for the scoring system. All computation
/// is serial (no parallel reduction) for deterministic output.
///
/// # Arguments
///
/// * `curvature` - Surface curvature at this Z (0.0-1.0)
/// * `external_surface_fraction` - Fraction of external surface at this Z (0.0-1.0)
/// * `stress_factor` - Stress level at this Z (0.0-1.0)
/// * `min_height` - Minimum layer height in mm
/// * `max_height` - Maximum layer height in mm
#[must_use]
pub fn compute_objective_scores(
    curvature: f64,
    external_surface_fraction: f64,
    stress_factor: f64,
    min_height: f64,
    max_height: f64,
) -> ObjectiveScores {
    let quality_height =
        compute_quality_height(curvature, external_surface_fraction, min_height, max_height);
    let speed_height = compute_speed_height(min_height, max_height);
    let strength_height = compute_strength_height(stress_factor, min_height, max_height);
    let material_height = compute_material_height(min_height, max_height);

    ObjectiveScores {
        quality_height,
        speed_height,
        strength_height,
        material_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_H: f64 = 0.05;
    const MAX_H: f64 = 0.3;

    #[test]
    fn quality_high_curvature_returns_near_min() {
        let h = compute_quality_height(0.8, 1.0, MIN_H, MAX_H);
        // effective_curvature = 0.8, so h = 0.3 - 0.25 * 0.8 = 0.1
        assert!(
            h < (MIN_H + MAX_H) / 2.0,
            "High curvature should produce height below midpoint, got {h}"
        );
        let expected = MAX_H - (MAX_H - MIN_H) * 0.8;
        assert!(
            (h - expected).abs() < 1e-9,
            "Expected {expected}, got {h}"
        );
    }

    #[test]
    fn quality_zero_curvature_returns_max() {
        let h = compute_quality_height(0.0, 1.0, MIN_H, MAX_H);
        assert!(
            (h - MAX_H).abs() < 1e-9,
            "Zero curvature should return max_height, got {h}"
        );
    }

    #[test]
    fn quality_internal_surface_gets_max() {
        // High curvature but zero external surface fraction -> max height
        let h = compute_quality_height(1.0, 0.0, MIN_H, MAX_H);
        assert!(
            (h - MAX_H).abs() < 1e-9,
            "Internal surface (fraction=0) should return max_height, got {h}"
        );
    }

    #[test]
    fn speed_always_returns_max() {
        let h = compute_speed_height(MIN_H, MAX_H);
        assert!(
            (h - MAX_H).abs() < 1e-9,
            "Speed should always return max_height, got {h}"
        );
    }

    #[test]
    fn material_always_returns_max() {
        let h = compute_material_height(MIN_H, MAX_H);
        assert!(
            (h - MAX_H).abs() < 1e-9,
            "Material should always return max_height, got {h}"
        );
    }

    #[test]
    fn strength_high_stress_returns_near_min() {
        let h = compute_strength_height(1.0, MIN_H, MAX_H);
        assert!(
            (h - MIN_H).abs() < 1e-9,
            "Max stress should return min_height, got {h}"
        );
    }

    #[test]
    fn strength_no_stress_returns_max() {
        let h = compute_strength_height(0.0, MIN_H, MAX_H);
        assert!(
            (h - MAX_H).abs() < 1e-9,
            "Zero stress should return max_height, got {h}"
        );
    }

    #[test]
    fn combined_scores_returns_all_four() {
        let scores = compute_objective_scores(0.5, 1.0, 0.3, MIN_H, MAX_H);
        // quality: 0.3 - 0.25 * 0.5 = 0.175
        let expected_quality = MAX_H - (MAX_H - MIN_H) * 0.5;
        assert!(
            (scores.quality_height - expected_quality).abs() < 1e-9,
            "Quality height mismatch"
        );
        assert!(
            (scores.speed_height - MAX_H).abs() < 1e-9,
            "Speed height should be max"
        );
        let expected_strength = MAX_H - (MAX_H - MIN_H) * 0.3;
        assert!(
            (scores.strength_height - expected_strength).abs() < 1e-9,
            "Strength height mismatch"
        );
        assert!(
            (scores.material_height - MAX_H).abs() < 1e-9,
            "Material height should be max"
        );
    }

    #[test]
    fn objective_scoring_is_deterministic() {
        let first = compute_objective_scores(0.7, 0.8, 0.4, MIN_H, MAX_H);
        for _ in 0..100 {
            let again = compute_objective_scores(0.7, 0.8, 0.4, MIN_H, MAX_H);
            assert!(
                (again.quality_height - first.quality_height).abs() < 1e-15,
                "Quality height not deterministic"
            );
            assert!(
                (again.speed_height - first.speed_height).abs() < 1e-15,
                "Speed height not deterministic"
            );
            assert!(
                (again.strength_height - first.strength_height).abs() < 1e-15,
                "Strength height not deterministic"
            );
            assert!(
                (again.material_height - first.material_height).abs() < 1e-15,
                "Material height not deterministic"
            );
        }
    }
}
