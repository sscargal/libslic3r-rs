// CSG operation benchmarks for slicecore-mesh.
//
// Run:   cargo bench -p slicecore-mesh
// Save baseline: cargo bench -p slicecore-mesh -- --save-baseline before
// Compare:       cargo bench -p slicecore-mesh -- --baseline before

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use slicecore_mesh::bvh::BVH;
use slicecore_mesh::csg::{
    hollow_mesh, mesh_difference, mesh_intersection, mesh_split_at_plane, mesh_union,
    primitive_box, primitive_cylinder, primitive_sphere, primitive_torus, HollowOptions,
    SplitOptions, SplitPlane,
};
use slicecore_mesh::TriangleMesh;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Rebuilds a mesh from vertices and indices (cheap clone workaround).
fn rebuild(mesh: &TriangleMesh) -> TriangleMesh {
    TriangleMesh::new(mesh.vertices().to_vec(), mesh.indices().to_vec()).expect("valid mesh")
}

/// Returns two overlapping boxes (12 triangles each).
fn two_boxes() -> (TriangleMesh, TriangleMesh) {
    let a = primitive_box(2.0, 2.0, 2.0);
    let b = primitive_box(2.0, 2.0, 2.0);
    let mut verts: Vec<_> = b.vertices().to_vec();
    for v in &mut verts {
        v.x += 1.0;
        v.y += 1.0;
        v.z += 1.0;
    }
    let b_shifted = TriangleMesh::new(verts, b.indices().to_vec()).expect("valid mesh");
    (a, b_shifted)
}

/// Returns two overlapping spheres at the given segment count.
fn two_spheres(segments: u32) -> (TriangleMesh, TriangleMesh) {
    let a = primitive_sphere(1.0, segments);
    let b = primitive_sphere(1.0, segments);
    let mut verts: Vec<_> = b.vertices().to_vec();
    for v in &mut verts {
        v.x += 0.5;
        v.y += 0.5;
    }
    let b_shifted = TriangleMesh::new(verts, b.indices().to_vec()).expect("valid mesh");
    (a, b_shifted)
}

// ---------------------------------------------------------------------------
// Group 1: Boolean operation throughput
// ---------------------------------------------------------------------------

fn bench_boolean_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("boolean_ops");

    let sizes: &[(&str, fn() -> (TriangleMesh, TriangleMesh))] = &[
        ("small_box_12tri", two_boxes as fn() -> _),
        ("medium_sphere_16seg", || two_spheres(16)),
        ("large_sphere_32seg", || two_spheres(32)),
        ("xl_sphere_64seg", || two_spheres(64)),
    ];

    for &(label, make_pair) in sizes {
        let (a, b) = make_pair();
        group.bench_function(BenchmarkId::new("union", label), |bench| {
            bench.iter_batched(
                || (rebuild(&a), rebuild(&b)),
                |(ref a, ref b)| mesh_union(black_box(a), black_box(b)),
                criterion::BatchSize::SmallInput,
            );
        });

        group.bench_function(BenchmarkId::new("difference", label), |bench| {
            bench.iter_batched(
                || (rebuild(&a), rebuild(&b)),
                |(ref a, ref b)| mesh_difference(black_box(a), black_box(b)),
                criterion::BatchSize::SmallInput,
            );
        });

        group.bench_function(BenchmarkId::new("intersection", label), |bench| {
            bench.iter_batched(
                || (rebuild(&a), rebuild(&b)),
                |(ref a, ref b)| mesh_intersection(black_box(a), black_box(b)),
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 2: Primitive generation
// ---------------------------------------------------------------------------

fn bench_primitives(c: &mut Criterion) {
    let mut group = c.benchmark_group("primitives");

    group.bench_function("box", |b| {
        b.iter(|| primitive_box(black_box(2.0), black_box(3.0), black_box(4.0)));
    });

    group.bench_function("sphere_32seg", |b| {
        b.iter(|| primitive_sphere(black_box(1.0), black_box(32)));
    });

    group.bench_function("cylinder_32seg", |b| {
        b.iter(|| primitive_cylinder(black_box(1.0), black_box(2.0), black_box(32)));
    });

    group.bench_function("torus_32x16", |b| {
        b.iter(|| {
            primitive_torus(
                black_box(2.0),
                black_box(0.5),
                black_box(32),
                black_box(16),
            )
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 3: Plane split
// ---------------------------------------------------------------------------

fn bench_plane_split(c: &mut Criterion) {
    let mut group = c.benchmark_group("plane_split");

    for segments in [16u32, 32, 64] {
        let sphere = primitive_sphere(1.0, segments);
        let plane = SplitPlane::xy(0.0); // equator
        let opts = SplitOptions::default();

        group.bench_function(
            BenchmarkId::new("equator_split", format!("{segments}seg")),
            |bench| {
                bench.iter(|| {
                    mesh_split_at_plane(black_box(&sphere), black_box(&plane), black_box(&opts))
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 4: Hollowing pipeline
// ---------------------------------------------------------------------------

fn bench_hollow(c: &mut Criterion) {
    let mut group = c.benchmark_group("hollow");

    let box_mesh = primitive_box(10.0, 10.0, 10.0);
    let opts = HollowOptions {
        wall_thickness: 2.0,
        drain_hole: None,
    };

    group.bench_function(BenchmarkId::new("hollow", "box"), |bench| {
        bench.iter(|| hollow_mesh(black_box(&box_mesh), black_box(&opts)));
    });

    let sphere_mesh = primitive_sphere(5.0, 32);
    group.bench_function(BenchmarkId::new("hollow", "sphere_32seg"), |bench| {
        bench.iter(|| hollow_mesh(black_box(&sphere_mesh), black_box(&opts)));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 5: BVH construction overhead
// ---------------------------------------------------------------------------

fn bench_bvh(c: &mut Criterion) {
    let mut group = c.benchmark_group("bvh_build");

    for segments in [16u32, 32, 64] {
        let sphere = primitive_sphere(1.0, segments);
        let verts = sphere.vertices().to_vec();
        let idxs = sphere.indices().to_vec();

        group.bench_function(BenchmarkId::new("build", format!("{segments}seg")), |bench| {
            bench.iter(|| BVH::build(black_box(&verts), black_box(&idxs)));
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Group 6: Parallel vs sequential (only with `parallel` feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "parallel")]
fn bench_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_vs_sequential");
    let (a, b) = two_spheres(32);

    group.bench_function("union_large", |bench| {
        bench.iter(|| mesh_union(black_box(&a), black_box(&b)));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

#[cfg(not(feature = "parallel"))]
criterion_group!(
    benches,
    bench_boolean_ops,
    bench_primitives,
    bench_plane_split,
    bench_hollow,
    bench_bvh,
);

#[cfg(feature = "parallel")]
criterion_group!(
    benches,
    bench_boolean_ops,
    bench_primitives,
    bench_plane_split,
    bench_hollow,
    bench_bvh,
    bench_parallel,
);

criterion_main!(benches);
