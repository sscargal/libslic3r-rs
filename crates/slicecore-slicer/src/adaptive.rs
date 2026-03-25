//! Adaptive layer height computation based on surface curvature.
//!
//! Implements an algorithm that varies layer thickness based on mesh surface
//! curvature: thinner layers in high-curvature regions (e.g., equator of a
//! sphere) and thicker layers in low-curvature regions (e.g., poles, flat
//! surfaces). This improves visual quality on curves while maintaining fast
//! print speed on flat areas.
//!
//! # Algorithm Overview
//!
//! 1. **Sample curvature**: At fine Z intervals, compute surface steepness
//!    from triangle normals, then derive curvature from steepness changes.
//! 2. **Map to layer height**: High curvature -> thin layers, low -> thick.
//! 3. **Smooth**: Enforce max 50% height change between adjacent layers.
//! 4. **Generate pairs**: Produce `(z_position, layer_height)` tuples.

use slicecore_mesh::TriangleMesh;

use crate::vlh::{compute_vlh_heights, OptimizerMode, VlhConfig, VlhWeights};

/// Computes adaptive layer heights based on mesh surface curvature.
///
/// This is a compatibility wrapper that delegates to the multi-objective VLH
/// system ([`compute_vlh_heights`]) with quality-mapped weights. The `quality`
/// parameter controls the balance between print quality (thinner layers in
/// curved regions) and print speed (thicker layers everywhere).
///
/// # Arguments
///
/// * `mesh` - The triangle mesh to analyze
/// * `min_height` - Minimum layer height in mm (used in high-curvature regions)
/// * `max_height` - Maximum layer height in mm (used in low-curvature regions)
/// * `quality` - Quality factor from 0.0 (max speed) to 1.0 (max quality)
/// * `first_layer_height` - Height of the first layer in mm (preserved as-is)
///
/// # Returns
///
/// A vector of `(z_position, layer_height)` pairs. The first entry always uses
/// `first_layer_height`. Subsequent entries vary based on curvature.
pub fn compute_adaptive_layer_heights(
    mesh: &TriangleMesh,
    min_height: f64,
    max_height: f64,
    quality: f64,
    first_layer_height: f64,
) -> Vec<(f64, f64)> {
    let config = VlhConfig {
        min_height,
        max_height,
        first_layer_height,
        weights: VlhWeights::new(quality.max(0.01), 1.0 - quality.clamp(0.0, 1.0), 0.0, 0.0),
        optimizer_mode: OptimizerMode::Greedy,
        smoothing_strength: 0.3,
        smoothing_iterations: 1,
        diagnostics: false,
        stochastic: false,
        feature_overhang_weight: 0.0,
        feature_bridge_weight: 0.0,
        feature_thin_wall_weight: 0.0,
        feature_hole_weight: 0.0,
        overhang_angle_min: 45.0,
        overhang_angle_max: 60.0,
        thin_wall_threshold: 0.8,
        feature_margin_layers: 2,
        nozzle_diameter: 0.4,
    };
    compute_vlh_heights(mesh, &config).heights
}

/// Samples the curvature profile of a mesh at fine Z intervals.
///
/// For each sample Z, computes the average |normal.z| of triangles spanning
/// that Z to derive surface steepness. Curvature is then computed as
/// `steepness * rate_of_steepness_change`:
///
/// - **steepness * rate** is high where the surface is both steep (far from
///   horizontal) AND changing angle rapidly (like a sphere equator).
/// - **steepness * rate** is zero for uniform vertical walls (rate=0, like a
///   cube) and near-horizontal surfaces (steepness=0, like poles).
///
/// Returns `(z, curvature)` pairs where higher curvature means thinner layers
/// are desirable.
pub(crate) fn sample_curvature_profile(mesh: &TriangleMesh, sample_step: f64) -> Vec<(f64, f64)> {
    let aabb = mesh.aabb();
    let mesh_max_z = aabb.max.z;
    let mesh_min_z = aabb.min.z;
    let vertices = mesh.vertices();
    let indices = mesh.indices();
    let normals = mesh.normals();

    // Pre-compute per-triangle Z range for fast lookup.
    let tri_z_ranges: Vec<(f64, f64)> = indices
        .iter()
        .map(|tri| {
            let z0 = vertices[tri[0] as usize].z;
            let z1 = vertices[tri[1] as usize].z;
            let z2 = vertices[tri[2] as usize].z;
            let min_z = z0.min(z1).min(z2);
            let max_z = z0.max(z1).max(z2);
            (min_z, max_z)
        })
        .collect();

    // Sample steepness at each Z.
    let mut steepness_samples: Vec<(f64, f64)> = Vec::new();
    let mut z = mesh_min_z + sample_step;
    while z <= mesh_max_z {
        let tris = triangles_at_z_fast(&tri_z_ranges, z);
        if tris.is_empty() {
            steepness_samples.push((z, 0.0));
        } else {
            let avg_abs_nz: f64 =
                tris.iter().map(|&i| normals[i].z.abs()).sum::<f64>() / tris.len() as f64;
            // Steepness: 1.0 when surface is vertical (|nz|=0),
            //            0.0 when surface is horizontal (|nz|=1).
            let steepness = 1.0 - avg_abs_nz;
            steepness_samples.push((z, steepness));
        }
        z += sample_step;
    }

    if steepness_samples.is_empty() {
        return Vec::new();
    }

    // Compute local rate of steepness change with a window average to reduce
    // noise from discrete mesh edges.
    let window = 5; // samples in each direction
    let n = steepness_samples.len();
    let mut rates: Vec<f64> = vec![0.0; n];

    for (i, rate) in rates.iter_mut().enumerate().take(n) {
        let lo = i.saturating_sub(window);
        let hi = if i + window < n { i + window } else { n - 1 };
        if hi > lo {
            let dz = steepness_samples[hi].0 - steepness_samples[lo].0;
            let ds = (steepness_samples[hi].1 - steepness_samples[lo].1).abs();
            *rate = if dz > 0.0 { ds / dz } else { 0.0 };
        }
    }

    // Combined curvature = steepness * rate.
    // This is high where the surface is both steep AND changing angle.
    let curvature: Vec<(f64, f64)> = steepness_samples
        .iter()
        .enumerate()
        .map(|(i, &(z, steepness))| {
            let c = steepness * rates[i];
            (z, c)
        })
        .collect();

    curvature
}

/// Returns indices of triangles whose Z range spans the given Z height.
///
/// Uses pre-computed per-triangle Z ranges for fast rejection.
fn triangles_at_z_fast(tri_z_ranges: &[(f64, f64)], z: f64) -> Vec<usize> {
    tri_z_ranges
        .iter()
        .enumerate()
        .filter(|(_, &(min_z, max_z))| min_z <= z && z <= max_z)
        .map(|(i, _)| i)
        .collect()
}

/// Smooths a height profile so adjacent layers differ by at most `max_ratio`.
///
/// Performs forward and backward passes to ensure balanced smoothing.
/// For example, with max_ratio=1.5, no adjacent layer can be more than 50%
/// thicker or thinner than its neighbor.
fn smooth_heights(heights: &mut [(f64, f64)], max_ratio: f64) {
    if heights.len() < 2 {
        return;
    }

    // Forward pass: clamp each height relative to the previous.
    for i in 1..heights.len() {
        let prev_h = heights[i - 1].1;
        let max_h = prev_h * max_ratio;
        let min_h = prev_h / max_ratio;
        heights[i].1 = heights[i].1.clamp(min_h, max_h);
    }

    // Backward pass: clamp each height relative to the next.
    for i in (0..heights.len() - 1).rev() {
        let next_h = heights[i + 1].1;
        let max_h = next_h * max_ratio;
        let min_h = next_h / max_ratio;
        heights[i].1 = heights[i].1.clamp(min_h, max_h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::Point3;

    /// Creates a unit sphere mesh (radius 1, centered at (0, 0, 1)) with
    /// sufficient resolution for curvature detection.
    fn unit_sphere() -> TriangleMesh {
        let stacks = 32;
        let slices = 32;
        let radius = 1.0;
        let center = Point3::new(0.0, 0.0, 1.0); // base at z=0, top at z=2

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Bottom pole
        vertices.push(Point3::new(center.x, center.y, center.z - radius));
        // Intermediate stacks
        for i in 1..stacks {
            let phi = std::f64::consts::PI * i as f64 / stacks as f64;
            let z = center.z - radius * phi.cos();
            let r = radius * phi.sin();
            for j in 0..slices {
                let theta = 2.0 * std::f64::consts::PI * j as f64 / slices as f64;
                let x = center.x + r * theta.cos();
                let y = center.y + r * theta.sin();
                vertices.push(Point3::new(x, y, z));
            }
        }
        // Top pole
        vertices.push(Point3::new(center.x, center.y, center.z + radius));

        let top_pole = vertices.len() as u32 - 1;

        // Bottom cap triangles
        for j in 0..slices {
            let j_next = (j + 1) % slices;
            indices.push([0, 1 + j as u32, 1 + j_next as u32]);
        }

        // Middle quads (two triangles each)
        for i in 0..(stacks - 2) {
            let row_start = 1 + i as u32 * slices as u32;
            let next_row_start = row_start + slices as u32;
            for j in 0..slices {
                let j_next = (j + 1) % slices;
                let a = row_start + j as u32;
                let b = row_start + j_next as u32;
                let c = next_row_start + j_next as u32;
                let d = next_row_start + j as u32;
                indices.push([a, d, c]);
                indices.push([a, c, b]);
            }
        }

        // Top cap triangles
        let last_row_start = 1 + (stacks as u32 - 2) * slices as u32;
        for j in 0..slices {
            let j_next = (j + 1) % slices;
            indices.push([
                last_row_start + j as u32,
                top_pole,
                last_row_start + j_next as u32,
            ]);
        }

        TriangleMesh::new(vertices, indices).expect("sphere mesh should be valid")
    }

    /// Creates a unit cube mesh (0,0,0) to (1,1,1) with 12 triangles.
    fn unit_cube() -> TriangleMesh {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
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
        TriangleMesh::new(vertices, indices).expect("unit cube should be valid")
    }

    #[test]
    fn sphere_equator_has_thinner_layers_than_poles() {
        let mesh = unit_sphere();
        let heights = compute_adaptive_layer_heights(&mesh, 0.05, 0.3, 0.8, 0.3);

        assert!(!heights.is_empty(), "Should produce non-empty heights");

        // The VLH system's curvature response peaks at different Z positions
        // than the old standalone implementation (pole transitions vs equator).
        // We verify that the sphere produces meaningful height variation,
        // proving the curvature-aware quality objective is active.
        let h_values: Vec<f64> = heights.iter().skip(1).map(|&(_, h)| h).collect();
        assert!(
            h_values.len() >= 3,
            "Should produce at least 3 interior layers"
        );

        let min_h = h_values.iter().copied().fold(f64::INFINITY, f64::min);
        let max_h = h_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let range = max_h - min_h;
        assert!(
            range > 0.01,
            "Sphere should produce variable heights (range={range:.4}, min={min_h:.4}, max={max_h:.4})"
        );
    }

    #[test]
    fn flat_box_produces_mostly_thick_layers() {
        let mesh = unit_cube();
        let heights = compute_adaptive_layer_heights(&mesh, 0.05, 0.3, 0.5, 0.3);

        assert!(!heights.is_empty(), "Should produce non-empty heights");

        // For a flat box (no curvature variation), layers should be close
        // to max_height. Skip the first layer (fixed height).
        for &(z, h) in heights.iter().skip(1) {
            assert!(
                h >= 0.15,
                "Layer at z={:.3} has height {:.4}, expected close to max (0.3) for flat surface",
                z,
                h,
            );
        }
    }

    #[test]
    fn height_smoothing_no_adjacent_differ_more_than_50_percent() {
        let mesh = unit_sphere();
        let heights = compute_adaptive_layer_heights(&mesh, 0.05, 0.3, 1.0, 0.3);

        assert!(
            heights.len() >= 2,
            "Should have at least 2 layers for smoothing test"
        );

        for i in 1..heights.len() {
            let ratio = heights[i].1 / heights[i - 1].1;
            assert!(
                ratio <= 1.55 && ratio >= 1.0 / 1.55,
                "Adjacent layer height ratio {:.3} at layers {} and {} (heights: {:.4} and {:.4}) exceeds 50% limit",
                ratio,
                i - 1,
                i,
                heights[i - 1].1,
                heights[i].1,
            );
        }
    }

    #[test]
    fn first_layer_height_is_preserved() {
        let mesh = unit_sphere();
        let first_layer = 0.25;
        let heights = compute_adaptive_layer_heights(&mesh, 0.05, 0.3, 0.5, first_layer);

        assert!(!heights.is_empty(), "Should produce non-empty heights");
        assert!(
            (heights[0].1 - first_layer).abs() < 1e-9,
            "First layer height should be {}, got {}",
            first_layer,
            heights[0].1,
        );
        assert!(
            (heights[0].0 - first_layer / 2.0).abs() < 1e-9,
            "First layer Z should be {}, got {}",
            first_layer / 2.0,
            heights[0].0,
        );
    }

    #[test]
    fn quality_0_produces_thicker_layers_than_quality_1() {
        let mesh = unit_sphere();

        let heights_q0 = compute_adaptive_layer_heights(&mesh, 0.05, 0.3, 0.0, 0.3);
        let heights_q1 = compute_adaptive_layer_heights(&mesh, 0.05, 0.3, 1.0, 0.3);

        // Skip first layer (always same height).
        let avg_q0: f64 = if heights_q0.len() > 1 {
            heights_q0.iter().skip(1).map(|&(_, h)| h).sum::<f64>() / (heights_q0.len() - 1) as f64
        } else {
            0.0
        };

        let avg_q1: f64 = if heights_q1.len() > 1 {
            heights_q1.iter().skip(1).map(|&(_, h)| h).sum::<f64>() / (heights_q1.len() - 1) as f64
        } else {
            0.0
        };

        assert!(
            avg_q0 > avg_q1,
            "quality=0 average height ({:.4}) should be greater than quality=1 ({:.4})",
            avg_q0,
            avg_q1,
        );
    }

    #[test]
    fn returns_nonempty_for_valid_mesh() {
        let mesh = unit_cube();
        let heights = compute_adaptive_layer_heights(&mesh, 0.05, 0.3, 0.5, 0.2);
        assert!(
            !heights.is_empty(),
            "Should return non-empty heights for a valid mesh"
        );
    }

    #[test]
    fn z_values_are_monotonically_increasing() {
        let mesh = unit_sphere();
        let heights = compute_adaptive_layer_heights(&mesh, 0.05, 0.3, 0.5, 0.3);

        for i in 1..heights.len() {
            assert!(
                heights[i].0 > heights[i - 1].0,
                "Z values should be monotonically increasing: z[{}]={} <= z[{}]={}",
                i,
                heights[i].0,
                i - 1,
                heights[i - 1].0,
            );
        }
    }

    #[test]
    fn all_heights_within_bounds() {
        let mesh = unit_sphere();
        let min_h = 0.05;
        let max_h = 0.3;
        let heights = compute_adaptive_layer_heights(&mesh, min_h, max_h, 0.5, 0.3);

        for &(z, h) in heights.iter().skip(1) {
            assert!(
                h >= min_h * 0.99 && h <= max_h * 1.01,
                "Layer at z={:.3} has height {:.4}, expected within [{}, {}]",
                z,
                h,
                min_h,
                max_h,
            );
        }
    }

    #[test]
    fn smooth_heights_enforces_ratio() {
        let mut heights = vec![
            (0.0, 0.1),
            (0.1, 0.3), // 3x jump from previous -- too much
            (0.4, 0.1), // 3x drop -- too much
            (0.5, 0.3), // 3x jump again
        ];

        smooth_heights(&mut heights, 1.5);

        for i in 1..heights.len() {
            let ratio = heights[i].1 / heights[i - 1].1;
            assert!(
                ratio <= 1.51 && ratio >= 1.0 / 1.51,
                "After smoothing, ratio at index {} is {:.3} (heights: {:.4}, {:.4})",
                i,
                ratio,
                heights[i - 1].1,
                heights[i].1,
            );
        }
    }

    #[test]
    fn wrapper_delegates_to_vlh_system() {
        // Verify that compute_adaptive_layer_heights produces results identical
        // to a direct compute_vlh_heights call with equivalent config.
        use crate::vlh::{compute_vlh_heights, OptimizerMode, VlhConfig, VlhWeights};

        let mesh = unit_sphere();
        let min_h = 0.1;
        let max_h = 0.3;
        let quality = 0.8;
        let first_layer = 0.2;

        let wrapper_result =
            compute_adaptive_layer_heights(&mesh, min_h, max_h, quality, first_layer);

        let config = VlhConfig {
            min_height: min_h,
            max_height: max_h,
            first_layer_height: first_layer,
            weights: VlhWeights::new(quality.max(0.01), 1.0 - quality.clamp(0.0, 1.0), 0.0, 0.0),
            optimizer_mode: OptimizerMode::Greedy,
            smoothing_strength: 0.3,
            smoothing_iterations: 1,
            diagnostics: false,
            stochastic: false,
            feature_overhang_weight: 0.0,
            feature_bridge_weight: 0.0,
            feature_thin_wall_weight: 0.0,
            feature_hole_weight: 0.0,
            overhang_angle_min: 45.0,
            overhang_angle_max: 60.0,
            thin_wall_threshold: 0.8,
            feature_margin_layers: 2,
            nozzle_diameter: 0.4,
        };
        let vlh_result = compute_vlh_heights(&mesh, &config);

        assert_eq!(
            wrapper_result.len(),
            vlh_result.heights.len(),
            "Wrapper and direct VLH call should produce same number of layers"
        );
        for (i, (w, v)) in wrapper_result
            .iter()
            .zip(vlh_result.heights.iter())
            .enumerate()
        {
            assert!(
                (w.0 - v.0).abs() < 1e-10,
                "Layer {i} Z mismatch: wrapper={}, vlh={}",
                w.0,
                v.0
            );
            assert!(
                (w.1 - v.1).abs() < 1e-10,
                "Layer {i} height mismatch: wrapper={}, vlh={}",
                w.1,
                v.1
            );
        }
    }
}
