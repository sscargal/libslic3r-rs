//! SAH-based Bounding Volume Hierarchy for accelerated spatial queries.
//!
//! Implements a BVH tree with Surface Area Heuristic (SAH) for optimal split
//! decisions, following PBRT Chapter 4 methodology. The BVH accelerates:
//!
//! - **Plane intersection queries** (`query_plane`): Returns all triangles
//!   whose AABBs span a given Z height. This is the critical query for slicing.
//! - **Ray intersection queries** (`intersect_ray`): Returns the closest triangle
//!   hit by a ray. Used for support generation.
//!
//! The tree is stored in a flat `Vec<BVHNode>` for cache-friendly traversal.

use slicecore_math::{BBox3, Point3, Vec3};

/// Number of SAH evaluation buckets per axis.
const NUM_BUCKETS: usize = 12;

/// Maximum triangles in a leaf node before splitting.
const MAX_LEAF_TRIS: usize = 4;

/// Traversal cost relative to intersection cost (SAH parameter).
const TRAVERSAL_COST: f64 = 1.0;

/// Intersection cost per triangle (SAH parameter).
const INTERSECTION_COST: f64 = 1.0;

/// A Bounding Volume Hierarchy for accelerated spatial queries on triangle meshes.
///
/// Built using the Surface Area Heuristic (SAH) for optimal partitioning.
/// All nodes are stored in a flat vector for cache-friendly traversal.
pub struct BVH {
    /// Flat array of BVH nodes.
    nodes: Vec<BVHNode>,
    /// Reordered triangle indices mapping leaf ranges back to original mesh indices.
    tri_indices: Vec<u32>,
}

/// A node in the BVH tree.
pub(crate) enum BVHNode {
    /// A leaf node containing a range of triangles.
    Leaf {
        aabb: BBox3,
        first_tri: u32,
        tri_count: u32,
    },
    /// An interior node with two children.
    Interior {
        aabb: BBox3,
        left: u32,
        right: u32,
        #[allow(dead_code)]
        split_axis: u8,
    },
}

/// Result of a ray-triangle intersection test.
#[derive(Clone, Debug)]
pub struct RayHit {
    /// Index of the hit triangle in the original mesh.
    pub triangle_idx: usize,
    /// Parameter t along the ray at the hit point (origin + t * direction).
    pub t: f64,
    /// Barycentric coordinate u.
    pub u: f64,
    /// Barycentric coordinate v.
    pub v: f64,
}

/// Temporary per-triangle info used during BVH construction.
struct TriInfo {
    /// Index into the original mesh triangle array.
    original_idx: u32,
    /// AABB of this triangle.
    aabb: BBox3,
    /// Centroid of this triangle's AABB.
    centroid: Point3,
}

/// SAH evaluation bucket.
struct Bucket {
    count: usize,
    bounds: Option<BBox3>,
}

impl Bucket {
    fn new() -> Self {
        Self {
            count: 0,
            bounds: None,
        }
    }

    fn add(&mut self, aabb: &BBox3) {
        self.count += 1;
        self.bounds = Some(match &self.bounds {
            Some(b) => b.union(aabb),
            None => *aabb,
        });
    }
}

impl BVH {
    /// Builds a BVH from mesh vertices and triangle indices using SAH.
    pub fn build(vertices: &[Point3], indices: &[[u32; 3]]) -> Self {
        if indices.is_empty() {
            return Self {
                nodes: Vec::new(),
                tri_indices: Vec::new(),
            };
        }

        // Compute per-triangle AABB and centroid.
        let mut tri_infos: Vec<TriInfo> = indices
            .iter()
            .enumerate()
            .filter_map(|(i, tri)| {
                let v0 = vertices[tri[0] as usize];
                let v1 = vertices[tri[1] as usize];
                let v2 = vertices[tri[2] as usize];

                let aabb = BBox3::from_points(&[v0, v1, v2])?;

                // Filter out degenerate triangles (zero-area AABB in all dimensions).
                let edge1 = Vec3::from_points(v0, v1);
                let edge2 = Vec3::from_points(v0, v2);
                let cross = edge1.cross(edge2);
                if cross.length_squared() < 1e-30 {
                    return None;
                }

                let centroid = aabb.center();
                Some(TriInfo {
                    original_idx: i as u32,
                    aabb,
                    centroid,
                })
            })
            .collect();

        if tri_infos.is_empty() {
            return Self {
                nodes: Vec::new(),
                tri_indices: Vec::new(),
            };
        }

        let mut nodes = Vec::with_capacity(2 * tri_infos.len());
        let mut ordered_indices = Vec::with_capacity(tri_infos.len());

        let len = tri_infos.len();
        Self::build_recursive(&mut tri_infos, 0, len, &mut nodes, &mut ordered_indices);

        Self {
            nodes,
            tri_indices: ordered_indices,
        }
    }

    /// Recursive BVH construction with SAH split selection.
    fn build_recursive(
        tri_infos: &mut [TriInfo],
        start: usize,
        end: usize,
        nodes: &mut Vec<BVHNode>,
        ordered_indices: &mut Vec<u32>,
    ) -> u32 {
        let count = end - start;
        let slice = &tri_infos[start..end];

        // Compute bounds for all triangles in this range.
        let mut bounds = slice[0].aabb;
        for info in &slice[1..] {
            bounds = bounds.union(&info.aabb);
        }

        // If few enough triangles, create a leaf.
        if count <= MAX_LEAF_TRIS {
            let first_tri = ordered_indices.len() as u32;
            for info in &tri_infos[start..end] {
                ordered_indices.push(info.original_idx);
            }
            let node_idx = nodes.len() as u32;
            nodes.push(BVHNode::Leaf {
                aabb: bounds,
                first_tri,
                tri_count: count as u32,
            });
            return node_idx;
        }

        // Compute centroid bounds to determine split axis candidates.
        let mut centroid_min = slice[0].centroid;
        let mut centroid_max = slice[0].centroid;
        for info in &slice[1..] {
            centroid_min = Point3::new(
                centroid_min.x.min(info.centroid.x),
                centroid_min.y.min(info.centroid.y),
                centroid_min.z.min(info.centroid.z),
            );
            centroid_max = Point3::new(
                centroid_max.x.max(info.centroid.x),
                centroid_max.y.max(info.centroid.y),
                centroid_max.z.max(info.centroid.z),
            );
        }

        // Evaluate SAH for each axis.
        let mut best_cost = f64::INFINITY;
        let mut best_axis = 0u8;
        let mut best_bucket = 0usize;

        for axis in 0..3u8 {
            let (c_min, c_max) = match axis {
                0 => (centroid_min.x, centroid_max.x),
                1 => (centroid_min.y, centroid_max.y),
                _ => (centroid_min.z, centroid_max.z),
            };

            let extent = c_max - c_min;
            if extent < 1e-30 {
                continue; // All centroids coincide on this axis
            }

            // Fill buckets.
            let mut buckets = Vec::with_capacity(NUM_BUCKETS);
            for _ in 0..NUM_BUCKETS {
                buckets.push(Bucket::new());
            }

            for info in &tri_infos[start..end] {
                let c = match axis {
                    0 => info.centroid.x,
                    1 => info.centroid.y,
                    _ => info.centroid.z,
                };
                let b = ((c - c_min) / extent * NUM_BUCKETS as f64) as usize;
                let b = b.min(NUM_BUCKETS - 1);
                buckets[b].add(&info.aabb);
            }

            // Evaluate SAH at each split position (between buckets).
            let parent_sa = surface_area(&bounds);

            for split in 1..NUM_BUCKETS {
                let mut left_count = 0usize;
                let mut left_bounds: Option<BBox3> = None;
                for bucket in &buckets[..split] {
                    left_count += bucket.count;
                    if let Some(ref bb) = bucket.bounds {
                        left_bounds = Some(match left_bounds {
                            Some(ref lb) => lb.union(bb),
                            None => *bb,
                        });
                    }
                }

                let mut right_count = 0usize;
                let mut right_bounds: Option<BBox3> = None;
                for bucket in &buckets[split..] {
                    right_count += bucket.count;
                    if let Some(ref bb) = bucket.bounds {
                        right_bounds = Some(match right_bounds {
                            Some(ref rb) => rb.union(bb),
                            None => *bb,
                        });
                    }
                }

                if left_count == 0 || right_count == 0 {
                    continue;
                }

                let left_sa = left_bounds.map_or(0.0, |b| surface_area(&b));
                let right_sa = right_bounds.map_or(0.0, |b| surface_area(&b));

                let cost = TRAVERSAL_COST
                    + INTERSECTION_COST
                        * (left_sa * left_count as f64 + right_sa * right_count as f64)
                        / parent_sa;

                if cost < best_cost {
                    best_cost = cost;
                    best_axis = axis;
                    best_bucket = split;
                }
            }
        }

        // If SAH found no good split, or leaf cost is better, create leaf.
        let leaf_cost = INTERSECTION_COST * count as f64;
        if best_cost >= leaf_cost || best_cost == f64::INFINITY {
            let first_tri = ordered_indices.len() as u32;
            for info in &tri_infos[start..end] {
                ordered_indices.push(info.original_idx);
            }
            let node_idx = nodes.len() as u32;
            nodes.push(BVHNode::Leaf {
                aabb: bounds,
                first_tri,
                tri_count: count as u32,
            });
            return node_idx;
        }

        // Partition triangles by the best split.
        let (c_min, c_max) = match best_axis {
            0 => (centroid_min.x, centroid_max.x),
            1 => (centroid_min.y, centroid_max.y),
            _ => (centroid_min.z, centroid_max.z),
        };
        let extent = c_max - c_min;

        // Partition in-place.
        let mid = partition_by_bucket(
            &mut tri_infos[start..end],
            best_axis,
            best_bucket,
            c_min,
            extent,
        );
        let mid = start + mid;

        // Guard against degenerate partitions.
        if mid == start || mid == end {
            let first_tri = ordered_indices.len() as u32;
            for info in &tri_infos[start..end] {
                ordered_indices.push(info.original_idx);
            }
            let node_idx = nodes.len() as u32;
            nodes.push(BVHNode::Leaf {
                aabb: bounds,
                first_tri,
                tri_count: count as u32,
            });
            return node_idx;
        }

        // Reserve node slot, then build children.
        let node_idx = nodes.len() as u32;
        nodes.push(BVHNode::Leaf {
            aabb: bounds,
            first_tri: 0,
            tri_count: 0,
        }); // placeholder

        let left = Self::build_recursive(tri_infos, start, mid, nodes, ordered_indices);
        let right = Self::build_recursive(tri_infos, mid, end, nodes, ordered_indices);

        nodes[node_idx as usize] = BVHNode::Interior {
            aabb: bounds,
            left,
            right,
            split_axis: best_axis,
        };

        node_idx
    }

    /// Queries all triangles whose AABBs span the given Z height.
    ///
    /// This is the critical query for slicing: it returns the indices of all
    /// triangles that potentially intersect the horizontal plane at height `z`.
    pub fn query_plane(&self, z: f64) -> Vec<usize> {
        if self.nodes.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        self.query_plane_recursive(0, z, &mut result);
        result
    }

    fn query_plane_recursive(&self, node_idx: u32, z: f64, result: &mut Vec<usize>) {
        match &self.nodes[node_idx as usize] {
            BVHNode::Leaf {
                aabb,
                first_tri,
                tri_count,
            } => {
                if aabb.min.z <= z && z <= aabb.max.z {
                    // For leaf nodes, we return all triangle indices.
                    // The AABB of the leaf already spans z, so all contained
                    // triangles are candidates (individual per-triangle z-span
                    // filtering is done at a higher level if needed).
                    for i in 0..*tri_count {
                        let tri_idx =
                            self.tri_indices[(*first_tri + i) as usize] as usize;
                        result.push(tri_idx);
                    }
                }
            }
            BVHNode::Interior {
                aabb, left, right, ..
            } => {
                if aabb.min.z <= z && z <= aabb.max.z {
                    self.query_plane_recursive(*left, z, result);
                    self.query_plane_recursive(*right, z, result);
                }
            }
        }
    }

    /// Casts a ray and returns the closest triangle hit, if any.
    ///
    /// Uses the slab method for ray-AABB intersection and the Moller-Trumbore
    /// algorithm for ray-triangle intersection.
    pub fn intersect_ray(
        &self,
        origin: &Point3,
        direction: &Vec3,
        vertices: &[Point3],
        indices: &[[u32; 3]],
    ) -> Option<RayHit> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv_dir = Vec3::new(1.0 / direction.x, 1.0 / direction.y, 1.0 / direction.z);
        let mut closest: Option<RayHit> = None;
        let mut t_max = f64::INFINITY;
        self.intersect_ray_recursive(
            0,
            origin,
            direction,
            &inv_dir,
            vertices,
            indices,
            &mut closest,
            &mut t_max,
        );
        closest
    }

    #[allow(clippy::too_many_arguments)]
    fn intersect_ray_recursive(
        &self,
        node_idx: u32,
        origin: &Point3,
        direction: &Vec3,
        inv_dir: &Vec3,
        vertices: &[Point3],
        indices: &[[u32; 3]],
        closest: &mut Option<RayHit>,
        t_max: &mut f64,
    ) {
        let node = &self.nodes[node_idx as usize];
        let aabb = match node {
            BVHNode::Leaf { aabb, .. } => aabb,
            BVHNode::Interior { aabb, .. } => aabb,
        };

        // Quick AABB rejection.
        if !ray_intersects_aabb(origin, inv_dir, aabb, *t_max) {
            return;
        }

        match node {
            BVHNode::Leaf {
                first_tri,
                tri_count,
                ..
            } => {
                for i in 0..*tri_count {
                    let tri_idx =
                        self.tri_indices[(*first_tri + i) as usize] as usize;
                    let tri = &indices[tri_idx];
                    let v0 = vertices[tri[0] as usize];
                    let v1 = vertices[tri[1] as usize];
                    let v2 = vertices[tri[2] as usize];

                    if let Some(hit) = moller_trumbore(origin, direction, &v0, &v1, &v2, tri_idx) {
                        if hit.t > 1e-9 && hit.t < *t_max {
                            *t_max = hit.t;
                            *closest = Some(hit);
                        }
                    }
                }
            }
            BVHNode::Interior { left, right, .. } => {
                self.intersect_ray_recursive(
                    *left, origin, direction, inv_dir, vertices, indices, closest, t_max,
                );
                self.intersect_ray_recursive(
                    *right, origin, direction, inv_dir, vertices, indices, closest, t_max,
                );
            }
        }
    }

    /// Returns the total number of triangles stored in the BVH.
    pub fn triangle_count(&self) -> usize {
        self.tri_indices.len()
    }
}

/// Computes the surface area of a 3D AABB (used by SAH).
fn surface_area(aabb: &BBox3) -> f64 {
    let dx = aabb.max.x - aabb.min.x;
    let dy = aabb.max.y - aabb.min.y;
    let dz = aabb.max.z - aabb.min.z;
    2.0 * (dx * dy + dy * dz + dz * dx)
}

/// Partitions tri_infos slice so that elements in bucket < split_bucket come first.
/// Returns the index of the first element in the right partition.
fn partition_by_bucket(
    tri_infos: &mut [TriInfo],
    axis: u8,
    split_bucket: usize,
    c_min: f64,
    extent: f64,
) -> usize {
    let mut left = 0usize;
    let mut right = tri_infos.len();

    while left < right {
        let c = match axis {
            0 => tri_infos[left].centroid.x,
            1 => tri_infos[left].centroid.y,
            _ => tri_infos[left].centroid.z,
        };
        let b = ((c - c_min) / extent * NUM_BUCKETS as f64) as usize;
        let b = b.min(NUM_BUCKETS - 1);
        if b < split_bucket {
            left += 1;
        } else {
            right -= 1;
            tri_infos.swap(left, right);
        }
    }

    left
}

/// Ray-AABB intersection test using the slab method.
///
/// Returns `true` if the ray intersects the AABB within `[0, t_max]`.
fn ray_intersects_aabb(origin: &Point3, inv_dir: &Vec3, aabb: &BBox3, t_max: f64) -> bool {
    let tx1 = (aabb.min.x - origin.x) * inv_dir.x;
    let tx2 = (aabb.max.x - origin.x) * inv_dir.x;
    let mut tmin = tx1.min(tx2);
    let mut tmax_local = tx1.max(tx2);

    let ty1 = (aabb.min.y - origin.y) * inv_dir.y;
    let ty2 = (aabb.max.y - origin.y) * inv_dir.y;
    tmin = tmin.max(ty1.min(ty2));
    tmax_local = tmax_local.min(ty1.max(ty2));

    let tz1 = (aabb.min.z - origin.z) * inv_dir.z;
    let tz2 = (aabb.max.z - origin.z) * inv_dir.z;
    tmin = tmin.max(tz1.min(tz2));
    tmax_local = tmax_local.min(tz1.max(tz2));

    tmax_local >= tmin.max(0.0) && tmin < t_max
}

/// Moller-Trumbore ray-triangle intersection algorithm.
///
/// Returns a `RayHit` if the ray intersects the triangle, with parameter `t`,
/// and barycentric coordinates `(u, v)`.
fn moller_trumbore(
    origin: &Point3,
    direction: &Vec3,
    v0: &Point3,
    v1: &Point3,
    v2: &Point3,
    triangle_idx: usize,
) -> Option<RayHit> {
    let edge1 = Vec3::from_points(*v0, *v1);
    let edge2 = Vec3::from_points(*v0, *v2);

    let h = direction.cross(edge2);
    let a = edge1.dot(h);

    if a.abs() < 1e-12 {
        return None; // Ray is parallel to triangle
    }

    let f = 1.0 / a;
    let s = Vec3::new(origin.x - v0.x, origin.y - v0.y, origin.z - v0.z);
    let u = f * s.dot(h);

    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(edge1);
    let v = f * direction.dot(q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * edge2.dot(q);

    Some(RayHit {
        triangle_idx,
        t,
        u,
        v,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triangle_mesh::tests::unit_cube;

    #[test]
    fn build_bvh_from_single_triangle() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        let indices = vec![[0, 1, 2]];
        let bvh = BVH::build(&vertices, &indices);
        assert_eq!(bvh.triangle_count(), 1);
    }

    #[test]
    fn build_bvh_from_cube_creates_nodes() {
        let mesh = unit_cube();
        let bvh = BVH::build(mesh.vertices(), mesh.indices());
        assert!(!bvh.nodes.is_empty());
        assert_eq!(bvh.triangle_count(), 12);
    }

    #[test]
    fn query_plane_mid_height_returns_triangles() {
        let mesh = unit_cube();
        let bvh = mesh.bvh();
        let result = bvh.query_plane(0.5);
        // At z=0.5, the cube spans 0-1, so all triangles are candidates
        // (their AABBs all span z=0.5 since the cube is a single volume).
        assert!(!result.is_empty());
    }

    #[test]
    fn query_plane_below_cube_returns_empty() {
        let mesh = unit_cube();
        let bvh = mesh.bvh();
        let result = bvh.query_plane(-1.0);
        assert!(result.is_empty(), "Expected empty, got {} triangles", result.len());
    }

    #[test]
    fn query_plane_above_cube_returns_empty() {
        let mesh = unit_cube();
        let bvh = mesh.bvh();
        let result = bvh.query_plane(2.0);
        assert!(result.is_empty(), "Expected empty, got {} triangles", result.len());
    }

    #[test]
    fn ray_hits_unit_cube_front_face() {
        let mesh = unit_cube();
        let bvh = mesh.bvh();
        let origin = Point3::new(0.5, 0.5, -1.0);
        let direction = Vec3::new(0.0, 0.0, 1.0);
        let hit = bvh.intersect_ray(&origin, &direction, mesh.vertices(), mesh.indices());
        assert!(hit.is_some(), "Expected ray to hit cube");
    }

    #[test]
    fn ray_misses_when_pointing_away() {
        let mesh = unit_cube();
        let bvh = mesh.bvh();
        let origin = Point3::new(0.5, 0.5, -1.0);
        let direction = Vec3::new(0.0, 0.0, -1.0);
        let hit = bvh.intersect_ray(&origin, &direction, mesh.vertices(), mesh.indices());
        assert!(hit.is_none(), "Expected ray to miss cube");
    }

    #[test]
    fn ray_returns_correct_t_distance() {
        let mesh = unit_cube();
        let bvh = mesh.bvh();
        let origin = Point3::new(0.5, 0.5, -1.0);
        let direction = Vec3::new(0.0, 0.0, 1.0);
        let hit = bvh
            .intersect_ray(&origin, &direction, mesh.vertices(), mesh.indices())
            .expect("ray should hit cube");
        // The back face of the cube is at z=0, so t should be ~1.0 (distance from z=-1 to z=0).
        assert!(
            (hit.t - 1.0).abs() < 1e-9,
            "Expected t ~1.0, got {}",
            hit.t
        );
    }
}
