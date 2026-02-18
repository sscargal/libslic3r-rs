//! Integration tests for the ai-suggest CLI subcommand.

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
fn ai_suggest_help_works() {
    let output = Command::new(cli_binary())
        .args(["ai-suggest", "--help"])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "ai-suggest --help should exit 0: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("AI") || stdout.contains("ai"),
        "help should mention AI"
    );
    assert!(
        stdout.contains("mesh") || stdout.contains("Mesh") || stdout.contains("MESH"),
        "help should mention mesh"
    );
}

#[test]
fn ai_suggest_missing_input_fails() {
    let output = Command::new(cli_binary())
        .args(["ai-suggest"])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        !output.status.success(),
        "ai-suggest with no input should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    // clap reports missing required arguments
    assert!(
        stderr.contains("required") || stderr.contains("INPUT") || stderr.contains("Usage"),
        "error should mention missing required argument, got: {}",
        stderr
    );
}

#[test]
fn ai_suggest_nonexistent_file_fails() {
    let output = Command::new(cli_binary())
        .args(["ai-suggest", "/nonexistent/file.stl"])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        !output.status.success(),
        "ai-suggest with nonexistent file should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to read"),
        "stderr should mention 'Failed to read', got: {}",
        stderr
    );
}

#[test]
fn ai_suggest_default_provider_connection_error() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args(["ai-suggest", stl_path.to_str().unwrap()])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        !output.status.success(),
        "ai-suggest should fail when no AI provider is reachable"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to connect") || stderr.contains("AI suggestion failed")
            || stderr.contains("error"),
        "stderr should mention connection or AI error, got: {}",
        stderr
    );
}

#[test]
fn ai_suggest_invalid_ai_config_fails() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let bad_config = dir.path().join("bad.toml");
    std::fs::write(&bad_config, "this is not valid toml = = = {{{").unwrap();

    let output = Command::new(cli_binary())
        .args([
            "ai-suggest",
            stl_path.to_str().unwrap(),
            "--ai-config",
            bad_config.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        !output.status.success(),
        "ai-suggest with invalid config should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to parse AI config"),
        "stderr should mention config parse error, got: {}",
        stderr
    );
}

#[test]
fn ai_suggest_json_format_flag_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args([
            "ai-suggest",
            stl_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("failed to run slicecore CLI");

    // The command will fail (no Ollama running), but the --format flag should be accepted.
    // If the flag was not recognized, clap would print an "unknown argument" error.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("unknown argument"),
        "--format json should be accepted as a valid argument, got: {}",
        stderr
    );

    // Verify the error is about the provider, not argument parsing.
    assert!(
        stderr.contains("Failed to connect")
            || stderr.contains("AI suggestion failed")
            || stderr.contains("error"),
        "error should be about AI provider, not argument parsing, got: {}",
        stderr
    );
}
