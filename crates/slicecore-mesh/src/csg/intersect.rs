//! Intersection curve computation between two triangle meshes.
//!
//! Finds all triangle-triangle intersection segments between mesh A and mesh B
//! using BVH-accelerated broad-phase and exact geometric predicates via
//! [`perturbed_orient3d`] for narrow-phase.
//!
//! Intersection points are canonicalized through a point registry to prevent
//! T-junctions from inconsistent floating-point computation.

use std::collections::HashMap;

use slicecore_math::{BBox3, Point3};

use crate::bvh::BVH;
use crate::triangle_mesh::TriangleMesh;

use super::perturb::perturbed_orient3d;

/// A point where two meshes intersect.
#[derive(Clone, Debug)]
pub struct IntersectionPoint {
    /// 3D position of the intersection point.
    pub position: Point3,
    /// Index of the triangle in mesh A that contains this point.
    pub mesh_a_triangle: usize,
    /// Index of the triangle in mesh B that contains this point.
    pub mesh_b_triangle: usize,
    /// Parameter along the edge where the intersection occurs (0.0 to 1.0).
    pub edge_param: f64,
}

/// A segment of an intersection curve between two meshes.
///
/// Indices refer into [`IntersectionResult::points`].
#[derive(Clone, Debug)]
pub struct IntersectionSegment {
    /// Index of the start point in the point registry.
    pub start: usize,
    /// Index of the end point in the point registry.
    pub end: usize,
    /// Index of the triangle in mesh A that produced this segment.
    pub tri_a: usize,
    /// Index of the triangle in mesh B that produced this segment.
    pub tri_b: usize,
}

/// All intersection data between two meshes.
#[derive(Clone, Debug, Default)]
pub struct IntersectionResult {
    /// Canonicalized intersection points (arena).
    pub points: Vec<IntersectionPoint>,
    /// Intersection segments referencing points by index.
    pub segments: Vec<IntersectionSegment>,
}

/// Merge tolerance for canonicalizing intersection points.
const MERGE_TOL: f64 = 1e-10;

/// Registry for canonicalizing intersection points.
///
/// Uses spatial hashing to find nearby points within the merge tolerance.
struct PointRegistry {
    points: Vec<IntersectionPoint>,
    /// Maps discretized grid cells to point indices for fast lookup.
    grid: HashMap<(i64, i64, i64), Vec<usize>>,
    cell_size: f64,
}

impl PointRegistry {
    fn new() -> Self {
        Self {
            points: Vec::new(),
            grid: HashMap::new(),
            cell_size: MERGE_TOL * 10.0, // Grid cells larger than tolerance
        }
    }

    /// Registers a point, returning its canonical index.
    ///
    /// If an existing point is within [`MERGE_TOL`], returns that index instead.
    fn register(
        &mut self,
        position: Point3,
        mesh_a_triangle: usize,
        mesh_b_triangle: usize,
        edge_param: f64,
    ) -> usize {
        let cell = self.cell_of(&position);

        // Search this cell and all 26 neighbors.
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let neighbor = (cell.0 + dx, cell.1 + dy, cell.2 + dz);
                    if let Some(indices) = self.grid.get(&neighbor) {
                        for &idx in indices {
                            let existing = &self.points[idx];
                            let dist_sq = distance_sq(&existing.position, &position);
                            if dist_sq < MERGE_TOL * MERGE_TOL {
                                return idx;
                            }
                        }
                    }
                }
            }
        }

        // No existing point found; add new one.
        let idx = self.points.len();
        self.points.push(IntersectionPoint {
            position,
            mesh_a_triangle,
            mesh_b_triangle,
            edge_param,
        });
        self.grid.entry(cell).or_default().push(idx);
        idx
    }

    fn cell_of(&self, p: &Point3) -> (i64, i64, i64) {
        (
            (p.x / self.cell_size).floor() as i64,
            (p.y / self.cell_size).floor() as i64,
            (p.z / self.cell_size).floor() as i64,
        )
    }
}

/// Computes all intersection curves between two triangle meshes.
///
/// Uses BVH broad-phase culling and exact geometric predicates for
/// narrow-phase triangle-triangle intersection computation.
///
/// # Arguments
///
/// * `mesh_a` -- First input mesh.
/// * `mesh_b` -- Second input mesh.
///
/// # Returns
///
/// An [`IntersectionResult`] containing all intersection points and segments.
/// Empty if the meshes do not intersect.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::intersect::compute_intersection_curves;
/// use slicecore_mesh::csg::primitive_box;
///
/// let box_a = primitive_box(2.0, 2.0, 2.0);
/// let box_b = primitive_box(2.0, 2.0, 2.0);
/// // Identical boxes at the same position: handled by perturbation
/// let result = compute_intersection_curves(&box_a, &box_b);
/// // Result depends on perturbation tie-breaking
/// ```
///
/// # Feature `parallel`
///
/// When the `parallel` feature is enabled, the outer triangle loop runs
/// via rayon `par_iter()` for improved throughput on large meshes.
pub fn compute_intersection_curves(
    mesh_a: &TriangleMesh,
    mesh_b: &TriangleMesh,
) -> IntersectionResult {
    // Build BVH for mesh B.
    let bvh_b = BVH::build(mesh_b.vertices(), mesh_b.indices());

    let verts_a = mesh_a.vertices();
    let indices_a = mesh_a.indices();
    let verts_b = mesh_b.vertices();
    let indices_b = mesh_b.indices();

    // Collect raw intersection hits (parallelizable).
    let raw_hits = collect_raw_hits(verts_a, indices_a, verts_b, indices_b, &bvh_b);

    // Canonicalize points through the registry (sequential).
    let mut registry = PointRegistry::new();
    let mut segments = Vec::new();

    for hit in raw_hits {
        let start = registry.register(hit.p0, hit.tri_a, hit.tri_b, hit.t0);
        let end = registry.register(hit.p1, hit.tri_a, hit.tri_b, hit.t1);

        if start != end {
            segments.push(IntersectionSegment {
                start,
                end,
                tri_a: hit.tri_a,
                tri_b: hit.tri_b,
            });
        }
    }

    IntersectionResult {
        points: registry.points,
        segments,
    }
}

/// A raw intersection hit before canonicalization.
struct RawHit {
    p0: Point3,
    p1: Point3,
    t0: f64,
    t1: f64,
    tri_a: usize,
    tri_b: usize,
}

/// Finds all raw triangle-triangle intersection hits between the two meshes.
///
/// When the `parallel` feature is enabled, runs the outer triangle loop via
/// rayon `par_iter` for improved throughput on large meshes.
#[cfg(feature = "parallel")]
fn collect_raw_hits(
    verts_a: &[Point3],
    indices_a: &[[u32; 3]],
    verts_b: &[Point3],
    indices_b: &[[u32; 3]],
    bvh_b: &BVH,
) -> Vec<RawHit> {
    use rayon::prelude::*;

    indices_a
        .par_iter()
        .enumerate()
        .flat_map(|(tri_a_idx, tri_a)| {
            find_hits_for_triangle(tri_a_idx, tri_a, verts_a, verts_b, indices_b, bvh_b)
        })
        .collect()
}

/// Sequential version without rayon.
#[cfg(not(feature = "parallel"))]
fn collect_raw_hits(
    verts_a: &[Point3],
    indices_a: &[[u32; 3]],
    verts_b: &[Point3],
    indices_b: &[[u32; 3]],
    bvh_b: &BVH,
) -> Vec<RawHit> {
    indices_a
        .iter()
        .enumerate()
        .flat_map(|(tri_a_idx, tri_a)| {
            find_hits_for_triangle(tri_a_idx, tri_a, verts_a, verts_b, indices_b, bvh_b)
        })
        .collect()
}

/// Finds raw intersection hits for a single triangle in mesh A against mesh B.
fn find_hits_for_triangle(
    tri_a_idx: usize,
    tri_a: &[u32; 3],
    verts_a: &[Point3],
    verts_b: &[Point3],
    indices_b: &[[u32; 3]],
    bvh_b: &BVH,
) -> Vec<RawHit> {
    let a0 = verts_a[tri_a[0] as usize];
    let a1 = verts_a[tri_a[1] as usize];
    let a2 = verts_a[tri_a[2] as usize];

    let aabb_a = match BBox3::from_points(&[a0, a1, a2]) {
        Some(bb) => bb,
        None => return Vec::new(),
    };

    let candidates = bvh_b.query_aabb_overlaps(&aabb_a);
    let mut hits = Vec::new();

    for tri_b_idx in candidates {
        let tri_b = &indices_b[tri_b_idx];
        let b0 = verts_b[tri_b[0] as usize];
        let b1 = verts_b[tri_b[1] as usize];
        let b2 = verts_b[tri_b[2] as usize];

        if let Some((p0, p1, t0, t1)) = intersect_triangles(
            &a0,
            &a1,
            &a2,
            &b0,
            &b1,
            &b2,
            tri_a[0] as usize,
            tri_a[1] as usize,
            tri_a[2] as usize,
            tri_b[0] as usize,
            tri_b[1] as usize,
            tri_b[2] as usize,
        ) {
            // Skip degenerate segments (both endpoints identical).
            if distance_sq(&p0, &p1) < MERGE_TOL * MERGE_TOL {
                continue;
            }

            hits.push(RawHit {
                p0,
                p1,
                t0,
                t1,
                tri_a: tri_a_idx,
                tri_b: tri_b_idx,
            });
        }
    }

    hits
}

/// Computes the intersection segment between two triangles, if any.
///
/// Returns `Some((point0, point1, t0, t1))` where point0 and point1 are the
/// endpoints of the intersection segment, and t0, t1 are edge parameters.
///
/// Uses `perturbed_orient3d` for all plane-side tests to handle coplanar cases.
#[allow(clippy::too_many_arguments)]
fn intersect_triangles(
    a0: &Point3,
    a1: &Point3,
    a2: &Point3,
    b0: &Point3,
    b1: &Point3,
    b2: &Point3,
    idx_a0: usize,
    idx_a1: usize,
    idx_a2: usize,
    idx_b0: usize,
    idx_b1: usize,
    idx_b2: usize,
) -> Option<(Point3, Point3, f64, f64)> {
    let pa = [[a0.x, a0.y, a0.z], [a1.x, a1.y, a1.z], [a2.x, a2.y, a2.z]];
    let pb = [[b0.x, b0.y, b0.z], [b1.x, b1.y, b1.z], [b2.x, b2.y, b2.z]];
    let idx_a = [idx_a0, idx_a1, idx_a2];
    let idx_b = [idx_b0, idx_b1, idx_b2];

    // Classify vertices of tri_a against plane of tri_b.
    let da: [f64; 3] = std::array::from_fn(|i| {
        perturbed_orient3d(
            pb[0], pb[1], pb[2], pa[i], idx_b[0], idx_b[1], idx_b[2], idx_a[i],
        )
    });

    // If all vertices of tri_a are on the same side of tri_b's plane, no intersection.
    if da.iter().all(|&d| d > 0.0) || da.iter().all(|&d| d < 0.0) {
        return None;
    }

    // Classify vertices of tri_b against plane of tri_a.
    let db: [f64; 3] = std::array::from_fn(|i| {
        perturbed_orient3d(
            pa[0], pa[1], pa[2], pb[i], idx_a[0], idx_a[1], idx_a[2], idx_b[i],
        )
    });

    // If all vertices of tri_b are on the same side of tri_a's plane, no intersection.
    if db.iter().all(|&d| d > 0.0) || db.iter().all(|&d| d < 0.0) {
        return None;
    }

    // Find the intersection segment endpoints.
    // For tri_a: find the two points where tri_a's edges cross tri_b's plane.
    let pts_a = plane_crossing_points(a0, a1, a2, &da);
    // For tri_b: find the two points where tri_b's edges cross tri_a's plane.
    let pts_b = plane_crossing_points(b0, b1, b2, &db);

    // The intersection segment is the overlap of the two intervals on the
    // line of intersection of the two planes.
    // Project onto the axis with largest extent for numerical stability.
    let interval_a = project_to_line(&pts_a);
    let interval_b = project_to_line(&pts_b);

    // Compute the overlap of the two intervals.
    let overlap_start = interval_a.0.max(interval_b.0);
    let overlap_end = interval_a.1.min(interval_b.1);

    if overlap_start >= overlap_end {
        return None;
    }

    // Interpolate back to get 3D points from the intervals.
    let total_a = interval_a.1 - interval_a.0;
    let total_b = interval_b.1 - interval_b.0;

    let p0 = if total_a.abs() > 1e-30 {
        let t = (overlap_start - interval_a.0) / total_a;
        lerp_point(&pts_a.0, &pts_a.1, t)
    } else {
        pts_a.0
    };

    let p1 = if total_a.abs() > 1e-30 {
        let t = (overlap_end - interval_a.0) / total_a;
        lerp_point(&pts_a.0, &pts_a.1, t)
    } else {
        pts_a.1
    };

    // Use the midpoints of b-side too for better accuracy.
    let p0_b = if total_b.abs() > 1e-30 {
        let t = (overlap_start - interval_b.0) / total_b;
        lerp_point(&pts_b.0, &pts_b.1, t)
    } else {
        pts_b.0
    };

    let p1_b = if total_b.abs() > 1e-30 {
        let t = (overlap_end - interval_b.0) / total_b;
        lerp_point(&pts_b.0, &pts_b.1, t)
    } else {
        pts_b.1
    };

    // Average both computations for best accuracy.
    let final_p0 = midpoint(&p0, &p0_b);
    let final_p1 = midpoint(&p1, &p1_b);

    let t0 = if total_a.abs() > 1e-30 {
        (overlap_start - interval_a.0) / total_a
    } else {
        0.0
    };
    let t1 = if total_a.abs() > 1e-30 {
        (overlap_end - interval_a.0) / total_a
    } else {
        1.0
    };

    Some((final_p0, final_p1, t0, t1))
}

/// Finds the two points where a triangle's edges cross a plane.
///
/// `d` contains the signed distances of each vertex to the plane.
/// Returns the two crossing points as a pair of `Point3`.
fn plane_crossing_points(v0: &Point3, v1: &Point3, v2: &Point3, d: &[f64; 3]) -> (Point3, Point3) {
    let verts = [v0, v1, v2];
    let mut crossings = Vec::with_capacity(2);

    // Check each edge for sign change.
    for i in 0..3 {
        let j = (i + 1) % 3;
        if (d[i] > 0.0 && d[j] < 0.0) || (d[i] < 0.0 && d[j] > 0.0) {
            let t = d[i] / (d[i] - d[j]);
            let p = lerp_point(verts[i], verts[j], t);
            crossings.push(p);
        }
    }

    // If we have exactly one crossing, one vertex is on the plane.
    // Find the vertex on the plane (d == 0 never happens with perturbed_orient3d,
    // but handle gracefully).
    while crossings.len() < 2 {
        // Find vertex closest to plane.
        let min_idx = d
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        crossings.push(*verts[min_idx]);
    }

    (crossings[0], crossings[1])
}

/// Projects two 3D points onto a scalar value (the dominant axis).
fn project_to_line(pts: &(Point3, Point3)) -> (f64, f64) {
    let dx = (pts.1.x - pts.0.x).abs();
    let dy = (pts.1.y - pts.0.y).abs();
    let dz = (pts.1.z - pts.0.z).abs();

    let (v0, v1) = if dx >= dy && dx >= dz {
        (pts.0.x, pts.1.x)
    } else if dy >= dz {
        (pts.0.y, pts.1.y)
    } else {
        (pts.0.z, pts.1.z)
    };

    if v0 <= v1 {
        (v0, v1)
    } else {
        (v1, v0)
    }
}

/// Linearly interpolates between two points.
#[inline]
fn lerp_point(a: &Point3, b: &Point3, t: f64) -> Point3 {
    Point3::new(
        a.x + t * (b.x - a.x),
        a.y + t * (b.y - a.y),
        a.z + t * (b.z - a.z),
    )
}

/// Computes the midpoint of two points.
#[inline]
fn midpoint(a: &Point3, b: &Point3) -> Point3 {
    Point3::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5)
}

/// Squared distance between two points.
#[inline]
fn distance_sq(a: &Point3, b: &Point3) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::primitives::{primitive_box, primitive_sphere};

    /// Creates a box mesh translated by (dx, dy, dz).
    fn translated_box(w: f64, h: f64, d: f64, dx: f64, dy: f64, dz: f64) -> TriangleMesh {
        let hw = w / 2.0;
        let hh = h / 2.0;
        let hd = d / 2.0;

        let vertices = vec![
            Point3::new(-hw + dx, -hh + dy, -hd + dz),
            Point3::new(hw + dx, -hh + dy, -hd + dz),
            Point3::new(hw + dx, hh + dy, -hd + dz),
            Point3::new(-hw + dx, hh + dy, -hd + dz),
            Point3::new(-hw + dx, -hh + dy, hd + dz),
            Point3::new(hw + dx, -hh + dy, hd + dz),
            Point3::new(hw + dx, hh + dy, hd + dz),
            Point3::new(-hw + dx, hh + dy, hd + dz),
        ];

        let indices = vec![
            [4, 5, 6],
            [4, 6, 7], // Front
            [1, 0, 3],
            [1, 3, 2], // Back
            [1, 2, 6],
            [1, 6, 5], // Right
            [0, 4, 7],
            [0, 7, 3], // Left
            [3, 7, 6],
            [3, 6, 2], // Top
            [0, 1, 5],
            [0, 5, 4], // Bottom
        ];

        TriangleMesh::new(vertices, indices).unwrap()
    }

    #[test]
    fn non_intersecting_boxes_return_empty() {
        let box_a = translated_box(1.0, 1.0, 1.0, 0.0, 0.0, 0.0);
        let box_b = translated_box(1.0, 1.0, 1.0, 5.0, 0.0, 0.0);

        let result = compute_intersection_curves(&box_a, &box_b);
        assert!(
            result.segments.is_empty(),
            "non-intersecting boxes should have no segments, got {}",
            result.segments.len()
        );
        assert!(
            result.points.is_empty(),
            "non-intersecting boxes should have no points, got {}",
            result.points.len()
        );
    }

    #[test]
    fn overlapping_boxes_have_segments() {
        let box_a = translated_box(2.0, 2.0, 2.0, 0.0, 0.0, 0.0);
        let box_b = translated_box(2.0, 2.0, 2.0, 1.0, 0.0, 0.0);

        let result = compute_intersection_curves(&box_a, &box_b);
        assert!(
            !result.segments.is_empty(),
            "overlapping boxes should produce intersection segments"
        );
        assert!(
            !result.points.is_empty(),
            "overlapping boxes should produce intersection points"
        );
    }

    #[test]
    fn identical_boxes_handled_by_perturbation() {
        let box_a = primitive_box(2.0, 2.0, 2.0);
        let box_b = primitive_box(2.0, 2.0, 2.0);

        // Should not panic. Perturbation ensures coplanar faces are resolved.
        let result = compute_intersection_curves(&box_a, &box_b);
        // The result may or may not have segments depending on perturbation
        // tie-breaking, but it must not crash.
        let _ = result;
    }

    #[test]
    fn box_and_sphere_partial_overlap() {
        let box_a = translated_box(2.0, 2.0, 2.0, 0.0, 0.0, 0.0);
        let sphere = primitive_sphere(0.8, 16);

        let result = compute_intersection_curves(&box_a, &sphere);
        // A sphere partially inside a box should produce intersection segments.
        // (the sphere at origin with r=0.8 is fully inside the 2x2x2 box centered
        // at origin, so we need a bigger sphere or offset)
        // Actually, r=0.8 < 1.0 half-extent, so sphere is fully inside.
        // Let's use a sphere that extends outside.
        let big_sphere = primitive_sphere(1.5, 16);
        let result2 = compute_intersection_curves(&box_a, &big_sphere);
        assert!(
            !result2.segments.is_empty(),
            "box and larger sphere should have intersection segments"
        );
        // Small sphere fully inside should have no intersection with boundary.
        // BVH AABB overlaps might still find candidates but narrow-phase should reject.
        // Actually, the sphere surface IS inside the box volume but its triangles
        // do intersect the box triangles? No, a sphere fully inside a box
        // has no triangle-triangle intersections.
        assert!(
            result.segments.is_empty(),
            "sphere fully inside box should have no surface-surface intersections"
        );
    }

    #[test]
    fn intersection_segments_reference_valid_points() {
        let box_a = translated_box(2.0, 2.0, 2.0, 0.0, 0.0, 0.0);
        let box_b = translated_box(2.0, 2.0, 2.0, 1.0, 1.0, 0.0);

        let result = compute_intersection_curves(&box_a, &box_b);
        for seg in &result.segments {
            assert!(
                seg.start < result.points.len(),
                "segment start index {} out of bounds ({})",
                seg.start,
                result.points.len()
            );
            assert!(
                seg.end < result.points.len(),
                "segment end index {} out of bounds ({})",
                seg.end,
                result.points.len()
            );
            assert_ne!(seg.start, seg.end, "segment should not be degenerate");
        }
    }

    #[test]
    fn single_shared_edge_two_triangles() {
        // Two triangles sharing an edge but in different planes.
        let vertices_a = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let vertices_b = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, -1.0, 0.5),
        ];
        let mesh_a = TriangleMesh::new(vertices_a, vec![[0, 1, 2]]).unwrap();
        let mesh_b = TriangleMesh::new(vertices_b, vec![[0, 1, 2]]).unwrap();

        // Should not panic.
        let result = compute_intersection_curves(&mesh_a, &mesh_b);
        // The shared edge is the intersection line.
        // Whether segments are found depends on the plane-crossing logic.
        let _ = result;
    }
}
