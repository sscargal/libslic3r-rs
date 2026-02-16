//! Slice layer types and the main mesh slicing function.
//!
//! A [`SliceLayer`] represents the 2D cross-section of a mesh at a specific
//! Z height, containing validated contour polygons (outer boundaries and holes).
//!
//! The [`slice_mesh`] function is the main entry point: it computes layer
//! heights from the mesh bounding box and slices the mesh at each height.

use slicecore_geo::ValidPolygon;
use slicecore_math::BBox3;
use slicecore_mesh::TriangleMesh;

use crate::contour::slice_at_height;

/// A single horizontal slice layer of a 3D mesh.
///
/// Contains the Z height, layer thickness, and all contour polygons
/// extracted at that height. Contours follow the winding convention:
/// CCW = outer boundary, CW = hole.
pub struct SliceLayer {
    /// Z height in mm (the cutting plane position).
    pub z: f64,
    /// Height of this layer in mm.
    pub layer_height: f64,
    /// Outer boundaries and holes at this Z height.
    pub contours: Vec<ValidPolygon>,
}

/// Computes the Z heights at which to slice a mesh.
///
/// Returns a vector of `(z_height, layer_height)` tuples. The first layer
/// is positioned at `first_layer_height / 2` (midpoint of the first layer).
/// Subsequent layers increment by `layer_height`. Each tuple records both
/// the Z position and the thickness for that layer.
///
/// # Arguments
///
/// * `aabb` - The axis-aligned bounding box of the mesh
/// * `layer_height` - The standard layer height in mm
/// * `first_layer_height` - The height of the first layer in mm
pub fn compute_layer_heights(
    aabb: &BBox3,
    layer_height: f64,
    first_layer_height: f64,
) -> Vec<(f64, f64)> {
    let max_z = aabb.max.z;
    let mut heights = Vec::new();

    // First layer at midpoint of first layer height
    let first_z = first_layer_height / 2.0;
    if first_z > max_z {
        return heights;
    }
    heights.push((first_z, first_layer_height));

    // Subsequent layers
    let mut z = first_layer_height + layer_height / 2.0;
    while z <= max_z {
        heights.push((z, layer_height));
        z += layer_height;
    }

    heights
}

/// Slices a triangle mesh into horizontal layers.
///
/// Computes layer heights from the mesh AABB, then slices the mesh at each
/// height to produce contour polygons. Layers are processed sequentially.
///
/// # Arguments
///
/// * `mesh` - The triangle mesh to slice
/// * `layer_height` - The standard layer height in mm
/// * `first_layer_height` - The height of the first layer in mm
pub fn slice_mesh(
    mesh: &TriangleMesh,
    layer_height: f64,
    first_layer_height: f64,
) -> Vec<SliceLayer> {
    let heights = compute_layer_heights(mesh.aabb(), layer_height, first_layer_height);

    heights
        .into_iter()
        .map(|(z, lh)| {
            let contours = slice_at_height(mesh, z);
            SliceLayer {
                z,
                layer_height: lh,
                contours,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::Point3;

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
    fn compute_layer_heights_1mm_cube_02mm_layers() {
        let aabb = BBox3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0));
        let heights = compute_layer_heights(&aabb, 0.2, 0.2);

        // First layer at z=0.1, then z=0.3, 0.5, 0.7, 0.9 = 5 layers
        assert_eq!(
            heights.len(),
            5,
            "1mm cube with 0.2mm layers should produce 5 layers, got {:?}",
            heights
        );

        // First layer height
        assert!(
            (heights[0].0 - 0.1).abs() < 1e-9,
            "First layer z should be 0.1, got {}",
            heights[0].0
        );
        assert!(
            (heights[0].1 - 0.2).abs() < 1e-9,
            "First layer height should be 0.2, got {}",
            heights[0].1
        );

        // Last layer
        assert!(
            (heights[4].0 - 0.9).abs() < 1e-9,
            "Last layer z should be 0.9, got {}",
            heights[4].0
        );
    }

    #[test]
    fn compute_layer_heights_with_different_first_layer() {
        let aabb = BBox3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0));
        let heights = compute_layer_heights(&aabb, 0.2, 0.3);

        // First layer at z=0.15 (0.3/2), then at z=0.3 + 0.1 = 0.4, 0.6, 0.8, 1.0
        assert!(
            (heights[0].0 - 0.15).abs() < 1e-9,
            "First layer z should be 0.15, got {}",
            heights[0].0
        );
        assert!(
            (heights[0].1 - 0.3).abs() < 1e-9,
            "First layer height should be 0.3, got {}",
            heights[0].1
        );
    }

    #[test]
    fn compute_layer_heights_very_thin_object() {
        let aabb = BBox3::new(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.05));
        let heights = compute_layer_heights(&aabb, 0.2, 0.2);

        // Object is only 0.05mm tall, first layer z=0.1 > max_z
        // Should still produce a layer if first layer midpoint <= max_z
        // z=0.1 > 0.05, so no layers
        assert!(
            heights.is_empty(),
            "Object thinner than half first layer should produce no layers"
        );
    }

    #[test]
    fn slice_mesh_unit_cube() {
        let mesh = unit_cube();
        let layers = slice_mesh(&mesh, 0.2, 0.2);

        assert_eq!(
            layers.len(),
            5,
            "Unit cube with 0.2mm layers should produce 5 layers"
        );

        // Each layer should have exactly 1 contour (square cross-section)
        for (i, layer) in layers.iter().enumerate() {
            assert_eq!(
                layer.contours.len(),
                1,
                "Layer {} at z={} should have 1 contour, got {}",
                i,
                layer.z,
                layer.contours.len()
            );

            // Each contour should be approximately a 1mm x 1mm square
            let area = layer.contours[0].area_mm2();
            assert!(
                (area - 1.0).abs() < 0.01,
                "Layer {} contour area should be ~1.0 mm^2, got {} mm^2",
                i,
                area
            );
        }
    }

    #[test]
    fn slice_mesh_layer_z_values_are_monotonic() {
        let mesh = unit_cube();
        let layers = slice_mesh(&mesh, 0.2, 0.2);

        for i in 1..layers.len() {
            assert!(
                layers[i].z > layers[i - 1].z,
                "Layer Z values should be monotonically increasing"
            );
        }
    }
}
