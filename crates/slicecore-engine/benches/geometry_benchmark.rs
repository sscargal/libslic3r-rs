//! Geometry hot-path micro-benchmarks for regression detection.
//!
//! Benchmarks the core geometric operations that dominate slicing performance:
//! 1. Polygon boolean operations (union, intersection, difference)
//! 2. Polygon offsetting (inward and outward)
//! 3. Point-in-polygon tests
//! 4. Mesh plane intersection (slicing hot path)
//! 5. BVH ray intersection

use std::collections::HashMap;
use std::f64::consts::PI;

use criterion::{criterion_group, criterion_main, Criterion};
use slicecore_geo::{
    offset_polygon, point_in_polygon, polygon_difference, polygon_intersection, polygon_union,
    JoinType, Polygon,
};
use slicecore_math::{mm_to_coord, IPoint2, Point3, Vec3};
use slicecore_mesh::{ray_cast, TriangleMesh};
use slicecore_slicer::slice_mesh;

// ---------------------------------------------------------------------------
// Polygon helpers
// ---------------------------------------------------------------------------

/// Creates a rectangle ValidPolygon from mm coordinates.
fn create_rect_polygon(x0: f64, y0: f64, x1: f64, y1: f64) -> slicecore_geo::ValidPolygon {
    Polygon::from_mm(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
        .validate()
        .expect("rectangle polygon should be valid")
}

/// Creates a star-shaped polygon with `n` points, centered at (cx, cy) mm,
/// with outer radius `r_outer` and inner radius `r_inner`.
fn create_star_polygon(
    cx: f64,
    cy: f64,
    r_outer: f64,
    r_inner: f64,
    n: usize,
) -> slicecore_geo::ValidPolygon {
    let mut points = Vec::with_capacity(2 * n);
    for i in 0..(2 * n) {
        let angle = PI * (i as f64) / (n as f64) - PI / 2.0;
        let r = if i % 2 == 0 { r_outer } else { r_inner };
        points.push((cx + r * angle.cos(), cy + r * angle.sin()));
    }
    Polygon::from_mm(&points)
        .validate()
        .expect("star polygon should be valid")
}

/// Creates a regular polygon with `n` vertices centered at (cx, cy) mm with radius `r`.
fn create_regular_polygon(cx: f64, cy: f64, r: f64, n: usize) -> slicecore_geo::ValidPolygon {
    let points: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let angle = 2.0 * PI * (i as f64) / (n as f64);
            (cx + r * angle.cos(), cy + r * angle.sin())
        })
        .collect();
    Polygon::from_mm(&points)
        .validate()
        .expect("regular polygon should be valid")
}

// ---------------------------------------------------------------------------
// Mesh helpers (for slicing and BVH benchmarks)
// ---------------------------------------------------------------------------

/// Subdivided icosahedron sphere centered at (100, 100, 10), radius 10mm.
fn build_sphere(subdivisions: u32) -> TriangleMesh {
    let cx = 100.0;
    let cy = 100.0;
    let cz = 10.0;
    let radius = 10.0;

    let t = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let base_verts: Vec<[f64; 3]> = vec![
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ];

    let base_tris: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    let mut vertices = base_verts;
    let mut tris = base_tris;

    for _ in 0..subdivisions {
        let mut midpoint_cache: HashMap<(u32, u32), u32> = HashMap::new();
        let mut new_tris = Vec::with_capacity(tris.len() * 4);

        for tri in &tris {
            let mids: [u32; 3] = [
                get_midpoint(tri[0], tri[1], &mut vertices, &mut midpoint_cache),
                get_midpoint(tri[1], tri[2], &mut vertices, &mut midpoint_cache),
                get_midpoint(tri[2], tri[0], &mut vertices, &mut midpoint_cache),
            ];
            new_tris.push([tri[0], mids[0], mids[2]]);
            new_tris.push([mids[0], tri[1], mids[1]]);
            new_tris.push([mids[2], mids[1], tri[2]]);
            new_tris.push([mids[0], mids[1], mids[2]]);
        }
        tris = new_tris;
    }

    let points: Vec<Point3> = vertices
        .iter()
        .map(|v| {
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            Point3::new(
                cx + radius * v[0] / len,
                cy + radius * v[1] / len,
                cz + radius * v[2] / len,
            )
        })
        .collect();

    TriangleMesh::new(points, tris).expect("sphere mesh")
}

fn get_midpoint(
    a: u32,
    b: u32,
    vertices: &mut Vec<[f64; 3]>,
    cache: &mut HashMap<(u32, u32), u32>,
) -> u32 {
    let key = if a < b { (a, b) } else { (b, a) };
    if let Some(&idx) = cache.get(&key) {
        return idx;
    }
    let va = vertices[a as usize];
    let vb = vertices[b as usize];
    let mid = [
        (va[0] + vb[0]) / 2.0,
        (va[1] + vb[1]) / 2.0,
        (va[2] + vb[2]) / 2.0,
    ];
    let idx = vertices.len() as u32;
    vertices.push(mid);
    cache.insert(key, idx);
    idx
}

// ---------------------------------------------------------------------------
// Polygon boolean benchmarks
// ---------------------------------------------------------------------------

fn bench_polygon_union(c: &mut Criterion) {
    let poly_a = create_rect_polygon(0.0, 0.0, 100.0, 100.0);
    let poly_b = create_rect_polygon(50.0, 0.0, 150.0, 100.0);

    c.bench_function("polygon_union_overlapping", |b| {
        b.iter(|| polygon_union(&[poly_a.clone()], &[poly_b.clone()]))
    });
}

fn bench_polygon_intersection(c: &mut Criterion) {
    let poly_a = create_rect_polygon(0.0, 0.0, 100.0, 100.0);
    let poly_b = create_rect_polygon(50.0, 0.0, 150.0, 100.0);

    c.bench_function("polygon_intersection_overlapping", |b| {
        b.iter(|| polygon_intersection(&[poly_a.clone()], &[poly_b.clone()]))
    });
}

fn bench_polygon_difference(c: &mut Criterion) {
    let poly_a = create_rect_polygon(0.0, 0.0, 100.0, 100.0);
    let poly_b = create_rect_polygon(50.0, 0.0, 150.0, 100.0);

    c.bench_function("polygon_difference_overlapping", |b| {
        b.iter(|| polygon_difference(&[poly_a.clone()], &[poly_b.clone()]))
    });
}

// ---------------------------------------------------------------------------
// Polygon offset benchmarks
// ---------------------------------------------------------------------------

fn bench_offset_outward(c: &mut Criterion) {
    let star = create_star_polygon(50.0, 50.0, 30.0, 15.0, 12);
    let delta = mm_to_coord(2.0); // 2mm outward

    c.bench_function("offset_star_12pt_outward_2mm", |b| {
        b.iter(|| offset_polygon(&star, delta, JoinType::Miter))
    });
}

fn bench_offset_inward(c: &mut Criterion) {
    let star = create_star_polygon(50.0, 50.0, 30.0, 15.0, 12);
    let delta = mm_to_coord(-2.0); // 2mm inward

    c.bench_function("offset_star_12pt_inward_2mm", |b| {
        b.iter(|| offset_polygon(&star, delta, JoinType::Miter))
    });
}

fn bench_offset_collapse(c: &mut Criterion) {
    // Offset inward far enough to collapse the polygon entirely.
    let rect = create_rect_polygon(0.0, 0.0, 4.0, 4.0);
    let delta = mm_to_coord(-3.0); // 3mm inward on a 4mm rect -> collapse

    c.bench_function("offset_rect_collapse", |b| {
        b.iter(|| offset_polygon(&rect, delta, JoinType::Miter))
    });
}

// ---------------------------------------------------------------------------
// Point-in-polygon benchmarks
// ---------------------------------------------------------------------------

fn bench_point_in_polygon(c: &mut Criterion) {
    let poly = create_regular_polygon(50.0, 50.0, 30.0, 20);
    let points_raw = poly.points().to_vec();

    // Inside point (center)
    let inside = IPoint2::from_mm(50.0, 50.0);
    // Outside point
    let outside = IPoint2::from_mm(100.0, 100.0);
    // Near boundary point
    let boundary = IPoint2::from_mm(50.0 + 30.0 * 0.99, 50.0);

    c.bench_function("point_in_polygon_inside", |b| {
        b.iter(|| point_in_polygon(&inside, &points_raw))
    });

    c.bench_function("point_in_polygon_outside", |b| {
        b.iter(|| point_in_polygon(&outside, &points_raw))
    });

    c.bench_function("point_in_polygon_boundary", |b| {
        b.iter(|| point_in_polygon(&boundary, &points_raw))
    });
}

// ---------------------------------------------------------------------------
// Mesh slicing benchmark
// ---------------------------------------------------------------------------

fn bench_slice_mesh(c: &mut Criterion) {
    let mesh = build_sphere(3); // ~1280 triangles

    c.bench_function("slice_mesh_sphere_1280tri_0.2mm", |b| {
        b.iter(|| slice_mesh(&mesh, 0.2, 0.3))
    });
}

// ---------------------------------------------------------------------------
// BVH ray intersection benchmark
// ---------------------------------------------------------------------------

fn bench_bvh_ray_intersection(c: &mut Criterion) {
    let mesh = build_sphere(3); // ~1280 triangles

    // Pre-generate 100 ray origins and directions aimed at the sphere center.
    let center = Point3::new(100.0, 100.0, 10.0);
    let rays: Vec<(Point3, Vec3)> = (0..100)
        .map(|i| {
            // Rays from different angles in a circle around the sphere.
            let angle = 2.0 * PI * (i as f64) / 100.0;
            let height = 10.0 + 5.0 * ((i as f64 * 0.7).sin());
            let origin = Point3::new(
                center.x + 50.0 * angle.cos(),
                center.y + 50.0 * angle.sin(),
                height,
            );
            let dir = Vec3::new(
                center.x - origin.x,
                center.y - origin.y,
                center.z - origin.z,
            );
            // Normalize direction.
            let len = (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt();
            let dir_n = Vec3::new(dir.x / len, dir.y / len, dir.z / len);
            (origin, dir_n)
        })
        .collect();

    // Force BVH build before benchmarking.
    let _ = mesh.bvh();

    c.bench_function("bvh_ray_intersect_100rays", |b| {
        b.iter(|| {
            let mut hits = 0u32;
            for (origin, dir) in &rays {
                if ray_cast(&mesh, origin, dir).is_some() {
                    hits += 1;
                }
            }
            hits
        })
    });
}

criterion_group!(
    benches,
    bench_polygon_union,
    bench_polygon_intersection,
    bench_polygon_difference,
    bench_offset_outward,
    bench_offset_inward,
    bench_offset_collapse,
    bench_point_in_polygon,
    bench_slice_mesh,
    bench_bvh_ray_intersection,
);
criterion_main!(benches);
