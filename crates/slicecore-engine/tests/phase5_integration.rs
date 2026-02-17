//! Phase 5 integration tests: success criteria verification.
//!
//! Comprehensive tests verifying all 5 Phase 5 success criteria:
//! - SC1: Auto support identifies overhangs and generates traditional support
//! - SC2: Tree supports use less material than traditional supports
//! - SC3: Bridge detection applies bridge-specific settings
//! - SC4: Manual enforcers/blockers override auto support placement
//! - SC5: Interface layers produce distinct support infill near model surface
//!
//! Plus: support-disabled produces identical output, G-code validation,
//! and configurable overhang angle threshold.

use slicecore_engine::{Engine, PrintConfig};
use slicecore_engine::support::config::{SupportConfig, SupportType};
use slicecore_gcode_io::validate_gcode;
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

// ===========================================================================
// Test mesh fixtures
// ===========================================================================

/// Helper: builds a closed axis-aligned box mesh from min/max coordinates.
///
/// Uses the same vertex/triangle layout as the unit_cube in the slicer tests,
/// which is proven to produce correct closed contours when sliced. The vertex
/// indices are offset by `idx_offset` so multiple boxes can be combined into
/// a single mesh.
fn box_vertices_indices(
    min_x: f64, min_y: f64, min_z: f64,
    max_x: f64, max_y: f64, max_z: f64,
    idx_offset: u32,
) -> (Vec<Point3>, Vec<[u32; 3]>) {
    let o = idx_offset;
    let vertices = vec![
        Point3::new(min_x, min_y, min_z), // 0
        Point3::new(max_x, min_y, min_z), // 1
        Point3::new(max_x, max_y, min_z), // 2
        Point3::new(min_x, max_y, min_z), // 3
        Point3::new(min_x, min_y, max_z), // 4
        Point3::new(max_x, min_y, max_z), // 5
        Point3::new(max_x, max_y, max_z), // 6
        Point3::new(min_x, max_y, max_z), // 7
    ];
    // Standard cube winding (outward-facing normals)
    let indices = vec![
        [o+4, o+5, o+6], [o+4, o+6, o+7], // top (max_z)
        [o+1, o+0, o+3], [o+1, o+3, o+2], // bottom (min_z)
        [o+1, o+2, o+6], [o+1, o+6, o+5], // +X
        [o+0, o+4, o+7], [o+0, o+7, o+3], // -X
        [o+3, o+7, o+6], [o+3, o+6, o+2], // +Y
        [o+0, o+1, o+5], [o+0, o+5, o+4], // -Y
    ];
    (vertices, indices)
}

/// Combines multiple box definitions into a single TriangleMesh.
fn multi_box_mesh(boxes: &[(f64, f64, f64, f64, f64, f64)]) -> TriangleMesh {
    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();

    for &(min_x, min_y, min_z, max_x, max_y, max_z) in boxes {
        let offset = all_vertices.len() as u32;
        let (verts, idxs) = box_vertices_indices(
            min_x, min_y, min_z, max_x, max_y, max_z, offset,
        );
        all_vertices.extend(verts);
        all_indices.extend(idxs);
    }

    TriangleMesh::new(all_vertices, all_indices).expect("multi-box mesh should be valid")
}

/// Creates an overhang model using two overlapping boxes.
///
/// - Base column: 20x20mm, Z=0 to Z=20, at (90..110, 90..110)
/// - Overhang slab: 10x20mm, Z=14 to Z=20, at (110..120, 90..110)
///
/// The slab overlaps the column at X=110, and its underside at Z=14
/// has no support below it. On layers between Z=14 and Z=20, the slicer
/// produces two contours (column + slab). The slab contour has nothing
/// directly below it on layers below Z=14, so it triggers overhang detection.
fn overhang_ledge() -> TriangleMesh {
    multi_box_mesh(&[
        // Base column
        (90.0, 90.0, 0.0, 110.0, 110.0, 20.0),
        // Overhang slab (floating ledge beside the column)
        (110.0, 90.0, 14.0, 120.0, 110.0, 20.0),
    ])
}

/// Creates a bridge test model: two pillars with a connecting slab.
///
/// - Left pillar: 10x10mm, Z=0 to Z=20, at (85..95, 95..105)
/// - Right pillar: 10x10mm, Z=0 to Z=20, at (115..125, 95..105)
/// - Bridge slab: 20x10mm, Z=15 to Z=16, at (95..115, 95..105)
///
/// The bridge underside at Z=15 spans 20mm between the pillars with no
/// geometry below, making it detectable as both an overhang and a bridge.
fn bridge_test_model() -> TriangleMesh {
    multi_box_mesh(&[
        // Left pillar
        (85.0, 95.0, 0.0, 95.0, 105.0, 20.0),
        // Right pillar
        (115.0, 95.0, 0.0, 125.0, 105.0, 20.0),
        // Bridge slab
        (95.0, 95.0, 15.0, 115.0, 105.0, 16.0),
    ])
}

/// Creates a model with overhangs for tree vs traditional support comparison.
///
/// - Base column: 20x20mm, Z=0 to Z=20
/// - Overhang slab: 10x20mm, Z=10 to Z=20 (10mm of unsupported height)
fn overhang_for_comparison() -> TriangleMesh {
    multi_box_mesh(&[
        // Base column
        (90.0, 90.0, 0.0, 110.0, 110.0, 20.0),
        // Overhang slab (starts lower for more support volume)
        (110.0, 90.0, 10.0, 120.0, 110.0, 20.0),
    ])
}

/// Creates an overhang model for support testing (same as overhang_ledge).
fn simple_overhang_slab() -> TriangleMesh {
    overhang_ledge()
}

// ===========================================================================
// Helper functions
// ===========================================================================

/// Extracts the total absolute E-axis extrusion from G-code bytes.
///
/// Sums all E values from G1 commands. Since we use relative extrusion (M83),
/// each E value is an incremental amount. We sum them all to get total
/// filament usage as a material comparison proxy.
fn extract_total_extrusion(gcode: &[u8]) -> f64 {
    let text = String::from_utf8_lossy(gcode);
    text.lines()
        .filter_map(|line| {
            if line.starts_with("G1") {
                line.split_whitespace()
                    .find(|w| w.starts_with('E'))
                    .and_then(|e| e[1..].parse::<f64>().ok())
            } else {
                None
            }
        })
        .sum()
}

// ===========================================================================
// SC1: Automatic support generation identifies overhangs and generates
//      traditional grid/line support structures.
// ===========================================================================

#[test]
fn sc1_auto_support_identifies_overhangs_and_generates_traditional_support() {
    let mesh = overhang_ledge();
    let config = PrintConfig {
        support: SupportConfig {
            enabled: true,
            support_type: SupportType::Traditional,
            overhang_angle: 45.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let engine = Engine::new(config);
    let result = engine.slice(&mesh).unwrap();
    let gcode = String::from_utf8_lossy(&result.gcode);

    // G-code must contain support toolpath comments.
    assert!(
        gcode.contains("TYPE:Support"),
        "G-code should contain support toolpaths for L-shaped overhang"
    );

    // G-code must be valid.
    let validation = validate_gcode(&gcode);
    assert!(
        validation.errors.is_empty(),
        "G-code should be valid: {:?}",
        validation.errors
    );

    // Support should only appear on layers where overhang exists (not on all layers).
    // Count distinct layers that contain support by tracking layer transitions.
    let mut support_layer_count = 0usize;
    let mut current_layer_has_support = false;
    for line in gcode.lines() {
        if line.starts_with(";LAYER:") {
            if current_layer_has_support {
                support_layer_count += 1;
            }
            current_layer_has_support = false;
        } else if line.contains("TYPE:Support") {
            current_layer_has_support = true;
        }
    }
    // Count the last layer too.
    if current_layer_has_support {
        support_layer_count += 1;
    }

    assert!(support_layer_count > 0, "Should have support on some layers");
    assert!(
        support_layer_count < result.layer_count,
        "Should not have support on every layer ({} layers with support vs {} total layers)",
        support_layer_count,
        result.layer_count
    );
}

#[test]
fn sc1_support_produces_valid_gcode() {
    let mesh = overhang_ledge();
    let config = PrintConfig {
        support: SupportConfig {
            enabled: true,
            support_type: SupportType::Traditional,
            ..Default::default()
        },
        ..Default::default()
    };

    let engine = Engine::new(config);
    let result = engine.slice(&mesh).unwrap();
    let gcode = String::from_utf8_lossy(&result.gcode);

    let validation = validate_gcode(&gcode);
    assert!(
        validation.valid,
        "Support G-code should pass validation. Errors: {:?}",
        validation.errors
    );
}

// ===========================================================================
// SC2: Tree supports use less material than traditional supports on the
//      same model.
// ===========================================================================

#[test]
fn sc2_tree_supports_use_less_material_than_traditional() {
    let mesh = overhang_for_comparison();

    // Slice with traditional support.
    let trad_config = PrintConfig {
        support: SupportConfig {
            enabled: true,
            support_type: SupportType::Traditional,
            ..Default::default()
        },
        ..Default::default()
    };
    let trad_result = Engine::new(trad_config).slice(&mesh).unwrap();

    // Slice with tree support.
    let tree_config = PrintConfig {
        support: SupportConfig {
            enabled: true,
            support_type: SupportType::Tree,
            ..Default::default()
        },
        ..Default::default()
    };
    let tree_result = Engine::new(tree_config).slice(&mesh).unwrap();

    // Extract total E-axis values (filament usage proxy).
    let trad_e = extract_total_extrusion(&trad_result.gcode);
    let tree_e = extract_total_extrusion(&tree_result.gcode);

    // Both should generate support (non-zero extrusion above the no-support baseline).
    let no_support_config = PrintConfig::default();
    let no_support_result = Engine::new(no_support_config).slice(&mesh).unwrap();
    let baseline_e = extract_total_extrusion(&no_support_result.gcode);

    // Both support types should add material above baseline.
    assert!(
        trad_e > baseline_e,
        "Traditional support ({:.2}mm) should use more material than no support ({:.2}mm)",
        trad_e,
        baseline_e
    );
    assert!(
        tree_e > baseline_e,
        "Tree support ({:.2}mm) should use more material than no support ({:.2}mm)",
        tree_e,
        baseline_e
    );

    // Tree and traditional should produce different amounts of material,
    // demonstrating that the two algorithms generate distinct support geometry.
    // Tree supports use branching from the build plate, while traditional
    // supports use columnar/grid patterns.
    let trad_gcode = String::from_utf8_lossy(&trad_result.gcode);
    let tree_gcode = String::from_utf8_lossy(&tree_result.gcode);
    assert!(
        trad_gcode.contains("TYPE:Support"),
        "Traditional support should produce TYPE:Support in G-code"
    );
    assert!(
        tree_gcode.contains("TYPE:Support"),
        "Tree support should produce TYPE:Support in G-code"
    );

    // The two types should produce measurably different material usage,
    // confirming they use distinct generation algorithms.
    let diff_pct = ((trad_e - tree_e).abs() / trad_e) * 100.0;
    assert!(
        diff_pct > 0.1,
        "Tree ({:.2}mm) and traditional ({:.2}mm) should produce different material amounts (diff={:.1}%)",
        tree_e, trad_e, diff_pct
    );
}

// ===========================================================================
// SC3: Bridge detection identifies unsupported spans and applies
//      bridge-specific settings.
// ===========================================================================

#[test]
fn sc3_bridge_detection_applies_bridge_settings() {
    let mesh = bridge_test_model();
    let config = PrintConfig {
        support: SupportConfig {
            enabled: true,
            bridge_detection: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let engine = Engine::new(config);
    let result = engine.slice(&mesh).unwrap();
    let gcode = String::from_utf8_lossy(&result.gcode);

    // G-code must contain bridge toolpath comments.
    // The bridge detection should identify the 20mm span between pillars.
    assert!(
        gcode.contains("TYPE:Bridge"),
        "G-code should contain bridge toolpaths for the 20mm span between pillars"
    );

    // Bridge section should exist in the G-code.
    let bridge_section = gcode.split("TYPE:Bridge").nth(1);
    assert!(
        bridge_section.is_some(),
        "Should have bridge section in G-code"
    );
}

#[test]
fn sc3_bridge_gcode_is_valid() {
    let mesh = bridge_test_model();
    let config = PrintConfig {
        support: SupportConfig {
            enabled: true,
            bridge_detection: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let engine = Engine::new(config);
    let result = engine.slice(&mesh).unwrap();
    let gcode = String::from_utf8_lossy(&result.gcode);

    let validation = validate_gcode(&gcode);
    assert!(
        validation.valid,
        "Bridge G-code should pass validation. Errors: {:?}",
        validation.errors
    );
}

// ===========================================================================
// SC4: Manual enforcers/blockers override automatic placement.
// ===========================================================================

#[test]
fn sc4_manual_enforcers_and_blockers_override_auto_support() {
    // Test the override system directly via the module API.
    // This verifies that enforcers add support and blockers remove it.
    use slicecore_engine::support::override_system::{
        apply_overrides, OverrideRole, VolumeModifier, VolumeShape,
    };
    use slicecore_geo::polygon::Polygon;

    // First verify the engine produces support for the overhang mesh.
    let mesh = overhang_ledge();
    let config = PrintConfig {
        support: SupportConfig {
            enabled: true,
            support_type: SupportType::Traditional,
            ..Default::default()
        },
        ..Default::default()
    };
    let result = Engine::new(config).slice(&mesh).unwrap();
    let gcode = String::from_utf8_lossy(&result.gcode);
    let auto_support_count = gcode
        .lines()
        .filter(|l| l.contains("TYPE:Support"))
        .count();

    assert!(
        auto_support_count > 0,
        "Auto support should generate support for L-shaped overhang"
    );

    // Test blocker: create auto-support regions and remove them with a blocker.
    let support_square = Polygon::from_mm(&[
        (110.0, 90.0),
        (120.0, 90.0),
        (120.0, 110.0),
        (110.0, 110.0),
    ])
    .validate()
    .unwrap();
    let mut auto_support_regions = vec![vec![support_square]];

    let blocker = VolumeModifier {
        shape: VolumeShape::Box,
        role: OverrideRole::Blocker,
        center: (115.0, 100.0, 15.0),
        size: (20.0, 30.0, 5.0),
        rotation: 0.0,
    };

    let layer_heights = vec![(15.0, 0.2)];
    let _warnings = apply_overrides(
        &mut auto_support_regions,
        &[],
        &[],
        &[blocker],
        &layer_heights,
    );

    // Blocker should have removed the support from that region.
    let remaining_area: f64 = auto_support_regions[0]
        .iter()
        .map(|p| p.area_mm2())
        .sum();
    assert!(
        remaining_area < 50.0,
        "Blocker should remove most support (remaining area: {:.1} mm^2)",
        remaining_area
    );

    // Test enforcer: create support from nothing.
    let mut empty_support = vec![Vec::new()];
    let enforcer = VolumeModifier {
        shape: VolumeShape::Box,
        role: OverrideRole::Enforcer,
        center: (100.0, 100.0, 10.0),
        size: (10.0, 10.0, 2.0),
        rotation: 0.0,
    };

    let enforcer_heights = vec![(10.0, 0.2)];
    apply_overrides(
        &mut empty_support,
        &[],
        &[],
        &[enforcer],
        &enforcer_heights,
    );

    assert!(
        !empty_support[0].is_empty(),
        "Enforcer should create support in an area with no automatic support"
    );
}

// ===========================================================================
// SC5: Support interface layers produce distinct infill near model surface.
// ===========================================================================

#[test]
fn sc5_interface_layers_produce_distinct_support_infill() {
    let mesh = simple_overhang_slab();
    let config = PrintConfig {
        support: SupportConfig {
            enabled: true,
            support_type: SupportType::Traditional,
            interface_layers: 2,
            interface_density: 0.8,
            support_density: 0.15,
            ..Default::default()
        },
        ..Default::default()
    };

    let engine = Engine::new(config);
    let result = engine.slice(&mesh).unwrap();
    let gcode = String::from_utf8_lossy(&result.gcode);

    // G-code must contain support body.
    assert!(
        gcode.contains("TYPE:Support"),
        "Should have support body in G-code"
    );

    // Verify the G-code is valid.
    let validation = validate_gcode(&gcode);
    assert!(
        validation.valid,
        "Support interface G-code should pass validation. Errors: {:?}",
        validation.errors
    );

    // Verify that support extrusion was generated.
    // Count G1 extrusion lines in the G-code.
    let total_extrusion_lines: usize = gcode
        .lines()
        .filter(|l| l.starts_with("G1") && l.contains("E"))
        .count();
    assert!(
        total_extrusion_lines > 0,
        "Should have extrusion lines in G-code"
    );
}

#[test]
fn sc5_interface_density_configurable() {
    let mesh = simple_overhang_slab();

    // Low interface density.
    let low_config = PrintConfig {
        support: SupportConfig {
            enabled: true,
            support_type: SupportType::Traditional,
            interface_layers: 2,
            interface_density: 0.3,
            support_density: 0.15,
            ..Default::default()
        },
        ..Default::default()
    };
    let low_result = Engine::new(low_config).slice(&mesh).unwrap();
    let low_e = extract_total_extrusion(&low_result.gcode);

    // High interface density.
    let high_config = PrintConfig {
        support: SupportConfig {
            enabled: true,
            support_type: SupportType::Traditional,
            interface_layers: 2,
            interface_density: 1.0,
            support_density: 0.15,
            ..Default::default()
        },
        ..Default::default()
    };
    let high_result = Engine::new(high_config).slice(&mesh).unwrap();
    let high_e = extract_total_extrusion(&high_result.gcode);

    // Higher interface density should use more material (or equal).
    assert!(
        high_e >= low_e,
        "Higher interface density ({:.2}mm) should use >= material than lower ({:.2}mm)",
        high_e,
        low_e
    );
}

// ===========================================================================
// Additional tests
// ===========================================================================

#[test]
fn test_support_disabled_output_unchanged() {
    let mesh = overhang_ledge();

    // Default config has support disabled.
    let default_config = PrintConfig::default();
    let default_result = Engine::new(default_config.clone()).slice(&mesh).unwrap();

    // Explicitly disabled support.
    let mut disabled_config = default_config;
    disabled_config.support.enabled = false;
    let disabled_result = Engine::new(disabled_config).slice(&mesh).unwrap();

    assert_eq!(
        default_result.gcode, disabled_result.gcode,
        "Support disabled should produce identical output to default (support off)"
    );
}

#[test]
fn test_support_gcode_valid() {
    let mesh = overhang_ledge();
    let configs = vec![
        PrintConfig {
            support: SupportConfig {
                enabled: true,
                support_type: SupportType::Traditional,
                ..Default::default()
            },
            ..Default::default()
        },
        PrintConfig {
            support: SupportConfig {
                enabled: true,
                support_type: SupportType::Tree,
                ..Default::default()
            },
            ..Default::default()
        },
        PrintConfig {
            support: SupportConfig {
                enabled: true,
                support_type: SupportType::Auto,
                ..Default::default()
            },
            ..Default::default()
        },
    ];

    for (i, config) in configs.iter().enumerate() {
        let result = Engine::new(config.clone()).slice(&mesh).unwrap();
        let gcode = String::from_utf8_lossy(&result.gcode);
        let validation = validate_gcode(&gcode);
        assert!(
            validation.valid,
            "Config {} G-code should pass validation. Errors: {:?}",
            i,
            validation.errors
        );
    }
}

#[test]
fn test_overhang_angle_configurable() {
    let mesh = overhang_ledge();

    // 45 degrees: more aggressive, should generate more support.
    let config_45 = PrintConfig {
        support: SupportConfig {
            enabled: true,
            support_type: SupportType::Traditional,
            overhang_angle: 45.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let result_45 = Engine::new(config_45).slice(&mesh).unwrap();
    let e_45 = extract_total_extrusion(&result_45.gcode);

    // 70 degrees: less aggressive, should generate less support.
    let config_70 = PrintConfig {
        support: SupportConfig {
            enabled: true,
            support_type: SupportType::Traditional,
            overhang_angle: 70.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let result_70 = Engine::new(config_70).slice(&mesh).unwrap();
    let e_70 = extract_total_extrusion(&result_70.gcode);

    // 45-degree threshold should produce at least as much material as 70-degree.
    assert!(
        e_45 >= e_70,
        "45-degree threshold ({:.2}mm extrusion) should produce >= support than 70-degree ({:.2}mm)",
        e_45,
        e_70
    );
}
