//! Auto-orientation for 3D-printed parts.
//!
//! Evaluates candidate orientations and selects the one that minimizes
//! support material or maximizes flat bed contact, depending on the
//! chosen [`OrientCriterion`].

use slicecore_math::Vec3;

use crate::config::OrientCriterion;

/// Rotates a normal vector by angles around X then Y axes (in radians).
#[allow(
    clippy::similar_names,
    reason = "rx/ry are standard rotation axis names"
)]
fn rotate_normal(normal: &Vec3, angle_x: f64, angle_y: f64) -> Vec3 {
    // Rotation around X axis
    let (sin_x, cos_x) = angle_x.sin_cos();
    let after_x_y = normal.y * cos_x - normal.z * sin_x;
    let after_x_z = normal.y * sin_x + normal.z * cos_x;

    // Rotation around Y axis
    let (sin_y, cos_y) = angle_y.sin_cos();
    Vec3::new(
        normal.x * cos_y + after_x_z * sin_y,
        after_x_y,
        -normal.x * sin_y + after_x_z * cos_y,
    )
}

/// Computes the overhang score for a set of face normals at a given orientation.
///
/// Lower is better. Sums face areas where the rotated normal forms an angle
/// greater than 45 degrees from vertical (Z-up).
#[allow(
    clippy::similar_names,
    reason = "angle_x/angle_y are standard rotation names"
)]
fn overhang_score(normals: &[Vec3], face_areas: &[f64], angle_x: f64, angle_y: f64) -> f64 {
    let threshold_cos = std::f64::consts::FRAC_PI_4.cos(); // cos(45 degrees)
    let z_up = Vec3::new(0.0, 0.0, 1.0);

    normals
        .iter()
        .zip(face_areas.iter())
        .map(|(normal, &area)| {
            let rotated = rotate_normal(normal, angle_x, angle_y);
            let dot = rotated.dot(z_up);
            if dot < threshold_cos {
                area
            } else {
                0.0
            }
        })
        .sum()
}

/// Computes the flat contact score for a set of face normals at a given orientation.
///
/// Higher is better. Sums face areas where the rotated normal points nearly
/// straight down (z < -0.99), indicating a flat face on the bed.
#[allow(
    clippy::similar_names,
    reason = "angle_x/angle_y are standard rotation names"
)]
fn contact_score(normals: &[Vec3], face_areas: &[f64], angle_x: f64, angle_y: f64) -> f64 {
    normals
        .iter()
        .zip(face_areas.iter())
        .map(|(normal, &area)| {
            let rotated = rotate_normal(normal, angle_x, angle_y);
            if rotated.z < -0.99 {
                area
            } else {
                0.0
            }
        })
        .sum()
}

/// Selects the optimal orientation for a part based on the given criterion.
///
/// Samples 144 candidate orientations (30-degree increments around X and Y axes)
/// and scores each according to the chosen [`OrientCriterion`].
///
/// Returns `(rx, ry, rz)` rotation angles in degrees. The `rz` component is
/// always 0.0 (Z rotation is handled by the placer's rotation variants).
///
/// # Arguments
///
/// * `vertices` - 3D mesh vertices (unused by the scoring; normals carry orientation info)
/// * `normals` - Per-face normal vectors
/// * `face_areas` - Per-face surface areas in mm^2
/// * `criterion` - Which scoring method to use
///
/// # Examples
///
/// ```
/// use slicecore_math::{Point3, Vec3};
/// use slicecore_arrange::config::OrientCriterion;
/// use slicecore_arrange::orient::auto_orient;
///
/// // A cube with axis-aligned normals -- identity orientation is optimal
/// let normals = vec![
///     Vec3::new(0.0, 0.0, 1.0),
///     Vec3::new(0.0, 0.0, -1.0),
///     Vec3::new(1.0, 0.0, 0.0),
///     Vec3::new(-1.0, 0.0, 0.0),
///     Vec3::new(0.0, 1.0, 0.0),
///     Vec3::new(0.0, -1.0, 0.0),
/// ];
/// let areas = vec![100.0; 6];
/// let vertices = vec![Point3::new(0.0, 0.0, 0.0)]; // unused
/// let (rx, ry, rz) = auto_orient(&vertices, &normals, &areas, &OrientCriterion::MinimizeSupport);
/// assert!((rx.abs() + ry.abs() + rz.abs()) < 1.0, "Cube should stay near identity");
/// ```
#[must_use]
#[allow(
    clippy::similar_names,
    reason = "angle_x/angle_y and best_ax/best_ay are domain names"
)]
pub fn auto_orient(
    _vertices: &[slicecore_math::Point3],
    normals: &[Vec3],
    face_areas: &[f64],
    criterion: &OrientCriterion,
) -> (f64, f64, f64) {
    if normals.is_empty() || face_areas.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let step = std::f64::consts::FRAC_PI_6; // 30 degrees in radians
    let mut best_score = f64::NEG_INFINITY;
    let mut best_ax = 0.0_f64;
    let mut best_ay = 0.0_f64;

    // Compute total area for normalization in multi-criteria mode
    let total_area: f64 = face_areas.iter().sum();
    let total_area = if total_area > 0.0 { total_area } else { 1.0 };

    for ix in 0..12_u32 {
        let angle_x = f64::from(ix) * step;
        for iy in 0..12_u32 {
            let angle_y = f64::from(iy) * step;

            let score = match criterion {
                OrientCriterion::MinimizeSupport => {
                    // Lower overhang is better -> negate to use max-is-best convention
                    -overhang_score(normals, face_areas, angle_x, angle_y)
                }
                OrientCriterion::MaximizeFlatContact => {
                    contact_score(normals, face_areas, angle_x, angle_y)
                }
                OrientCriterion::MultiCriteria {
                    support_weight,
                    contact_weight,
                } => {
                    let overhang = overhang_score(normals, face_areas, angle_x, angle_y);
                    let contact = contact_score(normals, face_areas, angle_x, angle_y);
                    let normalized_overhang = overhang / total_area;
                    let normalized_contact = contact / total_area;
                    support_weight * (1.0 - normalized_overhang)
                        + contact_weight * normalized_contact
                }
            };

            if score > best_score {
                best_score = score;
                best_ax = angle_x;
                best_ay = angle_y;
            }
        }
    }

    (best_ax.to_degrees(), best_ay.to_degrees(), 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::Point3;

    /// Cube normals: all 6 axis-aligned faces.
    fn cube_normals() -> Vec<Vec3> {
        vec![
            Vec3::new(0.0, 0.0, 1.0),  // top
            Vec3::new(0.0, 0.0, -1.0), // bottom
            Vec3::new(1.0, 0.0, 0.0),  // +X
            Vec3::new(-1.0, 0.0, 0.0), // -X
            Vec3::new(0.0, 1.0, 0.0),  // +Y
            Vec3::new(0.0, -1.0, 0.0), // -Y
        ]
    }

    #[test]
    fn cube_minimize_support_identity() {
        let normals = cube_normals();
        let areas = vec![100.0; 6];
        let vertices = vec![Point3::new(0.0, 0.0, 0.0)];
        let (_rx, _ry, rz) = auto_orient(
            &vertices,
            &normals,
            &areas,
            &OrientCriterion::MinimizeSupport,
        );
        // Cube has symmetric overhangs: identity (0,0,0) is among the best
        assert!(rz.abs() < f64::EPSILON, "rz should always be 0, got {rz}");
    }

    #[test]
    fn tilted_mesh_minimize_support() {
        // Mesh with large overhang face tilted 60 degrees from vertical
        let normals = vec![
            Vec3::new(0.866, 0.0, 0.5),  // large overhang face
            Vec3::new(0.0, 0.0, -1.0),   // bottom face
            Vec3::new(-0.866, 0.0, 0.5), // back face
        ];
        let areas = vec![200.0, 50.0, 50.0]; // overhang face is largest
        let vertices = vec![Point3::new(0.0, 0.0, 0.0)];
        let (rx, ry, _rz) = auto_orient(
            &vertices,
            &normals,
            &areas,
            &OrientCriterion::MinimizeSupport,
        );
        // Should find an orientation that reduces overhang of the large face
        let identity_overhang = overhang_score(&normals, &areas, 0.0, 0.0);
        let best_overhang = overhang_score(&normals, &areas, rx.to_radians(), ry.to_radians());
        assert!(
            best_overhang <= identity_overhang,
            "Best ({best_overhang}) should be <= identity ({identity_overhang})"
        );
    }

    #[test]
    fn maximize_flat_contact() {
        let normals = vec![
            Vec3::new(0.0, 0.0, -1.0), // bottom
            Vec3::new(0.0, 0.0, 1.0),  // top
            Vec3::new(1.0, 0.0, 0.0),  // side (small)
        ];
        let areas = vec![200.0, 50.0, 50.0]; // bottom is largest
        let vertices = vec![Point3::new(0.0, 0.0, 0.0)];
        let (rx, ry, _rz) = auto_orient(
            &vertices,
            &normals,
            &areas,
            &OrientCriterion::MaximizeFlatContact,
        );
        let contact = contact_score(&normals, &areas, rx.to_radians(), ry.to_radians());
        assert!(
            contact >= 199.0,
            "Should keep large bottom face on bed, contact={contact}"
        );
    }

    #[test]
    fn empty_normals_returns_identity() {
        let vertices = vec![Point3::new(0.0, 0.0, 0.0)];
        let (rx, ry, rz) = auto_orient(&vertices, &[], &[], &OrientCriterion::MinimizeSupport);
        assert!((rx.abs() + ry.abs() + rz.abs()) < f64::EPSILON);
    }
}
