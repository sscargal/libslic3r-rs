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

// ===========================================================================
// E2E Calibrate Command Tests (Plan 31-06)
// ===========================================================================

#[test]
fn test_calibrate_temp_tower_generates_gcode() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("tower.gcode");

    let output = Command::new(cli_binary())
        .args([
            "calibrate",
            "temp-tower",
            "--start-temp",
            "190",
            "--end-temp",
            "220",
            "--step",
            "10",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}"
    );
    assert!(output_path.exists(), "G-code file should exist at {}", output_path.display());

    let gcode = std::fs::read_to_string(&output_path).unwrap();
    assert!(
        gcode.contains("M104") || gcode.contains("TEMPERATURE CHANGE"),
        "G-code should contain temperature commands, got:\n{}",
        &gcode[..gcode.len().min(500)]
    );
}

#[test]
fn test_calibrate_temp_tower_dry_run() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("tower.gcode");

    let output = Command::new(cli_binary())
        .args([
            "calibrate",
            "temp-tower",
            "--start-temp",
            "190",
            "--end-temp",
            "220",
            "--step",
            "10",
            "--dry-run",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}"
    );

    // Dry run should output model dimensions and temperature range to stderr
    assert!(
        stderr.contains("Dry Run") || stderr.contains("dimensions") || stderr.contains("190"),
        "Dry run should show model info on stderr, got:\n{stderr}"
    );

    // No G-code file should be created during dry run
    assert!(
        !output_path.exists(),
        "Dry run should not create G-code file"
    );
}

#[test]
fn test_calibrate_temp_tower_save_model() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("tower.gcode");
    let model_path = dir.path().join("tower.stl");

    let output = Command::new(cli_binary())
        .args([
            "calibrate",
            "temp-tower",
            "--start-temp",
            "190",
            "--end-temp",
            "220",
            "--step",
            "10",
            "-o",
            output_path.to_str().unwrap(),
            "--save-model",
            model_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}"
    );
    assert!(
        model_path.exists(),
        "STL model file should exist at {}",
        model_path.display()
    );
    // STL file should have non-trivial size (at least a valid header + triangles)
    let metadata = std::fs::metadata(&model_path).unwrap();
    assert!(
        metadata.len() > 84,
        "STL file should be larger than header-only (got {} bytes)",
        metadata.len()
    );
}

#[test]
fn test_calibrate_temp_tower_instructions() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("tower.gcode");

    let output = Command::new(cli_binary())
        .args([
            "calibrate",
            "temp-tower",
            "--start-temp",
            "190",
            "--end-temp",
            "220",
            "--step",
            "10",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}"
    );

    let instructions_path = output_path.with_extension("instructions.md");
    assert!(
        instructions_path.exists(),
        "Instructions file should be created at {}",
        instructions_path.display()
    );

    let instructions = std::fs::read_to_string(&instructions_path).unwrap();
    assert!(
        instructions.contains("Temperature Tower"),
        "Instructions should mention temperature tower"
    );
}

#[test]
fn test_calibrate_retraction_generates_gcode() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("retraction.gcode");

    let output = Command::new(cli_binary())
        .args([
            "calibrate",
            "retraction",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}"
    );
    assert!(output_path.exists(), "G-code file should exist");

    let gcode = std::fs::read_to_string(&output_path).unwrap();
    assert!(
        gcode.contains("RETRACTION SECTION") || gcode.contains("G1"),
        "G-code should contain retraction section comments or moves"
    );
}

#[test]
fn test_calibrate_flow_generates_gcode() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("flow.gcode");

    let output = Command::new(cli_binary())
        .args([
            "calibrate",
            "flow",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}"
    );
    assert!(output_path.exists(), "G-code file should exist");

    let gcode = std::fs::read_to_string(&output_path).unwrap();
    assert!(
        gcode.contains("M221") || gcode.contains("FLOW RATE"),
        "G-code should contain M221 flow commands or flow rate comments"
    );
}

#[test]
fn test_calibrate_first_layer_generates_gcode() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("first_layer.gcode");

    let output = Command::new(cli_binary())
        .args([
            "calibrate",
            "first-layer",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI failed: {stderr}"
    );
    assert!(output_path.exists(), "G-code file should exist");

    let gcode = std::fs::read_to_string(&output_path).unwrap();
    // First layer test produces very few layers; verify it has G-code moves
    assert!(
        gcode.contains("G1") || gcode.contains("G0"),
        "G-code should contain movement commands"
    );
}

#[test]
fn test_calibrate_list_shows_all() {
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

    // Should list all 4 calibration test names
    for name in &["temp-tower", "retraction", "flow", "first-layer"] {
        assert!(
            stdout.contains(name),
            "List output should contain '{name}', got:\n{stdout}"
        );
    }
}

// ===========================================================================
// Analyze-gcode cost output format tests (Plan 31-06)
// ===========================================================================

#[test]
fn test_analyze_gcode_cost_all_formats() {
    let dir = tempfile::tempdir().unwrap();
    let gcode_path = dir.path().join("test.gcode");
    write_minimal_gcode(&gcode_path);

    // Test JSON format
    let json_out = Command::new(cli_binary())
        .args([
            "analyze-gcode",
            gcode_path.to_str().unwrap(),
            "--filament-price",
            "25.0",
            "--json",
        ])
        .output()
        .expect("failed to run CLI");
    let json_stdout = String::from_utf8(json_out.stdout).unwrap();
    assert!(json_out.status.success(), "JSON mode failed");
    let _: serde_json::Value = serde_json::from_str(&json_stdout)
        .unwrap_or_else(|e| panic!("Invalid JSON output: {e}\n{json_stdout}"));

    // Test CSV format
    let csv_out = Command::new(cli_binary())
        .args([
            "analyze-gcode",
            gcode_path.to_str().unwrap(),
            "--filament-price",
            "25.0",
            "--csv",
            "--summary",
        ])
        .output()
        .expect("failed to run CLI");
    let csv_stdout = String::from_utf8(csv_out.stdout).unwrap();
    assert!(csv_out.status.success(), "CSV mode failed");
    assert!(csv_stdout.contains("component,"), "CSV should have header");

    // Test Markdown format
    let md_out = Command::new(cli_binary())
        .args([
            "analyze-gcode",
            gcode_path.to_str().unwrap(),
            "--filament-price",
            "25.0",
            "--markdown",
        ])
        .output()
        .expect("failed to run CLI");
    let md_stdout = String::from_utf8(md_out.stdout).unwrap();
    assert!(md_out.status.success(), "Markdown mode failed");
    assert!(
        md_stdout.contains("##") || md_stdout.contains("|"),
        "Markdown should contain headings or tables"
    );
}

#[test]
fn test_analyze_gcode_model_rough_estimate() {
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

    // Should contain rough estimate with disclaimer
    assert!(
        stdout.contains("Rough Estimate") || stdout.contains("Volume"),
        "Should show rough estimate section, got:\n{stdout}"
    );
    assert!(
        stdout.contains("accuracy") || stdout.contains("Disclaimer"),
        "Should show accuracy disclaimer, got:\n{stdout}"
    );
}

// ===========================================================================
// Error handling tests (Plan 31-06)
// ===========================================================================

#[test]
fn test_calibrate_bad_temp_range() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("bad.gcode");

    // start > end with positive step -- may produce 0 or 1 blocks but should not crash
    let output = Command::new(cli_binary())
        .args([
            "calibrate",
            "temp-tower",
            "--start-temp",
            "250",
            "--end-temp",
            "190",
            "--step",
            "10",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    // Either succeeds with degenerate output or fails gracefully -- should not panic
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The key assertion: the process should not crash with a panic/signal
    assert!(
        !stderr.contains("panicked"),
        "Should not panic on bad temp range. stderr: {stderr}\nstdout: {stdout}"
    );
}

#[test]
fn test_calibrate_invalid_output_dir() {
    let output = Command::new(cli_binary())
        .args([
            "calibrate",
            "temp-tower",
            "--start-temp",
            "190",
            "--end-temp",
            "220",
            "--step",
            "10",
            "-o",
            "/nonexistent/directory/tower.gcode",
        ])
        .output()
        .expect("failed to run slicecore CLI");

    // Should fail with a meaningful error, not a panic
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success() || stderr.contains("error") || stderr.contains("Error"),
        "Should fail or report error for nonexistent output directory"
    );
    assert!(
        !stderr.contains("panicked"),
        "Should not panic on invalid output dir. stderr: {stderr}"
    );
}
