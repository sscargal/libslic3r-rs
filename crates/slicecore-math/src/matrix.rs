//! Matrix types for affine transformations.
//!
//! Provides 3x3 matrices (2D transforms) and 4x4 matrices (3D transforms)
//! with factory methods for common operations like translation, rotation,
//! scaling, and mirroring.
//!
//! All matrices are stored in **row-major** order: `data[row][col]`.

use serde::{Deserialize, Serialize};

use crate::point::{Point2, Point3};
use crate::vec::Vec3;

/// A 3x3 matrix for 2D affine transformations (using homogeneous coordinates).
///
/// Stored in row-major order: `data[row][col]`.
///
/// The homogeneous representation allows translation via matrix multiplication:
/// ```text
/// | a  b  tx |   | x |   | a*x + b*y + tx |
/// | c  d  ty | * | y | = | c*x + d*y + ty |
/// | 0  0  1  |   | 1 |   |       1        |
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Matrix3x3 {
    pub data: [[f64; 3]; 3],
}

impl Matrix3x3 {
    /// Returns the 3x3 identity matrix.
    pub fn identity() -> Self {
        Self {
            data: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Multiplies two 3x3 matrices: `self * other`.
    #[allow(clippy::needless_range_loop)]
    pub fn multiply(&self, other: &Matrix3x3) -> Matrix3x3 {
        let mut result = [[0.0f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    result[i][j] += self.data[i][k] * other.data[k][j];
                }
            }
        }
        Matrix3x3 { data: result }
    }

    /// Transforms a 2D point using this matrix (homogeneous multiplication).
    ///
    /// The point is treated as `(x, y, 1)` in homogeneous coordinates.
    pub fn transform_point2(&self, p: Point2) -> Point2 {
        let x = self.data[0][0] * p.x + self.data[0][1] * p.y + self.data[0][2];
        let y = self.data[1][0] * p.x + self.data[1][1] * p.y + self.data[1][2];
        Point2::new(x, y)
    }

    /// Computes the determinant of this 3x3 matrix.
    pub fn determinant(&self) -> f64 {
        let d = &self.data;
        d[0][0] * (d[1][1] * d[2][2] - d[1][2] * d[2][1])
            - d[0][1] * (d[1][0] * d[2][2] - d[1][2] * d[2][0])
            + d[0][2] * (d[1][0] * d[2][1] - d[1][1] * d[2][0])
    }

    /// Returns the transpose of this matrix.
    #[allow(clippy::needless_range_loop)]
    pub fn transpose(&self) -> Matrix3x3 {
        let mut result = [[0.0f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                result[i][j] = self.data[j][i];
            }
        }
        Matrix3x3 { data: result }
    }
}

/// A 4x4 matrix for 3D affine transformations (using homogeneous coordinates).
///
/// Stored in row-major order: `data[row][col]`.
///
/// The bottom row is `[0, 0, 0, 1]` for affine transforms, enabling
/// translation, rotation, scaling, and mirroring to be combined via
/// matrix multiplication.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Matrix4x4 {
    pub data: [[f64; 4]; 4],
}

impl Matrix4x4 {
    /// Returns the 4x4 identity matrix.
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

    /// Multiplies two 4x4 matrices: `self * other`.
    #[allow(clippy::needless_range_loop)]
    pub fn multiply(&self, other: &Matrix4x4) -> Matrix4x4 {
        let mut result = [[0.0f64; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    result[i][j] += self.data[i][k] * other.data[k][j];
                }
            }
        }
        Matrix4x4 { data: result }
    }

    /// Transforms a 3D point using this matrix (homogeneous multiplication).
    ///
    /// The point is treated as `(x, y, z, 1)` in homogeneous coordinates.
    /// The result is divided by the W component for perspective-correct
    /// transforms (though for affine transforms W is always 1).
    pub fn transform_point3(&self, p: Point3) -> Point3 {
        let x =
            self.data[0][0] * p.x + self.data[0][1] * p.y + self.data[0][2] * p.z + self.data[0][3];
        let y =
            self.data[1][0] * p.x + self.data[1][1] * p.y + self.data[1][2] * p.z + self.data[1][3];
        let z =
            self.data[2][0] * p.x + self.data[2][1] * p.y + self.data[2][2] * p.z + self.data[2][3];
        let w =
            self.data[3][0] * p.x + self.data[3][1] * p.y + self.data[3][2] * p.z + self.data[3][3];
        if w != 1.0 && w != 0.0 {
            Point3::new(x / w, y / w, z / w)
        } else {
            Point3::new(x, y, z)
        }
    }

    /// Transforms a 3D vector using this matrix.
    ///
    /// Vectors are not affected by translation (the W component is 0),
    /// so only the upper-left 3x3 submatrix is used.
    pub fn transform_vec3(&self, v: Vec3) -> Vec3 {
        let x = self.data[0][0] * v.x + self.data[0][1] * v.y + self.data[0][2] * v.z;
        let y = self.data[1][0] * v.x + self.data[1][1] * v.y + self.data[1][2] * v.z;
        let z = self.data[2][0] * v.x + self.data[2][1] * v.y + self.data[2][2] * v.z;
        Vec3::new(x, y, z)
    }

    /// Computes the determinant of this 4x4 matrix.
    pub fn determinant(&self) -> f64 {
        let d = &self.data;
        let s0 = d[0][0] * d[1][1] - d[1][0] * d[0][1];
        let s1 = d[0][0] * d[1][2] - d[1][0] * d[0][2];
        let s2 = d[0][0] * d[1][3] - d[1][0] * d[0][3];
        let s3 = d[0][1] * d[1][2] - d[1][1] * d[0][2];
        let s4 = d[0][1] * d[1][3] - d[1][1] * d[0][3];
        let s5 = d[0][2] * d[1][3] - d[1][2] * d[0][3];

        let c5 = d[2][2] * d[3][3] - d[3][2] * d[2][3];
        let c4 = d[2][1] * d[3][3] - d[3][1] * d[2][3];
        let c3 = d[2][1] * d[3][2] - d[3][1] * d[2][2];
        let c2 = d[2][0] * d[3][3] - d[3][0] * d[2][3];
        let c1 = d[2][0] * d[3][2] - d[3][0] * d[2][2];
        let c0 = d[2][0] * d[3][1] - d[3][0] * d[2][1];

        s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0
    }

    /// Returns the transpose of this matrix.
    #[allow(clippy::needless_range_loop)]
    pub fn transpose(&self) -> Matrix4x4 {
        let mut result = [[0.0f64; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                result[i][j] = self.data[j][i];
            }
        }
        Matrix4x4 { data: result }
    }

    /// Computes the inverse of this matrix, returning `None` if the matrix
    /// is singular (determinant is zero or near-zero).
    pub fn inverse(&self) -> Option<Matrix4x4> {
        let det = self.determinant();
        if det.abs() < 1e-12 {
            return None;
        }
        let inv_det = 1.0 / det;
        let d = &self.data;

        // Cofactor expansion
        let mut inv = [[0.0f64; 4]; 4];

        inv[0][0] = (d[1][1] * (d[2][2] * d[3][3] - d[2][3] * d[3][2])
            - d[1][2] * (d[2][1] * d[3][3] - d[2][3] * d[3][1])
            + d[1][3] * (d[2][1] * d[3][2] - d[2][2] * d[3][1]))
            * inv_det;

        inv[0][1] = -(d[0][1] * (d[2][2] * d[3][3] - d[2][3] * d[3][2])
            - d[0][2] * (d[2][1] * d[3][3] - d[2][3] * d[3][1])
            + d[0][3] * (d[2][1] * d[3][2] - d[2][2] * d[3][1]))
            * inv_det;

        inv[0][2] = (d[0][1] * (d[1][2] * d[3][3] - d[1][3] * d[3][2])
            - d[0][2] * (d[1][1] * d[3][3] - d[1][3] * d[3][1])
            + d[0][3] * (d[1][1] * d[3][2] - d[1][2] * d[3][1]))
            * inv_det;

        inv[0][3] = -(d[0][1] * (d[1][2] * d[2][3] - d[1][3] * d[2][2])
            - d[0][2] * (d[1][1] * d[2][3] - d[1][3] * d[2][1])
            + d[0][3] * (d[1][1] * d[2][2] - d[1][2] * d[2][1]))
            * inv_det;

        inv[1][0] = -(d[1][0] * (d[2][2] * d[3][3] - d[2][3] * d[3][2])
            - d[1][2] * (d[2][0] * d[3][3] - d[2][3] * d[3][0])
            + d[1][3] * (d[2][0] * d[3][2] - d[2][2] * d[3][0]))
            * inv_det;

        inv[1][1] = (d[0][0] * (d[2][2] * d[3][3] - d[2][3] * d[3][2])
            - d[0][2] * (d[2][0] * d[3][3] - d[2][3] * d[3][0])
            + d[0][3] * (d[2][0] * d[3][2] - d[2][2] * d[3][0]))
            * inv_det;

        inv[1][2] = -(d[0][0] * (d[1][2] * d[3][3] - d[1][3] * d[3][2])
            - d[0][2] * (d[1][0] * d[3][3] - d[1][3] * d[3][0])
            + d[0][3] * (d[1][0] * d[3][2] - d[1][2] * d[3][0]))
            * inv_det;

        inv[1][3] = (d[0][0] * (d[1][2] * d[2][3] - d[1][3] * d[2][2])
            - d[0][2] * (d[1][0] * d[2][3] - d[1][3] * d[2][0])
            + d[0][3] * (d[1][0] * d[2][2] - d[1][2] * d[2][0]))
            * inv_det;

        inv[2][0] = (d[1][0] * (d[2][1] * d[3][3] - d[2][3] * d[3][1])
            - d[1][1] * (d[2][0] * d[3][3] - d[2][3] * d[3][0])
            + d[1][3] * (d[2][0] * d[3][1] - d[2][1] * d[3][0]))
            * inv_det;

        inv[2][1] = -(d[0][0] * (d[2][1] * d[3][3] - d[2][3] * d[3][1])
            - d[0][1] * (d[2][0] * d[3][3] - d[2][3] * d[3][0])
            + d[0][3] * (d[2][0] * d[3][1] - d[2][1] * d[3][0]))
            * inv_det;

        inv[2][2] = (d[0][0] * (d[1][1] * d[3][3] - d[1][3] * d[3][1])
            - d[0][1] * (d[1][0] * d[3][3] - d[1][3] * d[3][0])
            + d[0][3] * (d[1][0] * d[3][1] - d[1][1] * d[3][0]))
            * inv_det;

        inv[2][3] = -(d[0][0] * (d[1][1] * d[2][3] - d[1][3] * d[2][1])
            - d[0][1] * (d[1][0] * d[2][3] - d[1][3] * d[2][0])
            + d[0][3] * (d[1][0] * d[2][1] - d[1][1] * d[2][0]))
            * inv_det;

        inv[3][0] = -(d[1][0] * (d[2][1] * d[3][2] - d[2][2] * d[3][1])
            - d[1][1] * (d[2][0] * d[3][2] - d[2][2] * d[3][0])
            + d[1][2] * (d[2][0] * d[3][1] - d[2][1] * d[3][0]))
            * inv_det;

        inv[3][1] = (d[0][0] * (d[2][1] * d[3][2] - d[2][2] * d[3][1])
            - d[0][1] * (d[2][0] * d[3][2] - d[2][2] * d[3][0])
            + d[0][2] * (d[2][0] * d[3][1] - d[2][1] * d[3][0]))
            * inv_det;

        inv[3][2] = -(d[0][0] * (d[1][1] * d[3][2] - d[1][2] * d[3][1])
            - d[0][1] * (d[1][0] * d[3][2] - d[1][2] * d[3][0])
            + d[0][2] * (d[1][0] * d[3][1] - d[1][1] * d[3][0]))
            * inv_det;

        inv[3][3] = (d[0][0] * (d[1][1] * d[2][2] - d[1][2] * d[2][1])
            - d[0][1] * (d[1][0] * d[2][2] - d[1][2] * d[2][0])
            + d[0][2] * (d[1][0] * d[2][1] - d[1][1] * d[2][0]))
            * inv_det;

        Some(Matrix4x4 { data: inv })
    }

    // --- Factory methods ---

    /// Creates a translation matrix.
    pub fn translation(dx: f64, dy: f64, dz: f64) -> Self {
        Self {
            data: [
                [1.0, 0.0, 0.0, dx],
                [0.0, 1.0, 0.0, dy],
                [0.0, 0.0, 1.0, dz],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a scaling matrix.
    pub fn scaling(sx: f64, sy: f64, sz: f64) -> Self {
        Self {
            data: [
                [sx, 0.0, 0.0, 0.0],
                [0.0, sy, 0.0, 0.0],
                [0.0, 0.0, sz, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a rotation matrix around the X axis.
    ///
    /// `angle_rad` is in radians.
    pub fn rotation_x(angle_rad: f64) -> Self {
        let c = angle_rad.cos();
        let s = angle_rad.sin();
        Self {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, c, -s, 0.0],
                [0.0, s, c, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a rotation matrix around the Y axis.
    ///
    /// `angle_rad` is in radians.
    pub fn rotation_y(angle_rad: f64) -> Self {
        let c = angle_rad.cos();
        let s = angle_rad.sin();
        Self {
            data: [
                [c, 0.0, s, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [-s, 0.0, c, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a rotation matrix around the Z axis.
    ///
    /// `angle_rad` is in radians.
    pub fn rotation_z(angle_rad: f64) -> Self {
        let c = angle_rad.cos();
        let s = angle_rad.sin();
        Self {
            data: [
                [c, -s, 0.0, 0.0],
                [s, c, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Creates a mirror matrix that negates the X coordinate.
    pub fn mirror_x() -> Self {
        Self::scaling(-1.0, 1.0, 1.0)
    }

    /// Creates a mirror matrix that negates the Y coordinate.
    pub fn mirror_y() -> Self {
        Self::scaling(1.0, -1.0, 1.0)
    }

    /// Creates a mirror matrix that negates the Z coordinate.
    pub fn mirror_z() -> Self {
        Self::scaling(1.0, 1.0, -1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq_f64(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn approx_eq_point3(a: Point3, b: Point3) -> bool {
        approx_eq_f64(a.x, b.x) && approx_eq_f64(a.y, b.y) && approx_eq_f64(a.z, b.z)
    }

    fn approx_eq_matrix4x4(a: &Matrix4x4, b: &Matrix4x4) -> bool {
        for i in 0..4 {
            for j in 0..4 {
                if !approx_eq_f64(a.data[i][j], b.data[i][j]) {
                    return false;
                }
            }
        }
        true
    }

    // --- Matrix3x3 tests ---

    #[test]
    fn matrix3x3_identity() {
        let id = Matrix3x3::identity();
        assert_eq!(id.data[0][0], 1.0);
        assert_eq!(id.data[1][1], 1.0);
        assert_eq!(id.data[2][2], 1.0);
        assert_eq!(id.data[0][1], 0.0);
    }

    #[test]
    fn matrix3x3_identity_transform_point() {
        let id = Matrix3x3::identity();
        let p = Point2::new(3.0, 7.0);
        let result = id.transform_point2(p);
        assert_eq!(result, p);
    }

    #[test]
    fn matrix3x3_determinant_identity() {
        assert!(approx_eq_f64(Matrix3x3::identity().determinant(), 1.0));
    }

    #[test]
    fn matrix3x3_transpose() {
        let m = Matrix3x3 {
            data: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
        };
        let t = m.transpose();
        assert_eq!(t.data[0][1], 4.0);
        assert_eq!(t.data[1][0], 2.0);
        assert_eq!(t.data[2][0], 3.0);
    }

    #[test]
    fn matrix3x3_multiply_identity() {
        let m = Matrix3x3 {
            data: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
        };
        let result = m.multiply(&Matrix3x3::identity());
        assert_eq!(result, m);
    }

    // --- Matrix4x4 basic tests ---

    #[test]
    fn matrix4x4_identity_transform_point() {
        let id = Matrix4x4::identity();
        let p = Point3::new(1.0, 2.0, 3.0);
        let result = id.transform_point3(p);
        assert!(approx_eq_point3(result, p));
    }

    #[test]
    fn matrix4x4_identity_transform_vec() {
        let id = Matrix4x4::identity();
        let v = Vec3::new(1.0, 2.0, 3.0);
        let result = id.transform_vec3(v);
        assert!(approx_eq_f64(result.x, v.x));
        assert!(approx_eq_f64(result.y, v.y));
        assert!(approx_eq_f64(result.z, v.z));
    }

    #[test]
    fn matrix4x4_determinant_identity() {
        assert!(approx_eq_f64(Matrix4x4::identity().determinant(), 1.0));
    }

    // --- Translation tests ---

    #[test]
    fn translation_moves_point() {
        let t = Matrix4x4::translation(1.0, 2.0, 3.0);
        let p = Point3::new(0.0, 0.0, 0.0);
        let result = t.transform_point3(p);
        assert!(approx_eq_point3(result, Point3::new(1.0, 2.0, 3.0)));
    }

    #[test]
    fn translation_does_not_affect_vector() {
        let t = Matrix4x4::translation(10.0, 20.0, 30.0);
        let v = Vec3::new(1.0, 0.0, 0.0);
        let result = t.transform_vec3(v);
        assert!(approx_eq_f64(result.x, 1.0));
        assert!(approx_eq_f64(result.y, 0.0));
        assert!(approx_eq_f64(result.z, 0.0));
    }

    // --- Scaling tests ---

    #[test]
    fn scaling_scales_point() {
        let s = Matrix4x4::scaling(2.0, 3.0, 4.0);
        let p = Point3::new(1.0, 1.0, 1.0);
        let result = s.transform_point3(p);
        assert!(approx_eq_point3(result, Point3::new(2.0, 3.0, 4.0)));
    }

    // --- Rotation tests ---

    #[test]
    fn rotation_z_90_rotates_x_to_y() {
        let r = Matrix4x4::rotation_z(std::f64::consts::FRAC_PI_2);
        let p = Point3::new(1.0, 0.0, 0.0);
        let result = r.transform_point3(p);
        assert!(approx_eq_point3(result, Point3::new(0.0, 1.0, 0.0)));
    }

    #[test]
    fn rotation_x_90_rotates_y_to_z() {
        let r = Matrix4x4::rotation_x(std::f64::consts::FRAC_PI_2);
        let p = Point3::new(0.0, 1.0, 0.0);
        let result = r.transform_point3(p);
        assert!(approx_eq_point3(result, Point3::new(0.0, 0.0, 1.0)));
    }

    #[test]
    fn rotation_y_90_rotates_z_to_x() {
        let r = Matrix4x4::rotation_y(std::f64::consts::FRAC_PI_2);
        let p = Point3::new(0.0, 0.0, 1.0);
        let result = r.transform_point3(p);
        assert!(approx_eq_point3(result, Point3::new(1.0, 0.0, 0.0)));
    }

    // --- Inverse tests ---

    #[test]
    fn inverse_of_translation() {
        let t = Matrix4x4::translation(1.0, 2.0, 3.0);
        let inv = t.inverse().expect("translation is invertible");
        let expected = Matrix4x4::translation(-1.0, -2.0, -3.0);
        assert!(approx_eq_matrix4x4(&inv, &expected));
    }

    #[test]
    fn inverse_identity() {
        let inv = Matrix4x4::identity()
            .inverse()
            .expect("identity is invertible");
        assert!(approx_eq_matrix4x4(&inv, &Matrix4x4::identity()));
    }

    #[test]
    fn inverse_of_singular_matrix_returns_none() {
        // All-zeros matrix is singular
        let m = Matrix4x4 {
            data: [[0.0; 4]; 4],
        };
        assert!(m.inverse().is_none());
    }

    // --- Mirror tests ---

    #[test]
    fn mirror_x_negates_x() {
        let m = Matrix4x4::mirror_x();
        let p = Point3::new(5.0, 3.0, 1.0);
        let result = m.transform_point3(p);
        assert!(approx_eq_point3(result, Point3::new(-5.0, 3.0, 1.0)));
    }

    #[test]
    fn mirror_y_negates_y() {
        let m = Matrix4x4::mirror_y();
        let p = Point3::new(5.0, 3.0, 1.0);
        let result = m.transform_point3(p);
        assert!(approx_eq_point3(result, Point3::new(5.0, -3.0, 1.0)));
    }

    #[test]
    fn mirror_z_negates_z() {
        let m = Matrix4x4::mirror_z();
        let p = Point3::new(5.0, 3.0, 1.0);
        let result = m.transform_point3(p);
        assert!(approx_eq_point3(result, Point3::new(5.0, 3.0, -1.0)));
    }

    // --- Associativity test ---

    #[test]
    fn multiply_is_associative() {
        let a = Matrix4x4::translation(1.0, 2.0, 3.0);
        let b = Matrix4x4::scaling(2.0, 2.0, 2.0);
        let c = Matrix4x4::rotation_z(0.5);

        let ab_c = a.multiply(&b).multiply(&c);
        let a_bc = a.multiply(&b.multiply(&c));

        assert!(approx_eq_matrix4x4(&ab_c, &a_bc));
    }

    // --- Transpose test ---

    #[test]
    fn transpose_of_identity() {
        let id = Matrix4x4::identity();
        assert_eq!(id.transpose(), id);
    }

    // --- Determinant of scaling ---

    #[test]
    fn determinant_of_scaling() {
        let s = Matrix4x4::scaling(2.0, 3.0, 4.0);
        assert!(approx_eq_f64(s.determinant(), 24.0));
    }
}
