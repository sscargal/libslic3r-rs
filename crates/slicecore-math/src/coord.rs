//! Integer coordinate types for the slicing engine.
//!
//! # Precision Strategy
//!
//! All internal coordinates use integer arithmetic with nanometer precision.
//! `COORD_SCALE = 1_000_000` means 1 mm = 1,000,000 internal units.
//!
//! With `i64`, the representable range is approximately +/- 9.2e12 mm
//! (9.2 billion meters), far beyond any 3D printer build volume.
//! Even the largest industrial printers (~10m build volume) use only
//! ~1e10 internal units, well within i64 range.
//!
//! Nanometer precision (1e-6 mm) exceeds the mechanical precision of any
//! FDM/SLA/SLS printer by several orders of magnitude, ensuring that
//! coordinate conversion introduces no meaningful precision loss.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::convert::{coord_to_mm, mm_to_coord};

/// The fundamental integer coordinate type. All polygon and path operations
/// use this type for robust, deterministic arithmetic free of floating-point
/// error accumulation.
pub type Coord = i64;

/// Scaling factor: 1 mm = 1,000,000 internal coordinate units (nanometer precision).
pub const COORD_SCALE: f64 = 1_000_000.0;

/// A 2D point in integer coordinate space.
///
/// Used for polygon vertices, path waypoints, and all 2D geometric operations
/// where deterministic arithmetic is required.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(C)]
pub struct IPoint2 {
    pub x: Coord,
    pub y: Coord,
}

impl IPoint2 {
    /// Creates a new integer point from raw coordinate values.
    #[inline]
    pub fn new(x: Coord, y: Coord) -> Self {
        Self { x, y }
    }

    /// Creates a new integer point from millimeter values.
    ///
    /// Values are multiplied by [`COORD_SCALE`] and rounded to the nearest
    /// integer. Sub-nanometer rounding is the only precision loss.
    #[inline]
    pub fn from_mm(x: f64, y: f64) -> Self {
        Self {
            x: mm_to_coord(x),
            y: mm_to_coord(y),
        }
    }

    /// Converts this integer point back to millimeter values.
    ///
    /// The result is within 1 nanometer (1e-6 mm) of the original
    /// floating-point input due to the scaling factor.
    #[inline]
    pub fn to_mm(self) -> (f64, f64) {
        (coord_to_mm(self.x), coord_to_mm(self.y))
    }

    /// Returns the origin point (0, 0).
    #[inline]
    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }
}

impl Add for IPoint2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for IPoint2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Neg for IPoint2 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Mul<Coord> for IPoint2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Coord) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Div<Coord> for IPoint2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Coord) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipoint2_new_constructs_correctly() {
        let p = IPoint2::new(100, 200);
        assert_eq!(p.x, 100);
        assert_eq!(p.y, 200);
    }

    #[test]
    fn ipoint2_from_mm_positive() {
        let p = IPoint2::from_mm(1.0, 2.5);
        assert_eq!(p.x, 1_000_000);
        assert_eq!(p.y, 2_500_000);
    }

    #[test]
    fn ipoint2_from_mm_negative() {
        let p = IPoint2::from_mm(-1.0, -0.5);
        assert_eq!(p.x, -1_000_000);
        assert_eq!(p.y, -500_000);
    }

    #[test]
    fn ipoint2_to_mm_round_trip() {
        // Typical 3D printing value: center of a 200mm bed
        let p = IPoint2::from_mm(100.123, 200.456);
        let (x, y) = p.to_mm();
        assert!((x - 100.123).abs() < 1e-6, "x round-trip: {} vs 100.123", x);
        assert!((y - 200.456).abs() < 1e-6, "y round-trip: {} vs 200.456", y);
    }

    #[test]
    fn ipoint2_to_mm_round_trip_small_increments() {
        // 0.001mm increments (typical layer height resolution)
        for i in 0..100 {
            let val = i as f64 * 0.001;
            let p = IPoint2::from_mm(val, val);
            let (x, _y) = p.to_mm();
            assert!((x - val).abs() < 1e-6, "failed at {} mm", val);
        }
    }

    #[test]
    fn ipoint2_from_mm_large_value_no_overflow() {
        let p = IPoint2::from_mm(500.0, 500.0);
        assert_eq!(p.x, 500_000_000);
        assert_eq!(p.y, 500_000_000);
        // Verify round-trip
        let (x, y) = p.to_mm();
        assert!((x - 500.0).abs() < 1e-9);
        assert!((y - 500.0).abs() < 1e-9);
    }

    #[test]
    fn ipoint2_add() {
        let a = IPoint2::new(10, 20);
        let b = IPoint2::new(30, 40);
        let result = a + b;
        assert_eq!(result, IPoint2::new(40, 60));
    }

    #[test]
    fn ipoint2_sub() {
        let a = IPoint2::new(50, 60);
        let b = IPoint2::new(10, 20);
        let result = a - b;
        assert_eq!(result, IPoint2::new(40, 40));
    }

    #[test]
    fn ipoint2_neg() {
        let p = IPoint2::new(10, -20);
        let result = -p;
        assert_eq!(result, IPoint2::new(-10, 20));
    }

    #[test]
    fn ipoint2_mul_scalar() {
        let p = IPoint2::new(5, 10);
        let result = p * 3;
        assert_eq!(result, IPoint2::new(15, 30));
    }

    #[test]
    fn ipoint2_div_scalar() {
        let p = IPoint2::new(30, 60);
        let result = p / 3;
        assert_eq!(result, IPoint2::new(10, 20));
    }

    #[test]
    fn coord_scale_value() {
        assert_eq!(COORD_SCALE, 1_000_000.0);
    }

    #[test]
    fn ipoint2_zero() {
        let p = IPoint2::zero();
        assert_eq!(p.x, 0);
        assert_eq!(p.y, 0);
    }

    #[test]
    fn ipoint2_serde_round_trip() {
        let p = IPoint2::new(12345, -67890);
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: IPoint2 = serde_json::from_str(&json).unwrap();
        assert_eq!(p, deserialized);
    }

    #[test]
    fn ipoint2_ordering() {
        let a = IPoint2::new(1, 2);
        let b = IPoint2::new(1, 3);
        let c = IPoint2::new(2, 0);
        assert!(a < b);
        assert!(b < c);
    }
}
