//! Mesh hollowing via offset and CSG difference.
//!
//! Creates a hollow shell from a solid mesh by computing an inward-offset
//! inner shell and subtracting it from the original. Optionally punches
//! a drain hole for resin printing workflows.

use std::f64::consts::TAU;
use std::time::Instant;

use slicecore_math::{Point3, Vec3};

use crate::triangle_mesh::TriangleMesh;

use super::boolean::mesh_difference;
use super::error::CsgError;
use super::offset::mesh_offset;
use super::report::CsgReport;
use super::volume;

/// Specifies the position and geometry of a drain hole.
///
/// A drain hole is a cylindrical (or tapered) opening punched through the
/// shell wall, typically at the bottom of the model for resin drainage.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::hollow::DrainHole;
/// use slicecore_math::{Point3, Vec3};
///
/// let hole = DrainHole {
///     position: Point3::new(0.0, 0.0, -1.0),
///     direction: Vec3::new(0.0, 0.0, -1.0),
///     diameter: 3.0,
///     tapered: false,
/// };
/// assert!((hole.diameter - 3.0).abs() < 1e-12);
/// ```
#[derive(Clone, Debug)]
pub struct DrainHole {
    /// Center position of the hole opening on the mesh surface.
    pub position: Point3,
    /// Direction the hole punches through (typically pointing outward).
    pub direction: Vec3,
    /// Diameter of the hole in mm.
    pub diameter: f64,
    /// Whether to taper the hole (cone shape) instead of a straight cylinder.
    pub tapered: bool,
}

impl Default for DrainHole {
    fn default() -> Self {
        Self {
            position: Point3::new(0.0, 0.0, 0.0),
            direction: Vec3::new(0.0, 0.0, -1.0),
            diameter: 3.0,
            tapered: false,
        }
    }
}

/// Options for the mesh hollowing operation.
///
/// `wall_thickness` is mandatory and has no default. Use struct literal
/// syntax to construct.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::hollow::HollowOptions;
///
/// let opts = HollowOptions {
///     wall_thickness: 2.0,
///     drain_hole: None,
/// };
/// assert!((opts.wall_thickness - 2.0).abs() < 1e-12);
/// ```
#[derive(Clone, Debug)]
pub struct HollowOptions {
    /// Shell wall thickness in mm. Must be positive.
    pub wall_thickness: f64,
    /// Optional drain hole specification.
    pub drain_hole: Option<DrainHole>,
}

/// Hollows a solid mesh to create a shell with the specified wall thickness.
///
/// # Algorithm
///
/// 1. Create an inner shell by offsetting the mesh inward by `wall_thickness`.
/// 2. Flip the inner shell's winding so normals point inward.
/// 3. Subtract the inner shell from the original via [`mesh_difference`].
/// 4. Optionally punch a drain hole by subtracting a cylinder/cone primitive.
///
/// # Errors
///
/// Returns [`CsgError`] if the offset, difference, or drain hole operation fails.
///
/// # Examples
///
/// ```no_run
/// use slicecore_mesh::csg::hollow::{hollow_mesh, HollowOptions};
/// use slicecore_mesh::csg::primitive_box;
///
/// let mesh = primitive_box(10.0, 10.0, 10.0);
/// let opts = HollowOptions { wall_thickness: 2.0, drain_hole: None };
/// let (hollowed, report) = hollow_mesh(&mesh, &opts).unwrap();
/// assert!(report.output_triangles > 0);
/// ```
pub fn hollow_mesh(
    mesh: &TriangleMesh,
    options: &HollowOptions,
) -> Result<(TriangleMesh, CsgReport), CsgError> {
    let start = Instant::now();
    let original_volume = volume::signed_volume(mesh.vertices(), mesh.indices());

    // Step 1: Create inner solid via negative offset.
    // The offset mesh keeps the same winding (outward normals) but is smaller.
    let (inner_solid, _offset_report) = mesh_offset(mesh, -options.wall_thickness)?;

    // Step 2: Boolean difference to create hollow shell.
    // Subtract the smaller inner solid from the original to leave only the shell.
    let (mut shell, mut report) = mesh_difference(mesh, &inner_solid)?;

    // Step 4: Optionally punch drain hole.
    if let Some(ref drain) = options.drain_hole {
        let drain_mesh = create_drain_cylinder(drain);
        match mesh_difference(&shell, &drain_mesh) {
            Ok((drained, drain_report)) => {
                shell = drained;
                report.warnings.extend(drain_report.warnings);
            }
            Err(e) => {
                report
                    .warnings
                    .push(format!("drain hole subtraction failed: {e}"));
            }
        }
    }

    // Step 5: Compute volume metrics.
    let hollow_volume = volume::signed_volume(shell.vertices(), shell.indices());
    report.input_triangles_a = mesh.triangle_count();
    report.output_triangles = shell.triangle_count();
    report.volume = Some(hollow_volume);
    report.surface_area = Some(volume::surface_area(shell.vertices(), shell.indices()));

    // Check for overly thick walls.
    let aabb = mesh.aabb();
    let dims = [
        aabb.max.x - aabb.min.x,
        aabb.max.y - aabb.min.y,
        aabb.max.z - aabb.min.z,
    ];
    let smallest_dim = dims.iter().copied().fold(f64::INFINITY, f64::min);
    if options.wall_thickness > smallest_dim * 0.5 {
        report.warnings.push(format!(
            "wall thickness ({:.1}mm) exceeds 50% of smallest dimension ({:.1}mm)",
            options.wall_thickness, smallest_dim
        ));
    }

    // Add volume reduction info.
    if original_volume.abs() > 1e-12 {
        let pct_saved = ((original_volume - hollow_volume) / original_volume * 100.0).abs();
        report.warnings.push(format!(
            "volume reduction: {:.1}% (original: {:.2}, hollow: {:.2})",
            pct_saved, original_volume, hollow_volume
        ));
    }

    report.duration_ms = start.elapsed().as_millis() as u64;

    Ok((shell, report))
}

/// Creates a cylinder (or tapered cone) mesh for the drain hole.
fn create_drain_cylinder(drain: &DrainHole) -> TriangleMesh {
    let segments = 16u32;
    let radius = drain.diameter / 2.0;
    // Make the cylinder long enough to punch through any reasonable wall.
    let length = drain.diameter * 4.0;
    let half_len = length / 2.0;

    let dir = drain.direction.normalize();

    // Build local coordinate frame.
    let helper = if dir.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = dir.cross(helper).normalize();
    let v = dir.cross(u).normalize();

    let mut vertices = Vec::with_capacity(2 + 2 * segments as usize);
    let mut indices = Vec::with_capacity(4 * segments as usize);

    // Bottom center
    let bc = Point3::new(
        drain.position.x - dir.x * half_len,
        drain.position.y - dir.y * half_len,
        drain.position.z - dir.z * half_len,
    );
    // Top center
    let tc = Point3::new(
        drain.position.x + dir.x * half_len,
        drain.position.y + dir.y * half_len,
        drain.position.z + dir.z * half_len,
    );

    vertices.push(bc); // 0: bottom center
    vertices.push(tc); // 1: top center

    let top_radius = if drain.tapered { radius * 0.5 } else { radius };

    // Bottom ring
    for i in 0..segments {
        let angle = TAU * f64::from(i) / f64::from(segments);
        let px = bc.x + radius * (u.x * angle.cos() + v.x * angle.sin());
        let py = bc.y + radius * (u.y * angle.cos() + v.y * angle.sin());
        let pz = bc.z + radius * (u.z * angle.cos() + v.z * angle.sin());
        vertices.push(Point3::new(px, py, pz));
    }

    // Top ring
    for i in 0..segments {
        let angle = TAU * f64::from(i) / f64::from(segments);
        let px = tc.x + top_radius * (u.x * angle.cos() + v.x * angle.sin());
        let py = tc.y + top_radius * (u.y * angle.cos() + v.y * angle.sin());
        let pz = tc.z + top_radius * (u.z * angle.cos() + v.z * angle.sin());
        vertices.push(Point3::new(px, py, pz));
    }

    let bottom_start = 2u32;
    let top_start = bottom_start + segments;

    for i in 0..segments {
        let next = (i + 1) % segments;

        // Bottom cap
        indices.push([0, bottom_start + next, bottom_start + i]);
        // Top cap
        indices.push([1, top_start + i, top_start + next]);

        // Side
        let bl = bottom_start + i;
        let br = bottom_start + next;
        let tl = top_start + i;
        let tr = top_start + next;
        indices.push([bl, br, tr]);
        indices.push([bl, tr, tl]);
    }

    TriangleMesh::new(vertices, indices).expect("drain cylinder should be valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::primitives::primitive_box;

    #[test]
    fn hollow_box_reduces_volume() {
        let mesh = primitive_box(10.0, 10.0, 10.0);
        let orig_vol = volume::signed_volume(mesh.vertices(), mesh.indices());
        let opts = HollowOptions {
            wall_thickness: 2.0,
            drain_hole: None,
        };
        let (result, report) = hollow_mesh(&mesh, &opts).unwrap();
        assert!(result.triangle_count() > 0);
        let vol = report.volume.unwrap();
        eprintln!("original volume: {orig_vol}, hollow volume: {vol}");
        // Hollow volume should be less than original.
        // Original = 1000, inner box would be 6*6*6 = 216, shell = 1000 - 216 = 784.
        assert!(
            vol < orig_vol,
            "hollow volume ({vol}) should be less than original ({orig_vol})"
        );
    }
}
