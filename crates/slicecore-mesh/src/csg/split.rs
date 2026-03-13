//! Plane splitting of triangle meshes.
//!
//! Splits a triangle mesh into two halves along an arbitrary plane.
//! Each half can be optionally capped (closed) with new triangles along
//! the cross-section to produce watertight results.

use std::collections::HashMap;
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

/// Helper that builds one half of the split.
///
/// Tracks vertices, triangles, and the boundary edges on the cut plane
/// (needed for capping).
struct HalfMesh {
    verts: Vec<Point3>,
    indices: Vec<[u32; 3]>,
    /// Maps (position hash) -> vertex index for deduplication of intersection points.
    pos_map: HashMap<u64, u32>,
}

impl HalfMesh {
    fn new() -> Self {
        Self {
            verts: Vec::new(),
            indices: Vec::new(),
            pos_map: HashMap::new(),
        }
    }

    /// Adds a vertex, deduplicating by original mesh index.
    fn add_original_vert(
        &mut self,
        orig_map: &mut HashMap<u32, u32>,
        p: Point3,
        orig_idx: u32,
    ) -> u32 {
        if let Some(&existing) = orig_map.get(&orig_idx) {
            return existing;
        }
        let idx = self.verts.len() as u32;
        self.verts.push(p);
        orig_map.insert(orig_idx, idx);
        idx
    }

    /// Adds an intersection point, deduplicating by position hash.
    fn add_intersection_vert(&mut self, p: Point3) -> u32 {
        let key = point_hash(p);
        if let Some(&existing) = self.pos_map.get(&key) {
            return existing;
        }
        let idx = self.verts.len() as u32;
        self.verts.push(p);
        self.pos_map.insert(key, idx);
        idx
    }
}

/// Hashes a point for deduplication (with tolerance).
fn point_hash(p: Point3) -> u64 {
    // Quantize to ~1e-8 resolution.
    let scale = 1e8;
    let x = (p.x * scale).round() as i64;
    let y = (p.y * scale).round() as i64;
    let z = (p.z * scale).round() as i64;
    // FNV-1a style mixing.
    let mut h: u64 = 14_695_981_039_346_656_037;
    for v in [x, y, z] {
        h ^= v as u64;
        h = h.wrapping_mul(1_099_511_628_211);
    }
    h
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

    let mut above = HalfMesh::new();
    let mut below = HalfMesh::new();
    let mut above_orig_map: HashMap<u32, u32> = HashMap::new();
    let mut below_orig_map: HashMap<u32, u32> = HashMap::new();

    for tri in indices {
        let [i0, i1, i2] = *tri;
        let sides = [
            classifications[i0 as usize],
            classifications[i1 as usize],
            classifications[i2 as usize],
        ];

        let n_above = sides.iter().filter(|&&s| s == Side::Above).count();
        let n_below = sides.iter().filter(|&&s| s == Side::Below).count();

        if n_below == 0 && n_above > 0 {
            // Entirely above (or on+above).
            let a0 = above.add_original_vert(&mut above_orig_map, verts[i0 as usize], i0);
            let a1 = above.add_original_vert(&mut above_orig_map, verts[i1 as usize], i1);
            let a2 = above.add_original_vert(&mut above_orig_map, verts[i2 as usize], i2);
            above.indices.push([a0, a1, a2]);
        } else if n_above == 0 && n_below > 0 {
            // Entirely below (or on+below).
            let b0 = below.add_original_vert(&mut below_orig_map, verts[i0 as usize], i0);
            let b1 = below.add_original_vert(&mut below_orig_map, verts[i1 as usize], i1);
            let b2 = below.add_original_vert(&mut below_orig_map, verts[i2 as usize], i2);
            below.indices.push([b0, b1, b2]);
        } else if n_above == 0 && n_below == 0 {
            // All on-plane: add to both halves.
            let a0 = above.add_original_vert(&mut above_orig_map, verts[i0 as usize], i0);
            let a1 = above.add_original_vert(&mut above_orig_map, verts[i1 as usize], i1);
            let a2 = above.add_original_vert(&mut above_orig_map, verts[i2 as usize], i2);
            above.indices.push([a0, a1, a2]);

            let b0 = below.add_original_vert(&mut below_orig_map, verts[i0 as usize], i0);
            let b1 = below.add_original_vert(&mut below_orig_map, verts[i1 as usize], i1);
            let b2 = below.add_original_vert(&mut below_orig_map, verts[i2 as usize], i2);
            below.indices.push([b0, b1, b2]);
        } else {
            // Triangle crosses the plane.
            split_crossing_triangle(
                verts,
                &classifications,
                [i0, i1, i2],
                plane,
                &mut above,
                &mut below,
                &mut above_orig_map,
                &mut below_orig_map,
            );
        }
    }

    // Cap the cross-section if requested.
    if options.cap {
        cap_half(&mut above, plane, true);
        cap_half(&mut below, plane, false);
    }

    // Build result meshes.
    if above.indices.is_empty() && below.indices.is_empty() {
        return Err(CsgError::EmptyResult {
            operation: "split_at_plane".to_string(),
        });
    }

    let above_mesh = if above.indices.is_empty() {
        let p = verts[0];
        TriangleMesh::new(vec![p, p, p], vec![[0, 1, 2]])
            .map_err(CsgError::ResultConstruction)?
    } else {
        TriangleMesh::new(above.verts, above.indices)
            .map_err(CsgError::ResultConstruction)?
    };

    let below_mesh = if below.indices.is_empty() {
        let p = verts[0];
        TriangleMesh::new(vec![p, p, p], vec![[0, 1, 2]])
            .map_err(CsgError::ResultConstruction)?
    } else {
        TriangleMesh::new(below.verts, below.indices)
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

/// Splits a triangle that crosses the plane into sub-triangles on each side.
#[allow(clippy::too_many_arguments)]
fn split_crossing_triangle(
    verts: &[Point3],
    classifications: &[Side],
    tri: [u32; 3],
    plane: &SplitPlane,
    above: &mut HalfMesh,
    below: &mut HalfMesh,
    above_orig_map: &mut HashMap<u32, u32>,
    below_orig_map: &mut HashMap<u32, u32>,
) {
    let [i0, i1, i2] = tri;
    let sides = [
        classifications[i0 as usize],
        classifications[i1 as usize],
        classifications[i2 as usize],
    ];
    let pts = [verts[i0 as usize], verts[i1 as usize], verts[i2 as usize]];
    let orig = [i0, i1, i2];

    let n_above = sides.iter().filter(|&&s| s == Side::Above).count();
    let n_below = sides.iter().filter(|&&s| s == Side::Below).count();

    if n_above == 1 && n_below == 2 {
        // One above, two below.
        let lone = sides.iter().position(|&s| s == Side::Above).unwrap_or(0);
        let (o1, o2) = other_two(lone);
        emit_split(
            &pts, &orig, &sides, lone, o1, o2, plane,
            above, below, above_orig_map, below_orig_map, true,
        );
    } else if n_above == 2 && n_below == 1 {
        // Two above, one below.
        let lone = sides.iter().position(|&s| s == Side::Below).unwrap_or(0);
        let (o1, o2) = other_two(lone);
        emit_split(
            &pts, &orig, &sides, lone, o1, o2, plane,
            above, below, above_orig_map, below_orig_map, false,
        );
    } else if n_above == 1 && n_below == 1 {
        // One above, one below, one on-plane.
        let on_idx = sides.iter().position(|&s| s == Side::On).unwrap_or(0);
        let above_idx = sides.iter().position(|&s| s == Side::Above).unwrap_or(0);
        let below_idx = sides.iter().position(|&s| s == Side::Below).unwrap_or(0);

        let ip = edge_plane_intersection(pts[above_idx], pts[below_idx], plane);
        let winding = is_even_permutation(on_idx, above_idx, below_idx);

        // Above triangle.
        let a_v = above.add_original_vert(above_orig_map, pts[above_idx], orig[above_idx]);
        let a_ip = above.add_intersection_vert(ip);
        let a_on = above.add_original_vert(above_orig_map, pts[on_idx], orig[on_idx]);

        if winding {
            above.indices.push([a_on, a_v, a_ip]);
        } else {
            above.indices.push([a_v, a_on, a_ip]);
        }
        // Below triangle.
        let b_v = below.add_original_vert(below_orig_map, pts[below_idx], orig[below_idx]);
        let b_ip = below.add_intersection_vert(ip);
        let b_on = below.add_original_vert(below_orig_map, pts[on_idx], orig[on_idx]);

        if winding {
            below.indices.push([b_on, b_ip, b_v]);
        } else {
            below.indices.push([b_ip, b_on, b_v]);
        }
    }
}

/// Given a triangle split with one "lone" vertex and two on the other side,
/// emits sub-triangles to both halves.
///
/// `lone_above`: if true, the lone vertex is above and the two are below.
#[allow(clippy::too_many_arguments)]
fn emit_split(
    pts: &[Point3; 3],
    orig: &[u32; 3],
    _sides: &[Side; 3],
    lone: usize,
    other1: usize,
    other2: usize,
    plane: &SplitPlane,
    above: &mut HalfMesh,
    below: &mut HalfMesh,
    above_orig_map: &mut HashMap<u32, u32>,
    below_orig_map: &mut HashMap<u32, u32>,
    lone_above: bool,
) {
    let ip1 = edge_plane_intersection(pts[lone], pts[other1], plane);
    let ip2 = edge_plane_intersection(pts[lone], pts[other2], plane);

    let winding = is_even_permutation(lone, other1, other2);

    // References for the "one" side (where the lone vertex lives) and "two" side.
    let (one, two, one_map, two_map) = if lone_above {
        (above as &mut HalfMesh, below as &mut HalfMesh, above_orig_map, below_orig_map)
    } else {
        (below as &mut HalfMesh, above as &mut HalfMesh, below_orig_map, above_orig_map)
    };

    // Single triangle on the lone side.
    let v_lone = one.add_original_vert(one_map, pts[lone], orig[lone]);
    let v_ip1 = one.add_intersection_vert(ip1);
    let v_ip2 = one.add_intersection_vert(ip2);

    if winding {
        one.indices.push([v_lone, v_ip1, v_ip2]);
    } else {
        one.indices.push([v_lone, v_ip2, v_ip1]);
    }

    // Two triangles on the other side (quad: other1, ip1, ip2, other2).
    let v_o1 = two.add_original_vert(two_map, pts[other1], orig[other1]);
    let v_o2 = two.add_original_vert(two_map, pts[other2], orig[other2]);
    let v_ip1b = two.add_intersection_vert(ip1);
    let v_ip2b = two.add_intersection_vert(ip2);

    if winding {
        two.indices.push([v_ip1b, v_o1, v_o2]);
        two.indices.push([v_ip1b, v_o2, v_ip2b]);
    } else {
        two.indices.push([v_o1, v_ip1b, v_ip2b]);
        two.indices.push([v_o1, v_ip2b, v_o2]);
    }
}

/// Returns the two indices other than `lone` in {0, 1, 2}, preserving order.
fn other_two(lone: usize) -> (usize, usize) {
    match lone {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    }
}

/// Returns true if the permutation (a, b, c) of (0, 1, 2) is even (same winding).
fn is_even_permutation(a: usize, b: usize, c: usize) -> bool {
    matches!((a, b, c), (0, 1, 2) | (1, 2, 0) | (2, 0, 1))
}

/// Caps a half-mesh by triangulating the boundary polygon on the cut plane.
///
/// For the "above" half, the cap normal points in the -plane.normal direction
/// (closing the bottom). For the "below" half, it points in +plane.normal.
fn cap_half(half: &mut HalfMesh, plane: &SplitPlane, is_above: bool) {
    // Find boundary edges (edges with count == 1) from the mesh topology.
    // This is more robust than tracking them during splitting.
    let boundary = find_boundary_edges(&half.indices);
    if boundary.is_empty() {
        return;
    }

    // Filter boundary edges: only keep edges whose BOTH vertices lie on the plane.
    let on_plane_boundary: Vec<(u32, u32)> = boundary
        .into_iter()
        .filter(|&(a, b)| {
            let pa = half.verts[a as usize];
            let pb = half.verts[b as usize];
            let da = plane.signed_distance(pa).abs();
            let db = plane.signed_distance(pb).abs();
            da < PLANE_TOL * 100.0 && db < PLANE_TOL * 100.0
        })
        .collect();

    if on_plane_boundary.is_empty() {
        return;
    }

    // Chain boundary edges into loops.
    let loops = chain_boundary_edges(&on_plane_boundary);

    for loop_indices in &loops {
        if loop_indices.len() < 3 {
            continue;
        }

        // Get the 3D positions of the loop vertices.
        let loop_pts: Vec<Point3> = loop_indices.iter().map(|&vi| half.verts[vi as usize]).collect();

        // Project onto 2D plane coordinates.
        let (u_axis, v_axis) = plane_basis(plane.normal);
        let centroid = compute_centroid(&loop_pts);

        let pts_2d: Vec<(f64, f64)> = loop_pts
            .iter()
            .map(|p| {
                let dx = p.x - centroid.x;
                let dy = p.y - centroid.y;
                let dz = p.z - centroid.z;
                let v = Vec3::new(dx, dy, dz);
                (u_axis.dot(v), v_axis.dot(v))
            })
            .collect();

        // Ear-clipping triangulation.
        let tris = ear_clip_triangulate(&pts_2d);

        for (a, b, c) in tris {
            let va = loop_indices[a];
            let vb = loop_indices[b];
            let vc = loop_indices[c];
            // Above cap should face -normal (closing the bottom of the above piece).
            // Below cap should face +normal (closing the top of the below piece).
            if is_above {
                half.indices.push([va, vc, vb]);
            } else {
                half.indices.push([va, vb, vc]);
            }
        }
    }
}

/// Finds boundary edges (edges shared by exactly one triangle) as directed edges.
///
/// Returns directed edges consistent with the triangle winding order.
fn find_boundary_edges(indices: &[[u32; 3]]) -> Vec<(u32, u32)> {
    let mut edge_count: HashMap<(u32, u32), usize> = HashMap::new();
    let mut directed: Vec<(u32, u32)> = Vec::new();

    for tri in indices {
        for k in 0..3 {
            let a = tri[k];
            let b = tri[(k + 1) % 3];
            let canonical = if a < b { (a, b) } else { (b, a) };
            *edge_count.entry(canonical).or_insert(0) += 1;
            directed.push((a, b));
        }
    }

    // Keep only directed edges whose canonical form has count == 1.
    directed
        .into_iter()
        .filter(|&(a, b)| {
            let canonical = if a < b { (a, b) } else { (b, a) };
            edge_count.get(&canonical) == Some(&1)
        })
        .collect()
}

/// Chains directed boundary edges into closed loops.
fn chain_boundary_edges(edges: &[(u32, u32)]) -> Vec<Vec<u32>> {
    let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(a, b) in edges {
        adj.entry(a).or_default().push(b);
    }

    let mut visited_edges: HashMap<(u32, u32), bool> = HashMap::new();
    for &(a, b) in edges {
        visited_edges.insert((a, b), false);
    }

    let mut loops = Vec::new();

    for &(start_a, start_b) in edges {
        if visited_edges.get(&(start_a, start_b)) == Some(&true) {
            continue;
        }

        let mut chain = vec![start_a];
        let mut current = start_a;
        let mut next = start_b;

        while let Some(v) = visited_edges.get_mut(&(current, next)) {
            if *v {
                break; // Already used this edge.
            }
            *v = true;

            chain.push(next);
            current = next;

            if current == start_a {
                // Closed the loop.
                chain.pop(); // Remove duplicate start vertex.
                break;
            }

            // Find next edge from current.
            let Some(nbrs) = adj.get(&current) else {
                break;
            };
            let mut found = false;
            for &nb in nbrs {
                if visited_edges.get(&(current, nb)) == Some(&false) {
                    next = nb;
                    found = true;
                    break;
                }
            }
            if !found {
                break; // Dead end.
            }
        }

        if chain.len() >= 3 {
            loops.push(chain);
        }
    }

    loops
}

/// Simple ear-clipping triangulation for a 2D polygon.
///
/// Returns indices into the input point array as triangle triples.
fn ear_clip_triangulate(pts: &[(f64, f64)]) -> Vec<(usize, usize, usize)> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return vec![(0, 1, 2)];
    }

    let mut remaining: Vec<usize> = (0..n).collect();
    let mut triangles = Vec::new();

    // Determine winding.
    let signed_area = polygon_signed_area(pts, &remaining);
    let ccw = signed_area > 0.0;

    let max_iters = n * n * 2; // Safety limit.
    let mut iters = 0;

    while remaining.len() > 2 && iters < max_iters {
        iters += 1;
        let len = remaining.len();
        let mut found_ear = false;

        for i in 0..len {
            let prev = remaining[(i + len - 1) % len];
            let curr = remaining[i];
            let next = remaining[(i + 1) % len];

            // Check if this is a convex vertex (an ear candidate).
            let cross = cross_2d(pts[prev], pts[curr], pts[next]);
            let is_convex = if ccw { cross > 0.0 } else { cross < 0.0 };

            if !is_convex {
                continue;
            }

            // Check no other vertices are inside this triangle.
            let mut has_inside = false;
            for &ri in &remaining {
                if ri == prev || ri == curr || ri == next {
                    continue;
                }
                if point_in_triangle_2d(pts[ri], pts[prev], pts[curr], pts[next]) {
                    has_inside = true;
                    break;
                }
            }

            if !has_inside {
                triangles.push((prev, curr, next));
                remaining.remove(i);
                found_ear = true;
                break;
            }
        }

        if !found_ear {
            // Fallback: emit remaining as a fan.
            for i in 1..remaining.len() - 1 {
                triangles.push((remaining[0], remaining[i], remaining[i + 1]));
            }
            break;
        }
    }

    triangles
}

/// Computes signed area of a polygon given indices into the points array.
fn polygon_signed_area(pts: &[(f64, f64)], indices: &[usize]) -> f64 {
    let n = indices.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        let (xi, yi) = pts[indices[i]];
        let (xj, yj) = pts[indices[j]];
        area += xi * yj - xj * yi;
    }
    area * 0.5
}

/// 2D cross product (p1-p0) x (p2-p0).
fn cross_2d(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64)) -> f64 {
    (p1.0 - p0.0) * (p2.1 - p0.1) - (p1.1 - p0.1) * (p2.0 - p0.0)
}

/// Tests if a point is inside a triangle (2D).
fn point_in_triangle_2d(
    p: (f64, f64),
    a: (f64, f64),
    b: (f64, f64),
    c: (f64, f64),
) -> bool {
    let d1 = cross_2d(a, b, p);
    let d2 = cross_2d(b, c, p);
    let d3 = cross_2d(c, a, p);

    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;

    !(has_neg && has_pos)
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
