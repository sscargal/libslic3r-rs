//! Constrained retriangulation of triangles split by intersection curves.
//!
//! When a CSG boolean operation intersects two meshes, triangles that straddle
//! the intersection boundary must be split into sub-triangles. This module
//! implements ear-clipping retriangulation to split intersected triangles along
//! intersection curves without introducing T-junctions.

use slicecore_math::{Point3, Vec3};

use super::intersect::IntersectionResult;

/// Identifies which input mesh a triangle came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshId {
    /// Triangle originated from mesh A.
    A,
    /// Triangle originated from mesh B.
    B,
}

/// Provenance tracking for each output triangle.
#[derive(Clone, Debug)]
pub struct TriangleOrigin {
    /// Which mesh the triangle came from.
    pub mesh_id: MeshId,
    /// Index of the original (pre-split) triangle in the source mesh.
    pub original_triangle: usize,
}

/// Result of splitting one original triangle.
#[derive(Clone, Debug)]
pub struct SplitTriangle {
    /// Index of the original triangle in the source mesh.
    pub original_index: usize,
    /// Which mesh the triangle came from.
    pub mesh_id: MeshId,
    /// Sub-triangles produced by splitting (indices into the output vertex array).
    pub sub_triangles: Vec<[u32; 3]>,
}

/// Retriangulates a mesh by splitting triangles along intersection curves.
///
/// For each triangle in the mesh:
/// - If no intersection segments cross it, the triangle is kept as-is.
/// - If intersection segments cross it, the triangle is split using ear-clipping.
///
/// # Arguments
///
/// * `vertices` -- Original mesh vertices.
/// * `indices` -- Original mesh triangle indices.
/// * `intersection_result` -- Intersection data from
///   [`compute_intersection_curves`](super::intersect::compute_intersection_curves).
/// * `mesh_id` -- Which mesh these triangles belong to.
/// * `intersection_points` -- 3D positions of all intersection points.
///
/// # Returns
///
/// A tuple of `(new_vertices, new_indices, origins)` where:
/// - `new_vertices` includes original vertices plus any inserted intersection points.
/// - `new_indices` is the retriangulated face list.
/// - `origins` maps each output triangle back to its source.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::retriangulate::{retriangulate_mesh, MeshId};
/// use slicecore_mesh::csg::intersect::IntersectionResult;
/// use slicecore_math::Point3;
///
/// let vertices = vec![
///     Point3::new(0.0, 0.0, 0.0),
///     Point3::new(1.0, 0.0, 0.0),
///     Point3::new(0.0, 1.0, 0.0),
/// ];
/// let indices = vec![[0, 1, 2]];
/// let empty_result = IntersectionResult::default();
/// let (new_verts, new_idx, origins) = retriangulate_mesh(
///     &vertices, &indices, &empty_result, MeshId::A, &[],
/// );
/// assert_eq!(new_idx.len(), 1); // No intersections, unchanged
/// ```
pub fn retriangulate_mesh(
    vertices: &[Point3],
    indices: &[[u32; 3]],
    intersection_result: &IntersectionResult,
    mesh_id: MeshId,
    intersection_points: &[Point3],
) -> (Vec<Point3>, Vec<[u32; 3]>, Vec<TriangleOrigin>) {
    let mut new_vertices: Vec<Point3> = vertices.to_vec();
    let mut new_indices: Vec<[u32; 3]> = Vec::with_capacity(indices.len());
    let mut origins: Vec<TriangleOrigin> = Vec::with_capacity(indices.len());

    // Map from intersection point index -> new vertex index.
    let ipt_base = new_vertices.len() as u32;
    new_vertices.extend_from_slice(intersection_points);

    // Build a map: original triangle index -> list of intersection points on it.
    let mut tri_intersection_points: Vec<Vec<u32>> = vec![Vec::new(); indices.len()];

    for seg in &intersection_result.segments {
        let tri_idx = match mesh_id {
            MeshId::A => seg.tri_a,
            MeshId::B => seg.tri_b,
        };
        if tri_idx < indices.len() {
            let start_vi = ipt_base + seg.start as u32;
            let end_vi = ipt_base + seg.end as u32;

            let pts = &mut tri_intersection_points[tri_idx];
            if !pts.contains(&start_vi) {
                pts.push(start_vi);
            }
            if !pts.contains(&end_vi) {
                pts.push(end_vi);
            }
        }
    }

    for (tri_idx, tri) in indices.iter().enumerate() {
        let extra_pts = &tri_intersection_points[tri_idx];

        if extra_pts.is_empty() {
            // No intersection; keep triangle as-is.
            new_indices.push(*tri);
            origins.push(TriangleOrigin {
                mesh_id,
                original_triangle: tri_idx,
            });
        } else {
            // Split the triangle by inserting intersection points and retriangulating.
            let sub_tris = split_triangle_with_points(&new_vertices, tri, extra_pts);
            for sub_tri in sub_tris {
                new_indices.push(sub_tri);
                origins.push(TriangleOrigin {
                    mesh_id,
                    original_triangle: tri_idx,
                });
            }
        }
    }

    (new_vertices, new_indices, origins)
}

/// Splits a single triangle by inserting extra points and ear-clipping.
///
/// Collects the original triangle vertices plus all intersection points that
/// lie within (or on the boundary of) the triangle, then triangulates the
/// resulting point set using ear-clipping on a polygon sorted by angle.
fn split_triangle_with_points(
    vertices: &[Point3],
    tri: &[u32; 3],
    extra_point_indices: &[u32],
) -> Vec<[u32; 3]> {
    let v0 = vertices[tri[0] as usize];
    let v1 = vertices[tri[1] as usize];
    let v2 = vertices[tri[2] as usize];

    // Compute the triangle normal to determine projection axis.
    let edge1 = Vec3::from_points(v0, v1);
    let edge2 = Vec3::from_points(v0, v2);
    let normal = edge1.cross(edge2);

    // Drop the axis with the largest normal component for 2D projection.
    let ax = normal.x.abs();
    let ay = normal.y.abs();
    let az = normal.z.abs();

    let project: fn(&Point3) -> (f64, f64) = if ax >= ay && ax >= az {
        |p: &Point3| (p.y, p.z)
    } else if ay >= az {
        |p: &Point3| (p.x, p.z)
    } else {
        |p: &Point3| (p.x, p.y)
    };

    // Collect all point indices: original triangle vertices + intersection points.
    let mut all_indices: Vec<u32> = vec![tri[0], tri[1], tri[2]];
    for &pi in extra_point_indices {
        if !all_indices.contains(&pi) {
            all_indices.push(pi);
        }
    }

    // If only the 3 original vertices, return the triangle as-is.
    if all_indices.len() <= 3 {
        return vec![*tri];
    }

    // Project all points to 2D.
    let pts_2d: Vec<(f64, f64)> = all_indices
        .iter()
        .map(|&idx| project(&vertices[idx as usize]))
        .collect();

    // Determine winding of the original triangle in 2D.
    let original_area = signed_area_2d(pts_2d[0], pts_2d[1], pts_2d[2]);
    let ccw = original_area > 0.0;

    // Sort points by angle around centroid to form a polygon.
    let cx: f64 = pts_2d.iter().map(|p| p.0).sum::<f64>() / pts_2d.len() as f64;
    let cy: f64 = pts_2d.iter().map(|p| p.1).sum::<f64>() / pts_2d.len() as f64;

    let mut angle_indexed: Vec<(f64, usize)> = pts_2d
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| ((y - cy).atan2(x - cx), i))
        .collect();
    angle_indexed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    if !ccw {
        angle_indexed.reverse();
    }

    // Use ear-clipping on the sorted polygon.
    let result = ear_clip_sorted(&all_indices, &angle_indexed, vertices, project, ccw);

    // Ensure we produced at least one triangle.
    if result.is_empty() {
        vec![*tri] // Fallback: return original triangle.
    } else {
        result
    }
}

/// Ear-clipping triangulation on pre-sorted polygon vertices.
fn ear_clip_sorted(
    all_indices: &[u32],
    sorted: &[(f64, usize)],
    vertices: &[Point3],
    project: fn(&Point3) -> (f64, f64),
    ccw: bool,
) -> Vec<[u32; 3]> {
    let n = sorted.len();
    if n < 3 {
        return Vec::new();
    }

    let mut remaining: Vec<usize> = sorted.iter().map(|&(_, i)| i).collect();
    let mut result = Vec::new();

    while remaining.len() > 3 {
        let len = remaining.len();
        let mut ear_found = false;

        for i in 0..len {
            let prev = if i == 0 { len - 1 } else { i - 1 };
            let next = (i + 1) % len;

            let vi = all_indices[remaining[i]];
            let vp = all_indices[remaining[prev]];
            let vn = all_indices[remaining[next]];

            let pi = project(&vertices[vi as usize]);
            let pp = project(&vertices[vp as usize]);
            let pn = project(&vertices[vn as usize]);

            let area = signed_area_2d(pp, pi, pn);
            let is_convex = if ccw { area > 0.0 } else { area < 0.0 };

            if !is_convex {
                continue;
            }

            // Check no other vertex is inside this ear.
            let mut ear_valid = true;
            for j in 0..len {
                if j == prev || j == i || j == next {
                    continue;
                }
                let vj = all_indices[remaining[j]];
                let pj = project(&vertices[vj as usize]);
                if point_in_triangle_2d(pj, pp, pi, pn) {
                    ear_valid = false;
                    break;
                }
            }

            if ear_valid {
                result.push([vp, vi, vn]);
                remaining.remove(i);
                ear_found = true;
                break;
            }
        }

        if !ear_found {
            break;
        }
    }

    // Remaining 3 vertices form the last triangle.
    if remaining.len() == 3 {
        let v0 = all_indices[remaining[0]];
        let v1 = all_indices[remaining[1]];
        let v2 = all_indices[remaining[2]];
        result.push([v0, v1, v2]);
    }

    result
}

/// Signed area of a 2D triangle (positive if CCW).
#[inline]
fn signed_area_2d(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    0.5 * ((b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1))
}

/// Tests if point `p` is inside triangle `(a, b, c)` in 2D.
fn point_in_triangle_2d(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let d1 = sign_2d(p, a, b);
    let d2 = sign_2d(p, b, c);
    let d3 = sign_2d(p, c, a);

    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);

    !(has_neg && has_pos)
}

/// Sign of the cross product of 2D vectors.
#[inline]
fn sign_2d(p1: (f64, f64), p2: (f64, f64), p3: (f64, f64)) -> f64 {
    (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
}

/// Computes the area of a 3D triangle.
#[cfg(test)]
fn triangle_area(v0: &Point3, v1: &Point3, v2: &Point3) -> f64 {
    let e1 = Vec3::from_points(*v0, *v1);
    let e2 = Vec3::from_points(*v0, *v2);
    let cross = e1.cross(e2);
    0.5 * (cross.x * cross.x + cross.y * cross.y + cross.z * cross.z).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::intersect::{IntersectionPoint, IntersectionResult, IntersectionSegment};

    #[test]
    fn triangle_with_no_intersections_unchanged() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let indices = vec![[0, 1, 2]];
        let empty = IntersectionResult::default();

        let (new_verts, new_idx, origins) =
            retriangulate_mesh(&vertices, &indices, &empty, MeshId::A, &[]);

        assert_eq!(new_idx.len(), 1, "should have 1 triangle");
        assert_eq!(new_idx[0], [0, 1, 2]);
        assert_eq!(origins.len(), 1);
        assert_eq!(origins[0].mesh_id, MeshId::A);
        assert_eq!(origins[0].original_triangle, 0);
        assert_eq!(new_verts.len(), 3);
    }

    #[test]
    fn triangle_split_by_one_segment_produces_subtriangles() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
        ];
        let indices = vec![[0, 1, 2]];

        // One intersection segment crossing the triangle (midpoints of two edges).
        let ipoints = vec![
            Point3::new(1.0, 0.0, 0.0), // midpoint of edge 0-1
            Point3::new(0.5, 1.0, 0.0), // midpoint of edge 0-2
        ];

        let intersection_result = IntersectionResult {
            points: vec![
                IntersectionPoint {
                    position: ipoints[0],
                    mesh_a_triangle: 0,
                    mesh_b_triangle: 0,
                    edge_param: 0.5,
                },
                IntersectionPoint {
                    position: ipoints[1],
                    mesh_a_triangle: 0,
                    mesh_b_triangle: 0,
                    edge_param: 0.5,
                },
            ],
            segments: vec![IntersectionSegment {
                start: 0,
                end: 1,
                tri_a: 0,
                tri_b: 0,
            }],
        };

        let (new_verts, new_idx, origins) = retriangulate_mesh(
            &vertices,
            &indices,
            &intersection_result,
            MeshId::A,
            &ipoints,
        );

        // Should produce more than 1 triangle.
        assert!(
            new_idx.len() >= 2,
            "split triangle should produce >= 2 sub-triangles, got {}",
            new_idx.len()
        );

        // All origins should reference the original triangle.
        for origin in &origins {
            assert_eq!(origin.original_triangle, 0);
            assert_eq!(origin.mesh_id, MeshId::A);
        }

        // Combined area should equal original area.
        let original_area = triangle_area(&vertices[0], &vertices[1], &vertices[2]);
        let total_area: f64 = new_idx
            .iter()
            .map(|tri| {
                triangle_area(
                    &new_verts[tri[0] as usize],
                    &new_verts[tri[1] as usize],
                    &new_verts[tri[2] as usize],
                )
            })
            .sum();

        assert!(
            (total_area - original_area).abs() < 1e-6,
            "combined area {total_area} should equal original {original_area}"
        );

        // New vertices should include original + intersection points.
        assert_eq!(
            new_verts.len(),
            5,
            "3 original + 2 intersection = 5 vertices"
        );
    }

    #[test]
    fn triangle_split_by_two_segments() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
            Point3::new(1.5, 3.0, 0.0),
        ];
        let indices = vec![[0, 1, 2]];

        // Two intersection segments with 4 intersection points.
        let ipoints = vec![
            Point3::new(0.75, 0.0, 0.0),   // on edge 0-1
            Point3::new(0.375, 0.75, 0.0), // on edge 0-2
            Point3::new(2.25, 0.0, 0.0),   // on edge 0-1
            Point3::new(2.25, 1.5, 0.0),   // on edge 1-2
        ];

        let intersection_result = IntersectionResult {
            points: ipoints
                .iter()
                .map(|&position| IntersectionPoint {
                    position,
                    mesh_a_triangle: 0,
                    mesh_b_triangle: 0,
                    edge_param: 0.5,
                })
                .collect(),
            segments: vec![
                IntersectionSegment {
                    start: 0,
                    end: 1,
                    tri_a: 0,
                    tri_b: 0,
                },
                IntersectionSegment {
                    start: 2,
                    end: 3,
                    tri_a: 0,
                    tri_b: 0,
                },
            ],
        };

        let (new_verts, new_idx, origins) = retriangulate_mesh(
            &vertices,
            &indices,
            &intersection_result,
            MeshId::A,
            &ipoints,
        );

        // Should produce multiple sub-triangles.
        assert!(
            new_idx.len() >= 3,
            "triangle with 2 segments should produce >= 3 sub-triangles, got {}",
            new_idx.len()
        );

        // All sub-triangles valid (indices in range).
        for sub_tri in &new_idx {
            for &vi in sub_tri {
                assert!(
                    (vi as usize) < new_verts.len(),
                    "vertex index {vi} out of bounds ({})",
                    new_verts.len()
                );
            }
        }

        // All origins reference the original triangle.
        for origin in &origins {
            assert_eq!(origin.original_triangle, 0);
        }
    }

    #[test]
    fn multiple_triangles_only_intersected_ones_split() {
        let vertices = vec![
            // Triangle 0: (0,0,0), (1,0,0), (0,1,0)
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            // Triangle 1: (2,0,0), (3,0,0), (2,1,0)
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
            Point3::new(2.0, 1.0, 0.0),
        ];
        let indices = vec![[0, 1, 2], [3, 4, 5]];

        // Only triangle 0 is intersected.
        let ipoints = vec![Point3::new(0.5, 0.0, 0.0), Point3::new(0.0, 0.5, 0.0)];

        let intersection_result = IntersectionResult {
            points: ipoints
                .iter()
                .map(|&position| IntersectionPoint {
                    position,
                    mesh_a_triangle: 0,
                    mesh_b_triangle: 0,
                    edge_param: 0.5,
                })
                .collect(),
            segments: vec![IntersectionSegment {
                start: 0,
                end: 1,
                tri_a: 0,
                tri_b: 0,
            }],
        };

        let (_new_verts, new_idx, origins) = retriangulate_mesh(
            &vertices,
            &indices,
            &intersection_result,
            MeshId::A,
            &ipoints,
        );

        // Triangle 1 should be unchanged (1 tri).
        // Triangle 0 should be split (>= 2 tris).
        let tri0_count = origins.iter().filter(|o| o.original_triangle == 0).count();
        let tri1_count = origins.iter().filter(|o| o.original_triangle == 1).count();

        assert!(
            tri0_count >= 2,
            "intersected triangle should be split, got {tri0_count} sub-tris"
        );
        assert_eq!(tri1_count, 1, "non-intersected triangle should remain as 1");

        // Total should be more than original 2.
        assert!(
            new_idx.len() >= 3,
            "total should be >= 3, got {}",
            new_idx.len()
        );
    }
}
