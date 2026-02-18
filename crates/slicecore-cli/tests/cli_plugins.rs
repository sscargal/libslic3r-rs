//! Integration tests for plugin features via CLI.

use std::io::Write;
use std::process::Command;

/// Creates a minimal binary STL cube (20mm, centered at 100,100 on a 220x220 bed).
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
fn slice_plugin_dir_flag_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    let gcode_path = dir.path().join("cube.gcode");
    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args([
            "slice",
            stl_path.to_str().unwrap(),
            "--plugin-dir",
            "/tmp/nonexistent-plugins",
            "--output",
            gcode_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    // The nonexistent plugin dir should produce a warning but slicing should continue
    // with default (Rectilinear) infill since no plugin pattern is requested.
    assert!(
        output.status.success(),
        "slice with --plugin-dir pointing to nonexistent dir should still succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // G-code file should be produced.
    assert!(
        gcode_path.exists(),
        "gcode output should be created"
    );
}

#[test]
fn slice_plugin_infill_without_plugin_dir_fails() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    let gcode_path = dir.path().join("cube.gcode");
    write_cube_stl(&stl_path);

    // Create a config that requests a plugin infill pattern but has no plugin_dir.
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "infill_pattern = { plugin = \"zigzag\" }\n",
    )
    .unwrap();

    let output = Command::new(cli_binary())
        .args([
            "slice",
            stl_path.to_str().unwrap(),
            "--config",
            config_path.to_str().unwrap(),
            "--output",
            gcode_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        !output.status.success(),
        "slice with plugin infill but no plugin_dir should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("plugin") && stderr.contains("dir"),
        "error should mention plugin directory, got: {}",
        stderr
    );
}

#[test]
fn slice_plugin_dir_overrides_config() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    let gcode_path = dir.path().join("cube.gcode");
    write_cube_stl(&stl_path);

    // Config with a plugin_dir that does not exist.
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "plugin_dir = \"/original/path\"\n",
    )
    .unwrap();

    let output = Command::new(cli_binary())
        .args([
            "slice",
            stl_path.to_str().unwrap(),
            "--config",
            config_path.to_str().unwrap(),
            "--plugin-dir",
            "/override/path",
            "--output",
            gcode_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    // The command should succeed (plugin loading from /override/path may warn, but slicing
    // continues with default infill pattern since config does not request a plugin pattern).
    assert!(
        output.status.success(),
        "slice with --plugin-dir override should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // G-code file should be produced.
    assert!(
        gcode_path.exists(),
        "gcode output should be created"
    );
}

#[test]
fn slice_with_plugin_dir_loads_plugins() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    let gcode_path = dir.path().join("cube.gcode");
    write_cube_stl(&stl_path);

    // Create an empty plugin directory (valid dir, no plugins in it).
    let plugin_dir = dir.path().join("plugins");
    std::fs::create_dir(&plugin_dir).unwrap();

    let output = Command::new(cli_binary())
        .args([
            "slice",
            stl_path.to_str().unwrap(),
            "--plugin-dir",
            plugin_dir.to_str().unwrap(),
            "--output",
            gcode_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "slice with empty plugin dir should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify no error messages (only info/warning allowed).
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Error:"),
        "stderr should not contain errors, got: {}",
        stderr
    );
}

#[test]
fn help_text_documents_plugins_and_ai() {
    let output = Command::new(cli_binary())
        .args(["--help"])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "--help should exit 0"
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("PLUGIN SUPPORT"),
        "help should contain PLUGIN SUPPORT section"
    );
    assert!(
        stdout.contains("AI PROFILE SUGGESTIONS"),
        "help should contain AI PROFILE SUGGESTIONS section"
    );
}
