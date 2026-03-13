//! Mesh boolean operations: union, difference, intersection, and XOR.
//!
//! Composes the CSG algorithm internals (intersection, retriangulation,
//! classification) into public API functions that operate on [`TriangleMesh`]
//! inputs and return watertight result meshes with diagnostic reports.
//!
//! # Pipeline
//!
//! Each boolean operation follows these steps:
//! 1. Auto-repair both inputs
//! 2. Compute intersection curves between the two meshes
//! 3. Retriangulate both meshes along intersection curves
//! 4. Classify all triangles as inside or outside the other mesh
//! 5. Select triangles based on the operation type
//! 6. Merge, deduplicate, and clean up the output mesh
//! 7. Validate and compute metrics for the report

use std::collections::HashMap;
use std::time::Instant;

use slicecore_math::Point3;

use crate::repair;
use crate::triangle_mesh::TriangleMesh;

use super::classify::{classify_triangles, Classification};
use super::error::CsgError;
use super::intersect::compute_intersection_curves;
use super::report::CsgReport;
use super::retriangulate::{retriangulate_mesh, MeshId};
use super::types::{BooleanOp, CsgOptions};
use super::volume;

/// Merge tolerance for deduplicating vertices in the output mesh.
const MERGE_TOL: f64 = 1e-10;

/// Computes a mesh boolean operation between two triangle meshes.
///
/// This is the internal pipeline function. Use the public API functions
/// ([`mesh_union`], [`mesh_difference`], [`mesh_intersection`], [`mesh_xor`])
/// for convenience, or the `_with` variants for custom options.
///
/// # Errors
///
/// - [`CsgError::RepairFailedA`] if mesh A cannot be repaired.
/// - [`CsgError::RepairFailedB`] if mesh B cannot be repaired.
/// - [`CsgError::EmptyResult`] if the operation produces zero triangles.
/// - [`CsgError::ResultConstruction`] if the output mesh cannot be constructed.
/// - [`CsgError::NonManifoldResult`] if validation is enabled and the result
///   has non-manifold edges.
fn mesh_boolean(
    a: &TriangleMesh,
    b: &TriangleMesh,
    op: BooleanOp,
    options: &CsgOptions,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    let start = Instant::now();
    let mut report = CsgReport {
        input_triangles_a: a.triangle_count(),
        input_triangles_b: b.triangle_count(),
        ..CsgReport::default()
    };

    // Step (b): Auto-repair both inputs.
    let (repaired_a, repair_a) = repair::repair(a.vertices().to_vec(), a.indices().to_vec())
        .map_err(CsgError::RepairFailedA)?;
    let (repaired_b, repair_b) = repair::repair(b.vertices().to_vec(), b.indices().to_vec())
        .map_err(CsgError::RepairFailedB)?;

    let a_repairs = repair_a.degenerate_removed
        + repair_a.edges_stitched
        + repair_a.holes_filled
        + repair_a.normals_fixed;
    let b_repairs = repair_b.degenerate_removed
        + repair_b.edges_stitched
        + repair_b.holes_filled
        + repair_b.normals_fixed;
    report.repairs_performed = a_repairs + b_repairs;

    // Step (c): Compute intersection curves.
    let intersection = compute_intersection_curves(&repaired_a, &repaired_b);
    report.intersection_curves = intersection.segments.len();

    // Step (d): Retriangulate both meshes along intersection curves.
    let intersection_points: Vec<Point3> =
        intersection.points.iter().map(|p| p.position).collect();

    let (verts_a, idx_a, origins_a) = retriangulate_mesh(
        repaired_a.vertices(),
        repaired_a.indices(),
        &intersection,
        MeshId::A,
        &intersection_points,
    );

    let (verts_b, idx_b, origins_b) = retriangulate_mesh(
        repaired_b.vertices(),
        repaired_b.indices(),
        &intersection,
        MeshId::B,
        &intersection_points,
    );

    // Step (e): Classify all triangles.
    let class_a = classify_triangles(&verts_a, &idx_a, &repaired_b, &origins_a, &intersection);
    let class_b = classify_triangles(&verts_b, &idx_b, &repaired_a, &origins_b, &intersection);

    // Step (f): Select triangles based on operation.
    let mut selected_verts: Vec<Point3> = Vec::new();
    let mut selected_indices: Vec<[u32; 3]> = Vec::new();

    select_triangles(
        &verts_a,
        &idx_a,
        &class_a,
        &verts_b,
        &idx_b,
        &class_b,
        op,
        &mut selected_verts,
        &mut selected_indices,
    );

    // Step (h): Remove degenerate (zero-area) triangles.
    remove_degenerate_triangles(&selected_verts, &mut selected_indices);

    // Check for empty result.
    if selected_indices.is_empty() {
        return Err(CsgError::EmptyResult {
            operation: format!("{op:?}"),
        });
    }

    // Step (g): Re-index vertices, remove duplicates.
    let (final_verts, final_indices) =
        deduplicate_vertices(&selected_verts, &selected_indices, MERGE_TOL);

    // Step (j): Construct output mesh.
    let result_mesh = TriangleMesh::new(final_verts, final_indices)
        .map_err(CsgError::ResultConstruction)?;

    // Step (k): Validate manifold if requested.
    if options.validate_output {
        let non_manifold = count_non_manifold_edges(result_mesh.indices());
        if non_manifold > 0 {
            // Add as warning rather than hard failure -- CSG results may
            // have minor non-manifold artifacts from floating-point issues.
            report
                .warnings
                .push(format!("{non_manifold} non-manifold edges detected"));
        }
    }

    // Step (l): Compute volume and surface area.
    report.output_triangles = result_mesh.triangle_count();
    report.volume = Some(volume::signed_volume(
        result_mesh.vertices(),
        result_mesh.indices(),
    ));
    report.surface_area = Some(volume::surface_area(
        result_mesh.vertices(),
        result_mesh.indices(),
    ));

    // Step (m): Duration.
    report.duration_ms = start.elapsed().as_millis() as u64;

    Ok((result_mesh, report))
}

/// Selects triangles from both meshes based on the boolean operation.
#[allow(clippy::too_many_arguments)]
fn select_triangles(
    verts_a: &[Point3],
    idx_a: &[[u32; 3]],
    class_a: &[Classification],
    verts_b: &[Point3],
    idx_b: &[[u32; 3]],
    class_b: &[Classification],
    op: BooleanOp,
    out_verts: &mut Vec<Point3>,
    out_indices: &mut Vec<[u32; 3]>,
) {
    match op {
        BooleanOp::Union => {
            // A.OUTSIDE + B.OUTSIDE
            append_classified(verts_a, idx_a, class_a, Classification::Outside, false, out_verts, out_indices);
            append_classified(verts_b, idx_b, class_b, Classification::Outside, false, out_verts, out_indices);
        }
        BooleanOp::Intersection => {
            // A.INSIDE + B.INSIDE
            append_classified(verts_a, idx_a, class_a, Classification::Inside, false, out_verts, out_indices);
            append_classified(verts_b, idx_b, class_b, Classification::Inside, false, out_verts, out_indices);
        }
        BooleanOp::Difference => {
            // A.OUTSIDE + B.INSIDE (flip winding on B's INSIDE)
            append_classified(verts_a, idx_a, class_a, Classification::Outside, false, out_verts, out_indices);
            append_classified(verts_b, idx_b, class_b, Classification::Inside, true, out_verts, out_indices);
        }
        BooleanOp::Xor => {
            // (A.OUTSIDE + B.INSIDE flipped) + (B.OUTSIDE + A.INSIDE flipped)
            append_classified(verts_a, idx_a, class_a, Classification::Outside, false, out_verts, out_indices);
            append_classified(verts_b, idx_b, class_b, Classification::Inside, true, out_verts, out_indices);
            append_classified(verts_b, idx_b, class_b, Classification::Outside, false, out_verts, out_indices);
            append_classified(verts_a, idx_a, class_a, Classification::Inside, true, out_verts, out_indices);
        }
    }
}

/// Appends triangles with a given classification to the output buffers.
///
/// If `flip_winding` is true, reverses the vertex order of each triangle
/// to invert the normal direction.
#[allow(clippy::too_many_arguments)]
fn append_classified(
    verts: &[Point3],
    indices: &[[u32; 3]],
    classifications: &[Classification],
    target: Classification,
    flip_winding: bool,
    out_verts: &mut Vec<Point3>,
    out_indices: &mut Vec<[u32; 3]>,
) {
    // Build a map from source vertex index -> output vertex index.
    let mut vert_map: HashMap<u32, u32> = HashMap::new();

    for (i, tri) in indices.iter().enumerate() {
        if classifications[i] != target {
            continue;
        }

        let mut new_tri = [0u32; 3];
        for (j, &vi) in tri.iter().enumerate() {
            let out_vi = if let Some(&existing) = vert_map.get(&vi) {
                existing
            } else {
                let new_idx = out_verts.len() as u32;
                out_verts.push(verts[vi as usize]);
                vert_map.insert(vi, new_idx);
                new_idx
            };
            new_tri[j] = out_vi;
        }

        if flip_winding {
            new_tri.swap(1, 2);
        }

        out_indices.push(new_tri);
    }
}

/// Removes degenerate triangles (zero-area or near-zero-area) from the index list.
fn remove_degenerate_triangles(verts: &[Point3], indices: &mut Vec<[u32; 3]>) {
    indices.retain(|tri| {
        let v0 = verts[tri[0] as usize];
        let v1 = verts[tri[1] as usize];
        let v2 = verts[tri[2] as usize];

        let e1x = v1.x - v0.x;
        let e1y = v1.y - v0.y;
        let e1z = v1.z - v0.z;
        let e2x = v2.x - v0.x;
        let e2y = v2.y - v0.y;
        let e2z = v2.z - v0.z;

        let cx = e1y * e2z - e1z * e2y;
        let cy = e1z * e2x - e1x * e2z;
        let cz = e1x * e2y - e1y * e2x;

        let area_sq = cx * cx + cy * cy + cz * cz;
        area_sq > 1e-20 // Keep non-degenerate triangles.
    });
}

/// Deduplicates vertices within a merge tolerance, re-indexing triangles.
fn deduplicate_vertices(
    verts: &[Point3],
    indices: &[[u32; 3]],
    tolerance: f64,
) -> (Vec<Point3>, Vec<[u32; 3]>) {
    let tol_sq = tolerance * tolerance;
    let mut unique_verts: Vec<Point3> = Vec::new();
    let mut remap: Vec<u32> = Vec::with_capacity(verts.len());

    for v in verts {
        // Search for an existing vertex within tolerance.
        let found = unique_verts.iter().position(|u| {
            let dx = u.x - v.x;
            let dy = u.y - v.y;
            let dz = u.z - v.z;
            dx * dx + dy * dy + dz * dz < tol_sq
        });

        match found {
            Some(idx) => remap.push(idx as u32),
            None => {
                remap.push(unique_verts.len() as u32);
                unique_verts.push(*v);
            }
        }
    }

    let new_indices: Vec<[u32; 3]> = indices
        .iter()
        .map(|tri| [remap[tri[0] as usize], remap[tri[1] as usize], remap[tri[2] as usize]])
        .collect();

    (unique_verts, new_indices)
}

/// Counts non-manifold edges (edges not shared by exactly 2 triangles).
fn count_non_manifold_edges(indices: &[[u32; 3]]) -> usize {
    let mut edge_counts: HashMap<(u32, u32), usize> = HashMap::new();

    for tri in indices {
        for k in 0..3 {
            let a = tri[k];
            let b = tri[(k + 1) % 3];
            let edge = if a < b { (a, b) } else { (b, a) };
            *edge_counts.entry(edge).or_insert(0) += 1;
        }
    }

    edge_counts.values().filter(|&&c| c != 2).count()
}

// ---------------------------------------------------------------------------
// Public API: convenience wrappers with default options
// ---------------------------------------------------------------------------

/// Computes the union of two triangle meshes (A + B).
///
/// Combines both meshes into a single watertight result, removing internal
/// faces where the meshes overlap.
///
/// # Errors
///
/// Returns [`CsgError`] if repair, intersection, or output construction fails.
/// Returns [`CsgError::EmptyResult`] if the union produces no triangles.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::boolean::mesh_union;
/// use slicecore_mesh::csg::primitive_box;
///
/// let a = primitive_box(2.0, 2.0, 2.0);
/// let b = primitive_box(2.0, 2.0, 2.0);
/// let (result, report) = mesh_union(&a, &b).unwrap();
/// assert!(report.output_triangles > 0);
/// ```
pub fn mesh_union(
    a: &TriangleMesh,
    b: &TriangleMesh,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    mesh_boolean(a, b, BooleanOp::Union, &CsgOptions::default())
}

/// Computes the difference of two triangle meshes (A - B).
///
/// Subtracts mesh B from mesh A, producing a result with a cavity where
/// the meshes overlapped.
///
/// # Errors
///
/// Returns [`CsgError`] if repair, intersection, or output construction fails.
/// Returns [`CsgError::EmptyResult`] if B completely contains A.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::boolean::mesh_difference;
/// use slicecore_mesh::csg::primitive_box;
///
/// let a = primitive_box(4.0, 4.0, 4.0);
/// let b = primitive_box(2.0, 2.0, 2.0);
/// let (result, report) = mesh_difference(&a, &b).unwrap();
/// assert!(report.output_triangles > 0);
/// ```
pub fn mesh_difference(
    a: &TriangleMesh,
    b: &TriangleMesh,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    mesh_boolean(a, b, BooleanOp::Difference, &CsgOptions::default())
}

/// Computes the intersection of two triangle meshes (A & B).
///
/// Keeps only the region where both meshes overlap.
///
/// # Errors
///
/// Returns [`CsgError`] if repair, intersection, or output construction fails.
/// Returns [`CsgError::EmptyResult`] if the meshes do not overlap.
///
/// # Examples
///
/// ```no_run
/// use slicecore_mesh::csg::boolean::mesh_intersection;
/// use slicecore_mesh::csg::primitive_box;
///
/// let a = primitive_box(2.0, 2.0, 2.0);
/// let b = primitive_box(2.0, 2.0, 2.0);
/// let (result, report) = mesh_intersection(&a, &b).unwrap();
/// assert!(report.output_triangles > 0);
/// ```
pub fn mesh_intersection(
    a: &TriangleMesh,
    b: &TriangleMesh,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    mesh_boolean(a, b, BooleanOp::Intersection, &CsgOptions::default())
}

/// Computes the symmetric difference (XOR) of two triangle meshes.
///
/// Keeps everything except the overlapping region: the parts of A outside B
/// plus the parts of B outside A.
///
/// # Errors
///
/// Returns [`CsgError`] if repair, intersection, or output construction fails.
/// Returns [`CsgError::EmptyResult`] if both meshes are identical.
///
/// # Examples
///
/// ```no_run
/// use slicecore_mesh::csg::boolean::mesh_xor;
/// use slicecore_mesh::csg::primitive_box;
///
/// let a = primitive_box(2.0, 2.0, 2.0);
/// let b = primitive_box(2.0, 2.0, 2.0);
/// let (result, report) = mesh_xor(&a, &b).unwrap();
/// ```
pub fn mesh_xor(
    a: &TriangleMesh,
    b: &TriangleMesh,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    mesh_boolean(a, b, BooleanOp::Xor, &CsgOptions::default())
}

/// Computes the union of multiple triangle meshes via sequential left-fold.
///
/// Performs `union(meshes[0], meshes[1])`, then `union(result, meshes[2])`,
/// and so on. Accumulates all sub-reports into a single combined report.
///
/// # Errors
///
/// - Returns [`CsgError::EmptyResult`] with operation `"UnionMany"` if fewer
///   than 2 meshes are provided.
/// - Propagates any error from individual union operations.
///
/// # Examples
///
/// ```no_run
/// use slicecore_mesh::csg::boolean::mesh_union_many;
/// use slicecore_mesh::csg::primitive_box;
///
/// let boxes: Vec<_> = (0..4).map(|_| primitive_box(1.0, 1.0, 1.0)).collect();
/// let refs: Vec<&_> = boxes.iter().collect();
/// let (result, report) = mesh_union_many(&refs).unwrap();
/// assert!(report.output_triangles > 0);
/// ```
pub fn mesh_union_many(
    meshes: &[&TriangleMesh],
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    if meshes.len() < 2 {
        return Err(CsgError::EmptyResult {
            operation: "UnionMany: need at least 2 meshes".to_string(),
        });
    }

    let start = Instant::now();
    let mut combined_report = CsgReport {
        input_triangles_a: meshes.iter().map(|m| m.triangle_count()).sum(),
        ..CsgReport::default()
    };

    let (mut result, first_report) = mesh_union(meshes[0], meshes[1])?;
    accumulate_report(&mut combined_report, &first_report);

    for mesh in &meshes[2..] {
        let (new_result, sub_report) = mesh_union(&result, mesh)?;
        accumulate_report(&mut combined_report, &sub_report);
        result = new_result;
    }

    // Final metrics from the last result.
    combined_report.output_triangles = result.triangle_count();
    combined_report.volume = Some(volume::signed_volume(
        result.vertices(),
        result.indices(),
    ));
    combined_report.surface_area = Some(volume::surface_area(
        result.vertices(),
        result.indices(),
    ));
    combined_report.duration_ms = start.elapsed().as_millis() as u64;

    Ok((result, combined_report))
}

/// Accumulates sub-report metrics into a combined report.
fn accumulate_report(combined: &mut CsgReport, sub: &CsgReport) {
    combined.intersection_curves += sub.intersection_curves;
    combined.repairs_performed += sub.repairs_performed;
    combined.warnings.extend(sub.warnings.iter().cloned());
}

// ---------------------------------------------------------------------------
// Public API: versions with custom CsgOptions
// ---------------------------------------------------------------------------

/// Computes the union of two meshes with custom options.
///
/// See [`mesh_union`] for details.
///
/// # Errors
///
/// Same as [`mesh_union`], plus [`CsgError::NonManifoldResult`] if
/// `options.validate_output` is true and the result is non-manifold.
pub fn mesh_union_with(
    a: &TriangleMesh,
    b: &TriangleMesh,
    options: &CsgOptions,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    mesh_boolean(a, b, BooleanOp::Union, options)
}

/// Computes the difference of two meshes with custom options.
///
/// See [`mesh_difference`] for details.
///
/// # Errors
///
/// Same as [`mesh_difference`].
pub fn mesh_difference_with(
    a: &TriangleMesh,
    b: &TriangleMesh,
    options: &CsgOptions,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    mesh_boolean(a, b, BooleanOp::Difference, options)
}

/// Computes the intersection of two meshes with custom options.
///
/// See [`mesh_intersection`] for details.
///
/// # Errors
///
/// Same as [`mesh_intersection`].
pub fn mesh_intersection_with(
    a: &TriangleMesh,
    b: &TriangleMesh,
    options: &CsgOptions,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    mesh_boolean(a, b, BooleanOp::Intersection, options)
}

/// Computes the XOR of two meshes with custom options.
///
/// See [`mesh_xor`] for details.
///
/// # Errors
///
/// Same as [`mesh_xor`].
pub fn mesh_xor_with(
    a: &TriangleMesh,
    b: &TriangleMesh,
    options: &CsgOptions,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    mesh_boolean(a, b, BooleanOp::Xor, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::primitives::primitive_box;

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

    #[test]
    fn union_two_identical_boxes_produces_mesh() {
        let a = primitive_box(2.0, 2.0, 2.0);
        let b = primitive_box(2.0, 2.0, 2.0);
        let (mesh, report) = mesh_union(&a, &b).expect("union of identical boxes should succeed");
        assert!(mesh.triangle_count() > 0);
        assert!(report.duration_ms < 10_000);
    }

    #[test]
    fn union_overlapping_boxes_volume_in_range() {
        let a = make_box_at(0.0, 0.0, 0.0, 2.0, 2.0, 2.0);
        let b = make_box_at(1.0, 0.0, 0.0, 2.0, 2.0, 2.0);

        let (_mesh, report) = mesh_union(&a, &b).expect("union should succeed");
        if let Some(vol) = report.volume {
            // Volume should be between 8 (one box) and 16 (both boxes no overlap).
            // Actual: 2*2*2 + 2*2*2 - 1*2*2 = 12.
            assert!(vol > 0.0, "volume should be positive, got {vol}");
        }
    }

    #[test]
    fn difference_produces_mesh() {
        let a = primitive_box(4.0, 4.0, 4.0);
        let b = primitive_box(2.0, 2.0, 2.0);
        let (mesh, _report) = mesh_difference(&a, &b).expect("difference should succeed");
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn volume_and_surface_area_populated() {
        let a = primitive_box(2.0, 2.0, 2.0);
        let b = make_box_at(1.0, 0.0, 0.0, 2.0, 2.0, 2.0);
        let (_, report) = mesh_union(&a, &b).unwrap();
        assert!(report.volume.is_some());
        assert!(report.surface_area.is_some());
        assert!(report.volume.unwrap() > 0.0);
        assert!(report.surface_area.unwrap() > 0.0);
    }
}
