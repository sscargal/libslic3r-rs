//! Mesh repair pipeline.
//!
//! Implements the admesh-inspired repair order:
//! 1. Remove degenerate triangles
//! 2. Stitch nearby unconnected edges (Task 2)
//! 3. Fill holes in the mesh (Task 2)
//! 4. Fix normal directions via BFS flood-fill
//! 5. Recompute per-face normals
//! 6. Detect self-intersections (Task 2)
//!
//! The repair function takes raw vertex and index data and returns a valid
//! `TriangleMesh` along with a `RepairReport` documenting all changes made.

use serde::{Deserialize, Serialize};

use slicecore_math::Point3;

use crate::error::MeshError;
use crate::triangle_mesh::TriangleMesh;

pub mod degenerate;
pub mod normals;

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
    /// Whether the mesh required no repairs at all.
    pub was_already_clean: bool,
}

/// Runs the full mesh repair pipeline on raw vertex and index data.
///
/// Pipeline order:
/// 1. Remove degenerate triangles (zero-area, duplicate vertices, collinear)
/// 2. Stitch edges (merge nearby vertices within tolerance) -- TODO Task 2
/// 3. Fill holes (triangulate boundary edge loops) -- TODO Task 2
/// 4. Fix normal directions (BFS flood-fill for consistent winding)
/// 5. Recompute per-face normals
/// 6. Detect self-intersections -- TODO Task 2
///
/// Returns a valid `TriangleMesh` and a `RepairReport` documenting changes.
///
/// # Errors
///
/// Returns `MeshError` if the repaired mesh cannot be constructed (e.g., all
/// triangles were degenerate, leaving an empty mesh).
pub fn repair(
    vertices: Vec<Point3>,
    mut indices: Vec<[u32; 3]>,
) -> Result<(TriangleMesh, RepairReport), MeshError> {
    let mut report = RepairReport::default();

    // Step 1: Remove degenerate triangles.
    report.degenerate_removed = degenerate::remove_degenerate_triangles(&vertices, &mut indices);

    // Step 2: Stitch edges (placeholder -- implemented in Task 2).
    report.edges_stitched = 0;

    // Step 3: Fill holes (placeholder -- implemented in Task 2).
    report.holes_filled = 0;

    // Step 4: Fix normal directions.
    report.normals_fixed = normals::fix_normal_directions(&vertices, &mut indices);

    // Step 5: Recompute normals (handled by TriangleMesh::new).

    // Step 6: Detect self-intersections (placeholder -- implemented in Task 2).
    report.self_intersections_detected = 0;

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

    #[test]
    fn repair_clean_mesh_reports_already_clean() {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        let indices = vec![[0, 1, 2]];
        let (mesh, report) = repair(vertices, indices).unwrap();
        assert!(report.was_already_clean);
        assert_eq!(report.degenerate_removed, 0);
        assert_eq!(report.normals_fixed, 0);
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn repair_removes_degenerate_and_fixes_normals() {
        // 4 vertices forming a "bowtie" of two triangles plus one degenerate.
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            Point3::new(0.5, -1.0, 0.0),
        ];
        let indices = vec![
            [0, 1, 2],    // valid, edge 0->1
            [0, 1, 3],    // valid but inconsistent winding (same edge direction as tri 0)
            [0, 0, 1],    // degenerate: duplicate index
        ];
        let (mesh, report) = repair(vertices, indices).unwrap();
        assert_eq!(report.degenerate_removed, 1);
        assert_eq!(report.normals_fixed, 1);
        assert!(!report.was_already_clean);
        assert_eq!(mesh.triangle_count(), 2);
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
}
