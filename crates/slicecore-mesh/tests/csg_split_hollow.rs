//! Integration tests for plane split, mesh offset, and hollow operations.

use std::collections::HashMap;

use slicecore_mesh::csg::hollow::{hollow_mesh, DrainHole, HollowOptions};
use slicecore_mesh::csg::offset::mesh_offset;
use slicecore_mesh::csg::split::{mesh_split_at_plane, SplitOptions, SplitPlane};
use slicecore_mesh::csg::volume::{signed_volume, surface_area};
use slicecore_mesh::csg::{primitive_box, primitive_sphere};

use slicecore_math::{Point3, Vec3};

/// Checks that every edge in the mesh is shared by exactly 2 triangles (manifold).
fn is_watertight(mesh: &slicecore_mesh::TriangleMesh) -> bool {
    let mut edge_counts: HashMap<(u32, u32), usize> = HashMap::new();
    for tri in mesh.indices() {
        for k in 0..3 {
            let a = tri[k];
            let b = tri[(k + 1) % 3];
            let edge = if a < b { (a, b) } else { (b, a) };
            *edge_counts.entry(edge).or_insert(0) += 1;
        }
    }
    edge_counts.values().all(|&count| count == 2)
}

// ---------------------------------------------------------------------------
// Split tests
// ---------------------------------------------------------------------------

#[test]
fn test_split_box_at_midpoint() {
    let mesh = primitive_box(2.0, 2.0, 2.0);
    let plane = SplitPlane::xy(0.0); // Split at z=0.
    let result = mesh_split_at_plane(&mesh, &plane, &SplitOptions::default()).unwrap();

    // Both halves should have triangles.
    assert!(
        result.above.triangle_count() > 0,
        "above half should have triangles"
    );
    assert!(
        result.below.triangle_count() > 0,
        "below half should have triangles"
    );

    // Both halves should be watertight (capped).
    assert!(is_watertight(&result.above), "above half should be watertight");
    assert!(is_watertight(&result.below), "below half should be watertight");

    // Each half should have the correct bounding box extent.
    let above_aabb = result.above.aabb();
    let below_aabb = result.below.aabb();

    // Above half: z should be >= 0 (approximately).
    assert!(
        above_aabb.min.z >= -0.1,
        "above half min z ({}) should be near 0",
        above_aabb.min.z
    );
    assert!(
        (above_aabb.max.z - 1.0).abs() < 0.1,
        "above half max z ({}) should be near 1.0",
        above_aabb.max.z
    );

    // Below half: z should be <= 0 (approximately).
    assert!(
        below_aabb.max.z <= 0.1,
        "below half max z ({}) should be near 0",
        below_aabb.max.z
    );
    assert!(
        (below_aabb.min.z - (-1.0)).abs() < 0.1,
        "below half min z ({}) should be near -1.0",
        below_aabb.min.z
    );

    // Combined volume should approximate the original.
    let above_vol = signed_volume(result.above.vertices(), result.above.indices()).abs();
    let below_vol = signed_volume(result.below.vertices(), result.below.indices()).abs();
    let original_vol = signed_volume(mesh.vertices(), mesh.indices()).abs();
    let combined = above_vol + below_vol;
    assert!(
        (combined - original_vol).abs() / original_vol < 0.15,
        "combined volume ({combined}) should approximate original ({original_vol})"
    );
}

#[test]
fn test_split_box_uncapped() {
    let mesh = primitive_box(2.0, 2.0, 2.0);
    let plane = SplitPlane::xy(0.0);
    let uncapped = mesh_split_at_plane(&mesh, &plane, &SplitOptions { cap: false }).unwrap();

    assert!(uncapped.above.triangle_count() > 0);
    assert!(uncapped.below.triangle_count() > 0);

    // Uncapped halves should NOT be watertight.
    assert!(
        !is_watertight(&uncapped.above),
        "uncapped above should NOT be watertight"
    );
    assert!(
        !is_watertight(&uncapped.below),
        "uncapped below should NOT be watertight"
    );

    // Capped version should have more triangles.
    let capped = mesh_split_at_plane(&mesh, &plane, &SplitOptions::default()).unwrap();
    let uncapped_total =
        uncapped.above.triangle_count() + uncapped.below.triangle_count();
    let capped_total = capped.above.triangle_count() + capped.below.triangle_count();
    assert!(
        capped_total >= uncapped_total,
        "capped ({capped_total}) should have >= triangles than uncapped ({uncapped_total})"
    );
}

#[test]
fn test_split_sphere_at_equator() {
    let mesh = primitive_sphere(1.0, 32);
    let plane = SplitPlane::xy(0.0); // Split at z=0 (equator).
    let result = mesh_split_at_plane(&mesh, &plane, &SplitOptions::default()).unwrap();

    assert!(result.above.triangle_count() > 0);
    assert!(result.below.triangle_count() > 0);

    // Both halves should be watertight.
    assert!(
        is_watertight(&result.above),
        "above hemisphere should be watertight"
    );
    assert!(
        is_watertight(&result.below),
        "below hemisphere should be watertight"
    );

    // Each hemisphere should have approximately half the volume.
    let above_vol = signed_volume(result.above.vertices(), result.above.indices()).abs();
    let below_vol = signed_volume(result.below.vertices(), result.below.indices()).abs();
    let total_vol = signed_volume(mesh.vertices(), mesh.indices()).abs();

    assert!(
        (above_vol - total_vol / 2.0).abs() / total_vol < 0.2,
        "above hemisphere volume ({above_vol}) should be ~half of total ({total_vol})"
    );
    assert!(
        (below_vol - total_vol / 2.0).abs() / total_vol < 0.2,
        "below hemisphere volume ({below_vol}) should be ~half of total ({total_vol})"
    );
}

#[test]
fn test_split_no_intersection() {
    let mesh = primitive_box(2.0, 2.0, 2.0);
    // Plane well above the box (box goes from z=-1 to z=1).
    let plane = SplitPlane::xy(10.0);
    let result = mesh_split_at_plane(&mesh, &plane, &SplitOptions::default());

    // The plane doesn't intersect the mesh. One half should be the full mesh,
    // the other should be essentially empty.
    // Our implementation puts the full mesh in "below" since all vertices are below z=10.
    match result {
        Ok(r) => {
            // One half has all triangles, the other has very few (or a placeholder).
            let above_count = r.above.triangle_count();
            let below_count = r.below.triangle_count();
            assert!(
                above_count <= 1 || below_count <= 1,
                "one half should be empty/minimal: above={above_count}, below={below_count}"
            );
        }
        Err(_) => {
            // Also acceptable: error returned for no-intersection case.
        }
    }
}

#[test]
fn test_split_at_arbitrary_angle() {
    let mesh = primitive_box(2.0, 2.0, 2.0);
    // Diagonal plane through the center.
    let plane = SplitPlane::new(Vec3::new(1.0, 1.0, 0.0), 0.0);
    let result = mesh_split_at_plane(&mesh, &plane, &SplitOptions::default()).unwrap();

    assert!(
        result.above.triangle_count() > 0,
        "diagonal above should have triangles"
    );
    assert!(
        result.below.triangle_count() > 0,
        "diagonal below should have triangles"
    );

    // Both halves should be watertight.
    assert!(
        is_watertight(&result.above),
        "diagonal above should be watertight"
    );
    assert!(
        is_watertight(&result.below),
        "diagonal below should be watertight"
    );
}

// ---------------------------------------------------------------------------
// Hollow tests
// ---------------------------------------------------------------------------

#[test]
fn test_hollow_box() {
    let mesh = primitive_box(10.0, 10.0, 10.0);
    let original_vol = signed_volume(mesh.vertices(), mesh.indices());
    let opts = HollowOptions {
        wall_thickness: 2.0,
        drain_hole: None,
    };
    let (result, report) = hollow_mesh(&mesh, &opts).unwrap();

    assert!(result.triangle_count() > 0);

    // Volume should be less than original.
    let hollow_vol = report.volume.unwrap();
    assert!(
        hollow_vol < original_vol,
        "hollow volume ({hollow_vol}) should be less than original ({original_vol})"
    );
    assert!(
        hollow_vol > 0.0,
        "hollow volume ({hollow_vol}) should be positive"
    );
}

#[test]
fn test_hollow_sphere() {
    let mesh = primitive_sphere(5.0, 16);
    let original_vol = signed_volume(mesh.vertices(), mesh.indices()).abs();
    let opts = HollowOptions {
        wall_thickness: 1.0,
        drain_hole: None,
    };
    let (result, report) = hollow_mesh(&mesh, &opts).unwrap();

    assert!(result.triangle_count() > 0);
    let hollow_vol = report.volume.unwrap().abs();
    assert!(
        hollow_vol < original_vol,
        "hollow sphere volume ({hollow_vol}) should be less than original ({original_vol})"
    );
}

#[test]
fn test_hollow_with_drain_hole() {
    let mesh = primitive_box(10.0, 10.0, 10.0);
    let opts_no_drain = HollowOptions {
        wall_thickness: 2.0,
        drain_hole: None,
    };
    let (no_drain, _) = hollow_mesh(&mesh, &opts_no_drain).unwrap();

    let opts_drain = HollowOptions {
        wall_thickness: 2.0,
        drain_hole: Some(DrainHole {
            position: Point3::new(0.0, 0.0, -5.0),
            direction: Vec3::new(0.0, 0.0, -1.0),
            diameter: 3.0,
            tapered: false,
        }),
    };
    let (with_drain, _) = hollow_mesh(&mesh, &opts_drain).unwrap();

    // With drain hole should have more triangles (the hole adds geometry).
    assert!(
        with_drain.triangle_count() >= no_drain.triangle_count(),
        "drain hole version ({}) should have >= triangles than no-drain ({})",
        with_drain.triangle_count(),
        no_drain.triangle_count()
    );
}

#[test]
fn test_hollow_thin_wall_warning() {
    let mesh = primitive_box(4.0, 4.0, 4.0);
    // Wall thickness = 3.0 > 50% of smallest dimension (4.0) -- should warn.
    let opts = HollowOptions {
        wall_thickness: 3.0,
        drain_hole: None,
    };
    let result = hollow_mesh(&mesh, &opts);
    match result {
        Ok((_, report)) => {
            let has_warning = report
                .warnings
                .iter()
                .any(|w| w.contains("wall thickness") && w.contains("50%"));
            assert!(
                has_warning,
                "should warn about thick walls. Warnings: {:?}",
                report.warnings
            );
        }
        Err(_) => {
            // Also acceptable: thick walls may cause the operation to fail.
        }
    }
}

// ---------------------------------------------------------------------------
// Offset tests
// ---------------------------------------------------------------------------

#[test]
fn test_offset_box_positive() {
    let mesh = primitive_box(2.0, 2.0, 2.0);
    let original_vol = signed_volume(mesh.vertices(), mesh.indices());
    let original_area = surface_area(mesh.vertices(), mesh.indices());

    let (result, report) = mesh_offset(&mesh, 0.5).unwrap();

    // Volume should increase.
    let new_vol = signed_volume(result.vertices(), result.indices());
    assert!(
        new_vol > original_vol,
        "positive offset volume ({new_vol}) should exceed original ({original_vol})"
    );

    // Surface area should increase.
    let new_area = report.surface_area.unwrap();
    assert!(
        new_area > original_area,
        "positive offset area ({new_area}) should exceed original ({original_area})"
    );

    // Bounding box should be larger.
    let orig_aabb = mesh.aabb();
    let new_aabb = result.aabb();
    assert!(
        new_aabb.max.x > orig_aabb.max.x,
        "offset should grow in +X"
    );
    assert!(
        new_aabb.min.x < orig_aabb.min.x,
        "offset should grow in -X"
    );
}

#[test]
fn test_offset_box_negative() {
    let mesh = primitive_box(4.0, 4.0, 4.0);
    let original_vol = signed_volume(mesh.vertices(), mesh.indices());

    let (result, _) = mesh_offset(&mesh, -0.5).unwrap();

    // Volume should decrease.
    let new_vol = signed_volume(result.vertices(), result.indices());
    assert!(
        new_vol < original_vol,
        "negative offset volume ({new_vol}) should be less than original ({original_vol})"
    );

    // Bounding box should be smaller.
    let orig_aabb = mesh.aabb();
    let new_aabb = result.aabb();
    assert!(
        new_aabb.max.x < orig_aabb.max.x,
        "negative offset should shrink in +X"
    );
}
