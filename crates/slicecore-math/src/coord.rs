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
