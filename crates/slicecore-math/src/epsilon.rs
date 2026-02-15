//! Floating-point comparison utilities for geometric operations.
//!
//! Provides epsilon values and approximate comparison functions tuned for
//! 3D printing coordinate spaces (millimeter-scale with sub-micron precision).

/// Default epsilon for coordinate comparison (1 nanometer in mm).
pub const EPSILON: f64 = 1e-9;

/// Epsilon for area comparisons (1 square micrometer in mm^2).
pub const AREA_EPSILON: f64 = 1e-6;

/// Returns `true` if `a` and `b` are within `eps` of each other.
#[inline]
pub fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

/// Returns `true` if `a` is within `eps` of zero.
#[inline]
pub fn approx_zero(a: f64, eps: f64) -> bool {
    a.abs() < eps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epsilon_values() {
        assert_eq!(EPSILON, 1e-9);
        assert_eq!(AREA_EPSILON, 1e-6);
    }

    #[test]
    fn approx_eq_identical() {
        assert!(approx_eq(1.0, 1.0, EPSILON));
    }

    #[test]
    fn approx_eq_within_epsilon() {
        assert!(approx_eq(1.0, 1.0 + 1e-10, EPSILON));
    }

    #[test]
    fn approx_eq_outside_epsilon() {
        assert!(!approx_eq(1.0, 1.0 + 1e-8, EPSILON));
    }

    #[test]
    fn approx_zero_at_zero() {
        assert!(approx_zero(0.0, EPSILON));
    }

    #[test]
    fn approx_zero_small_value() {
        assert!(approx_zero(1e-10, EPSILON));
    }

    #[test]
    fn approx_zero_not_zero() {
        assert!(!approx_zero(1e-8, EPSILON));
    }
}
