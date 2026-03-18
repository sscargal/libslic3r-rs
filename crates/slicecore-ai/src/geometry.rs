//! Geometry feature extraction from triangle meshes.
//!
//! Analyzes a [`TriangleMesh`] to extract structured features used for
//! AI-based print profile suggestion. Features include bounding box dimensions,
//! volume, surface area, overhang analysis, and difficulty classification.

use serde::{Deserialize, Serialize};
use slicecore_math::Vec3;
use slicecore_mesh::{compute_stats, TriangleMesh};

/// Physical dimensions of a 3D model in millimeters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimensions {
    /// Width along the X axis in mm.
    pub width_mm: f64,
    /// Depth along the Y axis in mm.
    pub depth_mm: f64,
    /// Height along the Z axis in mm.
    pub height_mm: f64,
}

/// Classification of expected print difficulty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrintDifficulty {
    /// Simple geometry, no significant overhangs or thin features.
    Easy,
    /// Moderate overhangs or thin features present.
    Medium,
    /// Significant overhangs, very thin features, or very tall model.
    Hard,
}

/// Structured geometry features extracted from a triangle mesh.
///
/// These features are serialized to JSON and included in prompts sent to
/// LLMs for print profile suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryFeatures {
    /// Minimum corner of the axis-aligned bounding box [x, y, z].
    pub bounding_box_min: [f64; 3],
    /// Maximum corner of the axis-aligned bounding box [x, y, z].
    pub bounding_box_max: [f64; 3],
    /// Mesh volume in cubic millimeters.
    pub volume_mm3: f64,
    /// Total mesh surface area in square millimeters.
    pub surface_area_mm2: f64,
    /// Number of triangles in the mesh.
    pub triangle_count: usize,
    /// Whether the mesh is watertight (manifold, no boundary edges).
    pub is_watertight: bool,
    /// Physical dimensions computed from the bounding box.
    pub dimensions: Dimensions,
    /// Ratio of largest to smallest bounding box dimension.
    pub aspect_ratio: f64,
    /// Fraction of surface area that is overhang (>45 degrees from vertical).
    pub overhang_ratio: f64,
    /// Steepest downward-facing angle in degrees (0 = horizontal, 90 = vertical).
    pub max_overhang_angle_deg: f64,
    /// Heuristic: fraction of bounding box dimensions < 2mm.
    pub thin_wall_ratio: f64,
    /// Whether bridges are likely present (overhang_ratio > 0.05).
    pub has_bridges: bool,
    /// Whether small features are present (any dimension < 1mm).
    pub has_small_features: bool,
    /// Estimated print difficulty classification.
    pub estimated_difficulty: PrintDifficulty,
}

/// Extracts structured geometry features from a triangle mesh.
///
/// This function computes mesh statistics (volume, surface area, watertight status),
/// analyzes overhang surfaces by examining face normals against the Z-up direction,
/// and classifies print difficulty based on the extracted features.
///
/// # Overhang Analysis
///
/// For each downward-facing triangle (normal dot Z-up < 0), the angle from
/// vertical is computed. If the surface is more than 45 degrees from vertical
/// toward horizontal (i.e., the overhang angle from horizontal is < 45 degrees),
/// the triangle's area is counted as overhang area.
///
/// # Example
///
/// ```rust,ignore
/// use slicecore_ai::geometry::extract_geometry_features;
/// use slicecore_mesh::TriangleMesh;
///
/// let mesh = /* ... */;
/// let features = extract_geometry_features(&mesh);
/// println!("Volume: {} mm3", features.volume_mm3);
/// println!("Difficulty: {:?}", features.estimated_difficulty);
/// ```
pub fn extract_geometry_features(mesh: &TriangleMesh) -> GeometryFeatures {
    // 1. Compute mesh statistics via slicecore-mesh.
    let stats = compute_stats(mesh);

    let aabb = stats.aabb;
    let bounding_box_min = [aabb.min.x, aabb.min.y, aabb.min.z];
    let bounding_box_max = [aabb.max.x, aabb.max.y, aabb.max.z];

    // 2. Compute dimensions from AABB.
    let width = aabb.max.x - aabb.min.x;
    let depth = aabb.max.y - aabb.min.y;
    let height = aabb.max.z - aabb.min.z;
    let dimensions = Dimensions {
        width_mm: width,
        depth_mm: depth,
        height_mm: height,
    };

    // 3. Compute aspect ratio (max_dim / min_dim), clamping min to avoid division by zero.
    let dims = [width, depth, height];
    let max_dim = dims.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_dim = dims
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min)
        .max(0.001);
    let aspect_ratio = max_dim / min_dim;

    // 4. Overhang analysis.
    let z_up = Vec3::new(0.0, 0.0, 1.0);
    let normals = mesh.normals();
    let mut overhang_area = 0.0f64;
    let mut max_overhang_angle_deg = 0.0f64;
    let triangle_count = mesh.triangle_count();

    for (i, normal) in normals.iter().enumerate() {
        let dot = normal.dot(z_up);

        // Downward-facing: dot < 0 means normal points downward.
        if dot < 0.0 {
            // Angle of the face from horizontal plane.
            // dot = cos(angle between normal and Z-up)
            // If dot = -1, normal points straight down (face is horizontal, 0 degrees from horizontal)
            // If dot = 0, normal is horizontal (face is vertical, 90 degrees from horizontal)
            // angle_from_horizontal = acos(-dot) mapped from [0, PI/2]
            // Actually: angle between normal and -Z = acos(-dot)
            // This gives angle of the normal from straight-down direction.
            // When angle_from_horizontal = 0, face is horizontal (worst overhang).
            // When angle_from_horizontal = 90, face is vertical (no overhang).
            //
            // We count as overhang if the face is more than 45 degrees from vertical,
            // i.e., angle_from_horizontal < 45 degrees, i.e., acos(-dot) < 45 degrees.
            // Equivalently: -dot > cos(45) = 0.7071...
            //
            // But the plan says: "If angle from vertical > 45 degrees"
            // angle_from_vertical = 90 - angle_from_horizontal = 90 - acos(-dot)*180/PI
            // If angle_from_vertical > 45, then angle_from_horizontal < 45.
            // So overhang when acos(-dot) < PI/4, i.e., -dot > cos(PI/4).
            let neg_dot = -dot;
            let angle_from_down_rad = neg_dot.clamp(-1.0, 1.0).acos();
            let angle_from_down_deg = angle_from_down_rad.to_degrees();

            // angle_from_down_deg is 0 when face is horizontal (straight down),
            // 90 when face is vertical.
            // Overhang when angle_from_down_deg < 45 (face is more than 45 deg from vertical).
            if angle_from_down_deg < 45.0 {
                // Compute triangle area via cross product.
                let [v0, v1, v2] = mesh.triangle_vertices(i);
                let edge1 = Vec3::from_points(v0, v1);
                let edge2 = Vec3::from_points(v0, v2);
                let cross = edge1.cross(edge2);
                let tri_area = cross.length() * 0.5;
                overhang_area += tri_area;
            }

            // Track max overhang angle (angle from horizontal).
            // Smaller angle_from_down_deg means steeper overhang (more horizontal).
            // We want to report the steepest (smallest angle from horizontal = largest overhang angle from vertical).
            // max_overhang_angle_deg tracks the steepest downward angle in degrees from horizontal.
            // Actually, the plan says "steepest downward-facing angle in degrees".
            // Let's report angle_from_down_deg for the steepest face (smallest value = most horizontal).
            // But for a "max overhang angle" field, the convention is typically the most extreme overhang.
            // We track the maximum of (90 - angle_from_down_deg) which is angle from vertical.
            let angle_from_vertical = 90.0 - angle_from_down_deg;
            if angle_from_vertical > max_overhang_angle_deg {
                max_overhang_angle_deg = angle_from_vertical;
            }
        }
    }

    let total_surface_area = stats.surface_area;
    let overhang_ratio = if total_surface_area > 0.0 {
        overhang_area / total_surface_area
    } else {
        0.0
    };

    // 5. Thin wall heuristic: count how many of [width, depth, height] are < 2.0mm.
    let thin_count = dims.iter().filter(|&&d| d < 2.0).count();
    let thin_wall_ratio = thin_count as f64 / 3.0;

    // 6. Bridges heuristic.
    let has_bridges = overhang_ratio > 0.05;

    // 7. Small features heuristic.
    let actual_min_dim = dims.iter().cloned().fold(f64::INFINITY, f64::min);
    let has_small_features = actual_min_dim < 1.0;

    // 8. Difficulty classification.
    let estimated_difficulty = classify_difficulty(overhang_ratio, actual_min_dim, height);

    GeometryFeatures {
        bounding_box_min,
        bounding_box_max,
        volume_mm3: stats.volume.abs(),
        surface_area_mm2: total_surface_area,
        triangle_count,
        is_watertight: stats.is_watertight,
        dimensions,
        aspect_ratio,
        overhang_ratio,
        max_overhang_angle_deg,
        thin_wall_ratio,
        has_bridges,
        has_small_features,
        estimated_difficulty,
    }
}

/// Classifies print difficulty based on overhang ratio, minimum dimension, and height.
fn classify_difficulty(overhang_ratio: f64, min_dim: f64, height: f64) -> PrintDifficulty {
    if overhang_ratio > 0.15 || min_dim < 0.5 || height > 150.0 {
        PrintDifficulty::Hard
    } else if overhang_ratio > 0.05 || min_dim < 2.0 || height > 80.0 {
        PrintDifficulty::Medium
    } else {
        PrintDifficulty::Easy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::Point3;

    /// Creates a unit cube mesh (vertices from (0,0,0) to (1,1,1)) with 12 triangles.
    /// Winding order produces outward-facing normals.
    fn unit_cube() -> TriangleMesh {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0), // 0: left-bottom-back
            Point3::new(1.0, 0.0, 0.0), // 1: right-bottom-back
            Point3::new(1.0, 1.0, 0.0), // 2: right-top-back
            Point3::new(0.0, 1.0, 0.0), // 3: left-top-back
            Point3::new(0.0, 0.0, 1.0), // 4: left-bottom-front
            Point3::new(1.0, 0.0, 1.0), // 5: right-bottom-front
            Point3::new(1.0, 1.0, 1.0), // 6: right-top-front
            Point3::new(0.0, 1.0, 1.0), // 7: left-top-front
        ];

        // Two triangles per face, 6 faces = 12 triangles.
        // Winding order: outward-facing normals (CCW when viewed from outside).
        let indices = vec![
            // Front face (z=1): 4,5,6 and 4,6,7
            [4, 5, 6],
            [4, 6, 7],
            // Back face (z=0): 1,0,3 and 1,3,2
            [1, 0, 3],
            [1, 3, 2],
            // Right face (x=1): 1,2,6 and 1,6,5
            [1, 2, 6],
            [1, 6, 5],
            // Left face (x=0): 0,4,7 and 0,7,3
            [0, 4, 7],
            [0, 7, 3],
            // Top face (y=1): 3,7,6 and 3,6,2
            [3, 7, 6],
            [3, 6, 2],
            // Bottom face (y=0): 0,1,5 and 0,5,4
            [0, 1, 5],
            [0, 5, 4],
        ];

        TriangleMesh::new(vertices, indices).expect("unit cube should be valid")
    }

    #[test]
    fn unit_cube_dimensions() {
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        assert!(
            (features.dimensions.width_mm - 1.0).abs() < 1e-6,
            "Expected width ~1.0, got {}",
            features.dimensions.width_mm
        );
        assert!(
            (features.dimensions.depth_mm - 1.0).abs() < 1e-6,
            "Expected depth ~1.0, got {}",
            features.dimensions.depth_mm
        );
        assert!(
            (features.dimensions.height_mm - 1.0).abs() < 1e-6,
            "Expected height ~1.0, got {}",
            features.dimensions.height_mm
        );
    }

    #[test]
    fn unit_cube_aspect_ratio() {
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        assert!(
            (features.aspect_ratio - 1.0).abs() < 1e-6,
            "Expected aspect ratio ~1.0 for cube, got {}",
            features.aspect_ratio
        );
    }

    #[test]
    fn unit_cube_volume_and_surface_area() {
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        assert!(
            (features.volume_mm3 - 1.0).abs() < 1e-6,
            "Expected volume ~1.0 mm3, got {}",
            features.volume_mm3
        );
        assert!(
            (features.surface_area_mm2 - 6.0).abs() < 1e-6,
            "Expected surface area ~6.0 mm2, got {}",
            features.surface_area_mm2
        );
    }

    #[test]
    fn unit_cube_is_watertight() {
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        assert!(
            features.is_watertight,
            "Unit cube should be reported as watertight"
        );
    }

    #[test]
    fn unit_cube_triangle_count() {
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        assert_eq!(
            features.triangle_count, 12,
            "Unit cube should have 12 triangles"
        );
    }

    #[test]
    fn unit_cube_overhang_analysis() {
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        // The unit cube has 2 bottom face triangles (normals pointing -Y direction
        // in slicecore-mesh convention). Actually the bottom face (y=0) has normals
        // pointing in the -Y direction, but the Z-up overhang check uses the Z-axis.
        // The bottom face (indices [0,1,5] and [0,5,4]) - these are in the y=0 plane
        // with vertices at z=0 and z=1.
        //
        // Wait - looking at the cube definition:
        // Bottom face (y=0): vertices 0,1,5,4 - these have y=0.
        // The normal for [0,1,5]: edge1 = (1,0,0), edge2 = (1,0,1)
        // cross = (0*1 - 0*0, 0*1 - 1*1, 1*0 - 0*1) = (0, -1, 0)
        // So the bottom face normal is (0, -1, 0), not downward in Z.
        //
        // Actually the cube uses Y as the vertical axis for the "top/bottom" face names,
        // but in 3D printing Z is typically vertical. The Z-facing faces are:
        // - Front face (z=1): normal (0,0,1) - Z up, not overhang
        // - Back face (z=0): normal (0,0,-1) - Z down, IS overhang
        //
        // The back face (z=0) has normal (0,0,-1), dot with Z-up = -1.
        // angle_from_down = acos(1.0) = 0 degrees. Since 0 < 45, it IS overhang.
        // Back face area = 2 * 0.5 * |cross| for each triangle = 1.0 total.
        //
        // overhang_ratio = 1.0 / 6.0 = ~0.167
        assert!(
            features.overhang_ratio > 0.1,
            "Expected overhang ratio > 0.1 for cube (bottom face), got {}",
            features.overhang_ratio
        );

        // Max overhang angle should be 90 degrees (face is perfectly horizontal, pointing down)
        assert!(
            features.max_overhang_angle_deg > 80.0,
            "Expected max overhang angle > 80 degrees, got {}",
            features.max_overhang_angle_deg
        );
    }

    #[test]
    fn unit_cube_difficulty_classification() {
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        // Cube: 1mm dimensions -> min_dim < 2.0 -> Medium,
        // but also overhang_ratio > 0.15 -> Hard
        assert_eq!(
            features.estimated_difficulty,
            PrintDifficulty::Hard,
            "1mm cube with overhangs should be classified as Hard"
        );
    }

    #[test]
    fn unit_cube_bounding_box() {
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        for i in 0..3 {
            assert!(
                features.bounding_box_min[i].abs() < 1e-6,
                "bounding_box_min[{}] should be ~0.0, got {}",
                i,
                features.bounding_box_min[i]
            );
            assert!(
                (features.bounding_box_max[i] - 1.0).abs() < 1e-6,
                "bounding_box_max[{}] should be ~1.0, got {}",
                i,
                features.bounding_box_max[i]
            );
        }
    }

    #[test]
    fn unit_cube_thin_wall_and_small_features() {
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        // All 3 dimensions are 1.0mm, which is < 2.0, so thin_wall_ratio = 3/3 = 1.0
        assert!(
            (features.thin_wall_ratio - 1.0).abs() < 1e-6,
            "Expected thin_wall_ratio ~1.0 for 1mm cube, got {}",
            features.thin_wall_ratio
        );

        // All dimensions are 1.0mm, none < 1.0, so has_small_features = false
        assert!(
            !features.has_small_features,
            "1mm cube should not have small features (dimensions are exactly 1.0)"
        );
    }

    #[test]
    fn geometry_features_serializes_to_json() {
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        let json = serde_json::to_string_pretty(&features).unwrap();
        assert!(json.contains("volume_mm3"));
        assert!(json.contains("overhang_ratio"));
        assert!(json.contains("estimated_difficulty"));
    }

    #[test]
    fn classify_difficulty_easy() {
        assert_eq!(classify_difficulty(0.03, 5.0, 50.0), PrintDifficulty::Easy);
    }

    #[test]
    fn classify_difficulty_medium_by_overhang() {
        assert_eq!(
            classify_difficulty(0.10, 5.0, 50.0),
            PrintDifficulty::Medium
        );
    }

    #[test]
    fn classify_difficulty_medium_by_thin_wall() {
        assert_eq!(
            classify_difficulty(0.03, 1.5, 50.0),
            PrintDifficulty::Medium
        );
    }

    #[test]
    fn classify_difficulty_hard_by_overhang() {
        assert_eq!(classify_difficulty(0.20, 5.0, 50.0), PrintDifficulty::Hard);
    }

    #[test]
    fn classify_difficulty_hard_by_height() {
        assert_eq!(classify_difficulty(0.03, 5.0, 200.0), PrintDifficulty::Hard);
    }

    #[test]
    fn classify_difficulty_hard_by_thin() {
        assert_eq!(classify_difficulty(0.03, 0.3, 50.0), PrintDifficulty::Hard);
    }
}
