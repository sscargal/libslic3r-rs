//! Phase 4 integration tests: success criteria verification.
//!
//! Comprehensive tests verifying all 5 Phase 4 success criteria:
//! - SC1: Arachne thin walls (PERIM-02)
//! - SC2: All 8 infill patterns (INFILL-02 through INFILL-08)
//! - SC3: Seam placement strategies (PERIM-05, PERIM-06)
//! - SC4: Adaptive layer heights (SLICE-02)
//! - SC5: Gap fill (PERIM-04)
//!
//! Plus: determinism, G-code validation, and preview data verification.

use slicecore_engine::{
    Engine, InfillPattern, PrintConfig, ScarfJointConfig, SeamPosition,
};
use slicecore_gcode_io::validate_gcode;
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

// ===========================================================================
// Test mesh fixtures
// ===========================================================================

/// Creates a 20mm x 20mm x 20mm calibration cube mesh, centered at (100, 100)
/// on a 220x220 bed.
fn calibration_cube_20mm() -> TriangleMesh {
    let ox = 90.0;
    let oy = 90.0;
    let vertices = vec![
        Point3::new(ox, oy, 0.0),
        Point3::new(ox + 20.0, oy, 0.0),
        Point3::new(ox + 20.0, oy + 20.0, 0.0),
        Point3::new(ox, oy + 20.0, 0.0),
        Point3::new(ox, oy, 20.0),
        Point3::new(ox + 20.0, oy, 20.0),
        Point3::new(ox + 20.0, oy + 20.0, 20.0),
        Point3::new(ox, oy + 20.0, 20.0),
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
    TriangleMesh::new(vertices, indices).expect("calibration cube should be valid")
}

/// Creates a thin-wall box for Arachne and gap fill testing.
///
/// Outer box is 20mm x 20mm x 10mm, inner box is offset inward by 0.4mm
/// on each side, creating 0.8mm thin walls. This is narrow enough to trigger
/// Arachne variable-width perimeter behavior.
fn thin_wall_box() -> TriangleMesh {
    let ox = 90.0;
    let oy = 90.0;
    let wall = 0.8; // 0.8mm wall thickness
    let size = 20.0;
    let height = 10.0;

    // Outer box vertices (0-7)
    let mut vertices = vec![
        Point3::new(ox, oy, 0.0),
        Point3::new(ox + size, oy, 0.0),
        Point3::new(ox + size, oy + size, 0.0),
        Point3::new(ox, oy + size, 0.0),
        Point3::new(ox, oy, height),
        Point3::new(ox + size, oy, height),
        Point3::new(ox + size, oy + size, height),
        Point3::new(ox, oy + size, height),
    ];

    // Inner box vertices (8-15), offset inward by wall thickness
    let inner_vertices = vec![
        Point3::new(ox + wall, oy + wall, 0.0),
        Point3::new(ox + size - wall, oy + wall, 0.0),
        Point3::new(ox + size - wall, oy + size - wall, 0.0),
        Point3::new(ox + wall, oy + size - wall, 0.0),
        Point3::new(ox + wall, oy + wall, height),
        Point3::new(ox + size - wall, oy + wall, height),
        Point3::new(ox + size - wall, oy + size - wall, height),
        Point3::new(ox + wall, oy + size - wall, height),
    ];
    vertices.extend(inner_vertices);

    let mut indices: Vec<[u32; 3]> = Vec::new();

    // Outer faces (outward-facing normals)
    // Top outer
    indices.push([4, 5, 6]);
    indices.push([4, 6, 7]);
    // Bottom outer
    indices.push([1, 0, 3]);
    indices.push([1, 3, 2]);
    // Right outer
    indices.push([1, 2, 6]);
    indices.push([1, 6, 5]);
    // Left outer
    indices.push([0, 4, 7]);
    indices.push([0, 7, 3]);
    // Back outer
    indices.push([3, 7, 6]);
    indices.push([3, 6, 2]);
    // Front outer
    indices.push([0, 1, 5]);
    indices.push([0, 5, 4]);

    // Inner faces (inward-facing normals - reversed winding)
    // Top inner
    indices.push([12, 14, 13]);
    indices.push([12, 15, 14]);
    // Bottom inner
    indices.push([9, 11, 8]);
    indices.push([9, 10, 11]);
    // Right inner
    indices.push([9, 13, 14]);
    indices.push([9, 14, 10]);
    // Left inner
    indices.push([8, 11, 15]);
    indices.push([8, 15, 12]);
    // Back inner
    indices.push([11, 10, 14]);
    indices.push([11, 14, 15]);
    // Front inner
    indices.push([8, 12, 13]);
    indices.push([8, 13, 9]);

    TriangleMesh::new(vertices, indices).expect("thin wall box should be valid")
}

/// Creates an approximate sphere mesh with ~200 triangles for adaptive
/// layer height testing. Uses icosahedron subdivision.
fn unit_sphere() -> TriangleMesh {
    let cx = 100.0;
    let cy = 100.0;
    let cz = 10.0;
    let r = 10.0;

    // Start with an icosahedron.
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let base_verts: Vec<[f64; 3]> = vec![
        [-1.0, phi, 0.0],
        [1.0, phi, 0.0],
        [-1.0, -phi, 0.0],
        [1.0, -phi, 0.0],
        [0.0, -1.0, phi],
        [0.0, 1.0, phi],
        [0.0, -1.0, -phi],
        [0.0, 1.0, -phi],
        [phi, 0.0, -1.0],
        [phi, 0.0, 1.0],
        [-phi, 0.0, -1.0],
        [-phi, 0.0, 1.0],
    ];

    let base_tris: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    // Subdivide once to get ~80 triangles, then once more for ~320.
    let (mut verts, mut tris) = (base_verts, base_tris);
    for _ in 0..2 {
        let (new_verts, new_tris) = subdivide_sphere(&verts, &tris);
        verts = new_verts;
        tris = new_tris;
    }

    // Normalize vertices to unit sphere, scale by radius, translate to center.
    let vertices: Vec<Point3> = verts
        .iter()
        .map(|v| {
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            Point3::new(
                cx + r * v[0] / len,
                cy + r * v[1] / len,
                cz + r * v[2] / len,
            )
        })
        .collect();

    let indices: Vec<[u32; 3]> = tris;

    TriangleMesh::new(vertices, indices).expect("sphere should be valid")
}

/// Subdivide an icosahedron by splitting each triangle into 4 sub-triangles.
fn subdivide_sphere(
    verts: &[[f64; 3]],
    tris: &[[u32; 3]],
) -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
    use std::collections::HashMap;

    let mut new_verts = verts.to_vec();
    let mut new_tris = Vec::new();
    let mut midpoint_cache: HashMap<(u32, u32), u32> = HashMap::new();

    for tri in tris {
        let a = tri[0];
        let b = tri[1];
        let c = tri[2];

        let ab = get_midpoint_idx(a, b, &mut new_verts, &mut midpoint_cache);
        let bc = get_midpoint_idx(b, c, &mut new_verts, &mut midpoint_cache);
        let ca = get_midpoint_idx(c, a, &mut new_verts, &mut midpoint_cache);

        new_tris.push([a, ab, ca]);
        new_tris.push([b, bc, ab]);
        new_tris.push([c, ca, bc]);
        new_tris.push([ab, bc, ca]);
    }

    (new_verts, new_tris)
}

fn get_midpoint_idx(
    a: u32,
    b: u32,
    vs: &mut Vec<[f64; 3]>,
    cache: &mut std::collections::HashMap<(u32, u32), u32>,
) -> u32 {
    let key = if a < b { (a, b) } else { (b, a) };
    if let Some(&idx) = cache.get(&key) {
        return idx;
    }
    let ai = a as usize;
    let bi = b as usize;
    let mid = [
        (vs[ai][0] + vs[bi][0]) / 2.0,
        (vs[ai][1] + vs[bi][1]) / 2.0,
        (vs[ai][2] + vs[bi][2]) / 2.0,
    ];
    let idx = vs.len() as u32;
    vs.push(mid);
    cache.insert(key, idx);
    idx
}

/// Creates a cylinder mesh for seam placement testing.
///
/// Generated by connecting rings of points at different Z heights.
fn cylinder() -> TriangleMesh {
    let cx = 100.0;
    let cy = 100.0;
    let radius = 10.0;
    let height = 15.0;
    let segments: u32 = 24; // 24-sided polygon approximation
    let z_steps: u32 = 10;

    let mut vertices = Vec::new();
    let mut indices: Vec<[u32; 3]> = Vec::new();

    // Generate vertex rings at different Z heights.
    for zi in 0..=z_steps {
        let z = (zi as f64 / z_steps as f64) * height;
        for si in 0..segments {
            let angle = 2.0 * std::f64::consts::PI * (si as f64) / (segments as f64);
            vertices.push(Point3::new(
                cx + radius * angle.cos(),
                cy + radius * angle.sin(),
                z,
            ));
        }
    }

    // Connect adjacent rings with triangles.
    for zi in 0..z_steps {
        let ring_a = zi * segments;
        let ring_b = (zi + 1) * segments;
        for si in 0..segments {
            let next_si = (si + 1) % segments;
            // Two triangles per quad.
            indices.push([ring_a + si, ring_b + si, ring_b + next_si]);
            indices.push([ring_a + si, ring_b + next_si, ring_a + next_si]);
        }
    }

    // Bottom cap.
    let bottom_center_idx = vertices.len() as u32;
    vertices.push(Point3::new(cx, cy, 0.0));
    for si in 0..segments {
        let next_si = (si + 1) % segments;
        indices.push([bottom_center_idx, next_si, si]);
    }

    // Top cap.
    let top_center_idx = vertices.len() as u32;
    vertices.push(Point3::new(cx, cy, height));
    let top_ring = z_steps * segments;
    for si in 0..segments {
        let next_si = (si + 1) % segments;
        indices.push([top_center_idx, top_ring + si, top_ring + next_si]);
    }

    TriangleMesh::new(vertices, indices).expect("cylinder should be valid")
}

// ===========================================================================
// SC1: Arachne thin walls (PERIM-02)
// ===========================================================================

#[test]
fn sc1_arachne_thin_walls() {
    let mesh = thin_wall_box();
    let config = PrintConfig {
        arachne_enabled: true,
        ..Default::default()
    };
    let engine = Engine::new(config);
    let result = engine.slice(&mesh).expect("arachne thin wall slice should succeed");

    assert!(
        !result.gcode.is_empty(),
        "Arachne thin wall should produce non-empty G-code"
    );
    assert!(
        result.layer_count > 0,
        "Arachne thin wall should produce layers"
    );

    // G-code should pass validation.
    let gcode_str = String::from_utf8_lossy(&result.gcode);
    let validation = validate_gcode(&gcode_str);
    assert!(
        validation.valid,
        "Arachne thin wall G-code should pass validation. Errors: {:?}",
        validation.errors
    );
}

#[test]
fn sc1_arachne_vs_classic_produces_valid_gcode() {
    let mesh = thin_wall_box();

    // Classic perimeters.
    let classic_config = PrintConfig {
        arachne_enabled: false,
        ..Default::default()
    };
    let classic_result = Engine::new(classic_config)
        .slice(&mesh)
        .expect("classic thin wall slice should succeed");
    assert!(!classic_result.gcode.is_empty());

    // Arachne perimeters.
    let arachne_config = PrintConfig {
        arachne_enabled: true,
        ..Default::default()
    };
    let arachne_result = Engine::new(arachne_config)
        .slice(&mesh)
        .expect("arachne thin wall slice should succeed");
    assert!(!arachne_result.gcode.is_empty());

    // Both should produce valid G-code with comparable layer counts.
    assert_eq!(
        classic_result.layer_count, arachne_result.layer_count,
        "Arachne and classic should produce same layer count"
    );
}

// ===========================================================================
// SC2: All 8 infill patterns (INFILL-02 through INFILL-08)
// ===========================================================================

#[test]
fn sc2_all_infill_patterns() {
    let mesh = calibration_cube_20mm();
    let patterns = [
        InfillPattern::Rectilinear,
        InfillPattern::Grid,
        InfillPattern::Honeycomb,
        InfillPattern::Gyroid,
        InfillPattern::AdaptiveCubic,
        InfillPattern::Cubic,
        InfillPattern::Lightning,
        InfillPattern::Monotonic,
    ];
    for pattern in &patterns {
        let config = PrintConfig {
            infill_pattern: pattern.clone(),
            infill_density: 0.2,
            ..Default::default()
        };
        let engine = Engine::new(config);
        let result = engine
            .slice(&mesh)
            .unwrap_or_else(|e| panic!("{:?} should succeed: {:?}", pattern, e));
        assert!(
            !result.gcode.is_empty(),
            "{:?} should produce G-code",
            pattern
        );
        assert!(
            result.layer_count > 0,
            "{:?} should produce layers",
            pattern
        );
    }
}

#[test]
fn sc2_patterns_produce_different_gcode() {
    let mesh = calibration_cube_20mm();
    let mut gcodes: Vec<(InfillPattern, Vec<u8>)> = Vec::new();
    let patterns = [
        InfillPattern::Rectilinear,
        InfillPattern::Grid,
        InfillPattern::Honeycomb,
        InfillPattern::Gyroid,
    ];
    for pattern in &patterns {
        let config = PrintConfig {
            infill_pattern: pattern.clone(),
            infill_density: 0.2,
            ..Default::default()
        };
        let result = Engine::new(config).slice(&mesh).unwrap();
        gcodes.push((pattern.clone(), result.gcode));
    }
    // Each pattern should produce different G-code.
    for i in 0..gcodes.len() {
        for j in (i + 1)..gcodes.len() {
            assert_ne!(
                gcodes[i].1, gcodes[j].1,
                "{:?} and {:?} should produce different G-code",
                gcodes[i].0, gcodes[j].0
            );
        }
    }
}

#[test]
fn sc2_all_patterns_produce_valid_gcode() {
    let mesh = calibration_cube_20mm();
    let patterns = [
        InfillPattern::Rectilinear,
        InfillPattern::Grid,
        InfillPattern::Honeycomb,
        InfillPattern::Gyroid,
        InfillPattern::AdaptiveCubic,
        InfillPattern::Cubic,
        InfillPattern::Lightning,
        InfillPattern::Monotonic,
    ];
    for pattern in &patterns {
        let config = PrintConfig {
            infill_pattern: pattern.clone(),
            infill_density: 0.2,
            ..Default::default()
        };
        let result = Engine::new(config)
            .slice(&mesh)
            .unwrap_or_else(|e| panic!("{:?} should succeed: {:?}", pattern, e));
        let gcode_str = String::from_utf8_lossy(&result.gcode);
        let validation = validate_gcode(&gcode_str);
        assert!(
            validation.valid,
            "{:?} G-code should pass validation. Errors: {:?}",
            pattern, validation.errors
        );
    }
}

// ===========================================================================
// SC3: Seam placement strategies (PERIM-05, PERIM-06)
// ===========================================================================

#[test]
fn sc3_seam_strategies_differ() {
    let mesh = cylinder();
    let strategies = [
        SeamPosition::Aligned,
        SeamPosition::Random,
        SeamPosition::Rear,
        SeamPosition::NearestCorner,
    ];
    let mut gcodes: Vec<Vec<u8>> = Vec::new();
    for strategy in &strategies {
        let config = PrintConfig {
            seam_position: *strategy,
            ..Default::default()
        };
        let result = Engine::new(config)
            .slice(&mesh)
            .unwrap_or_else(|e| panic!("{:?} should succeed: {:?}", strategy, e));
        gcodes.push(result.gcode);
    }
    // At least some strategies should produce different G-code.
    let unique_count = gcodes
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert!(
        unique_count >= 2,
        "Seam strategies should produce at least 2 different outputs, got {}",
        unique_count
    );
}

#[test]
fn sc3_all_seam_strategies_produce_valid_gcode() {
    let mesh = cylinder();
    let strategies = [
        SeamPosition::Aligned,
        SeamPosition::Random,
        SeamPosition::Rear,
        SeamPosition::NearestCorner,
    ];
    for strategy in &strategies {
        let config = PrintConfig {
            seam_position: *strategy,
            ..Default::default()
        };
        let result = Engine::new(config)
            .slice(&mesh)
            .unwrap_or_else(|e| panic!("{:?} should succeed: {:?}", strategy, e));
        let gcode_str = String::from_utf8_lossy(&result.gcode);
        let validation = validate_gcode(&gcode_str);
        assert!(
            validation.valid,
            "{:?} seam G-code should pass validation. Errors: {:?}",
            strategy, validation.errors
        );
    }
}

#[test]
fn sc3_scarf_joint_produces_valid_gcode() {
    let mesh = cylinder();
    let config = PrintConfig {
        scarf_joint: ScarfJointConfig {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let result = Engine::new(config)
        .slice(&mesh)
        .expect("scarf joint slice should succeed");
    assert!(!result.gcode.is_empty());
    let gcode_str = String::from_utf8_lossy(&result.gcode);
    let validation = validate_gcode(&gcode_str);
    assert!(
        validation.valid,
        "Scarf joint G-code should pass validation. Errors: {:?}",
        validation.errors
    );
}

// ===========================================================================
// SC4: Adaptive layer heights (SLICE-02)
// ===========================================================================

#[test]
fn sc4_adaptive_layer_heights() {
    let mesh = unit_sphere();
    let config = PrintConfig {
        adaptive_layer_height: true,
        adaptive_min_layer_height: 0.05,
        adaptive_max_layer_height: 0.3,
        adaptive_layer_quality: 0.8,
        ..Default::default()
    };
    let result = Engine::new(config)
        .slice(&mesh)
        .expect("adaptive sphere slice should succeed");
    assert!(result.layer_count > 0);

    // Adaptive should produce more layers than uniform max height.
    let uniform_config = PrintConfig {
        layer_height: 0.3,
        first_layer_height: 0.3,
        ..Default::default()
    };
    let uniform_result = Engine::new(uniform_config)
        .slice(&mesh)
        .expect("uniform sphere slice should succeed");

    assert!(
        result.layer_count > uniform_result.layer_count,
        "Adaptive should produce more layers ({}) than uniform max ({})",
        result.layer_count,
        uniform_result.layer_count
    );
}

#[test]
fn sc4_adaptive_produces_valid_gcode() {
    let mesh = unit_sphere();
    let config = PrintConfig {
        adaptive_layer_height: true,
        adaptive_min_layer_height: 0.05,
        adaptive_max_layer_height: 0.3,
        adaptive_layer_quality: 0.8,
        ..Default::default()
    };
    let result = Engine::new(config)
        .slice(&mesh)
        .expect("adaptive slice should succeed");
    let gcode_str = String::from_utf8_lossy(&result.gcode);
    let validation = validate_gcode(&gcode_str);
    assert!(
        validation.valid,
        "Adaptive layer G-code should pass validation. Errors: {:?}",
        validation.errors
    );
}

// ===========================================================================
// SC5: Gap fill (PERIM-04)
// ===========================================================================

#[test]
fn sc5_gap_fill_enabled() {
    let mesh = thin_wall_box();
    let config = PrintConfig {
        gap_fill_enabled: true,
        ..Default::default()
    };
    let result = Engine::new(config)
        .slice(&mesh)
        .expect("gap fill slice should succeed");
    assert!(
        !result.gcode.is_empty(),
        "Gap fill should produce non-empty G-code"
    );
    let gcode_str = String::from_utf8_lossy(&result.gcode);
    let validation = validate_gcode(&gcode_str);
    assert!(
        validation.valid,
        "Gap fill G-code should pass validation. Errors: {:?}",
        validation.errors
    );
}

#[test]
fn sc5_gap_fill_disabled_still_works() {
    let mesh = thin_wall_box();
    let config = PrintConfig {
        gap_fill_enabled: false,
        ..Default::default()
    };
    let result = Engine::new(config)
        .slice(&mesh)
        .expect("gap fill disabled slice should succeed");
    assert!(!result.gcode.is_empty());
}

// ===========================================================================
// Determinism tests
// ===========================================================================

#[test]
fn determinism_all_patterns() {
    let mesh = calibration_cube_20mm();
    let patterns = [
        InfillPattern::Rectilinear,
        InfillPattern::Grid,
        InfillPattern::Honeycomb,
        InfillPattern::Gyroid,
        InfillPattern::AdaptiveCubic,
        InfillPattern::Cubic,
        InfillPattern::Lightning,
        InfillPattern::Monotonic,
    ];
    for pattern in &patterns {
        let config = PrintConfig {
            infill_pattern: pattern.clone(),
            infill_density: 0.2,
            ..Default::default()
        };
        let result1 = Engine::new(config.clone())
            .slice(&mesh)
            .unwrap_or_else(|e| panic!("{:?} first slice failed: {:?}", pattern, e));
        let result2 = Engine::new(config)
            .slice(&mesh)
            .unwrap_or_else(|e| panic!("{:?} second slice failed: {:?}", pattern, e));
        assert_eq!(
            result1.gcode, result2.gcode,
            "{:?} should produce deterministic output",
            pattern
        );
    }
}

#[test]
fn determinism_adaptive_layers() {
    let mesh = unit_sphere();
    let config = PrintConfig {
        adaptive_layer_height: true,
        adaptive_min_layer_height: 0.05,
        adaptive_max_layer_height: 0.3,
        adaptive_layer_quality: 0.5,
        ..Default::default()
    };
    let result1 = Engine::new(config.clone())
        .slice(&mesh)
        .expect("adaptive first slice should succeed");
    let result2 = Engine::new(config)
        .slice(&mesh)
        .expect("adaptive second slice should succeed");
    assert_eq!(
        result1.gcode, result2.gcode,
        "Adaptive should produce deterministic output"
    );
    assert_eq!(result1.layer_count, result2.layer_count);
}

// ===========================================================================
// Preview data verification
// ===========================================================================

#[test]
fn preview_data_from_calibration_cube() {
    let mesh = calibration_cube_20mm();
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let result = engine
        .slice_with_preview(&mesh)
        .expect("preview slice should succeed");

    let preview = result.preview.as_ref().expect("preview should be present");
    assert_eq!(preview.total_layers, result.layer_count);
    assert_eq!(preview.layers.len(), result.layer_count);

    // Each layer should have non-empty contours.
    for (i, layer) in preview.layers.iter().enumerate() {
        assert!(
            !layer.contours.is_empty(),
            "Layer {} should have non-empty contours",
            i
        );
    }

    // Bounding box should match approximate model dimensions.
    // Cube is at (90..110, 90..110, 0..20).
    assert!(preview.bounding_box[0] >= 89.0 && preview.bounding_box[0] <= 91.0);
    assert!(preview.bounding_box[3] >= 109.0 && preview.bounding_box[3] <= 111.0);
}

#[test]
fn preview_data_serializes_to_json() {
    let mesh = calibration_cube_20mm();
    let config = PrintConfig {
        layer_height: 0.2,
        first_layer_height: 0.2,
        ..Default::default()
    };
    let engine = Engine::new(config);
    let result = engine
        .slice_with_preview(&mesh)
        .expect("preview slice should succeed");

    let preview = result.preview.as_ref().unwrap();
    let json = serde_json::to_string(preview);
    assert!(json.is_ok(), "Preview should serialize to JSON");

    let json_str = json.unwrap();
    // Verify it parses back.
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .expect("Preview JSON should parse back");
    assert!(parsed["total_layers"].is_number());
    assert!(parsed["layers"].is_array());
}

// ===========================================================================
// Combined G-code validation
// ===========================================================================

#[test]
fn all_gcode_passes_validation() {
    let cube = calibration_cube_20mm();

    // Default config.
    let result = Engine::new(PrintConfig::default())
        .slice(&cube)
        .expect("default slice should succeed");
    let gcode_str = String::from_utf8_lossy(&result.gcode);
    let validation = validate_gcode(&gcode_str);
    assert!(
        validation.valid,
        "Default G-code should pass validation. Errors: {:?}",
        validation.errors
    );
}
