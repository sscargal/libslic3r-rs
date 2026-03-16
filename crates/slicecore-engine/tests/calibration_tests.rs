//! Engine-level integration tests for calibration mesh generation,
//! temperature injection, cost model, and bed validation.

use slicecore_engine::calibrate::{
    generate_first_layer_mesh, generate_flow_mesh, generate_retraction_mesh,
    generate_temp_tower_mesh, inject_temp_changes, temp_schedule, validate_bed_fit,
    FirstLayerParams, FirstLayerPattern, FlowParams, RetractionParams, TempTowerParams,
};
use slicecore_engine::config::MachineConfig;
use slicecore_engine::cost_model::{compute_cost, volume_estimate, CostInputs};
use slicecore_gcode_io::GcodeCommand;

// ---------------------------------------------------------------------------
// 1. Temperature tower mesh tests
// ---------------------------------------------------------------------------

#[test]
fn test_temp_tower_mesh_dimensions() {
    // 190-230 in steps of 5 = 9 blocks, each 8mm, plus 1mm base => ~73mm
    let params = TempTowerParams {
        start_temp: 190.0,
        end_temp: 230.0,
        step: 5.0,
        block_height: 8.0,
        base_width: 30.0,
        base_depth: 30.0,
    };

    let schedule = temp_schedule(&params);
    let num_blocks = schedule.len();
    assert_eq!(num_blocks, 9, "190..230 step 5 = 9 blocks");

    let mesh = generate_temp_tower_mesh(&params);
    assert!(mesh.vertex_count() > 0, "mesh must have vertices");
    assert!(mesh.triangle_count() > 0, "mesh must have triangles");

    let aabb = mesh.aabb();
    let height = aabb.max.z - aabb.min.z;
    let expected_height = 1.0 + 9.0 * 8.0; // 73mm
    assert!(
        (height - expected_height).abs() < 0.5,
        "height should be ~{expected_height}mm, got {height}"
    );
}

#[test]
fn test_temp_tower_mesh_fits_bed() {
    let params = TempTowerParams {
        start_temp: 190.0,
        end_temp: 230.0,
        step: 5.0,
        block_height: 8.0,
        base_width: 30.0,
        base_depth: 30.0,
    };
    let machine = MachineConfig {
        bed_x: 220.0,
        bed_y: 220.0,
        printable_height: 250.0,
        ..MachineConfig::default()
    };
    let schedule = temp_schedule(&params);
    let tower_height = 1.0 + schedule.len() as f64 * params.block_height;
    assert!(validate_bed_fit(params.base_width, params.base_depth, tower_height, &machine).is_ok());
}

#[test]
fn test_temp_tower_mesh_fails_small_bed() {
    let params = TempTowerParams {
        start_temp: 190.0,
        end_temp: 230.0,
        step: 5.0,
        block_height: 8.0,
        base_width: 30.0,
        base_depth: 30.0,
    };
    let machine = MachineConfig {
        bed_x: 40.0,
        bed_y: 40.0,
        printable_height: 250.0,
        ..MachineConfig::default()
    };
    let schedule = temp_schedule(&params);
    let tower_height = 1.0 + schedule.len() as f64 * params.block_height;
    let result = validate_bed_fit(params.base_width, params.base_depth, tower_height, &machine);
    assert!(result.is_err(), "30mm model should not fit on 40mm bed with 10mm margins (20mm usable)");
}

// ---------------------------------------------------------------------------
// 2. Temperature injection tests
// ---------------------------------------------------------------------------

#[test]
fn test_inject_temp_changes_correct_z() {
    let commands = vec![
        GcodeCommand::LinearMove {
            x: Some(10.0),
            y: Some(10.0),
            z: Some(1.0),
            e: Some(0.5),
            f: Some(600.0),
        },
        GcodeCommand::LinearMove {
            x: Some(20.0),
            y: Some(20.0),
            z: Some(9.0),
            e: Some(1.0),
            f: Some(600.0),
        },
        GcodeCommand::LinearMove {
            x: Some(30.0),
            y: Some(30.0),
            z: Some(17.0),
            e: Some(2.0),
            f: Some(600.0),
        },
        GcodeCommand::LinearMove {
            x: Some(40.0),
            y: Some(40.0),
            z: Some(25.0),
            e: Some(3.0),
            f: Some(600.0),
        },
    ];
    // Schedule: temp change at z=9 (210C) and z=17 (220C)
    let schedule = vec![(0.0, 200.0), (9.0, 210.0), (17.0, 220.0)];
    let result = inject_temp_changes(commands, &schedule);

    // Extract SetExtruderTemp commands
    let temp_cmds: Vec<f64> = result
        .iter()
        .filter_map(|c| match c {
            GcodeCommand::SetExtruderTemp { temp, .. } => Some(*temp),
            _ => None,
        })
        .collect();

    assert_eq!(
        temp_cmds,
        vec![210.0, 220.0],
        "should inject temps at z=9 and z=17"
    );
}

#[test]
fn test_inject_temp_preserves_all_commands() {
    let commands = vec![
        GcodeCommand::Comment("start".to_string()),
        GcodeCommand::LinearMove {
            x: Some(10.0),
            y: None,
            z: Some(5.0),
            e: None,
            f: Some(600.0),
        },
        GcodeCommand::LinearMove {
            x: Some(20.0),
            y: None,
            z: Some(15.0),
            e: None,
            f: Some(600.0),
        },
    ];
    let schedule = vec![(0.0, 200.0), (10.0, 210.0)];
    let result = inject_temp_changes(commands.clone(), &schedule);

    // Original commands must all be present; output >= input + injected
    let original_count = commands.len();
    let injected_count = 1; // one boundary crossed (z=10 boundary at z=15 move)
    assert!(
        result.len() >= original_count + injected_count,
        "output ({}) should be >= input ({}) + injected ({})",
        result.len(),
        original_count,
        injected_count,
    );
}

#[test]
fn test_empty_schedule_no_changes() {
    let commands = vec![
        GcodeCommand::LinearMove {
            x: Some(10.0),
            y: None,
            z: Some(5.0),
            e: None,
            f: Some(600.0),
        },
        GcodeCommand::Comment("end".to_string()),
    ];
    let result = inject_temp_changes(commands.clone(), &[]);
    assert_eq!(result.len(), commands.len(), "empty schedule should not change commands");
}

// ---------------------------------------------------------------------------
// 3. Cost model tests
// ---------------------------------------------------------------------------

#[test]
fn test_cost_model_full_inputs() {
    let inputs = CostInputs {
        filament_weight_g: 100.0,
        print_time_seconds: 7200.0, // 2 hours
        filament_price_per_kg: Some(25.0),
        electricity_rate: Some(0.12),
        printer_watts: Some(200.0),
        printer_cost: Some(500.0),
        expected_hours: Some(2000.0),
        labor_rate: Some(15.0),
        setup_time_minutes: Some(10.0),
    };
    let est = compute_cost(&inputs);

    // filament: 100g * 25/1000 = 2.50
    let fc = est.filament_cost.unwrap();
    assert!((fc - 2.50).abs() < 0.01, "filament_cost={fc}");

    // electricity: 2h * 0.2kW * 0.12 = 0.048
    let ec = est.electricity_cost.unwrap();
    assert!((ec - 0.048).abs() < 0.001, "electricity_cost={ec}");

    // depreciation: 500/2000 * 2 = 0.50
    let dc = est.depreciation_cost.unwrap();
    assert!((dc - 0.50).abs() < 0.01, "depreciation_cost={dc}");

    // labor: 15 * 10/60 = 2.50
    let lc = est.labor_cost.unwrap();
    assert!((lc - 2.50).abs() < 0.01, "labor_cost={lc}");

    // total
    let total = est.total_cost.unwrap();
    assert!((total - 5.548).abs() < 0.01, "total={total}");

    assert!(est.missing_hints.is_empty(), "no missing hints expected");
}

#[test]
fn test_cost_model_partial_inputs() {
    let inputs = CostInputs {
        filament_weight_g: 50.0,
        print_time_seconds: 3600.0,
        filament_price_per_kg: Some(25.0),
        ..CostInputs::default()
    };
    let est = compute_cost(&inputs);

    assert!(est.filament_cost.is_some(), "filament cost should be computed");
    assert!(est.electricity_cost.is_none(), "no watts => no electricity");
    assert!(est.depreciation_cost.is_none(), "no printer cost => no depreciation");
    assert!(est.labor_cost.is_none(), "no labor rate => no labor");
    assert!(!est.missing_hints.is_empty(), "should have hints for missing inputs");
}

#[test]
fn test_cost_model_zero_expected_hours() {
    let inputs = CostInputs {
        filament_weight_g: 50.0,
        print_time_seconds: 3600.0,
        printer_cost: Some(500.0),
        expected_hours: Some(0.0),
        ..CostInputs::default()
    };
    // Should not panic (no division by zero)
    let est = compute_cost(&inputs);
    assert!(
        est.depreciation_cost.is_none(),
        "zero expected_hours should skip depreciation"
    );
}

#[test]
fn test_volume_estimate_cube() {
    // 20mm cube = 8000 mm^3
    let est = volume_estimate(8000.0, 1.75, 1.24);
    assert!(est.filament_length_mm > 100.0, "length should be >100mm, got {}", est.filament_length_mm);
    assert!(est.filament_length_mm < 10000.0, "length should be <10000mm, got {}", est.filament_length_mm);
    assert!(est.filament_weight_g > 0.5, "weight should be >0.5g, got {}", est.filament_weight_g);
    assert!(est.filament_weight_g < 50.0, "weight should be <50g, got {}", est.filament_weight_g);
    assert!(est.rough_time_seconds > 0.0, "time should be positive");
}

#[test]
fn test_volume_estimate_disclaimer() {
    let est = volume_estimate(1000.0, 1.75, 1.24);
    assert!(!est.disclaimer.is_empty(), "disclaimer should not be empty");
}

// ---------------------------------------------------------------------------
// 4. Flow and first layer mesh tests
// ---------------------------------------------------------------------------

#[test]
fn test_flow_mesh_valid() {
    let params = FlowParams {
        baseline_multiplier: 1.0,
        step: 0.02,
        steps: 5,
    };
    let mesh = generate_flow_mesh(&params);
    assert!(mesh.vertex_count() > 0, "flow mesh should have vertices");
    assert!(mesh.triangle_count() > 0, "flow mesh should have triangles");
}

#[test]
fn test_first_layer_mesh_flat() {
    let params = FirstLayerParams {
        pattern: FirstLayerPattern::Grid,
        coverage_percent: 80.0,
    };
    let mesh = generate_first_layer_mesh(&params, 220.0, 220.0);
    assert!(mesh.triangle_count() > 0, "first layer mesh should have triangles");

    let aabb = mesh.aabb();
    let height = aabb.max.z - aabb.min.z;
    assert!(height < 1.0, "first layer height should be <1mm, got {height}");

    let width = aabb.max.x - aabb.min.x;
    let depth = aabb.max.y - aabb.min.y;
    // 80% of 220 = 176
    assert!(
        (width - 176.0).abs() < 1.0,
        "width should be ~176mm (80% of 220), got {width}"
    );
    assert!(
        (depth - 176.0).abs() < 1.0,
        "depth should be ~176mm (80% of 220), got {depth}"
    );
}

// ---------------------------------------------------------------------------
// 5. Retraction mesh tests
// ---------------------------------------------------------------------------

#[test]
fn test_retraction_mesh_valid() {
    let params = RetractionParams {
        start_distance: 0.5,
        end_distance: 3.0,
        step: 0.5,
        start_speed: 25.0,
        end_speed: 60.0,
    };
    let mesh = generate_retraction_mesh(&params);
    assert!(mesh.vertex_count() > 0, "retraction mesh should have vertices");
    assert!(mesh.triangle_count() > 0, "retraction mesh should have triangles");
}
