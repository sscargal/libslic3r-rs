//! Integration tests for slicecore-gcode-io: full-file generation and validation
//! across all 4 firmware dialects, plus error-catching validation tests.

use slicecore_gcode_io::{
    validate_gcode, EndConfig, GcodeCommand, GcodeDialect, GcodeWriter, StartConfig,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a complete G-code file (start + movements + end) for a given dialect.
fn generate_full_gcode(dialect: GcodeDialect) -> String {
    let mut buf = Vec::new();
    let mut writer = GcodeWriter::new(&mut buf, dialect);

    // Start sequence
    writer
        .write_start_gcode(&StartConfig {
            bed_temp: 60.0,
            nozzle_temp: 200.0,
            bed_x: 220.0,
            bed_y: 220.0,
        })
        .expect("write_start_gcode");

    // Simulate a square perimeter at z=0.3
    writer
        .write_command(&GcodeCommand::Comment("Layer 1".to_string()))
        .unwrap();
    writer
        .write_command(&GcodeCommand::LinearMove {
            x: Some(10.0),
            y: Some(10.0),
            z: Some(0.3),
            e: Some(0.5),
            f: Some(1800.0),
        })
        .unwrap();
    writer
        .write_command(&GcodeCommand::LinearMove {
            x: Some(110.0),
            y: Some(10.0),
            z: None,
            e: Some(2.5),
            f: None,
        })
        .unwrap();
    writer
        .write_command(&GcodeCommand::LinearMove {
            x: Some(110.0),
            y: Some(110.0),
            z: None,
            e: Some(2.5),
            f: None,
        })
        .unwrap();
    writer
        .write_command(&GcodeCommand::LinearMove {
            x: Some(10.0),
            y: Some(110.0),
            z: None,
            e: Some(2.5),
            f: None,
        })
        .unwrap();
    writer
        .write_command(&GcodeCommand::LinearMove {
            x: Some(10.0),
            y: Some(10.0),
            z: None,
            e: Some(2.5),
            f: None,
        })
        .unwrap();

    // End sequence
    writer
        .write_end_gcode(&EndConfig {
            retract_distance: 1.0,
        })
        .expect("write_end_gcode");

    String::from_utf8(buf).expect("G-code should be valid UTF-8")
}

// ---------------------------------------------------------------------------
// Full-file validation tests for each dialect
// ---------------------------------------------------------------------------

#[test]
fn marlin_full_file_passes_validation() {
    let gcode = generate_full_gcode(GcodeDialect::Marlin);
    let result = validate_gcode(&gcode);
    assert!(
        result.valid,
        "Marlin full G-code should pass validation with 0 errors: {:?}",
        result.errors
    );
    assert_eq!(result.errors.len(), 0);
}

#[test]
fn klipper_full_file_passes_validation() {
    let gcode = generate_full_gcode(GcodeDialect::Klipper);
    let result = validate_gcode(&gcode);
    assert!(
        result.valid,
        "Klipper full G-code should pass validation with 0 errors: {:?}",
        result.errors
    );
    assert_eq!(result.errors.len(), 0);
}

#[test]
fn reprap_full_file_passes_validation() {
    let gcode = generate_full_gcode(GcodeDialect::RepRapFirmware);
    let result = validate_gcode(&gcode);
    assert!(
        result.valid,
        "RepRap full G-code should pass validation with 0 errors: {:?}",
        result.errors
    );
    assert_eq!(result.errors.len(), 0);
}

#[test]
fn bambu_full_file_passes_validation() {
    let gcode = generate_full_gcode(GcodeDialect::Bambu);
    let result = validate_gcode(&gcode);
    assert!(
        result.valid,
        "Bambu full G-code should pass validation with 0 errors: {:?}",
        result.errors
    );
    assert_eq!(result.errors.len(), 0);
}

// ---------------------------------------------------------------------------
// Dialect distinctiveness test
// ---------------------------------------------------------------------------

#[test]
fn each_dialect_has_distinct_output() {
    let dialects = [
        GcodeDialect::Marlin,
        GcodeDialect::Klipper,
        GcodeDialect::RepRapFirmware,
        GcodeDialect::Bambu,
    ];

    let start_cfg = StartConfig {
        bed_temp: 60.0,
        nozzle_temp: 200.0,
        bed_x: 220.0,
        bed_y: 220.0,
    };

    let mut outputs = Vec::new();
    for &dialect in &dialects {
        let mut buf = Vec::new();
        let mut writer = GcodeWriter::new(&mut buf, dialect);
        writer.write_start_gcode(&start_cfg).unwrap();
        outputs.push(String::from_utf8(buf).unwrap());
    }

    // Not all identical: at least Klipper should differ (extended commands)
    let all_same = outputs.windows(2).all(|w| w[0] == w[1]);
    assert!(
        !all_same,
        "dialect outputs should not all be identical"
    );

    // Specifically, Klipper should be different from Marlin
    assert_ne!(
        outputs[0], outputs[1],
        "Marlin and Klipper start G-code should differ"
    );
}

// ---------------------------------------------------------------------------
// Validator error-catching tests
// ---------------------------------------------------------------------------

#[test]
fn validator_catches_invalid_temperature() {
    let gcode = "M109 S500\n";
    let result = validate_gcode(gcode);
    assert!(
        !result.valid,
        "temperature 500 should be invalid"
    );
    assert!(
        result.errors.iter().any(|e| e.contains("temperature")),
        "error should mention temperature: {:?}",
        result.errors
    );
}

#[test]
fn validator_catches_nan_coordinate() {
    let gcode = "G1 X1.000 YNaN\n";
    let result = validate_gcode(gcode);
    assert!(!result.valid, "NaN coordinate should be invalid");
    assert!(
        result.errors.iter().any(|e| e.contains("non-finite")),
        "error should mention non-finite: {:?}",
        result.errors
    );
}
