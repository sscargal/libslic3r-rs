//! 2D and 3D vector types for geometric operations.
//!
//! Vectors represent directions and displacements, as opposed to points which
//! represent positions. Operations like dot product, cross product, and
//! normalization are defined on vectors.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::point::{Point2, Point3};

/// A 2D vector in floating-point space.
///
/// Used for directions, offsets, normals, and displacement calculations
/// in the XY plane.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    /// Creates a new 2D vector.
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Returns the zero vector.
    #[inline]
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Creates a vector from one point to another: `to - from`.
    #[inline]
    pub fn from_points(from: Point2, to: Point2) -> Self {
        Self {
            x: to.x - from.x,
            y: to.y - from.y,
        }
    }

    /// Computes the dot product of two vectors.
    #[inline]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// Computes the 2D cross product (the Z component of the 3D cross product).
    ///
    /// Returns a scalar value. Positive if `other` is counter-clockwise from `self`,
    /// negative if clockwise, zero if collinear.
    #[inline]
    pub fn cross(self, other: Self) -> f64 {
        self.x * other.y - self.y * other.x
    }

    /// Returns the Euclidean length of this vector.
    #[inline]
    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    /// Returns the squared Euclidean length (avoids a sqrt when only comparison is needed).
    #[inline]
    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    /// Returns a unit-length vector in the same direction.
    ///
    /// If the vector is zero-length, returns the zero vector.
    #[inline]
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            Self::zero()
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
            }
        }
    }

    /// Returns a vector perpendicular to this one (rotated 90 degrees counter-clockwise).
    #[inline]
    pub fn perpendicular(self) -> Self {
        Self {
            x: -self.y,
            y: self.x,
        }
    }
}

impl Add for Vec2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Neg for Vec2 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Mul<f64> for Vec2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Div<f64> for Vec2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f64) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl From<Point2> for Vec2 {
    fn from(p: Point2) -> Self {
        Self { x: p.x, y: p.y }
    }
}

/// A 3D vector in floating-point space.
///
/// Used for surface normals, displacement vectors, and 3D geometric
/// calculations such as cross products and transformations.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    /// Creates a new 3D vector.
    #[inline]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Returns the zero vector.
    #[inline]
    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// Creates a vector from one point to another: `to - from`.
    #[inline]
    pub fn from_points(from: Point3, to: Point3) -> Self {
        Self {
            x: to.x - from.x,
            y: to.y - from.y,
            z: to.z - from.z,
        }
    }

    /// Computes the dot product of two vectors.
    #[inline]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Computes the 3D cross product.
    ///
    /// The result is perpendicular to both input vectors, with direction
    /// determined by the right-hand rule.
    #[inline]
    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    /// Returns the Euclidean length of this vector.
    #[inline]
    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    /// Returns the squared Euclidean length (avoids a sqrt when only comparison is needed).
    #[inline]
    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Returns a unit-length vector in the same direction.
    ///
    /// If the vector is zero-length, returns the zero vector.
    #[inline]
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            Self::zero()
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
            }
        }
    }
}

impl Add for Vec3 {
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

impl Sub for Vec3 {
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

impl Neg for Vec3 {
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

impl Mul<f64> for Vec3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

impl Div<f64> for Vec3 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f64) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
        }
    }
}

impl From<Point3> for Vec3 {
    fn from(p: Point3) -> Self {
        Self {
            x: p.x,
            y: p.y,
            z: p.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec2_dot_product() {
        let a = Vec2::new(1.0, 0.0);
        let b = Vec2::new(0.0, 1.0);
        assert!((a.dot(b) - 0.0).abs() < 1e-12); // perpendicular
    }

    #[test]
    fn vec2_dot_product_parallel() {
        let a = Vec2::new(3.0, 4.0);
        let b = Vec2::new(3.0, 4.0);
        assert!((a.dot(b) - 25.0).abs() < 1e-12);
    }

    #[test]
    fn vec2_cross_product() {
        let a = Vec2::new(1.0, 0.0);
        let b = Vec2::new(0.0, 1.0);
        assert!((a.cross(b) - 1.0).abs() < 1e-12); // CCW
    }

    #[test]
    fn vec2_cross_product_clockwise() {
        let a = Vec2::new(0.0, 1.0);
        let b = Vec2::new(1.0, 0.0);
        assert!((a.cross(b) - (-1.0)).abs() < 1e-12); // CW
    }

    #[test]
    fn vec3_dot_product() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert!((a.dot(b) - 32.0).abs() < 1e-12); // 1*4 + 2*5 + 3*6 = 32
    }

    #[test]
    fn vec3_cross_product_standard() {
        // (1,0,0) x (0,1,0) = (0,0,1)
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        let c = a.cross(b);
        assert!((c.x - 0.0).abs() < 1e-12);
        assert!((c.y - 0.0).abs() < 1e-12);
        assert!((c.z - 1.0).abs() < 1e-12);
    }

    #[test]
    fn vec3_cross_product_reversed() {
        // (0,1,0) x (1,0,0) = (0,0,-1)
        let a = Vec3::new(0.0, 1.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        let c = a.cross(b);
        assert!((c.z - (-1.0)).abs() < 1e-12);
    }

    #[test]
    fn vec2_normalize_unit_length() {
        let v = Vec2::new(3.0, 4.0);
        let n = v.normalize();
        assert!((n.length() - 1.0).abs() < 1e-12);
        assert!((n.x - 0.6).abs() < 1e-12);
        assert!((n.y - 0.8).abs() < 1e-12);
    }

    #[test]
    fn vec3_normalize_unit_length() {
        let v = Vec3::new(1.0, 2.0, 2.0);
        let n = v.normalize();
        assert!((n.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn vec2_normalize_zero_returns_zero() {
        let v = Vec2::zero();
        let n = v.normalize();
        assert_eq!(n, Vec2::zero());
    }

    #[test]
    fn vec3_normalize_zero_returns_zero() {
        let v = Vec3::zero();
        let n = v.normalize();
        assert_eq!(n, Vec3::zero());
    }

    #[test]
    fn vec2_length_and_length_squared_consistency() {
        let v = Vec2::new(3.0, 4.0);
        let len = v.length();
        let len_sq = v.length_squared();
        assert!((len * len - len_sq).abs() < 1e-12);
        assert!((len - 5.0).abs() < 1e-12);
    }

    #[test]
    fn vec3_length_and_length_squared_consistency() {
        let v = Vec3::new(1.0, 2.0, 2.0);
        let len = v.length();
        let len_sq = v.length_squared();
        assert!((len * len - len_sq).abs() < 1e-12);
        assert!((len - 3.0).abs() < 1e-12);
    }

    #[test]
    fn vec2_perpendicular_is_orthogonal() {
        let v = Vec2::new(3.0, 4.0);
        let perp = v.perpendicular();
        assert!((v.dot(perp) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn vec2_perpendicular_same_length() {
        let v = Vec2::new(3.0, 4.0);
        let perp = v.perpendicular();
        assert!((v.length() - perp.length()).abs() < 1e-12);
    }

    #[test]
    fn vec2_from_points() {
        let from = Point2::new(1.0, 2.0);
        let to = Point2::new(4.0, 6.0);
        let v = Vec2::from_points(from, to);
        assert_eq!(v, Vec2::new(3.0, 4.0));
    }

    #[test]
    fn vec3_from_points() {
        let from = Point3::new(1.0, 2.0, 3.0);
        let to = Point3::new(5.0, 7.0, 11.0);
        let v = Vec3::from_points(from, to);
        assert_eq!(v, Vec3::new(4.0, 5.0, 8.0));
    }

    #[test]
    fn vec2_add() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        assert_eq!(a + b, Vec2::new(4.0, 6.0));
    }

    #[test]
    fn vec2_sub() {
        let a = Vec2::new(5.0, 7.0);
        let b = Vec2::new(1.0, 2.0);
        assert_eq!(a - b, Vec2::new(4.0, 5.0));
    }

    #[test]
    fn vec2_neg() {
        let v = Vec2::new(3.0, -4.0);
        assert_eq!(-v, Vec2::new(-3.0, 4.0));
    }

    #[test]
    fn vec2_mul_scalar() {
        let v = Vec2::new(1.0, 2.0);
        assert_eq!(v * 3.0, Vec2::new(3.0, 6.0));
    }

    #[test]
    fn vec2_div_scalar() {
        let v = Vec2::new(6.0, 9.0);
        assert_eq!(v / 3.0, Vec2::new(2.0, 3.0));
    }

    #[test]
    fn vec3_add() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vec3::new(5.0, 7.0, 9.0));
    }

    #[test]
    fn vec3_neg() {
        let v = Vec3::new(1.0, -2.0, 3.0);
        assert_eq!(-v, Vec3::new(-1.0, 2.0, -3.0));
    }

    #[test]
    fn vec3_mul_scalar() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v * 2.0, Vec3::new(2.0, 4.0, 6.0));
    }

    #[test]
    fn vec2_from_point2() {
        let p = Point2::new(3.0, 4.0);
        let v: Vec2 = p.into();
        assert_eq!(v, Vec2::new(3.0, 4.0));
    }

    #[test]
    fn vec3_from_point3() {
        let p = Point3::new(1.0, 2.0, 3.0);
        let v: Vec3 = p.into();
        assert_eq!(v, Vec3::new(1.0, 2.0, 3.0));
    }
}
