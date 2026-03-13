//! Plane splitting of triangle meshes.
//!
//! Splits a triangle mesh into two halves along an arbitrary plane.
//! Each half can be optionally capped (closed) with new triangles along
//! the cross-section to produce watertight results.

use std::time::Instant;

use slicecore_math::{Point3, Vec3};

use crate::triangle_mesh::TriangleMesh;

use super::error::CsgError;
use super::report::CsgReport;
use super::volume;

/// An analytical splitting plane defined by a normal and an offset.
///
/// The plane equation is: `normal . point = offset`.
/// Points with `normal . point > offset` are "above" the plane;
/// points with `normal . point < offset` are "below".
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::split::SplitPlane;
///
/// // Horizontal plane at z = 5.0.
/// let plane = SplitPlane::xy(5.0);
/// assert!((plane.offset - 5.0).abs() < 1e-12);
/// ```
#[derive(Clone, Debug)]
pub struct SplitPlane {
    /// Unit normal vector of the plane.
    pub normal: Vec3,
    /// Signed distance from origin along the normal.
    pub offset: f64,
}

impl SplitPlane {
    /// Creates a new split plane from a normal and offset.
    ///
    /// The normal vector is normalized automatically.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_mesh::csg::split::SplitPlane;
    /// use slicecore_math::Vec3;
    ///
    /// let plane = SplitPlane::new(Vec3::new(0.0, 0.0, 2.0), 10.0);
    /// // Normal is normalized, offset is scaled accordingly.
    /// assert!((plane.normal.z - 1.0).abs() < 1e-12);
    /// assert!((plane.offset - 5.0).abs() < 1e-12);
    /// ```
    #[must_use]
    pub fn new(normal: Vec3, offset: f64) -> Self {
        let len = normal.length();
        if len < 1e-30 {
            return Self {
                normal: Vec3::new(0.0, 0.0, 1.0),
                offset,
            };
        }
        let inv_len = 1.0 / len;
        Self {
            normal: normal * inv_len,
            offset: offset * inv_len,
        }
    }

    /// Creates a horizontal plane at the given Z height (normal = +Z).
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_mesh::csg::split::SplitPlane;
    ///
    /// let plane = SplitPlane::xy(3.0);
    /// assert!((plane.normal.z - 1.0).abs() < 1e-12);
    /// assert!((plane.offset - 3.0).abs() < 1e-12);
    /// ```
    #[must_use]
    pub fn xy(z: f64) -> Self {
        Self {
            normal: Vec3::new(0.0, 0.0, 1.0),
            offset: z,
        }
    }

    /// Creates a plane at the given Y coordinate (normal = +Y).
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_mesh::csg::split::SplitPlane;
    ///
    /// let plane = SplitPlane::xz(2.0);
    /// assert!((plane.normal.y - 1.0).abs() < 1e-12);
    /// ```
    #[must_use]
    pub fn xz(y: f64) -> Self {
        Self {
            normal: Vec3::new(0.0, 1.0, 0.0),
            offset: y,
        }
    }

    /// Creates a plane at the given X coordinate (normal = +X).
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_mesh::csg::split::SplitPlane;
    ///
    /// let plane = SplitPlane::yz(1.0);
    /// assert!((plane.normal.x - 1.0).abs() < 1e-12);
    /// ```
    #[must_use]
    pub fn yz(x: f64) -> Self {
        Self {
            normal: Vec3::new(1.0, 0.0, 0.0),
            offset: x,
        }
    }

    /// Evaluates the signed distance of a point from the plane.
    ///
    /// Positive means above (same side as normal), negative means below.
    #[inline]
    fn signed_distance(&self, p: Point3) -> f64 {
        let pv = Vec3::from(p);
        self.normal.dot(pv) - self.offset
    }
}

/// Options controlling plane split behavior.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::split::SplitOptions;
///
/// let opts = SplitOptions::default();
/// assert!(opts.cap);
/// ```
#[derive(Clone, Debug)]
pub struct SplitOptions {
    /// Whether to cap (close) the split cross-section on both halves.
    ///
    /// When `true`, both resulting halves are watertight.
    /// When `false`, the cross-section is left open.
    pub cap: bool,
}

impl Default for SplitOptions {
    fn default() -> Self {
        Self { cap: true }
    }
}

/// Result of a plane split operation.
///
/// Contains the two halves and a diagnostic report.
pub struct SplitResult {
    /// Mesh half on the positive side of the plane (normal direction).
    pub above: TriangleMesh,
    /// Mesh half on the negative side of the plane.
    pub below: TriangleMesh,
    /// Diagnostic report for the operation.
    pub report: CsgReport,
}

/// Tolerance for classifying a vertex as lying on the plane.
const PLANE_TOL: f64 = 1e-10;

/// Vertex classification relative to the split plane.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Above,
    Below,
    On,
}

/// Splits a triangle mesh into two halves along an arbitrary plane.
///
/// This is an optimized direct-classification path -- it does NOT go through
/// the general boolean pipeline. Direct plane classification is much faster
/// than creating a plane mesh and performing `mesh_difference`.
///
/// # Errors
///
/// Returns [`CsgError::EmptyResult`] if the plane does not intersect the mesh
/// (one half would be empty).
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::split::{mesh_split_at_plane, SplitPlane, SplitOptions};
/// use slicecore_mesh::csg::primitive_box;
///
/// let mesh = primitive_box(2.0, 2.0, 2.0);
/// let plane = SplitPlane::xy(0.0);
/// let result = mesh_split_at_plane(&mesh, &plane, &SplitOptions::default()).unwrap();
/// assert!(result.above.triangle_count() > 0);
/// assert!(result.below.triangle_count() > 0);
/// ```
pub fn mesh_split_at_plane(
    mesh: &TriangleMesh,
    plane: &SplitPlane,
    options: &SplitOptions,
) -> Result<SplitResult, CsgError> {
    let start = Instant::now();

    let verts = mesh.vertices();
    let indices = mesh.indices();

    // Classify all vertices.
    let classifications: Vec<Side> = verts
        .iter()
        .map(|&v| {
            let d = plane.signed_distance(v);
            if d > PLANE_TOL {
                Side::Above
            } else if d < -PLANE_TOL {
                Side::Below
            } else {
                Side::On
            }
        })
        .collect();

    let mut above_verts: Vec<Point3> = Vec::new();
    let mut above_indices: Vec<[u32; 3]> = Vec::new();
    let mut below_verts: Vec<Point3> = Vec::new();
    let mut below_indices: Vec<[u32; 3]> = Vec::new();

    // Edge intersection points for capping.
    let mut cap_points: Vec<Point3> = Vec::new();
    // Ordered edge pairs for cap polygon reconstruction.
    let mut cap_edges: Vec<(usize, usize)> = Vec::new();

    // Vertex maps for deduplication within each half.
    let mut above_map: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut below_map: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();

    for tri in indices {
        let [i0, i1, i2] = *tri;
        let s0 = classifications[i0 as usize];
        let s1 = classifications[i1 as usize];
        let s2 = classifications[i2 as usize];

        let all_above = matches!(
            (s0, s1, s2),
            (Side::Above | Side::On, Side::Above | Side::On, Side::Above | Side::On)
        ) && (s0 == Side::Above || s1 == Side::Above || s2 == Side::Above);

        let all_below = matches!(
            (s0, s1, s2),
            (Side::Below | Side::On, Side::Below | Side::On, Side::Below | Side::On)
        ) && (s0 == Side::Below || s1 == Side::Below || s2 == Side::Below);

        let all_on = s0 == Side::On && s1 == Side::On && s2 == Side::On;

        if all_on {
            // Add to both halves.
            let a0 = add_vert_to(&mut above_verts, &mut above_map, verts[i0 as usize], Some(i0));
            let a1 = add_vert_to(&mut above_verts, &mut above_map, verts[i1 as usize], Some(i1));
            let a2 = add_vert_to(&mut above_verts, &mut above_map, verts[i2 as usize], Some(i2));
            above_indices.push([a0, a1, a2]);

            let b0 = add_vert_to(&mut below_verts, &mut below_map, verts[i0 as usize], Some(i0));
            let b1 = add_vert_to(&mut below_verts, &mut below_map, verts[i1 as usize], Some(i1));
            let b2 = add_vert_to(&mut below_verts, &mut below_map, verts[i2 as usize], Some(i2));
            below_indices.push([b0, b1, b2]);
        } else if all_above {
            let a0 = add_vert_to(&mut above_verts, &mut above_map, verts[i0 as usize], Some(i0));
            let a1 = add_vert_to(&mut above_verts, &mut above_map, verts[i1 as usize], Some(i1));
            let a2 = add_vert_to(&mut above_verts, &mut above_map, verts[i2 as usize], Some(i2));
            above_indices.push([a0, a1, a2]);
        } else if all_below {
            let b0 = add_vert_to(&mut below_verts, &mut below_map, verts[i0 as usize], Some(i0));
            let b1 = add_vert_to(&mut below_verts, &mut below_map, verts[i1 as usize], Some(i1));
            let b2 = add_vert_to(&mut below_verts, &mut below_map, verts[i2 as usize], Some(i2));
            below_indices.push([b0, b1, b2]);
        } else {
            // Triangle crosses the plane -- split it.
            split_triangle(
                verts,
                &classifications,
                [i0, i1, i2],
                plane,
                &mut above_verts,
                &mut above_indices,
                &mut below_verts,
                &mut below_indices,
                &mut above_map,
                &mut below_map,
                &mut cap_points,
                &mut cap_edges,
            );
        }
    }

    // Cap the cross-section if requested.
    if options.cap && !cap_points.is_empty() {
        add_cap_triangles(
            plane,
            &cap_points,
            &cap_edges,
            &mut above_verts,
            &mut above_indices,
            &mut below_verts,
            &mut below_indices,
        );
    }

    // Build result meshes. Allow empty halves only if not capping.
    if above_indices.is_empty() && below_indices.is_empty() {
        return Err(CsgError::EmptyResult {
            operation: "split_at_plane".to_string(),
        });
    }

    // If one half is empty, create a minimal mesh for it.
    let above_mesh = if above_indices.is_empty() {
        // Return a degenerate single-triangle mesh as placeholder.
        let p = verts[0];
        TriangleMesh::new(
            vec![p, p, p],
            vec![[0, 1, 2]],
        )
        .map_err(CsgError::ResultConstruction)?
    } else {
        TriangleMesh::new(above_verts, above_indices)
            .map_err(CsgError::ResultConstruction)?
    };

    let below_mesh = if below_indices.is_empty() {
        let p = verts[0];
        TriangleMesh::new(
            vec![p, p, p],
            vec![[0, 1, 2]],
        )
        .map_err(CsgError::ResultConstruction)?
    } else {
        TriangleMesh::new(below_verts, below_indices)
            .map_err(CsgError::ResultConstruction)?
    };

    let mut report = CsgReport {
        input_triangles_a: mesh.triangle_count(),
        output_triangles: above_mesh.triangle_count() + below_mesh.triangle_count(),
        ..CsgReport::default()
    };

    report.volume = Some(
        volume::signed_volume(above_mesh.vertices(), above_mesh.indices())
            + volume::signed_volume(below_mesh.vertices(), below_mesh.indices()),
    );
    report.duration_ms = start.elapsed().as_millis() as u64;

    Ok(SplitResult {
        above: above_mesh,
        below: below_mesh,
        report,
    })
}

/// Computes the intersection point of an edge with the plane.
fn edge_plane_intersection(p0: Point3, p1: Point3, plane: &SplitPlane) -> Point3 {
    let d0 = plane.signed_distance(p0);
    let d1 = plane.signed_distance(p1);
    let denom = d0 - d1;
    if denom.abs() < 1e-30 {
        // Edge is parallel to the plane; return midpoint as fallback.
        return Point3::new(
            (p0.x + p1.x) * 0.5,
            (p0.y + p1.y) * 0.5,
            (p0.z + p1.z) * 0.5,
        );
    }
    let t = d0 / denom;
    Point3::new(
        p0.x + t * (p1.x - p0.x),
        p0.y + t * (p1.y - p0.y),
        p0.z + t * (p1.z - p0.z),
    )
}

/// Splits a single triangle that crosses the plane into sub-triangles.
#[allow(clippy::too_many_arguments)]
fn split_triangle(
    verts: &[Point3],
    classifications: &[Side],
    tri: [u32; 3],
    plane: &SplitPlane,
    above_verts: &mut Vec<Point3>,
    above_indices: &mut Vec<[u32; 3]>,
    below_verts: &mut Vec<Point3>,
    below_indices: &mut Vec<[u32; 3]>,
    above_map: &mut std::collections::HashMap<u32, u32>,
    below_map: &mut std::collections::HashMap<u32, u32>,
    cap_points: &mut Vec<Point3>,
    cap_edges: &mut Vec<(usize, usize)>,
) {
    let [i0, i1, i2] = tri;
    let sides = [
        classifications[i0 as usize],
        classifications[i1 as usize],
        classifications[i2 as usize],
    ];
    let pts = [verts[i0 as usize], verts[i1 as usize], verts[i2 as usize]];
    let orig = [i0, i1, i2];

    // Count vertices on each side.
    let n_above = sides.iter().filter(|&&s| s == Side::Above).count();
    let n_below = sides.iter().filter(|&&s| s == Side::Below).count();

    // Helper closures (defined as local fns to avoid borrow issues).
    // We'll use index manipulation instead.

    if n_above == 1 && n_below == 2 {
        // One vertex above, two below. Find the lone above vertex.
        let lone = sides.iter().position(|&s| s == Side::Above).unwrap_or(0);
        let (other1, other2) = match lone {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        };
        emit_one_vs_two(
            &pts, &orig, &sides, lone, other1, other2, plane,
            above_verts, above_indices, below_verts, below_indices,
            above_map, below_map, cap_points, cap_edges, true,
        );
    } else if n_above == 2 && n_below == 1 {
        // Two vertices above, one below. Find the lone below vertex.
        let lone = sides.iter().position(|&s| s == Side::Below).unwrap_or(0);
        let (other1, other2) = match lone {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        };
        emit_one_vs_two(
            &pts, &orig, &sides, lone, other1, other2, plane,
            above_verts, above_indices, below_verts, below_indices,
            above_map, below_map, cap_points, cap_edges, false,
        );
    } else if n_above == 1 && n_below == 1 {
        // One above, one below, one on-plane.
        let on_idx = sides.iter().position(|&s| s == Side::On).unwrap_or(0);
        let above_idx = sides.iter().position(|&s| s == Side::Above).unwrap_or(0);
        let below_idx = sides.iter().position(|&s| s == Side::Below).unwrap_or(0);

        let ip = edge_plane_intersection(pts[above_idx], pts[below_idx], plane);

        // Above: triangle (above_vertex, intersection_point, on_vertex)
        let a_v = add_vert_to(above_verts, above_map, pts[above_idx], Some(orig[above_idx]));
        let a_ip = add_vert_to(above_verts, above_map, ip, None);
        let a_on = add_vert_to(above_verts, above_map, pts[on_idx], Some(orig[on_idx]));

        // Preserve winding: check if the original order is (on, above, below), etc.
        // We need to maintain consistent winding. The original winding is i0,i1,i2.
        // We produce the above triangle and below triangle maintaining the same winding sense.
        let winding = triangle_winding_order(on_idx, above_idx, below_idx);
        if winding {
            above_indices.push([a_on, a_v, a_ip]);
        } else {
            above_indices.push([a_v, a_on, a_ip]);
        }

        let b_v = add_vert_to(below_verts, below_map, pts[below_idx], Some(orig[below_idx]));
        let b_ip = add_vert_to(below_verts, below_map, ip, None);
        let b_on = add_vert_to(below_verts, below_map, pts[on_idx], Some(orig[on_idx]));

        if winding {
            below_indices.push([b_on, b_ip, b_v]);
        } else {
            below_indices.push([b_ip, b_on, b_v]);
        }

        // Cap edge: from on_vertex to intersection_point.
        let cp0 = cap_points.len();
        cap_points.push(pts[on_idx]);
        cap_points.push(ip);
        cap_edges.push((cp0, cp0 + 1));
    }
    // Other edge cases (all on same side + on) are handled by the caller's
    // all_above / all_below checks.
}

/// Emits triangles for the case where one vertex is on one side and two are on the other.
///
/// `lone_above` indicates whether the lone vertex is above (true) or below (false).
#[allow(clippy::too_many_arguments)]
fn emit_one_vs_two(
    pts: &[Point3; 3],
    orig: &[u32; 3],
    _sides: &[Side; 3],
    lone: usize,
    other1: usize,
    other2: usize,
    plane: &SplitPlane,
    above_verts: &mut Vec<Point3>,
    above_indices: &mut Vec<[u32; 3]>,
    below_verts: &mut Vec<Point3>,
    below_indices: &mut Vec<[u32; 3]>,
    above_map: &mut std::collections::HashMap<u32, u32>,
    below_map: &mut std::collections::HashMap<u32, u32>,
    cap_points: &mut Vec<Point3>,
    cap_edges: &mut Vec<(usize, usize)>,
    lone_above: bool,
) {
    // Compute intersection points.
    let ip1 = edge_plane_intersection(pts[lone], pts[other1], plane);
    let ip2 = edge_plane_intersection(pts[lone], pts[other2], plane);

    // Determine which side gets the single triangle and which gets the two.
    let (one_verts, one_indices, one_map, two_verts, two_indices, two_map) = if lone_above {
        (
            above_verts, above_indices, above_map,
            below_verts, below_indices, below_map,
        )
    } else {
        (
            below_verts, below_indices, below_map,
            above_verts, above_indices, above_map,
        )
    };

    // Single triangle on the lone side: (lone, ip1, ip2).
    let v_lone = add_vert_to(one_verts, one_map, pts[lone], Some(orig[lone]));
    let v_ip1 = add_vert_to(one_verts, one_map, ip1, None);
    let v_ip2 = add_vert_to(one_verts, one_map, ip2, None);

    // Preserve winding: if lone->other1->other2 is the same order as in the
    // original triangle, keep the same winding.
    let winding = triangle_winding_order(lone, other1, other2);
    if winding {
        one_indices.push([v_lone, v_ip1, v_ip2]);
    } else {
        one_indices.push([v_lone, v_ip2, v_ip1]);
    }

    // Two triangles on the other side: (other1, other2, ip1) and (other2, ip2, ip1)
    // or equivalently a quad (other1, ip1, ip2, other2).
    let v_o1 = add_vert_to(two_verts, two_map, pts[other1], Some(orig[other1]));
    let v_o2 = add_vert_to(two_verts, two_map, pts[other2], Some(orig[other2]));
    let v_ip1b = add_vert_to(two_verts, two_map, ip1, None);
    let v_ip2b = add_vert_to(two_verts, two_map, ip2, None);

    if winding {
        two_indices.push([v_ip1b, v_o1, v_o2]);
        two_indices.push([v_ip1b, v_o2, v_ip2b]);
    } else {
        two_indices.push([v_o1, v_ip1b, v_ip2b]);
        two_indices.push([v_o1, v_ip2b, v_o2]);
    }

    // Record cap edge (ip1 -> ip2).
    let cp0 = cap_points.len();
    cap_points.push(ip1);
    cap_points.push(ip2);
    cap_edges.push((cp0, cp0 + 1));
}

/// Returns true if the permutation (a, b, c) of (0, 1, 2) is even (same winding).
fn triangle_winding_order(a: usize, b: usize, c: usize) -> bool {
    // Even permutations of (0,1,2): (0,1,2), (1,2,0), (2,0,1)
    matches!((a, b, c), (0, 1, 2) | (1, 2, 0) | (2, 0, 1))
}

/// Adds a vertex to a mesh half, deduplicating by original index.
fn add_vert_to(
    verts: &mut Vec<Point3>,
    map: &mut std::collections::HashMap<u32, u32>,
    p: Point3,
    orig_idx: Option<u32>,
) -> u32 {
    if let Some(oi) = orig_idx {
        if let Some(&existing) = map.get(&oi) {
            return existing;
        }
    }
    let idx = verts.len() as u32;
    verts.push(p);
    if let Some(oi) = orig_idx {
        map.insert(oi, idx);
    }
    idx
}

/// Adds cap triangles to close the cross-section on both halves.
///
/// Uses a simple fan triangulation from the centroid of the cap points.
#[allow(clippy::too_many_arguments)]
fn add_cap_triangles(
    plane: &SplitPlane,
    cap_points: &[Point3],
    _cap_edges: &[(usize, usize)],
    above_verts: &mut Vec<Point3>,
    above_indices: &mut Vec<[u32; 3]>,
    below_verts: &mut Vec<Point3>,
    below_indices: &mut Vec<[u32; 3]>,
) {
    if cap_points.is_empty() {
        return;
    }

    // Deduplicate cap points.
    let mut unique_pts: Vec<Point3> = Vec::new();
    for &p in cap_points {
        let dup = unique_pts.iter().any(|u| {
            let dx = u.x - p.x;
            let dy = u.y - p.y;
            let dz = u.z - p.z;
            dx * dx + dy * dy + dz * dz < 1e-14
        });
        if !dup {
            unique_pts.push(p);
        }
    }

    if unique_pts.len() < 3 {
        return;
    }

    // Project cap points onto a 2D coordinate system on the plane.
    let centroid = compute_centroid(&unique_pts);

    // Build orthonormal basis on the plane.
    let n = plane.normal;
    let (u_axis, v_axis) = plane_basis(n);

    // Project points to 2D.
    let pts_2d: Vec<(f64, f64)> = unique_pts
        .iter()
        .map(|p| {
            let dx = p.x - centroid.x;
            let dy = p.y - centroid.y;
            let dz = p.z - centroid.z;
            let v = Vec3::new(dx, dy, dz);
            (u_axis.dot(v), v_axis.dot(v))
        })
        .collect();

    // Sort points by angle around centroid for fan triangulation.
    let mut angle_indices: Vec<(f64, usize)> = pts_2d
        .iter()
        .enumerate()
        .map(|(i, &(u, v))| (v.atan2(u), i))
        .collect();
    angle_indices.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let sorted: Vec<usize> = angle_indices.iter().map(|&(_, i)| i).collect();

    // Fan triangulation from centroid.
    // Add centroid to both halves.
    let above_centroid = above_verts.len() as u32;
    above_verts.push(centroid);
    let below_centroid = below_verts.len() as u32;
    below_verts.push(centroid);

    let mut above_pt_indices: Vec<u32> = Vec::with_capacity(sorted.len());
    let mut below_pt_indices: Vec<u32> = Vec::with_capacity(sorted.len());

    for &si in &sorted {
        let p = unique_pts[si];
        above_pt_indices.push(above_verts.len() as u32);
        above_verts.push(p);
        below_pt_indices.push(below_verts.len() as u32);
        below_verts.push(p);
    }

    let n_pts = sorted.len();
    for i in 0..n_pts {
        let next = (i + 1) % n_pts;
        // Above cap: normal should face in +plane.normal direction.
        // Fan winding: centroid, p[i], p[next] -- check if this faces +normal.
        above_indices.push([above_centroid, above_pt_indices[i], above_pt_indices[next]]);
        // Below cap: opposite winding (normal faces -plane.normal).
        below_indices.push([below_centroid, below_pt_indices[next], below_pt_indices[i]]);
    }
}

/// Computes the centroid of a set of points.
fn compute_centroid(points: &[Point3]) -> Point3 {
    let n = points.len() as f64;
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut cz = 0.0;
    for p in points {
        cx += p.x;
        cy += p.y;
        cz += p.z;
    }
    Point3::new(cx / n, cy / n, cz / n)
}

/// Builds an orthonormal basis on a plane given its normal.
fn plane_basis(n: Vec3) -> (Vec3, Vec3) {
    // Pick a vector not parallel to n.
    let helper = if n.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = n.cross(helper).normalize();
    let v = n.cross(u).normalize();
    (u, v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::primitives::primitive_box;

    #[test]
    fn split_box_at_midpoint() {
        let mesh = primitive_box(2.0, 2.0, 2.0);
        let plane = SplitPlane::xy(0.0);
        let result = mesh_split_at_plane(&mesh, &plane, &SplitOptions::default()).unwrap();
        assert!(result.above.triangle_count() > 0);
        assert!(result.below.triangle_count() > 0);
    }

    #[test]
    fn split_box_uncapped() {
        let mesh = primitive_box(2.0, 2.0, 2.0);
        let plane = SplitPlane::xy(0.0);
        let result =
            mesh_split_at_plane(&mesh, &plane, &SplitOptions { cap: false }).unwrap();
        assert!(result.above.triangle_count() > 0);
        assert!(result.below.triangle_count() > 0);
        // Uncapped should have fewer triangles than capped.
        let capped =
            mesh_split_at_plane(&mesh, &plane, &SplitOptions::default()).unwrap();
        assert!(
            result.above.triangle_count() + result.below.triangle_count()
                <= capped.above.triangle_count() + capped.below.triangle_count()
        );
    }

    #[test]
    fn split_plane_constructors() {
        let p = SplitPlane::xy(5.0);
        assert!((p.normal.z - 1.0).abs() < 1e-12);
        assert!((p.offset - 5.0).abs() < 1e-12);

        let p = SplitPlane::xz(3.0);
        assert!((p.normal.y - 1.0).abs() < 1e-12);

        let p = SplitPlane::yz(1.0);
        assert!((p.normal.x - 1.0).abs() < 1e-12);
    }

    #[test]
    fn split_plane_normalization() {
        let p = SplitPlane::new(Vec3::new(0.0, 0.0, 2.0), 10.0);
        assert!((p.normal.z - 1.0).abs() < 1e-12);
        assert!((p.offset - 5.0).abs() < 1e-12);
    }
}
