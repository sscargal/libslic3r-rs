//! Integration tests for the mesh repair pipeline with known-defect meshes.

use slicecore_math::Point3;
use slicecore_mesh::{compute_stats, repair};

// ---------------------------------------------------------------------------
// Helpers: create meshes with known defects
// ---------------------------------------------------------------------------

/// Unit cube (8 vertices, 12 triangles) -- valid closed mesh.
fn make_unit_cube() -> (Vec<Point3>, Vec<[u32; 3]>) {
    let vertices = vec![
        Point3::new(0.0, 0.0, 0.0), // 0
        Point3::new(1.0, 0.0, 0.0), // 1
        Point3::new(1.0, 1.0, 0.0), // 2
        Point3::new(0.0, 1.0, 0.0), // 3
        Point3::new(0.0, 0.0, 1.0), // 4
        Point3::new(1.0, 0.0, 1.0), // 5
        Point3::new(1.0, 1.0, 1.0), // 6
        Point3::new(0.0, 1.0, 1.0), // 7
    ];

    // Two triangles per face, 6 faces = 12 triangles.
    // CCW winding when viewed from outside.
    let indices = vec![
        // Front face (z=1)
        [4, 5, 6],
        [4, 6, 7],
        // Back face (z=0)
        [1, 0, 3],
        [1, 3, 2],
        // Right face (x=1)
        [1, 2, 6],
        [1, 6, 5],
        // Left face (x=0)
        [0, 4, 7],
        [0, 7, 3],
        // Top face (y=1)
        [3, 7, 6],
        [3, 6, 2],
        // Bottom face (y=0)
        [0, 1, 5],
        [0, 5, 4],
    ];

    (vertices, indices)
}

/// Unit cube plus one degenerate triangle (two identical vertex indices).
fn make_cube_with_degenerate() -> (Vec<Point3>, Vec<[u32; 3]>) {
    let (vertices, mut indices) = make_unit_cube();
    // Add a degenerate triangle: vertex 0 repeated twice
    indices.push([0, 0, 1]);
    (vertices, indices)
}

/// Unit cube with one triangle's winding reversed (inconsistent normal).
fn make_cube_with_flipped_normal() -> (Vec<Point3>, Vec<[u32; 3]>) {
    let (vertices, mut indices) = make_unit_cube();
    // Flip triangle 0: [4,5,6] -> [4,6,5] (reversed winding)
    indices[0] = [4, 6, 5];
    (vertices, indices)
}

/// Unit cube with one face (2 triangles) removed, creating a hole.
fn make_cube_with_hole() -> (Vec<Point3>, Vec<[u32; 3]>) {
    let (vertices, mut indices) = make_unit_cube();
    // Remove the front face (first 2 triangles: indices 0 and 1)
    indices.remove(0);
    indices.remove(0);
    (vertices, indices)
}

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

#[test]
fn repair_removes_degenerate_triangles() {
    let (vertices, indices) = make_cube_with_degenerate();
    assert_eq!(indices.len(), 13, "should have 12 + 1 degenerate");

    let (mesh, report) = repair(vertices, indices).expect("repair should succeed");

    assert_eq!(report.degenerate_removed, 1, "should remove 1 degenerate");
    assert_eq!(mesh.triangle_count(), 12, "should have 12 triangles after repair");
}

#[test]
fn repair_fixes_normals() {
    let (vertices, indices) = make_cube_with_flipped_normal();

    let (mesh, report) = repair(vertices, indices).expect("repair should succeed");

    assert!(
        report.normals_fixed >= 1,
        "should fix at least 1 normal, got {}",
        report.normals_fixed
    );
    assert_eq!(mesh.triangle_count(), 12);
}

#[test]
fn repair_fills_holes() {
    let (vertices, indices) = make_cube_with_hole();
    assert_eq!(indices.len(), 10, "should have 12 - 2 = 10 triangles");

    let (mesh, report) = repair(vertices, indices).expect("repair should succeed");

    assert!(
        report.holes_filled >= 2,
        "should fill at least 2 triangles for one quad hole, got {}",
        report.holes_filled
    );
    // After filling, the mesh should have at least 12 triangles
    assert!(
        mesh.triangle_count() >= 12,
        "should have >= 12 triangles after hole fill, got {}",
        mesh.triangle_count()
    );
}

#[test]
fn repair_clean_mesh_reports_already_clean() {
    let (vertices, indices) = make_unit_cube();

    let (_mesh, report) = repair(vertices, indices).expect("repair should succeed");

    // A valid unit cube should require no repairs (or at most trivial ones)
    assert_eq!(
        report.degenerate_removed, 0,
        "clean cube: no degenerates"
    );
    assert_eq!(
        report.holes_filled, 0,
        "clean cube: no holes"
    );
    // The clean cube may or may not have was_already_clean=true depending on
    // whether the normal fixer or stitcher made any changes. At minimum,
    // degenerate + holes should be zero.
}

#[test]
fn repair_then_stats_positive_volume() {
    // Repair a defective mesh (degenerate + flipped normals), then verify
    // the result has positive volume (indicating correct outward winding).
    let (vertices, mut indices) = make_unit_cube();
    // Introduce defects
    indices[2] = [indices[2][0], indices[2][2], indices[2][1]]; // flip one
    indices.push([0, 0, 3]); // add degenerate

    let (mesh, report) = repair(vertices, indices).expect("repair should succeed");

    assert!(report.degenerate_removed >= 1, "should remove degenerate");

    let stats = compute_stats(&mesh);
    assert!(
        stats.volume > 0.0,
        "volume should be positive after repair (correct winding), got {}",
        stats.volume
    );
    assert!(
        (stats.volume - 1.0).abs() < 0.5,
        "volume should be approximately 1.0 for unit cube, got {}",
        stats.volume
    );
}
