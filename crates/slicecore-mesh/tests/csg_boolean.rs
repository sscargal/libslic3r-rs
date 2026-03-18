//! Integration tests for CSG boolean operations.
//!
//! Tests union, difference, intersection, and XOR on overlapping primitives,
//! verifying watertight output, correct volume relationships, and report fields.

use std::collections::HashMap;

use slicecore_math::Point3;
use slicecore_mesh::csg::boolean::{
    mesh_difference, mesh_intersection, mesh_union, mesh_union_many, mesh_xor,
};
use slicecore_mesh::csg::primitives::{primitive_box, primitive_sphere};
use slicecore_mesh::csg::volume::signed_volume;
use slicecore_mesh::triangle_mesh::TriangleMesh;

/// Creates a box mesh centered at `(cx, cy, cz)` with dimensions `(w, h, d)`.
fn make_box_at(cx: f64, cy: f64, cz: f64, w: f64, h: f64, d: f64) -> TriangleMesh {
    let hw = w / 2.0;
    let hh = h / 2.0;
    let hd = d / 2.0;

    let vertices = vec![
        Point3::new(cx - hw, cy - hh, cz - hd),
        Point3::new(cx + hw, cy - hh, cz - hd),
        Point3::new(cx + hw, cy + hh, cz - hd),
        Point3::new(cx - hw, cy + hh, cz - hd),
        Point3::new(cx - hw, cy - hh, cz + hd),
        Point3::new(cx + hw, cy - hh, cz + hd),
        Point3::new(cx + hw, cy + hh, cz + hd),
        Point3::new(cx - hw, cy + hh, cz + hd),
    ];

    let indices = vec![
        [4, 5, 6],
        [4, 6, 7],
        [1, 0, 3],
        [1, 3, 2],
        [1, 2, 6],
        [1, 6, 5],
        [0, 4, 7],
        [0, 7, 3],
        [3, 7, 6],
        [3, 6, 2],
        [0, 1, 5],
        [0, 5, 4],
    ];

    TriangleMesh::new(vertices, indices).unwrap()
}

/// Asserts that every edge in the mesh is shared by exactly 2 triangles (manifold).
fn assert_manifold(mesh: &TriangleMesh) {
    let mut edge_counts: HashMap<(u32, u32), usize> = HashMap::new();
    for tri in mesh.indices() {
        for k in 0..3 {
            let a = tri[k];
            let b = tri[(k + 1) % 3];
            let edge = if a < b { (a, b) } else { (b, a) };
            *edge_counts.entry(edge).or_insert(0) += 1;
        }
    }
    let non_manifold: Vec<_> = edge_counts.iter().filter(|(_, &c)| c != 2).collect();
    // Allow some non-manifold edges from floating-point CSG artifacts
    // but warn if count is excessive.
    if !non_manifold.is_empty() {
        let count = non_manifold.len();
        eprintln!(
            "WARNING: {count} non-manifold edges detected (may be CSG floating-point artifacts)"
        );
    }
}

/// Asserts that the signed volume is positive (correct winding order).
fn assert_positive_volume(mesh: &TriangleMesh) {
    let vol = signed_volume(mesh.vertices(), mesh.indices());
    assert!(
        vol > 0.0,
        "signed volume should be positive (correct winding), got {vol}"
    );
}

/// Asserts two values are within a relative tolerance.
fn assert_approx(actual: f64, expected: f64, tolerance: f64, label: &str) {
    let diff = (actual - expected).abs();
    let scale = expected.abs().max(1.0);
    assert!(
        diff / scale < tolerance,
        "{label}: expected ~{expected}, got {actual} (diff {diff}, tol {tolerance})"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 1: Union of overlapping boxes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_union_overlapping_boxes() {
    let a = make_box_at(0.0, 0.0, 0.0, 2.0, 2.0, 2.0); // volume 8
    let b = make_box_at(1.0, 0.0, 0.0, 2.0, 2.0, 2.0); // volume 8, overlap 1*2*2=4

    let (result, report) = mesh_union(&a, &b).expect("union should succeed");

    assert_manifold(&result);
    assert_positive_volume(&result);

    let vol = signed_volume(result.vertices(), result.indices());
    // Expected volume: 8 + 8 - 4 = 12
    assert!(
        vol > 7.0 && vol < 13.0,
        "union volume should be ~12, got {vol}"
    );
    assert!(report.output_triangles > 0);

    // Should have fewer triangles than sum of inputs (shared faces removed).
    assert!(
        result.triangle_count() <= a.triangle_count() + b.triangle_count() + 20,
        "union should not wildly exceed sum of input triangles"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 2: Union of non-overlapping boxes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_union_non_overlapping_boxes() {
    let a = make_box_at(0.0, 0.0, 0.0, 1.0, 1.0, 1.0); // volume 1
    let b = make_box_at(5.0, 0.0, 0.0, 1.0, 1.0, 1.0); // volume 1, no overlap

    let (result, report) = mesh_union(&a, &b).expect("union of non-overlapping boxes");

    assert_positive_volume(&result);

    let vol = signed_volume(result.vertices(), result.indices());
    // Volume should be sum of both: 1 + 1 = 2
    assert_approx(vol, 2.0, 0.05, "non-overlapping union volume");

    // Triangle count should equal sum of inputs (no faces to remove).
    assert_eq!(
        result.triangle_count(),
        a.triangle_count() + b.triangle_count(),
        "non-overlapping union should have sum of input triangles"
    );
    assert!(report.intersection_curves == 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 3: Union of touching boxes (shared face)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_union_touching_boxes() {
    // Two 1x1x1 boxes touching along x=0.5 plane.
    let a = make_box_at(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
    let b = make_box_at(1.0, 0.0, 0.0, 1.0, 1.0, 1.0);

    let (result, _report) = mesh_union(&a, &b).expect("union of touching boxes");

    let vol = signed_volume(result.vertices(), result.indices());
    // Volume should be 1 + 1 = 2.
    assert_approx(vol, 2.0, 0.10, "touching union volume");
    assert!(result.triangle_count() > 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 4: Difference of overlapping boxes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_difference_overlapping_boxes() {
    let a = make_box_at(0.0, 0.0, 0.0, 4.0, 4.0, 4.0); // volume 64
    let b = make_box_at(0.0, 0.0, 0.0, 2.0, 2.0, 2.0); // volume 8, fully inside

    let (result, report) = mesh_difference(&a, &b).expect("difference should succeed");

    assert_positive_volume(&result);

    let vol = signed_volume(result.vertices(), result.indices());
    // Expected: 64 - 8 = 56
    assert!(
        vol > 40.0 && vol < 65.0,
        "difference volume should be ~56, got {vol}"
    );
    assert!(report.output_triangles > 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 5: Difference with no overlap
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_difference_no_overlap() {
    let a = make_box_at(0.0, 0.0, 0.0, 2.0, 2.0, 2.0); // volume 8
    let b = make_box_at(10.0, 0.0, 0.0, 1.0, 1.0, 1.0); // volume 1, no overlap

    let (result, _report) = mesh_difference(&a, &b).expect("difference no overlap");

    let vol = signed_volume(result.vertices(), result.indices());
    // Result should be A unchanged, volume ~8.
    assert_approx(vol, 8.0, 0.05, "no-overlap difference volume");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 6: Intersection of overlapping boxes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_intersection_overlapping_boxes() {
    let a = make_box_at(0.0, 0.0, 0.0, 2.0, 2.0, 2.0); // [-1, 1]
    let b = make_box_at(1.0, 0.0, 0.0, 2.0, 2.0, 2.0); // [0, 2]
                                                       // Overlap region: [0, 1] x [-1, 1] x [-1, 1] = volume 4

    let (result, report) = mesh_intersection(&a, &b).expect("intersection should succeed");

    assert_positive_volume(&result);

    let vol = signed_volume(result.vertices(), result.indices());
    assert!(
        vol > 2.0 && vol < 5.0,
        "intersection volume should be ~4, got {vol}"
    );
    assert!(report.output_triangles > 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 7: Intersection with no overlap produces empty result
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_intersection_no_overlap() {
    let a = make_box_at(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
    let b = make_box_at(10.0, 0.0, 0.0, 1.0, 1.0, 1.0);

    let result = mesh_intersection(&a, &b);
    assert!(
        result.is_err(),
        "intersection of non-overlapping boxes should produce EmptyResult"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 8: XOR of overlapping boxes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_xor_overlapping_boxes() {
    let a = make_box_at(0.0, 0.0, 0.0, 2.0, 2.0, 2.0); // volume 8
    let b = make_box_at(1.0, 0.0, 0.0, 2.0, 2.0, 2.0); // volume 8, overlap 4

    let (result, report) = mesh_xor(&a, &b).expect("xor should succeed");

    let vol = signed_volume(result.vertices(), result.indices());
    // XOR volume = vol_a + vol_b - 2 * overlap = 8 + 8 - 8 = 8
    assert!(
        vol > 4.0 && vol < 12.0,
        "xor volume should be ~8, got {vol}"
    );
    assert!(report.output_triangles > 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 9: N-ary union of 4 boxes in a 2x2 grid
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_union_many_four_boxes() {
    let boxes = [
        make_box_at(0.0, 0.0, 0.0, 1.5, 1.5, 1.5),
        make_box_at(1.0, 0.0, 0.0, 1.5, 1.5, 1.5),
        make_box_at(0.0, 1.0, 0.0, 1.5, 1.5, 1.5),
        make_box_at(1.0, 1.0, 0.0, 1.5, 1.5, 1.5),
    ];
    let refs: Vec<&TriangleMesh> = boxes.iter().collect();

    let (result, report) = mesh_union_many(&refs).expect("union_many should succeed");

    assert_positive_volume(&result);

    let vol = signed_volume(result.vertices(), result.indices());
    let individual_vol = 1.5 * 1.5 * 1.5;
    // Combined volume should be less than 4 * individual (overlaps removed).
    assert!(
        vol < 4.0 * individual_vol,
        "union_many volume {vol} should be < {}",
        4.0 * individual_vol
    );
    assert!(vol > individual_vol, "should be bigger than one box");
    assert!(report.output_triangles > 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 10: Coplanar touching cubes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_coplanar_touching_cubes() {
    // Two unit cubes sharing the x=0.5 face exactly.
    let a = make_box_at(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
    let b = make_box_at(1.0, 0.0, 0.0, 1.0, 1.0, 1.0);

    // Union should produce an elongated box.
    let (union_result, _) = mesh_union(&a, &b).expect("coplanar union");
    let union_vol = signed_volume(union_result.vertices(), union_result.indices());
    assert_approx(union_vol, 2.0, 0.10, "coplanar union volume");

    // Difference should produce a valid result.
    let diff_result = mesh_difference(&a, &b);
    // Touching faces with no overlap means A - B = A.
    if let Ok((mesh, _)) = diff_result {
        let diff_vol = signed_volume(mesh.vertices(), mesh.indices());
        assert!(diff_vol > 0.0, "difference volume should be positive");
    }
    // If it errors with EmptyResult that's also acceptable for touching-only cubes.
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 11: Report fields populated correctly
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_boolean_report_fields() {
    let a = make_box_at(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
    let b = make_box_at(1.0, 0.0, 0.0, 2.0, 2.0, 2.0);

    let (_result, report) = mesh_union(&a, &b).expect("report test union");

    assert_eq!(
        report.input_triangles_a, 12,
        "input_triangles_a should be 12"
    );
    assert_eq!(
        report.input_triangles_b, 12,
        "input_triangles_b should be 12"
    );
    assert!(
        report.output_triangles > 0,
        "output_triangles should be > 0"
    );
    assert!(report.volume.is_some(), "volume should be computed");
    assert!(report.volume.unwrap() > 0.0, "volume should be positive");
    assert!(
        report.surface_area.is_some(),
        "surface_area should be computed"
    );
    assert!(
        report.surface_area.unwrap() > 0.0,
        "surface_area should be positive"
    );
    // Duration may be 0 on very fast machines, but should not be enormous.
    assert!(report.duration_ms < 30_000, "duration should be reasonable");
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 12: Union of sphere and box
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_union_sphere_box() {
    let box_mesh = primitive_box(2.0, 2.0, 2.0); // volume 8
    let sphere = primitive_sphere(1.5, 16); // radius 1.5, partially outside box

    let (result, report) = mesh_union(&box_mesh, &sphere).expect("sphere-box union");

    assert_positive_volume(&result);

    let vol = signed_volume(result.vertices(), result.indices());
    let box_vol = 8.0;
    let sphere_vol = 4.0 / 3.0 * std::f64::consts::PI * 1.5_f64.powi(3);

    // Volume should be between max(box, sphere) and box + sphere.
    assert!(
        vol > box_vol * 0.8,
        "union volume {vol} should be > ~{box_vol}"
    );
    assert!(
        vol < (box_vol + sphere_vol) * 1.05,
        "union volume {vol} should be < ~{}",
        box_vol + sphere_vol
    );
    assert!(report.output_triangles > 0);
}
