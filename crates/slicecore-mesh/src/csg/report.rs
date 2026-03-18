//! Report struct for CSG operation results.

use serde::{Deserialize, Serialize};

/// Diagnostic report produced after a CSG boolean operation.
///
/// Contains input/output triangle counts, intersection statistics,
/// and optional geometric measurements (volume, surface area).
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::CsgReport;
///
/// let report = CsgReport {
///     input_triangles_a: 12,
///     input_triangles_b: 20,
///     output_triangles: 28,
///     intersection_curves: 4,
///     repairs_performed: 0,
///     warnings: vec!["near-degenerate triangle at index 5".to_string()],
///     volume: Some(3.14),
///     surface_area: Some(12.56),
///     duration_ms: 42,
/// };
///
/// // Round-trip through JSON.
/// let json = serde_json::to_string(&report).unwrap();
/// let restored: CsgReport = serde_json::from_str(&json).unwrap();
/// assert_eq!(restored.output_triangles, 28);
/// assert_eq!(restored.warnings.len(), 1);
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CsgReport {
    /// Number of triangles in input mesh A.
    pub input_triangles_a: usize,
    /// Number of triangles in input mesh B.
    pub input_triangles_b: usize,
    /// Number of triangles in the output mesh.
    pub output_triangles: usize,
    /// Number of intersection curves found between the two meshes.
    pub intersection_curves: usize,
    /// Number of mesh repairs performed before the operation.
    pub repairs_performed: usize,
    /// Non-fatal warnings encountered during the operation.
    pub warnings: Vec<String>,
    /// Signed volume of the output mesh (via divergence theorem), if computed.
    pub volume: Option<f64>,
    /// Surface area of the output mesh, if computed.
    pub surface_area: Option<f64>,
    /// Wall-clock duration of the operation in milliseconds.
    pub duration_ms: u64,
}
