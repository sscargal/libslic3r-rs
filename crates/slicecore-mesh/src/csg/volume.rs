//! Signed volume and surface area computation for triangle meshes.
//!
//! Uses the divergence theorem to compute signed volume from triangle
//! meshes with consistent winding order. Positive volume indicates
//! outward-facing normals (CCW winding convention).

use slicecore_math::Point3;

/// Computes the signed volume enclosed by a triangle mesh via the divergence theorem.
///
/// Each triangle and the origin form a signed tetrahedron. The sum of their
/// volumes gives the total enclosed volume. A positive result indicates
/// outward-facing normals (CCW winding convention); negative indicates
/// inward-facing normals.
///
/// # Arguments
///
/// * `vertices` -- Mesh vertex positions.
/// * `indices` -- Triangle face indices into `vertices`.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::volume::signed_volume;
/// use slicecore_mesh::csg::primitive_box;
///
/// let cube = primitive_box(1.0, 1.0, 1.0);
/// let vol = signed_volume(cube.vertices(), cube.indices());
/// assert!((vol - 1.0).abs() < 1e-6, "unit cube should have volume 1.0, got {vol}");
/// ```
pub fn signed_volume(vertices: &[Point3], indices: &[[u32; 3]]) -> f64 {
    let mut vol = 0.0;
    for tri in indices {
        let v0 = vertices[tri[0] as usize];
        let v1 = vertices[tri[1] as usize];
        let v2 = vertices[tri[2] as usize];

        // Signed volume of tetrahedron formed by triangle + origin:
        // V = (v0 . (v1 x v2)) / 6.0
        let cross_x = v1.y * v2.z - v1.z * v2.y;
        let cross_y = v1.z * v2.x - v1.x * v2.z;
        let cross_z = v1.x * v2.y - v1.y * v2.x;
        vol += v0.x * cross_x + v0.y * cross_y + v0.z * cross_z;
    }
    vol / 6.0
}

/// Computes the total surface area of a triangle mesh.
///
/// Sums the area of each triangle: `0.5 * |edge1 x edge2|`.
///
/// # Arguments
///
/// * `vertices` -- Mesh vertex positions.
/// * `indices` -- Triangle face indices into `vertices`.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::volume::surface_area;
/// use slicecore_mesh::csg::primitive_box;
///
/// let cube = primitive_box(1.0, 1.0, 1.0);
/// let area = surface_area(cube.vertices(), cube.indices());
/// assert!((area - 6.0).abs() < 1e-6, "unit cube should have area 6.0, got {area}");
/// ```
pub fn surface_area(vertices: &[Point3], indices: &[[u32; 3]]) -> f64 {
    let mut area = 0.0;
    for tri in indices {
        let v0 = vertices[tri[0] as usize];
        let v1 = vertices[tri[1] as usize];
        let v2 = vertices[tri[2] as usize];

        let e1x = v1.x - v0.x;
        let e1y = v1.y - v0.y;
        let e1z = v1.z - v0.z;
        let e2x = v2.x - v0.x;
        let e2y = v2.y - v0.y;
        let e2z = v2.z - v0.z;

        let cx = e1y * e2z - e1z * e2y;
        let cy = e1z * e2x - e1x * e2z;
        let cz = e1x * e2y - e1y * e2x;

        area += (cx * cx + cy * cy + cz * cz).sqrt();
    }
    area * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::primitives::{primitive_box, primitive_sphere};

    #[test]
    fn unit_cube_volume() {
        let cube = primitive_box(1.0, 1.0, 1.0);
        let vol = signed_volume(cube.vertices(), cube.indices());
        assert!(
            (vol - 1.0).abs() < 1e-6,
            "unit cube volume = {vol}, expected 1.0"
        );
    }

    #[test]
    fn unit_cube_surface_area() {
        let cube = primitive_box(1.0, 1.0, 1.0);
        let area = surface_area(cube.vertices(), cube.indices());
        assert!(
            (area - 6.0).abs() < 1e-6,
            "unit cube surface area = {area}, expected 6.0"
        );
    }

    #[test]
    fn sphere_volume_approximation() {
        let sphere = primitive_sphere(1.0, 32);
        let vol = signed_volume(sphere.vertices(), sphere.indices());
        let expected = 4.0 * std::f64::consts::PI / 3.0;
        // Sphere approximation with 32 segments should be within 5%.
        assert!(
            (vol - expected).abs() / expected < 0.05,
            "sphere volume = {vol}, expected ~{expected}"
        );
    }

    #[test]
    fn scaled_box_volume() {
        let cube = primitive_box(2.0, 3.0, 4.0);
        let vol = signed_volume(cube.vertices(), cube.indices());
        assert!(
            (vol - 24.0).abs() < 1e-6,
            "2x3x4 box volume = {vol}, expected 24.0"
        );
    }
}
