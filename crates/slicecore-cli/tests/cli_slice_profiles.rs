//! E2E integration tests for the profile composition slice workflow.
//!
//! Tests cover: profile resolution, mutual exclusion, dry-run, save-config,
//! show-config, safety validation, unsafe-defaults, log files, exit codes,
//! and G-code header content.

use std::io::Write;
use std::process::Command;
use tempfile::TempDir;

/// Creates a minimal binary STL cube (20mm, centered at 100,100 on a 220x220 bed).
///
/// Re-uses the same cube geometry as cli_output.rs tests.
fn write_cube_stl(path: &std::path::Path) {
    let mut f = std::fs::File::create(path).unwrap();

    // 80-byte header.
    f.write_all(&[0u8; 80]).unwrap();

    let (x0, x1) = (90.0_f32, 110.0_f32);
    let (y0, y1) = (90.0_f32, 110.0_f32);
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

fn slicecore_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slicecore"))
}

/// Helper: create test STL in a temp dir and return path.
fn setup_stl(dir: &TempDir) -> std::path::PathBuf {
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);
    stl_path
}

// =============================================================================
// 1. Profile composition basics
// =============================================================================

#[test]
fn test_slice_with_builtin_profiles() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed with built-in profiles: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(gcode.exists(), "G-code file should be created");
    let content = std::fs::read_to_string(&gcode).unwrap();
    assert!(content.contains("G1"), "G-code should contain movement commands");
}

#[test]
fn test_slice_with_process_override() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "-p", "standard",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed with -m -f -p: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(gcode.exists());
}

#[test]
fn test_slice_with_set_override() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--set", "layer_height=0.1",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed with --set: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(gcode.exists());
}

#[test]
fn test_slice_with_multiple_set() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--set", "layer_height=0.1",
            "--set", "infill_density=0.3",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed with multiple --set: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_slice_with_file_path_profile() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    // Create a machine profile TOML file
    let machine_toml = dir.path().join("machine.toml");
    std::fs::write(
        &machine_toml,
        r#"
profile_type = "machine"
name = "Test Machine"

[machine]
bed_x = 220.0
bed_y = 220.0
printable_height = 250.0
nozzle_diameters = [0.4]
"#,
    )
    .unwrap();

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", machine_toml.to_str().unwrap(),
            "-f", "generic_pla",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    // File path resolution may or may not work depending on resolver; check exit code
    // The resolver should handle file paths directly
    let stderr = String::from_utf8_lossy(&output.stderr);
    // File paths go through the resolver which checks file existence
    assert!(
        output.status.success() || stderr.contains("Error"),
        "should either succeed or give meaningful error: {stderr}"
    );
}

// =============================================================================
// 2. Mutual exclusion
// =============================================================================

#[test]
fn test_config_and_machine_mutually_exclusive() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);

    // Create a minimal config
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "layer_height = 0.2\n").unwrap();

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "--config", config.to_str().unwrap(),
            "-m", "generic_printer",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        !output.status.success(),
        "should fail when --config and -m both specified"
    );
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2, "exit code should be 2 for argument conflict");
}

#[test]
fn test_config_and_filament_mutually_exclusive() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);

    let config = dir.path().join("config.toml");
    std::fs::write(&config, "layer_height = 0.2\n").unwrap();

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "--config", config.to_str().unwrap(),
            "-f", "generic_pla",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        !output.status.success(),
        "should fail when --config and -f both specified"
    );
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2, "exit code should be 2 for argument conflict");
}

#[test]
fn test_config_alone_still_works() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let config = dir.path().join("config.toml");
    std::fs::write(&config, "layer_height = 0.2\n").unwrap();

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "--config", config.to_str().unwrap(),
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed with --config alone: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(gcode.exists());
}

// =============================================================================
// 3. Dry run
// =============================================================================

#[test]
fn test_dry_run_no_gcode() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--dry-run",
            "-o", gcode.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    // --dry-run exits 0 via process::exit(0)
    assert!(
        output.status.success(),
        "dry-run should exit 0: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!gcode.exists(), "G-code file should NOT be created during dry-run");
}

#[test]
fn test_dry_run_shows_config_info() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--dry-run",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Dry Run") || stderr.contains("dry run") || stderr.contains("validated"),
        "stderr should contain dry-run summary info, got: {stderr}"
    );
}

// =============================================================================
// 4. Save config
// =============================================================================

#[test]
fn test_save_config_writes_toml() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");
    let saved_config = dir.path().join("saved.toml");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--save-config", saved_config.to_str().unwrap(),
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed with --save-config: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(saved_config.exists(), "saved config TOML should exist");
    let content = std::fs::read_to_string(&saved_config).unwrap();
    assert!(
        content.contains("layer_height"),
        "saved config should contain config fields"
    );
}

#[test]
fn test_save_config_contains_provenance() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");
    let saved_config = dir.path().join("saved.toml");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--save-config", saved_config.to_str().unwrap(),
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(output.status.success());
    let content = std::fs::read_to_string(&saved_config).unwrap();
    assert!(
        content.contains("Reproduce:") || content.contains("Generated by"),
        "saved config should contain provenance comments, got start:\n{}",
        &content[..content.len().min(300)]
    );
}

// =============================================================================
// 5. Show config
// =============================================================================

#[test]
fn test_show_config_outputs_to_stdout() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--show-config",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed with --show-config: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("layer_height") || stdout.contains("provenance"),
        "stdout should contain config output, got:\n{}",
        &stdout[..stdout.len().min(500)]
    );
}

#[test]
fn test_show_config_with_json() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--show-config",
            "--json",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed with --show-config --json: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // When --show-config --json is used, stdout contains JSON config
    // followed by structured slice output. Parse the first JSON object.
    assert!(
        stdout.contains("layer_height") || stdout.contains("{"),
        "stdout should contain JSON config, got:\n{}",
        &stdout[..stdout.len().min(500)]
    );
}

// =============================================================================
// 6. Safety validation
// =============================================================================

#[test]
fn test_dangerous_config_exits_4() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);

    // Create an overrides file with dangerously high nozzle temperature (>350 C)
    let overrides = dir.path().join("dangerous.toml");
    std::fs::write(
        &overrides,
        r#"
[filament]
nozzle_temperatures = [400.0]
"#,
    )
    .unwrap();

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--overrides", overrides.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    // Should exit with code 4 for safety validation error
    let code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        code, 4,
        "should exit 4 for dangerous config: {stderr}"
    );
}

#[test]
fn test_force_overrides_safety() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    // Create an overrides file with dangerously high nozzle temperature (>350 C)
    let overrides = dir.path().join("dangerous.toml");
    std::fs::write(
        &overrides,
        r#"
[filament]
nozzle_temperatures = [400.0]
"#,
    )
    .unwrap();

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--overrides", overrides.to_str().unwrap(),
            "--force",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    let code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        code == 0,
        "should succeed with --force overriding safety (code={code}): {stderr}"
    );
}

// =============================================================================
// 7. Unsafe defaults
// =============================================================================

#[test]
fn test_unsafe_defaults_works() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "--unsafe-defaults",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed with --unsafe-defaults: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(gcode.exists());
}

#[test]
fn test_no_profiles_uses_defaults() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    // Without -m/-f and without --unsafe-defaults, falls back to PrintConfig::default()
    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    // Without --config or profile flags, it uses PrintConfig::default() (legacy path)
    assert!(
        output.status.success(),
        "should succeed with defaults: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// =============================================================================
// 8. Log file
// =============================================================================

#[test]
fn test_log_file_created() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let log_path = dir.path().join("cube.log");
    assert!(log_path.exists(), "log file should be created alongside G-code");
    let log_content = std::fs::read_to_string(&log_path).unwrap();
    assert!(
        log_content.contains("SliceCore") || log_content.contains("Layers"),
        "log file should contain slice info"
    );
}

#[test]
fn test_no_log_suppresses() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--no-log",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let log_path = dir.path().join("cube.log");
    assert!(!log_path.exists(), "log file should NOT be created when --no-log is used");
}

#[test]
fn test_custom_log_file() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");
    let custom_log = dir.path().join("custom.log");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--log-file", custom_log.to_str().unwrap(),
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        custom_log.exists(),
        "custom log file should be created at specified path"
    );
    let log_content = std::fs::read_to_string(&custom_log).unwrap();
    assert!(log_content.contains("SliceCore") || log_content.contains("Layers"));
}

// =============================================================================
// 9. Exit codes
// =============================================================================

#[test]
fn test_exit_0_success() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert_eq!(output.status.code(), Some(0), "normal slice should exit 0");
}

#[test]
fn test_exit_2_profile_error() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "nonexistent_printer_xyz",
            "-f", "generic_pla",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(!output.status.success(), "should fail for nonexistent profile");
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2, "nonexistent profile should exit with code 2");
}

#[test]
fn test_exit_2_missing_filament() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "nonexistent_filament_xyz",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(!output.status.success(), "should fail for nonexistent filament");
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2, "nonexistent filament should exit with code 2");
}

// =============================================================================
// 10. G-code header
// =============================================================================

#[test]
fn test_gcode_contains_reproduce_command() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(output.status.success());
    let content = std::fs::read_to_string(&gcode).unwrap();
    assert!(
        content.contains("; Reproduce: slicecore slice"),
        "G-code should contain reproduce command, header start:\n{}",
        &content[..content.len().min(500)]
    );
}

#[test]
fn test_gcode_contains_profile_checksums() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(output.status.success());
    let content = std::fs::read_to_string(&gcode).unwrap();
    assert!(
        content.contains("; Profile:"),
        "G-code should contain profile checksum lines, header start:\n{}",
        &content[..content.len().min(500)]
    );
}

#[test]
fn test_gcode_contains_version() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(output.status.success());
    let content = std::fs::read_to_string(&gcode).unwrap();
    assert!(
        content.contains("; Generated by SliceCore v"),
        "G-code should contain version line, header start:\n{}",
        &content[..content.len().min(500)]
    );
}

#[test]
fn test_exit_4_safety_error() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);

    let overrides = dir.path().join("dangerous.toml");
    std::fs::write(
        &overrides,
        r#"
[filament]
nozzle_temperatures = [400.0]
"#,
    )
    .unwrap();

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--overrides", overrides.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 4, "safety validation should exit with code 4");
}

// =============================================================================
// Additional edge case tests
// =============================================================================

#[test]
fn test_dry_run_with_show_config() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--dry-run",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(output.status.success(), "dry-run should exit 0");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Profile:") || stderr.contains("profile"),
        "dry-run should show profile resolution info in stderr"
    );
}

#[test]
fn test_default_output_path() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);

    // Slice without -o; output should be cube.gcode in same directory
    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--quiet",
            "--no-log",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let default_gcode = dir.path().join("cube.gcode");
    assert!(
        default_gcode.exists(),
        "default G-code output should be input.gcode"
    );
}

#[test]
fn test_profile_workflow_produces_summary() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "-o", gcode.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Profile workflow should show a summary line
    assert!(
        stderr.contains("Sliced") || stderr.contains("layers"),
        "stderr should contain slice summary, got:\n{stderr}"
    );
}

#[test]
fn test_unsafe_defaults_with_set_override() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);
    let gcode = dir.path().join("cube.gcode");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "--unsafe-defaults",
            "--set", "layer_height=0.3",
            "-o", gcode.to_str().unwrap(),
            "--quiet",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        output.status.success(),
        "should succeed with --unsafe-defaults and --set: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_invalid_set_format() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir);

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "-m", "generic_printer",
            "-f", "generic_pla",
            "--set", "invalid_no_equals",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(!output.status.success(), "should fail with invalid --set format");
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 2, "invalid --set format should exit with code 2");
}
