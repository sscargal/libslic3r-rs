//! Criterion benchmarks comparing parallel vs sequential slicing performance.
//!
//! Measures wall-time speedup from rayon parallelization on a 200-layer test mesh
//! (40mm tall calibration cube at 0.2mm layer height).
//!
//! Run with: `cargo bench -p slicecore-engine --bench parallel_benchmark --features parallel -- --quick`

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use slicecore_engine::{Engine, PrintConfig};
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

/// 20mm x 20mm x 40mm calibration cube centered at (100, 100) on bed.
/// At 0.2mm layer height this produces ~200 layers -- enough to benefit from parallelism.
fn build_tall_cube() -> TriangleMesh {
    let ox = 90.0;
    let oy = 90.0;
    let sx = 20.0;
    let sy = 20.0;
    let sz = 40.0;

    let vertices = vec![
        Point3::new(ox, oy, 0.0),
        Point3::new(ox + sx, oy, 0.0),
        Point3::new(ox + sx, oy + sy, 0.0),
        Point3::new(ox, oy + sy, 0.0),
        Point3::new(ox, oy, sz),
        Point3::new(ox + sx, oy, sz),
        Point3::new(ox + sx, oy + sy, sz),
        Point3::new(ox, oy + sy, sz),
    ];
    let indices = vec![
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
    TriangleMesh::new(vertices, indices).expect("tall cube mesh")
}

fn bench_parallel_vs_sequential(c: &mut Criterion) {
    let mesh = build_tall_cube();

    let mut group = c.benchmark_group("parallel_vs_sequential");
    group.sample_size(10);

    // Sequential mode
    group.bench_function(BenchmarkId::new("sequential", "40mm_cube"), |b| {
        let config = PrintConfig {
            parallel_slicing: false,
            ..PrintConfig::default()
        };
        let engine = Engine::new(config);
        b.iter(|| engine.slice(&mesh, None).unwrap());
    });

    // Parallel mode (default thread count -- all available cores)
    group.bench_function(BenchmarkId::new("parallel_auto", "40mm_cube"), |b| {
        let config = PrintConfig {
            parallel_slicing: true,
            ..PrintConfig::default()
        };
        let engine = Engine::new(config);
        b.iter(|| engine.slice(&mesh, None).unwrap());
    });

    // Parallel mode with 4 threads (shows scaling behavior)
    group.bench_function(BenchmarkId::new("parallel_4_threads", "40mm_cube"), |b| {
        let config = PrintConfig {
            parallel_slicing: true,
            thread_count: Some(4),
            ..PrintConfig::default()
        };
        let engine = Engine::new(config);
        b.iter(|| engine.slice(&mesh, None).unwrap());
    });

    group.finish();
}

criterion_group!(benches, bench_parallel_vs_sequential);
criterion_main!(benches);
