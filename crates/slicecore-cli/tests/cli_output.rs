//! Integration tests for CLI structured output (--json, --msgpack).

use std::io::Write;
use std::process::Command;

/// Creates a minimal binary STL cube (20mm, centered at 100,100 on a 220x220 bed).
///
/// The cube has 12 triangles (2 per face). Vertices are in the range
/// [90, 110] x [90, 110] x [0, 20] mm.
fn write_cube_stl(path: &std::path::Path) {
    let mut f = std::fs::File::create(path).unwrap();

    // 80-byte header.
    f.write_all(&[0u8; 80]).unwrap();

    // A simple cube from (90,90,0) to (110,110,20).
    let (x0, x1) = (90.0_f32, 110.0_f32);
    let (y0, y1) = (90.0_f32, 110.0_f32);
    let (z0, z1) = (0.0_f32, 20.0_f32);

    // 12 triangles (2 per face).
    let triangles: Vec<([f32; 3], [[f32; 3]; 3])> = vec![
        // Bottom face (z=0, normal -Z)
        ([0.0, 0.0, -1.0], [[x0, y0, z0], [x1, y1, z0], [x1, y0, z0]]),
        ([0.0, 0.0, -1.0], [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0]]),
        // Top face (z=20, normal +Z)
        ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1]]),
        ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y1, z1], [x0, y1, z1]]),
        // Front face (y=0, normal -Y)
        ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1]]),
        ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z1], [x0, y0, z1]]),
        // Back face (y=20, normal +Y)
        ([0.0, 1.0, 0.0], [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1]]),
        ([0.0, 1.0, 0.0], [[x0, y1, z0], [x1, y1, z1], [x1, y1, z0]]),
        // Left face (x=0, normal -X)
        ([-1.0, 0.0, 0.0], [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1]]),
        ([-1.0, 0.0, 0.0], [[x0, y0, z0], [x0, y1, z1], [x0, y1, z0]]),
        // Right face (x=20, normal +X)
        ([1.0, 0.0, 0.0], [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1]]),
        ([1.0, 0.0, 0.0], [[x1, y0, z0], [x1, y1, z1], [x1, y0, z1]]),
    ];

    // Triangle count.
    let count = triangles.len() as u32;
    f.write_all(&count.to_le_bytes()).unwrap();

    for (normal, verts) in &triangles {
        // Normal.
        for c in normal {
            f.write_all(&c.to_le_bytes()).unwrap();
        }
        // 3 vertices.
        for v in verts {
            for c in v {
                f.write_all(&c.to_le_bytes()).unwrap();
            }
        }
        // Attribute byte count.
        f.write_all(&0u16.to_le_bytes()).unwrap();
    }
}

/// Returns the path to the compiled CLI binary.
fn cli_binary() -> std::path::PathBuf {
    // cargo test builds into target/debug/deps; the binary is at target/debug/slicecore.
    let mut path = std::env::current_exe().unwrap();
    // Go up from deps/ to debug/.
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("slicecore");
    path
}

#[test]
fn json_flag_produces_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    let gcode_path = dir.path().join("cube.gcode");

    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args([
            "slice",
            stl_path.to_str().unwrap(),
            "--output",
            gcode_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.is_empty(), "JSON output should not be empty");

    // Parse as JSON.
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(v["layer_count"].as_u64().unwrap() > 0);
    assert!(v["time_estimate"]["total_seconds"].as_f64().is_some());
    assert!(v["filament_usage"]["length_mm"].as_f64().is_some());
    assert!(v["config_summary"]["layer_height"].as_f64().is_some());
    assert!(v["config_summary"]["infill_pattern"].as_str().is_some());
}

#[test]
fn msgpack_flag_produces_decodable_output() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    let gcode_path = dir.path().join("cube.gcode");

    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args([
            "slice",
            stl_path.to_str().unwrap(),
            "--output",
            gcode_path.to_str().unwrap(),
            "--msgpack",
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(!output.stdout.is_empty(), "MessagePack output should not be empty");

    // Decode with rmp-serde (same as from_msgpack).
    let metadata: slicecore_engine::SliceMetadata =
        rmp_serde::from_slice(&output.stdout).unwrap();

    assert!(metadata.layer_count > 0);
    assert!(metadata.time_estimate.total_seconds > 0.0);
    assert!(metadata.filament_usage.length_mm > 0.0);
}

#[test]
fn no_flag_produces_human_summary_on_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    let gcode_path = dir.path().join("cube.gcode");

    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args([
            "slice",
            stl_path.to_str().unwrap(),
            "--output",
            gcode_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    // Statistics display replaces the old "Slicing complete:" summary.
    assert!(
        stdout.contains("=== Slicing Statistics ==="),
        "stdout should contain statistics header, got:\n{}",
        &stdout[..stdout.len().min(500)]
    );
    assert!(stdout.contains("Layers:"), "stdout should mention layers");
    assert!(stdout.contains("Output:"), "stdout should mention output path");
}
