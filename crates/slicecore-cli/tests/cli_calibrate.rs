//! Integration tests for cost estimation in analyze-gcode and calibrate commands.

use std::io::Write;
use std::process::Command;

/// Creates a minimal G-code file with a few moves for testing cost estimation.
fn write_minimal_gcode(path: &std::path::Path) {
    let gcode = r#"; generated test gcode
; HEADER_BLOCK_START
; estimated printing time (normal mode) = 1h 30m 0s
; filament used [mm] = 5000.00
; filament used [g] = 12.50
; HEADER_BLOCK_END
G28 ; Home
G1 Z0.3 F3000
G1 X10 Y10 E1.0 F1500
G1 X50 Y10 E5.0
G1 X50 Y50 E10.0
G1 X10 Y50 E15.0
G1 X10 Y10 E20.0
G1 Z0.6
G1 X10 Y10 E21.0
G1 X50 Y10 E26.0
G1 X50 Y50 E31.0
G1 X10 Y50 E36.0
G1 X10 Y10 E41.0
G1 Z10.0 F3000
G1 X0 Y0
M84
"#;
    std::fs::write(path, gcode).unwrap();
}

/// Creates a minimal binary STL cube for testing model estimation.
fn write_cube_stl(path: &std::path::Path) {
    let mut f = std::fs::File::create(path).unwrap();

    // 80-byte header
    f.write_all(&[0u8; 80]).unwrap();

    let (x0, x1) = (0.0_f32, 20.0_f32);
    let (y0, y1) = (0.0_f32, 20.0_f32);
    let (z0, z1) = (0.0_f32, 20.0_f32);

    let triangles: Vec<([f32; 3], [[f32; 3]; 3])> = vec![
        ([0.0, 0.0, -1.0], [[x0, y0, z0], [x1, y1, z0], [x1, y0, z0]]),
        ([0.0, 0.0, -1.0], [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0]]),
        ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1]]),
        ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y1, z1], [x0, y1, z1]]),
        ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1]]),
        ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z1], [x0, y0, z1]]),
        ([0.0, 1.0, 0.0], [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1]]),
        ([0.0, 1.0, 0.0], [[x0, y1, z0], [x1, y1, z1], [x1, y1, z0]]),
        ([-1.0, 0.0, 0.0], [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1]]),
        ([-1.0, 0.0, 0.0], [[x0, y0, z0], [x0, y1, z1], [x0, y1, z0]]),
        ([1.0, 0.0, 0.0], [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1]]),
        ([1.0, 0.0, 0.0], [[x1, y0, z0], [x1, y1, z1], [x1, y0, z1]]),
    ];

    let count = triangles.len() as u32;
    f.write_all(&count.to_le_bytes()).unwrap();

    for (normal, verts) in &triangles {
        for c in normal {
            f.write_all(&c.to_le_bytes()).unwrap();
        }
        for v in verts {
            for c in v {
                f.write_all(&c.to_le_bytes()).unwrap();
            }
        }
        f.write_all(&0u16.to_le_bytes()).unwrap();
    }
}

/// Returns the path to the compiled CLI binary.
fn cli_binary() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("slicecore");
    path
}

#[test]
fn test_analyze_gcode_cost_flags() {
    let dir = tempfile::tempdir().unwrap();
    let gcode_path = dir.path().join("test.gcode");
    write_minimal_gcode(&gcode_path);

    let output = Command::new(cli_binary())
        .args([
            "analyze-gcode",
            gcode_path.to_str().unwrap(),
            "--filament-price",
            "25.0",
            "--printer-watts",
            "200",
            "--electricity-rate",
            "0.12",
            "--no-color",
            "--summary",
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}\nstdout: {stdout}"
    );

    // Should contain cost table with Filament and Electricity lines
    assert!(
        stdout.contains("Cost Estimate"),
        "Should show cost estimate section, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Filament"),
        "Should show Filament cost line, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Electricity"),
        "Should show Electricity cost line, got:\n{stdout}"
    );
    // Depreciation and Labor should show N/A since we didn't provide those flags
    assert!(
        stdout.contains("N/A"),
        "Missing cost inputs should show N/A, got:\n{stdout}"
    );
}

#[test]
fn test_analyze_gcode_cost_json() {
    let dir = tempfile::tempdir().unwrap();
    let gcode_path = dir.path().join("test.gcode");
    write_minimal_gcode(&gcode_path);

    let output = Command::new(cli_binary())
        .args([
            "analyze-gcode",
            gcode_path.to_str().unwrap(),
            "--filament-price",
            "25.0",
            "--printer-watts",
            "200",
            "--electricity-rate",
            "0.12",
            "--json",
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}\nstdout: {stdout}"
    );

    // Parse as JSON
    let v: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("Invalid JSON: {e}\n{stdout}"));
    assert!(
        v["cost_estimate"].is_object(),
        "JSON should have cost_estimate field"
    );
    assert!(
        v["cost_estimate"]["filament_cost"].is_number(),
        "Should have filament_cost"
    );
    assert!(
        v["cost_estimate"]["electricity_cost"].is_number(),
        "Should have electricity_cost"
    );
}

#[test]
fn test_analyze_gcode_model_estimation() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args([
            "analyze-gcode",
            stl_path.to_str().unwrap(),
            "--model",
            "--filament-price",
            "25.0",
            "--no-color",
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}\nstdout: {stdout}"
    );

    // Should contain rough estimate section
    assert!(
        stdout.contains("Volume-Based Rough Estimate"),
        "Should show volume estimate section, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Filament length"),
        "Should show filament length, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Filament weight"),
        "Should show filament weight, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Disclaimer") || stdout.contains("accuracy"),
        "Should show disclaimer, got:\n{stdout}"
    );
}

#[test]
fn test_analyze_gcode_cost_csv() {
    let dir = tempfile::tempdir().unwrap();
    let gcode_path = dir.path().join("test.gcode");
    write_minimal_gcode(&gcode_path);

    let output = Command::new(cli_binary())
        .args([
            "analyze-gcode",
            gcode_path.to_str().unwrap(),
            "--filament-price",
            "25.0",
            "--csv",
            "--summary",
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}\nstdout: {stdout}"
    );

    // CSV output should have cost rows
    assert!(
        stdout.contains("component,amount,hint"),
        "CSV should have cost header, got:\n{stdout}"
    );
    assert!(
        stdout.contains("filament,"),
        "CSV should have filament row, got:\n{stdout}"
    );
    assert!(
        stdout.contains("total,"),
        "CSV should have total row, got:\n{stdout}"
    );
}

#[test]
fn test_analyze_gcode_cost_markdown() {
    let dir = tempfile::tempdir().unwrap();
    let gcode_path = dir.path().join("test.gcode");
    write_minimal_gcode(&gcode_path);

    let output = Command::new(cli_binary())
        .args([
            "analyze-gcode",
            gcode_path.to_str().unwrap(),
            "--filament-price",
            "25.0",
            "--markdown",
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}\nstdout: {stdout}"
    );

    // Markdown output should contain markdown table markers
    assert!(
        stdout.contains("## Cost Estimate"),
        "Markdown should have cost heading, got:\n{stdout}"
    );
    assert!(
        stdout.contains("|--------"),
        "Markdown should have table separators, got:\n{stdout}"
    );
    assert!(
        stdout.contains("| Filament |"),
        "Markdown should have Filament row, got:\n{stdout}"
    );
}

#[test]
fn test_calibrate_list() {
    let output = Command::new(cli_binary())
        .args(["calibrate", "list"])
        .output()
        .expect("failed to run slicecore CLI");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}\nstdout: {stdout}"
    );

    // Should list all calibration tests
    assert!(
        stdout.contains("temp-tower"),
        "Should list temp-tower, got:\n{stdout}"
    );
    assert!(
        stdout.contains("retraction"),
        "Should list retraction, got:\n{stdout}"
    );
    assert!(
        stdout.contains("flow"),
        "Should list flow, got:\n{stdout}"
    );
    assert!(
        stdout.contains("first-layer"),
        "Should list first-layer, got:\n{stdout}"
    );
}

#[test]
fn test_calibrate_help() {
    let output = Command::new(cli_binary())
        .args(["calibrate", "--help"])
        .output()
        .expect("failed to run slicecore CLI");

    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should show subcommands in help text
    assert!(
        stdout.contains("temp-tower") || stdout.contains("Subcommands") || stdout.contains("Commands"),
        "Help should mention subcommands, got:\n{stdout}"
    );
}
