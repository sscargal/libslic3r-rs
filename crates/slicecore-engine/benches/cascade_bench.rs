//! Criterion benchmarks for cascade resolution and config merge overhead.
//!
//! Measures:
//! - Single-object cascade resolution (no overrides)
//! - Multi-object cascade resolution (10 objects with named override sets)
//! - Scaling from 1 to 50 objects with overrides
//! - TOML table merge cost (single-field overlay)
//!
//! Run: `cargo bench -p slicecore-engine --bench cascade_bench`
//! Compare: `cargo bench -p slicecore-engine --bench cascade_bench -- --save-baseline before`
//!   then: `cargo bench -p slicecore-engine --bench cascade_bench -- --baseline before`

use std::collections::HashMap;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use slicecore_engine::cascade::CascadeResolver;
use slicecore_engine::config::PrintConfig;
use slicecore_engine::plate_config::{ObjectConfig, PlateConfig};
use slicecore_engine::profile_compose::{
    merge_layer, ComposedConfig, FieldSource, ProfileComposer, SourceType,
};

/// Helper: compose a base config from defaults (layers 1-6).
fn base_composed() -> ComposedConfig {
    let composer = ProfileComposer::new();
    composer.compose().expect("default compose should work")
}

fn bench_cascade_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("cascade_resolution");

    // Benchmark: single object, no overrides
    group.bench_function("single_object_no_overrides", |b| {
        let plate = PlateConfig::single_object(PrintConfig::default());
        let base = base_composed();
        b.iter(|| {
            CascadeResolver::resolve_all(black_box(&plate), black_box(&base)).unwrap()
        });
    });

    // Benchmark: 10 objects with overrides
    group.bench_function("10_objects_with_overrides", |b| {
        let mut plate = PlateConfig::default();
        let mut override_table = toml::map::Map::new();
        override_table.insert("infill_density".to_string(), toml::Value::Float(0.5));
        override_table.insert("wall_count".to_string(), toml::Value::Integer(4));
        plate
            .override_sets
            .insert("test".to_string(), override_table);
        for i in 0..10 {
            plate.objects.push(ObjectConfig {
                name: Some(format!("object_{i}")),
                override_set: Some("test".to_string()),
                ..ObjectConfig::default()
            });
        }
        let base = base_composed();
        b.iter(|| {
            CascadeResolver::resolve_all(black_box(&plate), black_box(&base)).unwrap()
        });
    });

    // Benchmark: scaling with object count
    for count in [1, 5, 10, 25, 50] {
        group.bench_with_input(
            BenchmarkId::new("objects_with_overrides", count),
            &count,
            |b, &count| {
                let mut plate = PlateConfig::default();
                let mut override_table = toml::map::Map::new();
                override_table
                    .insert("infill_density".to_string(), toml::Value::Float(0.5));
                plate
                    .override_sets
                    .insert("test".to_string(), override_table);
                for i in 0..count {
                    plate.objects.push(ObjectConfig {
                        name: Some(format!("obj_{i}")),
                        override_set: Some("test".to_string()),
                        ..ObjectConfig::default()
                    });
                }
                let base = base_composed();
                b.iter(|| {
                    CascadeResolver::resolve_all(black_box(&plate), black_box(&base)).unwrap()
                });
            },
        );
    }

    group.finish();
}

fn bench_config_merge_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_merge");

    // Benchmark: TOML table merge (the core merge_layer operation)
    group.bench_function("merge_single_field", |b| {
        let base_config = PrintConfig::default();
        let base_toml = toml::Value::try_from(&base_config).unwrap();
        let base_table = match base_toml {
            toml::Value::Table(t) => t,
            _ => unreachable!(),
        };
        let mut overlay = toml::map::Map::new();
        overlay.insert("infill_density".to_string(), toml::Value::Float(0.8));

        let source = FieldSource {
            source_type: SourceType::PerObjectOverride {
                object_id: "bench".to_string(),
            },
            file_path: None,
            overrode: None,
        };

        b.iter(|| {
            let mut clone = base_table.clone();
            let mut provenance = HashMap::new();
            let mut warnings = Vec::new();
            merge_layer(
                &mut clone,
                black_box(&overlay),
                "",
                &source,
                &mut provenance,
                &mut warnings,
            );
        });
    });

    group.finish();
}

criterion_group!(benches, bench_cascade_resolution, bench_config_merge_overhead);
criterion_main!(benches);
