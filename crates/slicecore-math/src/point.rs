//! Floating-point 2D and 3D point types.
//!
//! These types represent positions in continuous (millimeter) space and are used
//! for mesh vertices, projected coordinates, and geometric calculations where
//! floating-point arithmetic is appropriate.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Neg, Sub};

use crate::convert::{coord_to_mm, mm_to_coord};
use crate::coord::IPoint2;
use crate::epsilon::EPSILON;

/// A 2D point in floating-point millimeter space.
///
/// Used for mesh vertex projections, 2D geometric calculations, and
/// intermediate results before conversion to integer coordinates.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    /// Creates a new 2D point.
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Returns the origin point (0.0, 0.0).
    #[inline]
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Computes the Euclidean distance to another point.
    #[inline]
    pub fn distance_to(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Returns the midpoint between this point and another.
    #[inline]
    pub fn midpoint(&self, other: &Self) -> Self {
        Self {
            x: (self.x + other.x) * 0.5,
            y: (self.y + other.y) * 0.5,
        }
    }

    /// Converts this floating-point point to an integer coordinate point.
    ///
    /// Uses [`mm_to_coord`] for the conversion, introducing at most
    /// sub-nanometer rounding.
    #[inline]
    pub fn to_ipoint2(self) -> IPoint2 {
        IPoint2 {
            x: mm_to_coord(self.x),
            y: mm_to_coord(self.y),
        }
    }
}

impl PartialEq for Point2 {
    fn eq(&self, other: &Self) -> bool {
        (self.x - other.x).abs() < EPSILON && (self.y - other.y).abs() < EPSILON
    }
}

impl Add for Point2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Point2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Neg for Point2 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl From<IPoint2> for Point2 {
    /// Converts an integer coordinate point to a floating-point point
    /// using [`coord_to_mm`].
    fn from(ip: IPoint2) -> Self {
        Self {
            x: coord_to_mm(ip.x),
            y: coord_to_mm(ip.y),
        }
    }
}

/// A 3D point in floating-point millimeter space.
///
/// Used for mesh vertex positions, 3D geometric calculations, and
/// transformation operations.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    /// Creates a new 3D point.
    #[inline]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Returns the origin point (0.0, 0.0, 0.0).
    #[inline]
    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// Computes the Euclidean distance to another point.
    #[inline]
    pub fn distance_to(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Returns the midpoint between this point and another.
    #[inline]
    pub fn midpoint(&self, other: &Self) -> Self {
        Self {
            x: (self.x + other.x) * 0.5,
            y: (self.y + other.y) * 0.5,
            z: (self.z + other.z) * 0.5,
        }
    }

    /// Projects this 3D point to 2D by dropping the Z coordinate.
    ///
    /// This is the standard projection used when converting from 3D mesh
    /// space to the 2D layer plane during slicing.
    #[inline]
    pub fn to_point2(self) -> Point2 {
        Point2 {
            x: self.x,
            y: self.y,
        }
    }
}

impl PartialEq for Point3 {
    fn eq(&self, other: &Self) -> bool {
        (self.x - other.x).abs() < EPSILON
            && (self.y - other.y).abs() < EPSILON
            && (self.z - other.z).abs() < EPSILON
    }
}

impl Add for Point3 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for Point3 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Neg for Point3 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}
