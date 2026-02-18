//! Mesh repair pipeline.
//!
//! Implements the admesh-inspired repair order:
//! 1. Remove degenerate triangles
//! 2. Stitch nearby unconnected edges
//! 3. Fix normal directions via BFS flood-fill
//! 4. Fill holes in the mesh (after normal fix to avoid false boundaries)
//! 5. Recompute per-face normals
//! 6. Detect self-intersections (report only)
//!
//! The repair function takes raw vertex and index data and returns a valid
//! `TriangleMesh` along with a `RepairReport` documenting all changes made.

use serde::{Deserialize, Serialize};

use slicecore_math::Point3;

use crate::error::MeshError;
use crate::triangle_mesh::TriangleMesh;

pub mod degenerate;
pub mod holes;
pub mod intersect;
pub mod normals;
pub mod stitch;

/// Report of all repairs performed on a mesh.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepairReport {
    /// Number of degenerate triangles removed.
    pub degenerate_removed: usize,
    /// Number of edges stitched by merging nearby vertices.
    pub edges_stitched: usize,
    /// Number of triangles added to fill holes.
    pub holes_filled: usize,
    /// Number of triangles whose winding order was flipped.
    pub normals_fixed: usize,
    /// Number of self-intersecting triangle pairs detected.
    pub self_intersections_detected: usize,
    /// Triangle pair indices for each self-intersection detected.
    pub intersecting_pairs: Vec<(usize, usize)>,
    /// Whether self-intersections are resolvable at slice time via contour union.
    pub self_intersections_resolvable: bool,
    /// Z-band affected by self-intersections (min_z, max_z of involved vertices).
    pub intersection_z_range: Option<(f64, f64)>,
    /// Whether the mesh required no repairs at all.
    pub was_already_clean: bool,
}

/// Runs the full mesh repair pipeline on raw vertex and index data.
///
/// Pipeline order:
/// 1. Remove degenerate triangles (zero-area, duplicate vertices, collinear)
/// 2. Stitch edges (merge nearby vertices within tolerance)
/// 3. Fix normal directions (BFS flood-fill for consistent winding)
/// 4. Fill holes (triangulate boundary edge loops) -- after normal fix to
///    avoid false boundary edges from inconsistent winding
/// 5. Recompute per-face normals
/// 6. Detect self-intersections (count only, not repaired)
///
/// Returns a valid `TriangleMesh` and a `RepairReport` documenting changes.
///
/// # Errors
///
/// Returns `MeshError` if the repaired mesh cannot be constructed (e.g., all
/// triangles were degenerate, leaving an empty mesh).
#[allow(clippy::field_reassign_with_default)]
pub fn repair(
    vertices: Vec<Point3>,
    mut indices: Vec<[u32; 3]>,
) -> Result<(TriangleMesh, RepairReport), MeshError> {
    let mut report = RepairReport::default();

    // Step 1: Remove degenerate triangles.
    report.degenerate_removed = degenerate::remove_degenerate_triangles(&vertices, &mut indices);

    // Step 2: Stitch edges (merge nearby vertices within tolerance).
    report.edges_stitched =
        stitch::stitch_edges(&vertices, &mut indices, stitch::STITCH_TOLERANCE);

    // Step 3: Fix normal directions BEFORE hole filling, because inconsistent
    // winding creates false boundary edges that confuse the hole detector.
    report.normals_fixed = normals::fix_normal_directions(&vertices, &mut indices);

    // Step 4: Fill holes (triangulate boundary edge loops).
    report.holes_filled = holes::fill_holes(&vertices, &mut indices);

    // Step 5: Recompute normals (handled by TriangleMesh::new).

    // Step 6: Detect self-intersections with pair reporting.
    let pairs = intersect::find_intersecting_pairs(&vertices, &indices);
    report.self_intersections_detected = pairs.len();
    report.self_intersections_resolvable = !pairs.is_empty();
    report.intersection_z_range = intersect::intersection_z_range(&vertices, &indices, &pairs);
    report.intersecting_pairs = pairs;

    // Determine if mesh was already clean.
    report.was_already_clean = report.degenerate_removed == 0
        && report.edges_stitched == 0
        && report.holes_filled == 0
        && report.normals_fixed == 0
        && report.self_intersections_detected == 0;

    // Construct the repaired mesh.
    let mesh = TriangleMesh::new(vertices, indices)?;

    Ok((mesh, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a closed tetrahedron mesh (4 faces, all edges shared by 2
    /// triangles). This is a minimal closed manifold mesh.
    fn tetrahedron() -> (Vec<Point3>, Vec<[u32; 3]>) {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            Point3::new(0.5, 0.5, 1.0),
        ];
        // Consistent CCW winding when viewed from outside.
        let indices = vec![
            [0, 2, 1], // bottom face (z=0)
            [0, 1, 3], // front face
            [1, 2, 3], // right face
            [2, 0, 3], // left face
        ];
        (vertices, indices)
    }

    #[test]
    fn repair_clean_closed_mesh_reports_already_clean() {
        let (vertices, indices) = tetrahedron();
        let (mesh, report) = repair(vertices, indices).unwrap();
        assert!(report.was_already_clean, "report: {:?}", report);
        assert_eq!(report.degenerate_removed, 0);
        assert_eq!(report.normals_fixed, 0);
        assert_eq!(report.holes_filled, 0);
        assert_eq!(mesh.triangle_count(), 4);
    }

    #[test]
    fn repair_removes_degenerate_and_fixes_normals() {
        let (vertices, mut indices) = tetrahedron();
        // Flip one triangle's winding to create inconsistent normals.
        indices[2].swap(1, 2); // [1, 2, 3] -> [1, 3, 2] (reversed winding)
        // Add a degenerate triangle.
        indices.push([0, 0, 1]);
        let (mesh, report) = repair(vertices, indices).unwrap();
        assert_eq!(report.degenerate_removed, 1);
        assert!(report.normals_fixed >= 1, "Expected at least 1 flip");
        assert!(!report.was_already_clean);
        assert_eq!(mesh.triangle_count(), 4);
    }

    #[test]
    fn repair_all_degenerate_returns_error() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
        ];
        let indices = vec![[0, 0, 1], [1, 1, 0]];
        let result = repair(vertices, indices);
        assert!(result.is_err());
    }

    #[test]
    fn full_pipeline_end_to_end() {
        // Start with a tetrahedron and introduce multiple defects:
        // - A degenerate triangle
        // - Inconsistent winding on one face
        let (vertices, mut indices) = tetrahedron();
        // Flip winding on face 1 to create normal inconsistency.
        indices[1].swap(1, 2);
        // Add degenerate triangle.
        indices.push([2, 2, 0]);
        let result = repair(vertices, indices);
        assert!(result.is_ok());
        let (mesh, report) = result.unwrap();
        assert_eq!(report.degenerate_removed, 1);
        assert!(report.normals_fixed >= 1);
        assert_eq!(mesh.triangle_count(), 4);
    }
}
