#![recursion_limit = "512"]
//! Phase 34 integration tests: support config, scarf joint, multi-material,
//! custom G-code hooks, P2 niche fields, G-code template translation,
//! passthrough threshold, and validation.

use slicecore_engine::config::PrintConfig;
use slicecore_engine::config_validate::validate_config;
use slicecore_engine::gcode_template::{
    build_orcaslicer_translation_table, build_prusaslicer_translation_table,
    translate_gcode_template,
};
use slicecore_engine::profile_import::import_upstream_profile;
use slicecore_engine::support::config::{InterfacePattern, SupportPattern, SupportType};

// ===========================================================================
// Group 1: Support config import tests
// ===========================================================================

#[test]
fn test_support_profile_import_json() {
    let json = serde_json::json!({
        "enable_support": "1",
        "support_type": "tree",
        "support_threshold_angle": "45",
        "support_top_z_distance": "0.2",
        "support_object_xy_distance": "0.5",
        "support_interface_top_layers": "3",
        "support_base_pattern": "rectilinear",
        "support_on_build_plate_only": "1",
        "support_expansion": "0.5",
        "support_critical_regions_only": "1",
        "support_remove_small_overhang": "0",
        "support_flow_ratio": "0.95",
        "support_interface_flow_ratio": "0.9",
        "support_material_synchronize_layers": "1",
        "enforce_support_layers": "5",
        "support_closing_radius": "3.0",
        "support_bottom_z_distance": "0.15",
        "support_bottom_interface_layers": "2",
        "support_interface_pattern": "concentric",
        "support_interface_spacing": "0.5"
    });

    let result = import_upstream_profile(&json).unwrap();
    let cfg = &result.config.support;

    assert!(cfg.enabled);
    assert_eq!(cfg.support_type, SupportType::Tree);
    assert!((cfg.overhang_angle - 45.0).abs() < 1e-9);
    assert!((cfg.z_gap - 0.2).abs() < 1e-9);
    assert!((cfg.xy_gap - 0.5).abs() < 1e-9);
    assert_eq!(cfg.interface_layers, 3);
    assert_eq!(cfg.support_pattern, SupportPattern::Rectilinear);
    assert!(cfg.build_plate_only);
    assert!((cfg.expansion - 0.5).abs() < 1e-9);
    assert!(cfg.critical_regions_only);
    assert!(!cfg.remove_small_overhang);
    assert!((cfg.flow_ratio - 0.95).abs() < 1e-9);
    assert!((cfg.interface_flow_ratio - 0.9).abs() < 1e-9);
    assert!(cfg.synchronize_layers);
    assert_eq!(cfg.enforce_layers, 5);
    assert!((cfg.closing_radius - 3.0).abs() < 1e-9);
    assert_eq!(cfg.bottom_z_gap, Some(0.15));
    assert_eq!(cfg.support_bottom_interface_layers, 2);
    assert_eq!(cfg.interface_pattern, InterfacePattern::Concentric);
    // interface_spacing = 0.5 -> density = 0.4/0.5 = 0.8
    assert!((cfg.interface_density - 0.8).abs() < 0.05);
}

#[test]
fn test_support_type_mapping() {
    // OrcaSlicer vocabulary
    let cases = vec![
        ("none", SupportType::None),
        ("normal", SupportType::Traditional),
        ("normal(auto)", SupportType::Traditional),
        ("normal(manual)", SupportType::Traditional),
        ("tree", SupportType::Tree),
        ("tree(auto)", SupportType::Tree),
        ("auto", SupportType::Auto),
        ("default", SupportType::Auto),
        // PrusaSlicer vocabulary
        ("grid", SupportType::Traditional),
        ("snug", SupportType::Traditional),
        ("organic", SupportType::Tree),
    ];

    for (input, expected) in cases {
        let json = serde_json::json!({ "support_type": input });
        let result = import_upstream_profile(&json).unwrap();
        assert_eq!(
            result.config.support.support_type, expected,
            "support_type '{}' should map to {:?}",
            input, expected
        );
    }
}

#[test]
fn test_bridge_config_import() {
    let json = serde_json::json!({
        "bridge_angle": "30",
        "bridge_density": "0.9",
        "thick_bridges": "1",
        "bridge_no_support": "1",
        "bridge_fan_speed": "200"
    });

    let result = import_upstream_profile(&json).unwrap();
    let bridge = &result.config.support.bridge;

    assert!((bridge.angle - 30.0).abs() < 1e-9);
    assert!((bridge.density - 0.9).abs() < 1e-9);
    assert!(bridge.thick_bridges);
    assert!(bridge.no_support);
    assert_eq!(bridge.fan_speed, 200);
}

#[test]
fn test_tree_support_import() {
    let json = serde_json::json!({
        "tree_support_branch_angle": "40",
        "tree_support_branch_diameter": "8.0",
        "tree_support_tip_diameter": "0.6",
        "tree_support_branch_distance": "3.0",
        "tree_support_branch_diameter_angle": "7.0",
        "tree_support_wall_count": "2",
        "tree_support_auto_brim": "0",
        "tree_support_brim_width": "5.0",
        "tree_support_adaptive_layer_height": "0",
        "tree_support_angle_slow": "20",
        "tree_support_top_rate": "0.5",
        "tree_support_with_infill": "1"
    });

    let result = import_upstream_profile(&json).unwrap();
    let tree = &result.config.support.tree;

    assert!((tree.branch_angle - 40.0).abs() < 1e-9);
    assert!((tree.max_trunk_diameter - 8.0).abs() < 1e-9);
    assert!((tree.tip_diameter - 0.6).abs() < 1e-9);
    assert!((tree.branch_distance - 3.0).abs() < 1e-9);
    assert!((tree.branch_diameter_angle - 7.0).abs() < 1e-9);
    assert_eq!(tree.wall_count, 2);
    assert!(!tree.auto_brim);
    assert!((tree.brim_width - 5.0).abs() < 1e-9);
    assert!(!tree.adaptive_layer_height);
    assert!((tree.angle_slow - 20.0).abs() < 1e-9);
    assert!((tree.top_rate - 0.5).abs() < 1e-9);
    assert!(tree.with_infill);
}

#[test]
fn test_support_density_from_spacing() {
    let json = serde_json::json!({
        "support_base_pattern_spacing": "2.0"
    });

    let result = import_upstream_profile(&json).unwrap();
    // Default line_width is 0.4, so density = 0.4 / 2.0 = 0.2
    assert!((result.config.support.support_density - 0.2).abs() < 0.05);
}

// ===========================================================================
// Group 2: ScarfJoint import tests
// ===========================================================================

#[test]
fn test_scarf_joint_import_json() {
    let json = serde_json::json!({
        "seam_slope_type": "contour",
        "seam_slope_conditional": "1",
        "seam_slope_start_height": "0.6",
        "seam_slope_entire_loop": "1",
        "seam_slope_min_length": "15.0",
        "seam_slope_steps": "8",
        "seam_slope_inner_walls": "1",
        "seam_slope_gap": "0.1",
        "wipe_on_loops": "1",
        "role_based_wipe_speed": "1",
        "wipe_speed": "50",
        "scarf_joint_speed": "80",
        "scarf_joint_flow_ratio": "0.95",
        "scarf_angle_threshold": "30",
        "scarf_overhang_threshold": "50",
        "override_filament_scarf_seam_setting": "1"
    });

    let result = import_upstream_profile(&json).unwrap();
    let scarf = &result.config.scarf_joint;

    assert!(scarf.enabled);
    assert!(scarf.conditional_scarf);
    assert!((scarf.scarf_start_height - 0.6).abs() < 1e-9);
    assert!(scarf.scarf_around_entire_wall);
    assert!((scarf.scarf_length - 15.0).abs() < 1e-9);
    assert_eq!(scarf.scarf_steps, 8);
    assert!(scarf.scarf_inner_walls);
    assert!((scarf.seam_gap - 0.1).abs() < 1e-9);
    assert!(scarf.wipe_on_loop);
    assert!(scarf.role_based_wipe_speed);
    assert!((scarf.wipe_speed - 50.0).abs() < 1e-9);
    assert!((scarf.scarf_speed - 80.0).abs() < 1e-9);
    assert!((scarf.scarf_flow_ratio - 0.95).abs() < 1e-9);
    assert!((scarf.scarf_angle_threshold - 30.0).abs() < 1e-9);
    assert!((scarf.scarf_overhang_threshold - 50.0).abs() < 1e-9);
    assert!(scarf.override_filament_setting);
}

// ===========================================================================
// Group 3: MultiMaterial import tests
// ===========================================================================

#[test]
fn test_multi_material_import_json() {
    let json = serde_json::json!({
        "enable_prime_tower": "1",
        "wipe_tower_x": "150",
        "wipe_tower_y": "100",
        "wipe_tower_width": "20",
        "prime_volume": "90",
        "wipe_tower_rotation_angle": "45",
        "wipe_tower_bridging": "12.5",
        "wipe_tower_cone_angle": "30",
        "wipe_tower_no_sparse_layers": "1",
        "single_extruder_multi_material": "1",
        "flush_into_infill": "1",
        "flush_into_objects": "0",
        "flush_into_support": "1",
        "purge_in_prime_tower": "1",
        "wall_filament": "2",
        "solid_infill_filament": "3",
        "support_filament": "1",
        "support_interface_filament": "4"
    });

    let result = import_upstream_profile(&json).unwrap();
    let mm = &result.config.multi_material;

    assert!(mm.enabled);
    assert!((mm.purge_tower_position[0] - 150.0).abs() < 1e-9);
    assert!((mm.purge_tower_position[1] - 100.0).abs() < 1e-9);
    assert!((mm.purge_tower_width - 20.0).abs() < 1e-9);
    assert!((mm.purge_volume - 90.0).abs() < 1e-9);
    assert!((mm.wipe_tower_rotation_angle - 45.0).abs() < 1e-9);
    assert!((mm.wipe_tower_bridging - 12.5).abs() < 1e-9);
    assert!((mm.wipe_tower_cone_angle - 30.0).abs() < 1e-9);
    assert!(mm.wipe_tower_no_sparse_layers);
    assert!(mm.single_extruder_mmu);
    assert!(mm.flush_into_infill);
    assert!(!mm.flush_into_objects);
    assert!(mm.flush_into_support);
    assert!(mm.purge_in_prime_tower);
    // 1-based to 0-based conversion
    assert_eq!(mm.wall_filament, Some(1));
    assert_eq!(mm.solid_infill_filament, Some(2));
    assert_eq!(mm.support_filament, Some(0));
    assert_eq!(mm.support_interface_filament, Some(3));
}

// ===========================================================================
// Group 4: CustomGcode import tests
// ===========================================================================

#[test]
fn test_custom_gcode_hooks_import() {
    let json = serde_json::json!({
        "before_layer_change_gcode": "G1 Z{layer_z} ; layer change\nM117 Layer {layer_num}",
        "change_filament_gcode": "T{next_extruder}\nG92 E0",
        "color_change_gcode": "M600",
        "machine_pause_gcode": "M0 ; pause",
        "between_objects_gcode": "G28 X"
    });

    let result = import_upstream_profile(&json).unwrap();
    let gcode = &result.config.custom_gcode;

    // Original is stored verbatim
    assert!(gcode.before_layer_change_original.contains("{layer_z}"));
    assert!(gcode.before_layer_change_original.contains("{layer_num}"));

    // Translated version should have our variable names (these are identity for layer_z/layer_num)
    assert!(gcode.before_layer_change.contains("{layer_z}"));
    assert!(gcode.before_layer_change.contains("{layer_num}"));

    // Tool change gcode
    assert!(gcode.tool_change_gcode_original.contains("{next_extruder}"));
    assert!(gcode.tool_change_gcode.contains("{next_extruder}"));

    // Color change
    assert_eq!(gcode.color_change, "M600");
    assert_eq!(gcode.color_change_original, "M600");

    // Pause print
    assert_eq!(gcode.pause_print, "M0 ; pause");
    assert_eq!(gcode.pause_print_original, "M0 ; pause");

    // Between objects
    assert_eq!(gcode.between_objects, "G28 X");
    assert_eq!(gcode.between_objects_original, "G28 X");
}

// ===========================================================================
// Group 5: P2 niche field tests
// ===========================================================================

#[test]
fn test_p2_fields_import() {
    let json = serde_json::json!({
        "slicing_tolerance": "nearest",
        "thumbnails": "96x96,400x300",
        "silent_mode": "1",
        "nozzle_hrc": "60",
        "timelapse_type": "smooth",
        "gcode_label_objects": "1",
        "gcode_comments": "1",
        "gcode_add_line_number": "0",
        "filename_format": "{input_filename_base}_{print_time}.gcode",
        "post_process": "/usr/bin/postproc.sh;/opt/cleanup.sh",
        "print_sequence": "by object",
        "ironing_angle": "45",
        "exclude_object": "1",
        "reduce_infill_retraction": "1",
        "reduce_crossing_wall": "1"
    });

    let result = import_upstream_profile(&json).unwrap();
    let cfg = &result.config;

    assert_eq!(
        cfg.slicing_tolerance,
        slicecore_engine::config::SlicingTolerance::Nearest
    );
    assert_eq!(cfg.thumbnails, vec!["96x96", "400x300"]);
    assert!(cfg.machine.silent_mode);
    assert_eq!(cfg.machine.nozzle_hrc, 60);
    assert!(cfg.post_process.timelapse.enabled);
    assert!(cfg.post_process.gcode_label_objects);
    assert!(cfg.post_process.gcode_comments);
    assert!(!cfg.post_process.gcode_add_line_number);
    assert_eq!(
        cfg.post_process.filename_format,
        "{input_filename_base}_{print_time}.gcode"
    );
    assert_eq!(
        cfg.post_process.scripts,
        vec!["/usr/bin/postproc.sh", "/opt/cleanup.sh"]
    );
    assert!(cfg.sequential.enabled);
    assert!((cfg.ironing.angle - 45.0).abs() < 1e-9);
    assert!(cfg.exclude_object);
    assert!(cfg.reduce_infill_retraction);
    assert!(cfg.reduce_crossing_wall);
}

#[test]
fn test_p2_fields_toml_roundtrip() {
    let mut config = PrintConfig::default();
    config.slicing_tolerance = slicecore_engine::config::SlicingTolerance::Gauss;
    config.thumbnails = vec!["96x96".to_string(), "400x300".to_string()];
    config.machine.silent_mode = true;
    config.machine.nozzle_hrc = 42;
    config.post_process.timelapse.enabled = true;
    config.post_process.gcode_label_objects = true;
    config.sequential.enabled = true;
    config.ironing.angle = 60.0;

    let toml_str = toml::to_string(&config).expect("serialize");
    let roundtrip: PrintConfig = toml::from_str(&toml_str).expect("deserialize");

    assert_eq!(
        roundtrip.slicing_tolerance,
        slicecore_engine::config::SlicingTolerance::Gauss
    );
    assert_eq!(roundtrip.thumbnails, vec!["96x96", "400x300"]);
    assert!(roundtrip.machine.silent_mode);
    assert_eq!(roundtrip.machine.nozzle_hrc, 42);
    assert!(roundtrip.post_process.timelapse.enabled);
    assert!(roundtrip.post_process.gcode_label_objects);
    assert!(roundtrip.sequential.enabled);
    assert!((roundtrip.ironing.angle - 60.0).abs() < 1e-9);
}

// ===========================================================================
// Group 6: G-code template translation tests
// ===========================================================================

#[test]
fn test_gcode_template_translation_orcaslicer() {
    let table = build_orcaslicer_translation_table();
    let input = "M104 S{nozzle_temperature_initial_layer}\nG1 Z{initial_layer_print_height}\nM117 {total_layer_count} layers\nBed: {curr_bed_type}";
    let result = translate_gcode_template(input, &table);

    assert!(result.contains("{first_layer_nozzle_temp}"));
    assert!(result.contains("{first_layer_height}"));
    assert!(result.contains("{total_layers}"));
    assert!(result.contains("{bed_type}"));
    // Originals should NOT be present
    assert!(!result.contains("{nozzle_temperature_initial_layer}"));
    assert!(!result.contains("{initial_layer_print_height}"));
    assert!(!result.contains("{total_layer_count}"));
    assert!(!result.contains("{curr_bed_type}"));
}

#[test]
fn test_gcode_template_translation_prusaslicer() {
    let table = build_prusaslicer_translation_table();
    let input =
        "M104 S[first_layer_temperature]\nM140 S[first_layer_bed_temperature]\n;LAYER:[layer_num]";
    let result = translate_gcode_template(input, &table);

    assert!(result.contains("{first_layer_nozzle_temp}"));
    assert!(result.contains("{first_layer_bed_temp}"));
    assert!(result.contains("{layer_num}"));
    // Square brackets should be gone
    assert!(!result.contains("[first_layer_temperature]"));
    assert!(!result.contains("[first_layer_bed_temperature]"));
}

#[test]
fn test_gcode_template_no_double_replacement() {
    // Ensure {layer_z} is not partially mangled by {layer_num} replacement
    // or other shorter keys.
    let table = build_orcaslicer_translation_table();
    let input = "G1 Z{layer_z} ; move to layer {layer_num}";
    let result = translate_gcode_template(input, &table);

    // Both variables should be preserved as-is (identity mapping)
    assert!(
        result.contains("{layer_z}"),
        "Expected {{layer_z}} in: {}",
        result
    );
    assert!(
        result.contains("{layer_num}"),
        "Expected {{layer_num}} in: {}",
        result
    );
}

// ===========================================================================
// Group 7: Passthrough threshold test
// ===========================================================================

#[test]
fn test_passthrough_threshold() {
    // Construct a representative OrcaSlicer profile with many fields.
    // This simulates a real profile that exercises all Phase 34 mapped sections.
    let json = serde_json::json!({
        // Metadata (skipped)
        "type": "process",
        "name": "Test Profile",
        "inherits": "Default",
        // Layer geometry
        "layer_height": "0.2",
        "initial_layer_print_height": "0.3",
        // Walls
        "wall_loops": "3",
        "outer_wall_speed": "60",
        "inner_wall_speed": "80",
        // Infill
        "sparse_infill_density": "20%",
        "sparse_infill_pattern": "grid",
        // Speeds
        "travel_speed": "200",
        "first_layer_speed": "30",
        "top_surface_speed": "40",
        // Support (Phase 34)
        "enable_support": "1",
        "support_type": "tree",
        "support_threshold_angle": "45",
        "support_top_z_distance": "0.2",
        "support_object_xy_distance": "0.5",
        "support_interface_top_layers": "3",
        "support_base_pattern": "rectilinear",
        "support_on_build_plate_only": "0",
        "support_expansion": "0.3",
        "support_flow_ratio": "1.0",
        "support_interface_flow_ratio": "1.0",
        "support_material_synchronize_layers": "0",
        "enforce_support_layers": "0",
        "support_closing_radius": "2.0",
        "support_bottom_z_distance": "0.2",
        "support_bottom_interface_layers": "2",
        "support_interface_pattern": "rectilinear",
        "support_critical_regions_only": "0",
        "support_remove_small_overhang": "1",
        // Tree support
        "tree_support_branch_angle": "45",
        "tree_support_branch_diameter": "10",
        "tree_support_tip_diameter": "0.8",
        "tree_support_branch_distance": "5",
        "tree_support_wall_count": "0",
        "tree_support_auto_brim": "1",
        "tree_support_brim_width": "3",
        "tree_support_adaptive_layer_height": "1",
        "tree_support_angle_slow": "25",
        "tree_support_top_rate": "0.3",
        "tree_support_with_infill": "0",
        // Bridge
        "bridge_angle": "0",
        "bridge_density": "1.0",
        "thick_bridges": "0",
        "bridge_no_support": "0",
        "bridge_fan_speed": "255",
        // Scarf joint (Phase 34)
        "seam_slope_type": "contour",
        "seam_slope_conditional": "0",
        "seam_slope_start_height": "0.5",
        "seam_slope_steps": "10",
        "seam_slope_inner_walls": "0",
        "scarf_joint_speed": "0",
        "scarf_joint_flow_ratio": "1.0",
        // Multi-material (Phase 34)
        "enable_prime_tower": "0",
        "wipe_tower_x": "200",
        "wipe_tower_y": "200",
        "wipe_tower_width": "15",
        "wipe_tower_rotation_angle": "0",
        "wipe_tower_no_sparse_layers": "0",
        "single_extruder_multi_material": "0",
        // Custom G-code (Phase 34)
        "before_layer_change_gcode": "G1 Z{layer_z}",
        "change_filament_gcode": "",
        // P2 fields
        "slicing_tolerance": "middle",
        "thumbnails": "96x96",
        "silent_mode": "0",
        "nozzle_hrc": "0",
        "timelapse_type": "none",
        "print_sequence": "by layer",
        "ironing_angle": "0",
        // Standard mapped fields
        "line_width": "0.4",
        "retraction_length": "0.8",
        "retraction_speed": "30",
        "fan_min_speed": "100",
        "fan_max_speed": "100",
        "nozzle_diameter": "0.4",
        "bed_type": "Textured PEI",
        "gcode_flavor": "marlin",
        "bridge_speed": "30",
        "bridge_flow": "0.85",
        "bridge_acceleration": "500",
        "infill_direction": "45",
        "skirt_loops": "1",
        "skirt_distance": "6",
        "brim_width": "0",
        "top_shell_layers": "3",
        "bottom_shell_layers": "3",
        "detect_thin_wall": "1",
        "spiral_mode": "0",
        "only_one_wall_top": "1",
        "resolution": "0.01"
    });

    let result = import_upstream_profile(&json).unwrap();

    // Count non-metadata keys
    let total_keys = result.mapped_fields.len() + result.passthrough_fields.len();
    let passthrough_count = result.passthrough_fields.len();

    let ratio = if total_keys > 0 {
        passthrough_count as f64 / total_keys as f64
    } else {
        0.0
    };

    assert!(
        ratio < 0.05,
        "Passthrough ratio {:.1}% ({}/{}) exceeds 5% threshold. \
         Passthrough fields: {:?}",
        ratio * 100.0,
        passthrough_count,
        total_keys,
        result.passthrough_fields
    );
}

// ===========================================================================
// Group 8: Range validation tests
// ===========================================================================

#[test]
fn test_support_range_validation() {
    let mut config = PrintConfig::default();
    config.support.enabled = true;
    config.support.overhang_angle = 100.0; // > 90, out of range
    config.support.support_density = 1.5; // > 1.0, out of range

    let issues = validate_config(&config);

    assert!(
        issues
            .iter()
            .any(|i| i.field.contains("support.overhang_angle")),
        "Expected validation issue for overhang_angle > 90, got: {:?}",
        issues.iter().map(|i| &i.field).collect::<Vec<_>>()
    );
    assert!(
        issues
            .iter()
            .any(|i| i.field.contains("support.support_density")),
        "Expected validation issue for support_density > 1.0, got: {:?}",
        issues.iter().map(|i| &i.field).collect::<Vec<_>>()
    );
}
