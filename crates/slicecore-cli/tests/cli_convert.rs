//! Integration tests for the `convert` CLI subcommand.
//!
//! Tests exercise the convert command by building test STL data, converting
//! between formats via the CLI binary, and verifying round-trip correctness.

use std::process::Command;
use tempfile::TempDir;

/// Build a minimal binary STL containing a single triangle.
fn minimal_stl_bytes() -> Vec<u8> {
    let mut buf = Vec::new();
    // 80-byte header (zeros)
    buf.extend_from_slice(&[0u8; 80]);
    // Triangle count: 1
    buf.extend_from_slice(&1u32.to_le_bytes());
    // Normal (0, 0, 1)
    buf.extend_from_slice(&0.0f32.to_le_bytes());
    buf.extend_from_slice(&0.0f32.to_le_bytes());
    buf.extend_from_slice(&1.0f32.to_le_bytes());
    // Vertex 1: (0, 0, 0)
    buf.extend_from_slice(&0.0f32.to_le_bytes());
    buf.extend_from_slice(&0.0f32.to_le_bytes());
    buf.extend_from_slice(&0.0f32.to_le_bytes());
    // Vertex 2: (1, 0, 0)
    buf.extend_from_slice(&1.0f32.to_le_bytes());
    buf.extend_from_slice(&0.0f32.to_le_bytes());
    buf.extend_from_slice(&0.0f32.to_le_bytes());
    // Vertex 3: (0, 1, 0)
    buf.extend_from_slice(&0.0f32.to_le_bytes());
    buf.extend_from_slice(&1.0f32.to_le_bytes());
    buf.extend_from_slice(&0.0f32.to_le_bytes());
    // Attribute byte count
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf
}

fn slicecore_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slicecore"))
}

#[test]
fn convert_stl_to_3mf() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.stl");
    let output = dir.path().join("output.3mf");

    std::fs::write(&input, minimal_stl_bytes()).unwrap();

    let result = slicecore_bin()
        .args(["convert", input.to_str().unwrap(), output.to_str().unwrap()])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "convert STL->3MF failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists(), "output 3MF file should exist");
    assert!(
        output.metadata().unwrap().len() > 0,
        "output should be non-empty"
    );

    // Verify the output can be re-imported
    let data = std::fs::read(&output).unwrap();
    let mesh = slicecore_fileio::load_mesh(&data).unwrap();
    assert_eq!(mesh.triangle_count(), 1);
}

#[test]
fn convert_stl_to_obj() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.stl");
    let output = dir.path().join("output.obj");

    std::fs::write(&input, minimal_stl_bytes()).unwrap();

    let result = slicecore_bin()
        .args(["convert", input.to_str().unwrap(), output.to_str().unwrap()])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "convert STL->OBJ failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists(), "output OBJ file should exist");

    // Verify the output can be re-imported
    let data = std::fs::read(&output).unwrap();
    let mesh = slicecore_fileio::load_mesh(&data).unwrap();
    assert_eq!(mesh.triangle_count(), 1);
}

#[test]
fn convert_stl_to_stl_roundtrip() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.stl");
    let output = dir.path().join("output.stl");

    std::fs::write(&input, minimal_stl_bytes()).unwrap();

    let result = slicecore_bin()
        .args(["convert", input.to_str().unwrap(), output.to_str().unwrap()])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "convert STL->STL failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let data = std::fs::read(&output).unwrap();
    let mesh = slicecore_fileio::load_mesh(&data).unwrap();
    assert_eq!(mesh.triangle_count(), 1);
}

#[test]
fn convert_unsupported_extension_fails() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.stl");
    let output = dir.path().join("output.xyz");

    std::fs::write(&input, minimal_stl_bytes()).unwrap();

    let result = slicecore_bin()
        .args(["convert", input.to_str().unwrap(), output.to_str().unwrap()])
        .output()
        .expect("failed to run slicecore");

    assert!(
        !result.status.success(),
        "should fail for unsupported extension"
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("Error") || stderr.contains("error"),
        "stderr should contain error message: {stderr}"
    );
}

#[test]
fn convert_missing_input_fails() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("nonexistent.stl");
    let output = dir.path().join("output.3mf");

    let result = slicecore_bin()
        .args(["convert", input.to_str().unwrap(), output.to_str().unwrap()])
        .output()
        .expect("failed to run slicecore");

    assert!(!result.status.success(), "should fail for missing input");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("Error") || stderr.contains("error"),
        "stderr should contain error message: {stderr}"
    );
}

#[test]
fn convert_shows_in_help() {
    let result = slicecore_bin()
        .args(["--help"])
        .output()
        .expect("failed to run slicecore");

    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("convert") || stdout.contains("Convert"),
        "help should mention convert subcommand: {stdout}"
    );
}
