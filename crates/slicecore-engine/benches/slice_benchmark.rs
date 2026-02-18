//! Full-pipeline slice benchmarks for 5 synthetic model types.
//!
//! Each model is generated in-memory (no external STL files) and sliced through
//! the complete Engine pipeline: mesh slicing, perimeters, surface classification,
//! infill, toolpath assembly, and G-code generation.
//!
//! Models:
//! 1. Calibration cube (20mm, 12 triangles)
//! 2. Cylinder (64 sides, ~256 triangles)
//! 3. Dense sphere (icosahedron with 3 subdivisions, ~1280 triangles)
//! 4. Thin-wall box (0.8mm walls, hollow)
//! 5. Multi-overhang model (box with shelf overhangs)

use std::collections::HashMap;
use std::f64::consts::PI;

use criterion::{criterion_group, criterion_main, Criterion};
use slicecore_engine::{Engine, InfillPattern, PrintConfig, SupportConfig};
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

// ---------------------------------------------------------------------------
// Model generators
// ---------------------------------------------------------------------------

/// 20mm calibration cube centered at (100, 100) on bed. 8 vertices, 12 triangles.
fn build_calibration_cube() -> TriangleMesh {
    let ox = 90.0;
    let oy = 90.0;
    let s = 20.0;
    let vertices = vec![
        Point3::new(ox, oy, 0.0),
        Point3::new(ox + s, oy, 0.0),
        Point3::new(ox + s, oy + s, 0.0),
        Point3::new(ox, oy + s, 0.0),
        Point3::new(ox, oy, s),
        Point3::new(ox + s, oy, s),
        Point3::new(ox + s, oy + s, s),
        Point3::new(ox, oy + s, s),
    ];
    let indices = vec![
        // top
        [4, 5, 6],
        [4, 6, 7],
        // bottom
        [1, 0, 3],
        [1, 3, 2],
        // right
        [1, 2, 6],
        [1, 6, 5],
        // left
        [0, 4, 7],
        [0, 7, 3],
        // back
        [3, 7, 6],
        [3, 6, 2],
        // front
        [0, 1, 5],
        [0, 5, 4],
    ];
    TriangleMesh::new(vertices, indices).expect("calibration cube mesh")
}

/// Cylinder with `sides` lateral facets, centered at (100, 100), radius 10mm, height 20mm.
/// Generates 2 * sides lateral triangles + 2 * (sides - 2) cap triangles.
fn build_cylinder(sides: u32) -> TriangleMesh {
    let cx = 100.0;
    let cy = 100.0;
    let radius = 10.0;
    let height = 20.0;

    let mut vertices = Vec::with_capacity((2 * sides + 2) as usize);
    let mut indices = Vec::new();

    // Bottom center and top center vertices.
    let bot_center = vertices.len() as u32;
    vertices.push(Point3::new(cx, cy, 0.0));
    let top_center = vertices.len() as u32;
    vertices.push(Point3::new(cx, cy, height));

    // Ring vertices: bottom ring then top ring.
    let bot_start = vertices.len() as u32;
    for i in 0..sides {
        let angle = 2.0 * PI * (i as f64) / (sides as f64);
        let x = cx + radius * angle.cos();
        let y = cy + radius * angle.sin();
        vertices.push(Point3::new(x, y, 0.0));
    }
    let top_start = vertices.len() as u32;
    for i in 0..sides {
        let angle = 2.0 * PI * (i as f64) / (sides as f64);
        let x = cx + radius * angle.cos();
        let y = cy + radius * angle.sin();
        vertices.push(Point3::new(x, y, height));
    }

    // Bottom cap (fan from center, CW when viewed from below -> CCW normals point down).
    for i in 0..sides {
        let next = (i + 1) % sides;
        indices.push([bot_center, bot_start + next, bot_start + i]);
    }

    // Top cap (fan from center, CCW when viewed from above -> normals point up).
    for i in 0..sides {
        let next = (i + 1) % sides;
        indices.push([top_center, top_start + i, top_start + next]);
    }

    // Lateral faces: two triangles per quad.
    for i in 0..sides {
        let next = (i + 1) % sides;
        let b0 = bot_start + i;
        let b1 = bot_start + next;
        let t0 = top_start + i;
        let t1 = top_start + next;
        indices.push([b0, b1, t1]);
        indices.push([b0, t1, t0]);
    }

    TriangleMesh::new(vertices, indices).expect("cylinder mesh")
}

/// Subdivided icosahedron sphere centered at (100, 100, 10), radius 10mm.
/// `subdivisions` = 3 yields ~1280 triangles.
fn build_sphere(subdivisions: u32) -> TriangleMesh {
    let cx = 100.0;
    let cy = 100.0;
    let cz = 10.0;
    let radius = 10.0;

    // Start with icosahedron.
    let t = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let base_verts = vec![
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

    let mut vertices: Vec<[f64; 3]> = base_verts;
    let mut tris = base_tris;

    // Subdivision: split each triangle into 4 by adding midpoints.
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

    // Normalize vertices to sphere and translate.
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

/// Thin-wall hollow box: outer 20mm cube minus inner cube with 0.8mm walls.
/// Composed of outer + inner box meshes as separate triangle sets.
fn build_thin_wall_box() -> TriangleMesh {
    let ox = 90.0;
    let oy = 90.0;
    let s = 20.0;
    let wall = 0.8;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Outer box
    add_box_triangles(
        ox,
        oy,
        0.0,
        ox + s,
        oy + s,
        s,
        &mut vertices,
        &mut indices,
        false,
    );

    // Inner box (inverted normals = hole)
    add_box_triangles(
        ox + wall,
        oy + wall,
        0.0,
        ox + s - wall,
        oy + s - wall,
        s,
        &mut vertices,
        &mut indices,
        true,
    );

    TriangleMesh::new(vertices, indices).expect("thin-wall box mesh")
}

/// Multi-overhang model: main box (20mm) with two shelf overhangs at different heights.
fn build_multi_overhang() -> TriangleMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Main body: 20x20x30 tall box
    add_box_triangles(
        90.0,
        90.0,
        0.0,
        110.0,
        110.0,
        30.0,
        &mut vertices,
        &mut indices,
        false,
    );

    // Shelf 1 at z=10: extends 10mm to the right, 2mm thick
    add_box_triangles(
        110.0,
        90.0,
        10.0,
        120.0,
        110.0,
        12.0,
        &mut vertices,
        &mut indices,
        false,
    );

    // Shelf 2 at z=20: extends 8mm to the left, 2mm thick
    add_box_triangles(
        82.0,
        90.0,
        20.0,
        90.0,
        110.0,
        22.0,
        &mut vertices,
        &mut indices,
        false,
    );

    TriangleMesh::new(vertices, indices).expect("multi-overhang mesh")
}

/// Helper: appends an axis-aligned box's vertices and triangles.
/// If `invert` is true, triangle winding is reversed (for inner surfaces / holes).
fn add_box_triangles(
    x0: f64,
    y0: f64,
    z0: f64,
    x1: f64,
    y1: f64,
    z1: f64,
    vertices: &mut Vec<Point3>,
    indices: &mut Vec<[u32; 3]>,
    invert: bool,
) {
    let base = vertices.len() as u32;
    vertices.extend_from_slice(&[
        Point3::new(x0, y0, z0),
        Point3::new(x1, y0, z0),
        Point3::new(x1, y1, z0),
        Point3::new(x0, y1, z0),
        Point3::new(x0, y0, z1),
        Point3::new(x1, y0, z1),
        Point3::new(x1, y1, z1),
        Point3::new(x0, y1, z1),
    ]);

    let box_tris: Vec<[u32; 3]> = vec![
        // top
        [4, 5, 6],
        [4, 6, 7],
        // bottom
        [1, 0, 3],
        [1, 3, 2],
        // right (+X)
        [1, 2, 6],
        [1, 6, 5],
        // left (-X)
        [0, 4, 7],
        [0, 7, 3],
        // back (+Y)
        [3, 7, 6],
        [3, 6, 2],
        // front (-Y)
        [0, 1, 5],
        [0, 5, 4],
    ];

    for tri in box_tris {
        if invert {
            indices.push([base + tri[0], base + tri[2], base + tri[1]]);
        } else {
            indices.push([base + tri[0], base + tri[1], base + tri[2]]);
        }
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_slice_calibration_cube(c: &mut Criterion) {
    let mesh = build_calibration_cube();
    let config = PrintConfig::default();
    let engine = Engine::new(config);

    c.bench_function("slice_calibration_cube", |b| {
        b.iter(|| engine.slice(&mesh).unwrap())
    });
}

fn bench_slice_cylinder(c: &mut Criterion) {
    let mesh = build_cylinder(64);
    let config = PrintConfig::default();
    let engine = Engine::new(config);

    c.bench_function("slice_cylinder_64sides", |b| {
        b.iter(|| engine.slice(&mesh).unwrap())
    });
}

fn bench_slice_sphere(c: &mut Criterion) {
    let mesh = build_sphere(3);
    let config = PrintConfig::default();
    let engine = Engine::new(config);

    c.bench_function("slice_dense_sphere_1280tri", |b| {
        b.iter(|| engine.slice(&mesh).unwrap())
    });
}

fn bench_slice_thin_wall(c: &mut Criterion) {
    let mesh = build_thin_wall_box();
    let config = PrintConfig::default();
    let engine = Engine::new(config);

    c.bench_function("slice_thin_wall_box", |b| {
        b.iter(|| engine.slice(&mesh).unwrap())
    });
}

fn bench_slice_multi_overhang(c: &mut Criterion) {
    let mesh = build_multi_overhang();
    let config = PrintConfig::default();
    let engine = Engine::new(config);

    c.bench_function("slice_multi_overhang", |b| {
        b.iter(|| engine.slice(&mesh).unwrap())
    });
}

fn bench_slice_cube_full_config(c: &mut Criterion) {
    let mesh = build_calibration_cube();
    let config = PrintConfig {
        infill_pattern: InfillPattern::Gyroid,
        infill_density: 0.20,
        support: SupportConfig {
            enabled: true,
            ..SupportConfig::default()
        },
        adaptive_layer_height: true,
        ..PrintConfig::default()
    };
    let engine = Engine::new(config);

    c.bench_function("slice_cube_full_config", |b| {
        b.iter(|| engine.slice(&mesh).unwrap())
    });
}

fn bench_memory_estimate(c: &mut Criterion) {
    let mesh = build_calibration_cube();
    let config = PrintConfig::default();
    let engine = Engine::new(config);

    c.bench_function("memory_estimate_cube", |b| {
        b.iter(|| {
            let result = engine.slice(&mesh).unwrap();
            // Capture output metrics that correlate with memory usage.
            let gcode_bytes = result.gcode.len();
            let layers = result.layer_count;
            // Read peak RSS from /proc/self/status on Linux.
            let peak_rss_kb = read_peak_rss_kb().unwrap_or(0);
            (gcode_bytes, layers, peak_rss_kb)
        })
    });
}

/// Reads VmHWM (peak resident set size) from /proc/self/status on Linux.
/// Returns None on non-Linux platforms.
fn read_peak_rss_kb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if line.starts_with("VmHWM:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    return parts[1].parse().ok();
                }
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

criterion_group!(
    benches,
    bench_slice_calibration_cube,
    bench_slice_cylinder,
    bench_slice_sphere,
    bench_slice_thin_wall,
    bench_slice_multi_overhang,
    bench_slice_cube_full_config,
    bench_memory_estimate,
);
criterion_main!(benches);
