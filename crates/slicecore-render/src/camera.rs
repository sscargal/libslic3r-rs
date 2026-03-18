//! Camera setup: view/projection matrices, camera angles, and vertex normal computation.

use slicecore_math::{BBox3, Point3};

use crate::types::{Mat4f, Vec3f};

/// Predefined camera viewing angles for thumbnail generation.
#[derive(Clone, Debug, PartialEq)]
pub enum CameraAngle {
    Front,
    Back,
    Left,
    Right,
    Top,
    Isometric,
}

impl CameraAngle {
    /// Returns all 6 camera angle variants.
    pub fn all() -> Vec<CameraAngle> {
        vec![
            CameraAngle::Front,
            CameraAngle::Back,
            CameraAngle::Left,
            CameraAngle::Right,
            CameraAngle::Top,
            CameraAngle::Isometric,
        ]
    }

    /// Returns the (eye_direction, up_vector) pair for this camera angle.
    /// The eye direction points FROM the camera TOWARD the subject.
    pub fn direction_and_up(&self) -> ([f32; 3], [f32; 3]) {
        match self {
            CameraAngle::Front => ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
            CameraAngle::Back => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            CameraAngle::Left => ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            CameraAngle::Right => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            CameraAngle::Top => ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
            CameraAngle::Isometric => {
                // Looking from upper-right-front toward origin
                let inv_sqrt3 = 1.0 / 3.0f32.sqrt();
                ([-inv_sqrt3, -inv_sqrt3, -inv_sqrt3], [0.0, 0.0, 1.0])
            }
        }
    }
}

/// Constructs a right-handed look-at view matrix.
pub(crate) fn look_at(eye: Vec3f, target: Vec3f, up: Vec3f) -> Mat4f {
    let forward = target.sub(eye).normalize(); // z-axis (into screen)
    let right = forward.cross(up).normalize(); // x-axis
    let true_up = right.cross(forward); // y-axis

    Mat4f {
        data: [
            [right.x, right.y, right.z, -right.dot(eye)],
            [true_up.x, true_up.y, true_up.z, -true_up.dot(eye)],
            [-forward.x, -forward.y, -forward.z, forward.dot(eye)],
            [0.0, 0.0, 0.0, 1.0],
        ],
    }
}

/// Constructs an orthographic projection matrix.
pub(crate) fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4f {
    let rml = right - left;
    let tmb = top - bottom;
    let fmn = far - near;

    Mat4f {
        data: [
            [2.0 / rml, 0.0, 0.0, -(right + left) / rml],
            [0.0, 2.0 / tmb, 0.0, -(top + bottom) / tmb],
            [0.0, 0.0, -2.0 / fmn, -(far + near) / fmn],
            [0.0, 0.0, 0.0, 1.0],
        ],
    }
}

/// Builds view and projection matrices for a given camera angle and model bounding box.
/// The camera auto-fits so the model fills approximately 80% of the viewport.
pub(crate) fn build_camera(
    angle: &CameraAngle,
    aabb: &BBox3,
    width: u32,
    height: u32,
) -> (Mat4f, Mat4f) {
    let center = Vec3f::new(
        (aabb.min.x + aabb.max.x) as f32 * 0.5,
        (aabb.min.y + aabb.max.y) as f32 * 0.5,
        (aabb.min.z + aabb.max.z) as f32 * 0.5,
    );

    // Bounding sphere radius
    let half_size = Vec3f::new(
        (aabb.max.x - aabb.min.x) as f32 * 0.5,
        (aabb.max.y - aabb.min.y) as f32 * 0.5,
        (aabb.max.z - aabb.min.z) as f32 * 0.5,
    );
    let radius = half_size.length();

    let (dir, up) = angle.direction_and_up();
    let dir = Vec3f::new(dir[0], dir[1], dir[2]);
    let up = Vec3f::new(up[0], up[1], up[2]);

    // Place camera at distance = 2 * radius along the view direction
    let distance = radius * 2.0;
    let eye = center.sub(dir.scale(distance));

    let view = look_at(eye, center, up);

    // Orthographic projection: fit model with ~80% fill
    let padding = radius / 0.8; // 1.25x radius gives ~80% fill
    let aspect = width as f32 / height as f32;

    let (half_w, half_h) = if aspect >= 1.0 {
        (padding * aspect, padding)
    } else {
        (padding, padding / aspect)
    };

    let projection = ortho(-half_w, half_w, -half_h, half_h, 0.01, distance * 4.0);

    (view, projection)
}

/// Computes area-weighted vertex normals from face normals.
/// For each vertex, accumulates the cross product (area-weighted normal) of all
/// adjacent faces, then normalizes.
pub(crate) fn compute_vertex_normals(vertices: &[Point3], indices: &[[u32; 3]]) -> Vec<Vec3f> {
    let mut normals = vec![Vec3f::zero(); vertices.len()];

    for tri in indices {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;

        let v0 = Vec3f::from_point3(&vertices[i0]);
        let v1 = Vec3f::from_point3(&vertices[i1]);
        let v2 = Vec3f::from_point3(&vertices[i2]);

        let edge1 = v1.sub(v0);
        let edge2 = v2.sub(v0);
        let face_normal = edge1.cross(edge2); // area-weighted (not normalized)

        normals[i0] = normals[i0].add(face_normal);
        normals[i1] = normals[i1].add(face_normal);
        normals[i2] = normals[i2].add(face_normal);
    }

    for n in &mut normals {
        *n = n.normalize();
    }

    normals
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::Point3;

    #[test]
    fn all_returns_six_variants() {
        let all = CameraAngle::all();
        assert_eq!(all.len(), 6);
    }

    #[test]
    fn each_angle_produces_distinct_direction() {
        let all = CameraAngle::all();
        let dirs: Vec<_> = all.iter().map(|a| a.direction_and_up().0).collect();
        // Each direction should be distinct
        for i in 0..dirs.len() {
            for j in (i + 1)..dirs.len() {
                let same = (dirs[i][0] - dirs[j][0]).abs() < 1e-5
                    && (dirs[i][1] - dirs[j][1]).abs() < 1e-5
                    && (dirs[i][2] - dirs[j][2]).abs() < 1e-5;
                assert!(!same, "Angles {} and {} have same direction", i, j);
            }
        }
    }

    #[test]
    fn look_at_produces_nonzero_determinant() {
        let eye = Vec3f::new(0.0, 0.0, 5.0);
        let target = Vec3f::zero();
        let up = Vec3f::new(0.0, 1.0, 0.0);
        let view = look_at(eye, target, up);
        assert!(view.determinant().abs() > 1e-5, "View matrix is singular");
    }

    #[test]
    fn ortho_produces_nonzero_determinant() {
        let proj = ortho(-1.0, 1.0, -1.0, 1.0, 0.1, 100.0);
        assert!(
            proj.determinant().abs() > 1e-10,
            "Ortho projection matrix is singular"
        );
    }

    #[test]
    fn build_camera_auto_fit() {
        let aabb = BBox3::new(
            Point3::new(-10.0, -10.0, 0.0),
            Point3::new(10.0, 10.0, 20.0),
        );
        let (view, proj) = build_camera(&CameraAngle::Isometric, &aabb, 300, 300);
        assert!(
            view.determinant().abs() > 1e-5,
            "View matrix should be invertible"
        );
        assert!(
            proj.determinant().abs() > 1e-10,
            "Proj matrix should be invertible"
        );
    }

    #[test]
    fn build_camera_letterboxing() {
        let aabb = BBox3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(20.0, 20.0, 20.0));
        // Non-square viewport
        let (view, proj) = build_camera(&CameraAngle::Front, &aabb, 400, 200);
        assert!(view.determinant().abs() > 1e-5);
        assert!(proj.determinant().abs() > 1e-10);
    }

    #[test]
    fn compute_vertex_normals_cube() {
        // Simple cube: 8 vertices, 12 triangles (2 per face)
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0), // 0
            Point3::new(1.0, 0.0, 0.0), // 1
            Point3::new(1.0, 1.0, 0.0), // 2
            Point3::new(0.0, 1.0, 0.0), // 3
            Point3::new(0.0, 0.0, 1.0), // 4
            Point3::new(1.0, 0.0, 1.0), // 5
            Point3::new(1.0, 1.0, 1.0), // 6
            Point3::new(0.0, 1.0, 1.0), // 7
        ];
        let indices = vec![
            // Front face (z=0, normal -Z)
            [0, 2, 1],
            [0, 3, 2],
            // Back face (z=1, normal +Z)
            [4, 5, 6],
            [4, 6, 7],
            // Bottom (y=0, normal -Y)
            [0, 1, 5],
            [0, 5, 4],
            // Top (y=1, normal +Y)
            [3, 6, 2],
            [3, 7, 6],
            // Left (x=0, normal -X)
            [0, 4, 7],
            [0, 7, 3],
            // Right (x=1, normal +X)
            [1, 2, 6],
            [1, 6, 5],
        ];

        let normals = compute_vertex_normals(&vertices, &indices);
        assert_eq!(normals.len(), 8);

        // Each corner vertex should have a non-zero normal
        for (i, n) in normals.iter().enumerate() {
            let len = n.length();
            assert!(
                len > 0.9 && len < 1.1,
                "Vertex {} normal length {} not ~1.0",
                i,
                len
            );
        }

        // Corner 0 is adjacent to -X, -Y, -Z faces, so normal should point roughly (-1,-1,-1)
        let n0 = normals[0].normalize();
        assert!(n0.x < 0.0, "Vertex 0 normal x should be negative");
        assert!(n0.y < 0.0, "Vertex 0 normal y should be negative");
        assert!(n0.z < 0.0, "Vertex 0 normal z should be negative");
    }
}
