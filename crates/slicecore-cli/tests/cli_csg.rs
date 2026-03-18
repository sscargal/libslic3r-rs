//! Integration tests for the `csg` CLI subcommand.
//!
//! Tests exercise CSG operations by generating primitive meshes, running
//! boolean/split/hollow operations via the CLI binary, and verifying outputs.

use std::process::Command;
use tempfile::TempDir;

fn slicecore_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slicecore"))
}

/// Helper: generate a box primitive STL at the given path.
fn generate_box_stl(
    dir: &std::path::Path,
    name: &str,
    w: f64,
    h: f64,
    d: f64,
) -> std::path::PathBuf {
    let path = dir.join(name);
    let result = slicecore_bin()
        .args([
            "csg",
            "primitive",
            "box",
            "--dims",
            &w.to_string(),
            &h.to_string(),
            &d.to_string(),
            "-o",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore csg primitive");

    assert!(
        result.status.success(),
        "primitive box failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(path.exists(), "primitive output should exist");
    path
}

#[test]
fn test_csg_primitive_box() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("box.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "primitive",
            "box",
            "--dims",
            "10",
            "20",
            "30",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "primitive box failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists());
    let data = std::fs::read(&output).unwrap();
    let mesh = slicecore_fileio::load_mesh(&data).unwrap();
    assert_eq!(mesh.triangle_count(), 12, "box should have 12 triangles");
}

#[test]
fn test_csg_primitive_sphere() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("sphere.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "primitive",
            "sphere",
            "--dims",
            "5",
            "--segments",
            "16",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "primitive sphere failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists());
    let data = std::fs::read(&output).unwrap();
    let mesh = slicecore_fileio::load_mesh(&data).unwrap();
    assert!(mesh.triangle_count() > 0, "sphere should have triangles");
}

#[test]
fn test_csg_union_stl() {
    let dir = TempDir::new().unwrap();
    let a = generate_box_stl(dir.path(), "a.stl", 10.0, 10.0, 10.0);
    let b = generate_box_stl(dir.path(), "b.stl", 10.0, 10.0, 10.0);
    let output = dir.path().join("union.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "union",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "csg union failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists());
    let data = std::fs::read(&output).unwrap();
    let mesh = slicecore_fileio::load_mesh(&data).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn test_csg_difference_stl() {
    let dir = TempDir::new().unwrap();
    let a = generate_box_stl(dir.path(), "a.stl", 20.0, 20.0, 20.0);
    let b = generate_box_stl(dir.path(), "b.stl", 10.0, 10.0, 10.0);
    let output = dir.path().join("diff.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "difference",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "csg difference failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists());
    let data = std::fs::read(&output).unwrap();
    let mesh = slicecore_fileio::load_mesh(&data).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn test_csg_intersection_stl() {
    let dir = TempDir::new().unwrap();
    let a = generate_box_stl(dir.path(), "a.stl", 10.0, 10.0, 10.0);
    let b = generate_box_stl(dir.path(), "b.stl", 10.0, 10.0, 10.0);
    let output = dir.path().join("intersect.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "intersection",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "csg intersection failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists());
    let data = std::fs::read(&output).unwrap();
    let mesh = slicecore_fileio::load_mesh(&data).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn test_csg_xor_stl() {
    let dir = TempDir::new().unwrap();
    let a = generate_box_stl(dir.path(), "a.stl", 15.0, 15.0, 15.0);
    let b = generate_box_stl(dir.path(), "b.stl", 10.0, 10.0, 10.0);
    let output = dir.path().join("xor.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "xor",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "csg xor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists());
    let data = std::fs::read(&output).unwrap();
    let mesh = slicecore_fileio::load_mesh(&data).unwrap();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn test_csg_split_box() {
    let dir = TempDir::new().unwrap();
    let input = generate_box_stl(dir.path(), "box.stl", 10.0, 10.0, 10.0);
    let top = dir.path().join("top.stl");
    let bottom = dir.path().join("bottom.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "split",
            input.to_str().unwrap(),
            "--plane",
            "0,0,1,0",
            "-o",
            top.to_str().unwrap(),
            bottom.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "csg split failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(top.exists(), "top half should exist");
    assert!(bottom.exists(), "bottom half should exist");
}

#[test]
fn test_csg_hollow_box() {
    let dir = TempDir::new().unwrap();
    let input = generate_box_stl(dir.path(), "box.stl", 20.0, 20.0, 20.0);
    let output = dir.path().join("hollow.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "hollow",
            input.to_str().unwrap(),
            "--wall",
            "2",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "csg hollow failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists());
    let data = std::fs::read(&output).unwrap();
    let mesh = slicecore_fileio::load_mesh(&data).unwrap();
    assert!(
        mesh.triangle_count() > 12,
        "hollow box should have more triangles than a solid box"
    );
}

#[test]
fn test_csg_info_json() {
    let dir = TempDir::new().unwrap();
    let input = generate_box_stl(dir.path(), "box.stl", 10.0, 10.0, 10.0);

    let result = slicecore_bin()
        .args(["csg", "info", input.to_str().unwrap(), "--json"])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "csg info failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stdout = String::from_utf8_lossy(&result.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("invalid JSON: {e}\nstdout: {stdout}"));

    assert_eq!(json["triangle_count"], 12);
    assert!(json["vertex_count"].as_u64().unwrap() > 0);
    assert!(json["volume"].as_f64().unwrap() > 0.0);
    assert!(json["surface_area"].as_f64().unwrap() > 0.0);
    assert!(json["is_manifold"].as_bool().is_some());
}

#[test]
fn test_csg_info_table() {
    let dir = TempDir::new().unwrap();
    let input = generate_box_stl(dir.path(), "box.stl", 10.0, 10.0, 10.0);

    let result = slicecore_bin()
        .args(["csg", "info", input.to_str().unwrap()])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "csg info (table) failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("Triangle count"),
        "should show triangle count label"
    );
    assert!(stdout.contains("Volume"), "should show volume label");
    assert!(
        stdout.contains("Surface area"),
        "should show surface area label"
    );
}

#[test]
fn test_csg_union_json_report() {
    let dir = TempDir::new().unwrap();
    let a = generate_box_stl(dir.path(), "a.stl", 10.0, 10.0, 10.0);
    let b = generate_box_stl(dir.path(), "b.stl", 10.0, 10.0, 10.0);
    let output = dir.path().join("union.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "union",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "csg union --json failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stdout = String::from_utf8_lossy(&result.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("invalid CsgReport JSON: {e}\nstdout: {stdout}"));

    assert!(json["output_triangles"].as_u64().unwrap() > 0);
    assert!(json["input_triangles_a"].as_u64().unwrap() > 0);
    assert!(json["input_triangles_b"].as_u64().unwrap() > 0);
}

#[test]
fn test_csg_verbose_output() {
    let dir = TempDir::new().unwrap();
    let a = generate_box_stl(dir.path(), "a.stl", 10.0, 10.0, 10.0);
    let b = generate_box_stl(dir.path(), "b.stl", 10.0, 10.0, 10.0);
    let output = dir.path().join("union.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "union",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-v",
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        result.status.success(),
        "csg union -v failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("Loading mesh A"),
        "verbose should show loading info"
    );
    assert!(
        stderr.contains("triangles"),
        "verbose should mention triangle counts"
    );
    assert!(stderr.contains("Done in"), "verbose should show timing");
}

#[test]
fn test_csg_invalid_input() {
    let dir = TempDir::new().unwrap();
    let nonexistent = dir.path().join("nonexistent.stl");
    let output = dir.path().join("out.stl");

    let result = slicecore_bin()
        .args([
            "csg",
            "union",
            nonexistent.to_str().unwrap(),
            nonexistent.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore");

    assert!(
        !result.status.success(),
        "should fail for nonexistent input"
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("Error") || stderr.contains("error") || stderr.contains("failed"),
        "stderr should contain error message, got: {stderr}"
    );
}
