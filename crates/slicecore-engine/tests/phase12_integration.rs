//! Phase 12 integration tests: Self-intersection resolution pipeline.
//!
//! Verifies all 5 Phase 12 success criteria:
//! - SC1: Clipper2 boolean union resolves overlapping contours
//! - SC2: RepairReport shows before/after metrics with intersection count
//! - SC3: Real-world-like self-intersecting models repair and slice successfully
//! - SC4: Repaired mesh output (resolved contours) passes validation
//! - SC5: Resolution completes in <5 seconds for models with <10k triangles

use slicecore_engine::{Engine, PrintConfig};
use slicecore_geo::Winding;
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Creates the vertices and indices for a single axis-aligned cube.
fn make_cube(min: Point3, max: Point3) -> (Vec<Point3>, Vec<[u32; 3]>) {
    let vertices = vec![
        Point3::new(min.x, min.y, min.z), // 0
        Point3::new(max.x, min.y, min.z), // 1
        Point3::new(max.x, max.y, min.z), // 2
        Point3::new(min.x, max.y, min.z), // 3
        Point3::new(min.x, min.y, max.z), // 4
        Point3::new(max.x, min.y, max.z), // 5
        Point3::new(max.x, max.y, max.z), // 6
        Point3::new(min.x, max.y, max.z), // 7
    ];
    let indices = vec![
        [4, 5, 6],
        [4, 6, 7], // Front (z=max)
        [1, 0, 3],
        [1, 3, 2], // Back (z=min)
        [1, 2, 6],
        [1, 6, 5], // Right (x=max)
        [0, 4, 7],
        [0, 7, 3], // Left (x=min)
        [3, 7, 6],
        [3, 6, 2], // Top (y=max)
        [0, 1, 5],
        [0, 5, 4], // Bottom (y=min)
    ];
    (vertices, indices)
}

/// Creates a mesh with two overlapping 10mm cubes.
///
/// Cube A: (0,0,0) to (10,10,10)
/// Cube B: (5,5,0) to (15,15,10)
///
/// The overlapping region (x: 5-10, y: 5-10) creates self-intersecting
/// triangles. Total: 24 triangles.
fn make_two_overlapping_cubes() -> TriangleMesh {
    let (verts_a, indices_a) = make_cube(Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 10.0, 10.0));
    let (verts_b, indices_b) = make_cube(Point3::new(5.0, 5.0, 0.0), Point3::new(15.0, 15.0, 10.0));

    let offset = verts_a.len() as u32;
    let mut vertices = verts_a;
    vertices.extend(verts_b);

    let mut indices = indices_a;
    indices.extend(
        indices_b
            .into_iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );

    TriangleMesh::new(vertices, indices).expect("overlapping cubes mesh should be valid")
}

/// Creates a mesh with three overlapping cubes in a chain.
///
/// Cube A: (0,0,0) to (10,10,10)
/// Cube B: (5,5,0) to (15,15,10)
/// Cube C: (10,10,0) to (20,20,10)
///
/// A overlaps B, B overlaps C. ~36 triangles.
fn make_three_overlapping_cubes() -> TriangleMesh {
    let cubes = vec![
        (Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 10.0, 10.0)),
        (Point3::new(5.0, 5.0, 0.0), Point3::new(15.0, 15.0, 10.0)),
        (Point3::new(10.0, 10.0, 0.0), Point3::new(20.0, 20.0, 10.0)),
    ];

    let mut all_verts = Vec::new();
    let mut all_indices = Vec::new();

    for (min, max) in cubes {
        let offset = all_verts.len() as u32;
        let (verts, indices) = make_cube(min, max);
        all_verts.extend(verts);
        all_indices.extend(
            indices
                .into_iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );
    }

    TriangleMesh::new(all_verts, all_indices).expect("three overlapping cubes mesh should be valid")
}

/// Generates n overlapping cube pairs arranged in a grid for performance testing.
///
/// Each pair consists of two overlapping cubes offset by half their size.
/// Produces ~24*n triangles (12 per cube, 2 cubes per pair).
fn make_large_overlapping_mesh(n: usize) -> TriangleMesh {
    let mut all_verts = Vec::new();
    let mut all_indices = Vec::new();

    let cols = (n as f64).sqrt().ceil() as usize;

    for i in 0..n {
        let col = i % cols;
        let row = i / cols;
        let base_x = col as f64 * 30.0;
        let base_y = row as f64 * 30.0;

        // Cube A
        let offset = all_verts.len() as u32;
        let (verts_a, indices_a) = make_cube(
            Point3::new(base_x, base_y, 0.0),
            Point3::new(base_x + 10.0, base_y + 10.0, 10.0),
        );
        all_verts.extend(verts_a);
        all_indices.extend(
            indices_a
                .into_iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );

        // Cube B (overlapping A)
        let offset = all_verts.len() as u32;
        let (verts_b, indices_b) = make_cube(
            Point3::new(base_x + 5.0, base_y + 5.0, 0.0),
            Point3::new(base_x + 15.0, base_y + 15.0, 10.0),
        );
        all_verts.extend(verts_b);
        all_indices.extend(
            indices_b
                .into_iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );
    }

    TriangleMesh::new(all_verts, all_indices).expect("large overlapping mesh should be valid")
}

/// Creates two offset boxes with different Z ranges to create non-axis-aligned
/// intersections (simulating an offset shell model).
fn make_offset_shell_model() -> TriangleMesh {
    // Box A: centered at origin, slightly rotated via offset
    let (verts_a, indices_a) = make_cube(Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 10.0, 10.0));
    // Box B: offset by half the size in X and Y, and partially overlapping in Z
    let (verts_b, indices_b) = make_cube(Point3::new(3.0, 3.0, 2.0), Point3::new(13.0, 13.0, 12.0));

    let offset = verts_a.len() as u32;
    let mut vertices = verts_a;
    vertices.extend(verts_b);

    let mut indices = indices_a;
    indices.extend(
        indices_b
            .into_iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );

    TriangleMesh::new(vertices, indices).expect("offset shell model should be valid")
}

// ---------------------------------------------------------------------------
// SC1: Self-intersection resolution uses Clipper2 boolean union
// ---------------------------------------------------------------------------

#[test]
fn sc1_clipper2_union_resolves_overlapping_contours() {
    use slicecore_geo::polygon_union;
    use slicecore_slicer::{resolve_contour_intersections, slice_at_height};

    let mesh = make_two_overlapping_cubes();

    // Slice at z=5.0 (in the overlap region).
    // The slicer chains intersection segments into contours. With two overlapping
    // cubes, this may produce a single self-intersecting contour (figure-8) or
    // two separate contours depending on segment chaining.
    let raw_contours = slice_at_height(&mesh, 5.0);
    assert!(
        !raw_contours.is_empty(),
        "Should have contours at z=5.0 in the overlap region"
    );

    let raw_area: f64 = raw_contours.iter().map(|c| c.area_mm2()).sum();

    // Apply Clipper2 union resolution
    let resolved = resolve_contour_intersections(&raw_contours);
    assert!(
        !resolved.is_empty(),
        "Resolved contours should be non-empty"
    );

    let resolved_area: f64 = resolved.iter().map(|c| c.area_mm2()).sum();

    // Also verify union directly to confirm Clipper2 is being used
    let direct_union = polygon_union(&raw_contours, &[]).expect("polygon_union should succeed");
    let direct_area: f64 = direct_union.iter().map(|c| c.area_mm2()).sum();

    // SC1 key check: resolve_contour_intersections produces same result as
    // direct polygon_union (proving it uses Clipper2 boolean union internally).
    assert!(
        (resolved_area - direct_area).abs() < 1.0,
        "resolve_contour_intersections ({:.1}) should match direct polygon_union ({:.1})",
        resolved_area,
        direct_area
    );

    // The union area should approximate the true union of two overlapping cubes.
    // Cube A cross-section: (0,0)-(10,10) = 100 mm^2
    // Cube B cross-section: (5,5)-(15,15) = 100 mm^2
    // Union = 175 mm^2 (L-shaped merged region)
    assert!(
        (resolved_area - 175.0).abs() < 10.0,
        "Expected union area ~175 mm^2, got {:.1}",
        resolved_area
    );

    // Resolved area should be less than or equal to raw area (union removes overlap)
    assert!(
        resolved_area <= raw_area + 1.0,
        "Resolved area ({:.1}) should be <= raw area ({:.1})",
        resolved_area,
        raw_area
    );

    // Verify resolved contours are valid (no degenerate polygons)
    for contour in &resolved {
        assert!(
            contour.len() >= 3,
            "Each contour should have at least 3 points, got {}",
            contour.len()
        );
        assert!(
            contour.area_mm2() > 0.0,
            "Each contour should have positive area, got {}",
            contour.area_mm2()
        );
    }
}

// ---------------------------------------------------------------------------
// SC2: RepairReport shows before/after metrics with intersection count
// ---------------------------------------------------------------------------

#[test]
fn sc2_repair_report_shows_intersection_metrics() {
    use slicecore_mesh::repair::repair;

    // Build raw vertex/index data for two overlapping cubes
    let (verts_a, indices_a) = make_cube(Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 10.0, 10.0));
    let (verts_b, indices_b) = make_cube(Point3::new(5.0, 5.0, 0.0), Point3::new(15.0, 15.0, 10.0));

    let offset = verts_a.len() as u32;
    let mut vertices = verts_a;
    vertices.extend(verts_b);

    let mut indices = indices_a;
    indices.extend(
        indices_b
            .into_iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );

    let (_mesh, report) = repair(vertices, indices).expect("repair should succeed");

    // Self-intersections should be detected
    assert!(
        report.self_intersections_detected > 0,
        "Should detect self-intersections, got {}",
        report.self_intersections_detected
    );

    // Intersecting pairs should be non-empty with valid indices
    assert!(
        !report.intersecting_pairs.is_empty(),
        "intersecting_pairs should be non-empty"
    );
    for &(i, j) in &report.intersecting_pairs {
        assert!(i < j, "Pair indices should satisfy i < j: ({}, {})", i, j);
    }

    // Resolvable flag should be true
    assert!(
        report.self_intersections_resolvable,
        "self_intersections_resolvable should be true"
    );

    // Z-range should cover the overlap region (both cubes span z=0 to z=10)
    assert!(
        report.intersection_z_range.is_some(),
        "intersection_z_range should be Some"
    );
    let (z_min, z_max) = report.intersection_z_range.unwrap();
    assert!(
        z_min <= 0.0 + 0.1,
        "z_min should be near 0.0, got {}",
        z_min
    );
    assert!(
        z_max >= 10.0 - 0.1,
        "z_max should be near 10.0, got {}",
        z_max
    );
}

#[test]
fn sc2_clean_mesh_has_zero_intersections() {
    use slicecore_mesh::repair::repair;

    let (verts, indices) = make_cube(Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 10.0, 10.0));

    let (_mesh, report) = repair(verts, indices).expect("repair should succeed");

    assert_eq!(
        report.self_intersections_detected, 0,
        "Clean mesh should have zero self-intersections"
    );
    assert!(
        report.intersecting_pairs.is_empty(),
        "Clean mesh should have empty intersecting_pairs"
    );
    assert!(
        !report.self_intersections_resolvable,
        "Clean mesh should have self_intersections_resolvable == false"
    );
}

// ---------------------------------------------------------------------------
// SC3: Real-world-like self-intersecting models repair and slice successfully
// ---------------------------------------------------------------------------

#[test]
fn sc3_two_overlapping_cubes_slices_end_to_end() {
    let mesh = make_two_overlapping_cubes();
    let config = PrintConfig::default();
    let engine = Engine::new(config);

    let result = engine
        .slice(&mesh, None)
        .expect("two overlapping cubes should slice successfully");

    assert!(
        !result.gcode.is_empty(),
        "G-code should be non-empty for two overlapping cubes"
    );
    assert!(
        result.layer_count > 0,
        "Layer count should be positive, got {}",
        result.layer_count
    );
}

#[test]
fn sc3_three_overlapping_cubes_slices_end_to_end() {
    let mesh = make_three_overlapping_cubes();
    let config = PrintConfig::default();
    let engine = Engine::new(config);

    let result = engine
        .slice(&mesh, None)
        .expect("three overlapping cubes should slice successfully");

    assert!(
        !result.gcode.is_empty(),
        "G-code should be non-empty for three overlapping cubes"
    );
    assert!(
        result.layer_count > 0,
        "Layer count should be positive, got {}",
        result.layer_count
    );
}

#[test]
fn sc3_offset_shell_slices_successfully() {
    let mesh = make_offset_shell_model();
    let config = PrintConfig::default();
    let engine = Engine::new(config);

    let result = engine
        .slice(&mesh, None)
        .expect("offset shell model should slice successfully");

    assert!(
        !result.gcode.is_empty(),
        "G-code should be non-empty for offset shell model"
    );
}

// ---------------------------------------------------------------------------
// SC4: Repaired mesh output (resolved contours) passes validation
// ---------------------------------------------------------------------------

#[test]
fn sc4_resolved_contours_are_valid_polygons() {
    use slicecore_slicer::slice_at_height_resolved;

    let mesh = make_two_overlapping_cubes();

    // Check 5 Z-heights in the overlap region
    for z in [1.0, 3.0, 5.0, 7.0, 9.0] {
        let contours = slice_at_height_resolved(&mesh, z);
        assert!(
            !contours.is_empty(),
            "Should have at least 1 contour at z={:.1}",
            z
        );

        for (i, contour) in contours.iter().enumerate() {
            assert!(
                contour.len() >= 3,
                "Contour {} at z={:.1} should have >= 3 points, got {}",
                i,
                z,
                contour.len()
            );
            assert!(
                contour.area_mm2() > 0.0,
                "Contour {} at z={:.1} should have area > 0, got {}",
                i,
                z,
                contour.area_mm2()
            );
        }
    }
}

#[test]
fn sc4_resolved_contours_have_correct_winding() {
    use slicecore_slicer::slice_at_height_resolved;

    let mesh = make_two_overlapping_cubes();

    // Check multiple Z-heights
    for z in [2.0, 5.0, 8.0] {
        let contours = slice_at_height_resolved(&mesh, z);
        assert!(!contours.is_empty(), "Should have contours at z={:.1}", z);

        for (i, contour) in contours.iter().enumerate() {
            let winding = contour.winding();
            let signed_area = contour.area_i64();

            match winding {
                Winding::CounterClockwise => {
                    // Outer boundaries: CCW = positive area
                    assert!(
                        signed_area > 0,
                        "CCW contour {} at z={:.1} should have positive signed area, got {}",
                        i,
                        z,
                        signed_area
                    );
                }
                Winding::Clockwise => {
                    // Holes: CW = negative area
                    assert!(
                        signed_area < 0,
                        "CW contour {} at z={:.1} should have negative signed area, got {}",
                        i,
                        z,
                        signed_area
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SC5: Performance -- resolution completes in <5 seconds for <10k triangles
// ---------------------------------------------------------------------------

#[test]
fn sc5_performance_under_5_seconds() {
    use slicecore_mesh::repair::{intersect::detect_self_intersections, repair};
    use slicecore_slicer::slice_mesh_resolved;
    use std::time::Instant;

    // 400 cube pairs * 24 triangles/pair = 9600 triangles
    let n_pairs = 400;
    let mesh = make_large_overlapping_mesh(n_pairs);

    let triangle_count = mesh.triangle_count();
    assert!(
        triangle_count <= 10_000,
        "Mesh should have <10k triangles, got {}",
        triangle_count
    );
    assert!(
        triangle_count >= 5_000,
        "Mesh should have >=5k triangles for meaningful test, got {}",
        triangle_count
    );

    let start = Instant::now();

    // Step 1: Repair (detect self-intersections)
    let vertices = mesh.vertices().to_vec();
    let indices = mesh.indices().to_vec();
    let (_repaired_mesh, report) = repair(vertices, indices).expect("repair should succeed");
    let repair_elapsed = start.elapsed();

    // Step 2: Detect self-intersections on repaired mesh
    let detect_start = Instant::now();
    let intersection_count =
        detect_self_intersections(_repaired_mesh.vertices(), _repaired_mesh.indices());
    let detect_elapsed = detect_start.elapsed();

    // Step 3: Slice with resolution
    let slice_start = Instant::now();
    let layers = slice_mesh_resolved(&_repaired_mesh, 0.2, 0.3);
    let slice_elapsed = slice_start.elapsed();

    let total_elapsed = start.elapsed();

    // Print diagnostics
    eprintln!(
        "SC5 Performance: {} triangles, {} intersections, {} layers",
        triangle_count,
        intersection_count,
        layers.len()
    );
    eprintln!(
        "  Repair: {:.2}s, Detect: {:.2}s, Slice+Resolve: {:.2}s, Total: {:.2}s",
        repair_elapsed.as_secs_f64(),
        detect_elapsed.as_secs_f64(),
        slice_elapsed.as_secs_f64(),
        total_elapsed.as_secs_f64()
    );
    eprintln!(
        "  Repair report: {} intersections, resolvable={}",
        report.self_intersections_detected, report.self_intersections_resolvable
    );

    assert!(
        total_elapsed.as_secs() < 5,
        "Resolution pipeline (repair + detect + slice + resolve) should complete in <5 seconds, took {:.2}s",
        total_elapsed.as_secs_f64()
    );
}
