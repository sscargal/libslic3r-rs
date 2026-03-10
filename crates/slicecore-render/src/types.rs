//! Internal f32 math types for the rasterizer.
//!
//! These types are intentionally separate from `slicecore-math` (which uses f64)
//! to keep the rasterizer in f32 throughout for performance. They are crate-internal only.

use slicecore_math::{Point3, Vec3};

/// A 3D vector in f32 space.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3f {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    #[inline]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[inline]
    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    #[inline]
    pub fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    #[inline]
    pub fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }

    #[inline]
    pub fn scale(self, s: f32) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }

    #[inline]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    #[inline]
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len < 1e-10 {
            Self::zero()
        } else {
            self.scale(1.0 / len)
        }
    }

    /// Convert from slicecore_math::Vec3 (f64) to internal f32.
    pub fn from_vec3(v: &Vec3) -> Self {
        Self {
            x: v.x as f32,
            y: v.y as f32,
            z: v.z as f32,
        }
    }

    /// Convert from slicecore_math::Point3 (f64) to internal f32 vector.
    pub fn from_point3(p: &Point3) -> Self {
        Self {
            x: p.x as f32,
            y: p.y as f32,
            z: p.z as f32,
        }
    }
}

/// A 4D vector in f32 space (homogeneous coordinates).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Vec4f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4f {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

/// A 4x4 matrix in f32 space (row-major).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mat4f {
    pub data: [[f32; 4]; 4],
}

impl Mat4f {
    /// Returns the identity matrix.
    pub fn identity() -> Self {
        Self {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Matrix multiplication: self * other.
    #[allow(clippy::needless_range_loop)]
    pub fn multiply(&self, other: &Self) -> Self {
        let mut result = [[0.0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    result[i][j] += self.data[i][k] * other.data[k][j];
                }
            }
        }
        Self { data: result }
    }

    /// Transform a 3D point (w=1) and return the resulting Vec4f.
    pub fn transform_point3(&self, p: Vec3f) -> Vec4f {
        Vec4f {
            x: self.data[0][0] * p.x
                + self.data[0][1] * p.y
                + self.data[0][2] * p.z
                + self.data[0][3],
            y: self.data[1][0] * p.x
                + self.data[1][1] * p.y
                + self.data[1][2] * p.z
                + self.data[1][3],
            z: self.data[2][0] * p.x
                + self.data[2][1] * p.y
                + self.data[2][2] * p.z
                + self.data[2][3],
            w: self.data[3][0] * p.x
                + self.data[3][1] * p.y
                + self.data[3][2] * p.z
                + self.data[3][3],
        }
    }

    /// Transform a 3D direction vector (w=0).
    pub fn transform_vec3(&self, v: Vec3f) -> Vec3f {
        Vec3f {
            x: self.data[0][0] * v.x + self.data[0][1] * v.y + self.data[0][2] * v.z,
            y: self.data[1][0] * v.x + self.data[1][1] * v.y + self.data[1][2] * v.z,
            z: self.data[2][0] * v.x + self.data[2][1] * v.y + self.data[2][2] * v.z,
        }
    }

    /// Compute the determinant of the matrix.
    pub fn determinant(&self) -> f32 {
        let m = &self.data;
        let a = m[0][0]
            * (m[1][1] * (m[2][2] * m[3][3] - m[2][3] * m[3][2])
                - m[1][2] * (m[2][1] * m[3][3] - m[2][3] * m[3][1])
                + m[1][3] * (m[2][1] * m[3][2] - m[2][2] * m[3][1]));
        let b = m[0][1]
            * (m[1][0] * (m[2][2] * m[3][3] - m[2][3] * m[3][2])
                - m[1][2] * (m[2][0] * m[3][3] - m[2][3] * m[3][0])
                + m[1][3] * (m[2][0] * m[3][2] - m[2][2] * m[3][0]));
        let c = m[0][2]
            * (m[1][0] * (m[2][1] * m[3][3] - m[2][3] * m[3][1])
                - m[1][1] * (m[2][0] * m[3][3] - m[2][3] * m[3][0])
                + m[1][3] * (m[2][0] * m[3][1] - m[2][1] * m[3][0]));
        let d = m[0][3]
            * (m[1][0] * (m[2][1] * m[3][2] - m[2][2] * m[3][1])
                - m[1][1] * (m[2][0] * m[3][2] - m[2][2] * m[3][0])
                + m[1][2] * (m[2][0] * m[3][1] - m[2][1] * m[3][0]));
        a - b + c - d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec3f_normalize() {
        let v = Vec3f::new(3.0, 4.0, 0.0);
        let n = v.normalize();
        assert!((n.length() - 1.0).abs() < 1e-5);
        assert!((n.x - 0.6).abs() < 1e-5);
        assert!((n.y - 0.8).abs() < 1e-5);
    }

    #[test]
    fn vec3f_zero_normalize() {
        let v = Vec3f::zero();
        let n = v.normalize();
        assert!(n.length() < 1e-5);
    }

    #[test]
    fn vec3f_dot_cross() {
        let a = Vec3f::new(1.0, 0.0, 0.0);
        let b = Vec3f::new(0.0, 1.0, 0.0);
        assert!((a.dot(b)).abs() < 1e-5);
        let c = a.cross(b);
        assert!((c.z - 1.0).abs() < 1e-5);
    }

    #[test]
    fn mat4f_identity_multiply() {
        let id = Mat4f::identity();
        let result = id.multiply(&id);
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (result.data[i][j] - expected).abs() < 1e-5,
                    "mat[{}][{}] = {} expected {}",
                    i,
                    j,
                    result.data[i][j],
                    expected
                );
            }
        }
    }

    #[test]
    fn mat4f_transform_point() {
        let mut m = Mat4f::identity();
        m.data[0][3] = 10.0; // translate x by 10
        let p = Vec3f::new(1.0, 2.0, 3.0);
        let r = m.transform_point3(p);
        assert!((r.x - 11.0).abs() < 1e-5);
        assert!((r.y - 2.0).abs() < 1e-5);
        assert!((r.z - 3.0).abs() < 1e-5);
        assert!((r.w - 1.0).abs() < 1e-5);
    }

    #[test]
    fn mat4f_determinant_identity() {
        let id = Mat4f::identity();
        assert!((id.determinant() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn from_point3_conversion() {
        let p = Point3::new(1.5, 2.5, 3.5);
        let v = Vec3f::from_point3(&p);
        assert!((v.x - 1.5).abs() < 1e-5);
        assert!((v.y - 2.5).abs() < 1e-5);
        assert!((v.z - 3.5).abs() < 1e-5);
    }
}
