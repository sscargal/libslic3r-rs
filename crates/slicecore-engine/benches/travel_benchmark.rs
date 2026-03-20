//! Criterion benchmarks for TSP-based travel move optimization.
//!
//! Benchmarks the various algorithm variants on synthetic multi-object plates
//! of different sizes (4-object, 9-object, 25-object, scattered).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use slicecore_engine::{optimize_tour, TravelOptAlgorithm, TravelOptConfig, TspNode};
use slicecore_math::Point2;

// ---------------------------------------------------------------------------
// Synthetic plate generators
// ---------------------------------------------------------------------------

/// Generates a grid of objects with perimeter (closed) and infill (open) nodes.
///
/// Each grid cell at `(col * spacing, row * spacing)` gets 4 perimeter nodes
/// (closed paths near the center) and 3 infill lines (open, reversible).
fn generate_grid_plate(rows: usize, cols: usize, spacing: f64) -> Vec<TspNode> {
    let mut nodes = Vec::new();
    let mut idx = 0;

    for row in 0..rows {
        for col in 0..cols {
            let cx = col as f64 * spacing;
            let cy = row as f64 * spacing;

            // 4 perimeter nodes (closed paths: entry == exit)
            for &(dx, dy) in &[(5.0, 0.0), (0.0, 5.0), (-5.0, 0.0), (0.0, -5.0)] {
                let pt = Point2::new(cx + dx, cy + dy);
                nodes.push(TspNode {
                    entry: pt,
                    exit: pt,
                    reversible: false,
                    original_index: idx,
                });
                idx += 1;
            }

            // 3 infill lines (open paths, reversible)
            for i in 0..3 {
                let offset = (i as f64 - 1.0) * 3.0;
                nodes.push(TspNode {
                    entry: Point2::new(cx - 8.0, cy + offset),
                    exit: Point2::new(cx + 8.0, cy + offset),
                    reversible: true,
                    original_index: idx,
                });
                idx += 1;
            }
        }
    }

    nodes
}

/// Generates scattered objects using a simple deterministic pseudo-random
/// sequence (no external dependency needed for reproducibility).
fn generate_scattered_plate(count: usize, bed_size: f64) -> Vec<TspNode> {
    let mut nodes = Vec::new();
    let mut idx = 0;
    // Simple LCG for deterministic positions (seed = 42).
    let mut rng_state: u64 = 42;

    for _ in 0..count {
        rng_state = rng_state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let x = (rng_state >> 33) as f64 / (u32::MAX as f64) * bed_size;
        rng_state = rng_state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let y = (rng_state >> 33) as f64 / (u32::MAX as f64) * bed_size;

        // 3 perimeter nodes
        for &(dx, dy) in &[(4.0, 0.0), (0.0, 4.0), (-4.0, 0.0)] {
            let pt = Point2::new(x + dx, y + dy);
            nodes.push(TspNode {
                entry: pt,
                exit: pt,
                reversible: false,
                original_index: idx,
            });
            idx += 1;
        }

        // 1 infill line
        nodes.push(TspNode {
            entry: Point2::new(x - 6.0, y),
            exit: Point2::new(x + 6.0, y),
            reversible: true,
            original_index: idx,
        });
        idx += 1;
    }

    nodes
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

fn bench_travel_opt_4obj(c: &mut Criterion) {
    let mut group = c.benchmark_group("travel_opt_4obj");
    let nodes = generate_grid_plate(2, 2, 100.0);
    let node_count = nodes.len();
    let start = Point2::new(0.0, 0.0);

    let algorithms = [
        ("auto", TravelOptAlgorithm::Auto),
        ("nearest_neighbor", TravelOptAlgorithm::NearestNeighbor),
        ("greedy_edge", TravelOptAlgorithm::GreedyEdgeInsertion),
        ("nn_only", TravelOptAlgorithm::NearestNeighborOnly),
        ("greedy_only", TravelOptAlgorithm::GreedyOnly),
    ];

    for (name, alg) in &algorithms {
        group.bench_with_input(
            BenchmarkId::new(*name, node_count),
            &nodes,
            |b, nodes| {
                let config = TravelOptConfig {
                    algorithm: *alg,
                    ..TravelOptConfig::default()
                };
                b.iter(|| {
                    optimize_tour(black_box(nodes), black_box(start), black_box(&config))
                });
            },
        );
    }
    group.finish();
}

fn bench_travel_opt_9obj(c: &mut Criterion) {
    let mut group = c.benchmark_group("travel_opt_9obj");
    let nodes = generate_grid_plate(3, 3, 80.0);
    let node_count = nodes.len();
    let start = Point2::new(0.0, 0.0);

    let algorithms = [
        ("auto", TravelOptAlgorithm::Auto),
        ("nearest_neighbor", TravelOptAlgorithm::NearestNeighbor),
        ("greedy_edge", TravelOptAlgorithm::GreedyEdgeInsertion),
        ("nn_only", TravelOptAlgorithm::NearestNeighborOnly),
        ("greedy_only", TravelOptAlgorithm::GreedyOnly),
    ];

    for (name, alg) in &algorithms {
        group.bench_with_input(
            BenchmarkId::new(*name, node_count),
            &nodes,
            |b, nodes| {
                let config = TravelOptConfig {
                    algorithm: *alg,
                    ..TravelOptConfig::default()
                };
                b.iter(|| {
                    optimize_tour(black_box(nodes), black_box(start), black_box(&config))
                });
            },
        );
    }
    group.finish();
}

fn bench_travel_opt_25obj(c: &mut Criterion) {
    let mut group = c.benchmark_group("travel_opt_25obj");
    let nodes = generate_grid_plate(5, 5, 40.0);
    let node_count = nodes.len();
    let start = Point2::new(0.0, 0.0);

    // Only Auto and NearestNeighborOnly for scaling comparison.
    let algorithms = [
        ("auto", TravelOptAlgorithm::Auto),
        ("nn_only", TravelOptAlgorithm::NearestNeighborOnly),
    ];

    for (name, alg) in &algorithms {
        group.bench_with_input(
            BenchmarkId::new(*name, node_count),
            &nodes,
            |b, nodes| {
                let config = TravelOptConfig {
                    algorithm: *alg,
                    ..TravelOptConfig::default()
                };
                b.iter(|| {
                    optimize_tour(black_box(nodes), black_box(start), black_box(&config))
                });
            },
        );
    }
    group.finish();
}

fn bench_travel_opt_scattered(c: &mut Criterion) {
    let mut group = c.benchmark_group("travel_opt_scattered");
    let nodes = generate_scattered_plate(20, 200.0);
    let node_count = nodes.len();
    let start = Point2::new(0.0, 0.0);

    group.bench_with_input(
        BenchmarkId::new("auto", node_count),
        &nodes,
        |b, nodes| {
            let config = TravelOptConfig::default();
            b.iter(|| {
                optimize_tour(black_box(nodes), black_box(start), black_box(&config))
            });
        },
    );
    group.finish();
}

criterion_group!(
    benches,
    bench_travel_opt_4obj,
    bench_travel_opt_9obj,
    bench_travel_opt_25obj,
    bench_travel_opt_scattered
);
criterion_main!(benches);
