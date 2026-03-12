//! Mesh primitive generators for CSG operations.
//!
//! Each function produces a watertight (manifold) [`TriangleMesh`] centered
//! at the origin with outward-facing normals (CCW winding convention).

use crate::triangle_mesh::TriangleMesh;

/// Creates an axis-aligned box centered at the origin.
///
/// Produces 12 triangles (2 per face).
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::primitive_box;
///
/// let mesh = primitive_box(2.0, 3.0, 4.0);
/// assert_eq!(mesh.triangle_count(), 12);
/// ```
pub fn primitive_box(_width: f64, _height: f64, _depth: f64) -> TriangleMesh {
    todo!("implemented in Task 2")
}

/// Creates a box with filleted edges and rounded corners.
///
/// The `fillet_radius` is clamped to half the smallest dimension.
///
/// # Examples
///
/// ```ignore
/// use slicecore_mesh::csg::primitive_rounded_box;
///
/// let mesh = primitive_rounded_box(2.0, 2.0, 2.0, 0.2, 8);
/// assert!(mesh.triangle_count() > 12);
/// ```
pub fn primitive_rounded_box(
    _width: f64,
    _height: f64,
    _depth: f64,
    _fillet_radius: f64,
    _segments: u32,
) -> TriangleMesh {
    todo!("implemented in Task 2")
}

/// Creates a cylinder centered at the origin.
///
/// The cylinder extends from `z = -height/2` to `z = +height/2`.
///
/// # Examples
///
/// ```ignore
/// use slicecore_mesh::csg::primitive_cylinder;
///
/// let mesh = primitive_cylinder(1.0, 2.0, 32);
/// assert!(mesh.triangle_count() > 0);
/// ```
pub fn primitive_cylinder(_radius: f64, _height: f64, _segments: u32) -> TriangleMesh {
    todo!("implemented in Task 2")
}

/// Creates a UV-sphere centered at the origin.
///
/// # Examples
///
/// ```ignore
/// use slicecore_mesh::csg::primitive_sphere;
///
/// let mesh = primitive_sphere(1.0, 16);
/// assert!(mesh.triangle_count() > 0);
/// ```
pub fn primitive_sphere(_radius: f64, _segments: u32) -> TriangleMesh {
    todo!("implemented in Task 2")
}

/// Creates a cone with apex at `z = +height/2` and base at `z = -height/2`.
///
/// # Examples
///
/// ```ignore
/// use slicecore_mesh::csg::primitive_cone;
///
/// let mesh = primitive_cone(1.0, 2.0, 32);
/// assert!(mesh.triangle_count() > 0);
/// ```
pub fn primitive_cone(_radius: f64, _height: f64, _segments: u32) -> TriangleMesh {
    todo!("implemented in Task 2")
}

/// Creates a torus centered at the origin in the XY plane.
///
/// # Examples
///
/// ```ignore
/// use slicecore_mesh::csg::primitive_torus;
///
/// let mesh = primitive_torus(2.0, 0.5, 32, 16);
/// assert!(mesh.triangle_count() > 0);
/// ```
pub fn primitive_torus(
    _major_radius: f64,
    _minor_radius: f64,
    _major_segments: u32,
    _minor_segments: u32,
) -> TriangleMesh {
    todo!("implemented in Task 2")
}

/// Creates a flat rectangular plane at `z = 0`.
///
/// Produces 2 triangles.
///
/// # Examples
///
/// ```ignore
/// use slicecore_mesh::csg::primitive_plane;
///
/// let mesh = primitive_plane(10.0, 10.0);
/// assert_eq!(mesh.triangle_count(), 2);
/// ```
pub fn primitive_plane(_width: f64, _depth: f64) -> TriangleMesh {
    todo!("implemented in Task 2")
}

/// Creates a right-angle wedge (triangular prism).
///
/// # Examples
///
/// ```ignore
/// use slicecore_mesh::csg::primitive_wedge;
///
/// let mesh = primitive_wedge(2.0, 1.0, 3.0);
/// assert_eq!(mesh.triangle_count(), 8);
/// ```
pub fn primitive_wedge(_width: f64, _height: f64, _depth: f64) -> TriangleMesh {
    todo!("implemented in Task 2")
}

/// Creates a regular N-sided polygon extruded to a given height.
///
/// `sides` is clamped to a minimum of 3.
///
/// # Examples
///
/// ```ignore
/// use slicecore_mesh::csg::primitive_ngon_prism;
///
/// let mesh = primitive_ngon_prism(6, 1.0, 2.0);
/// assert!(mesh.triangle_count() > 0);
/// ```
pub fn primitive_ngon_prism(_sides: u32, _radius: f64, _height: f64) -> TriangleMesh {
    todo!("implemented in Task 2")
}
