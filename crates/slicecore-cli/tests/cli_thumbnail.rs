//! CLI integration tests for the thumbnail subcommand (RENDER-08).

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

/// Returns the path to the slicecore CLI binary (built by cargo).
fn cli_bin() -> PathBuf {
    // cargo test builds binaries in target/debug
    let mut path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    path.push("slicecore");
    path
}

/// Write a minimal binary STL file (single triangle) to the given path.
fn write_minimal_stl(path: &std::path::Path) {
    let mut f = std::fs::File::create(path).unwrap();

    // 80-byte header (zeros)
    f.write_all(&[0u8; 80]).unwrap();

    // Triangle count: 1
    f.write_all(&1u32.to_le_bytes()).unwrap();

    // Normal vector (0, 0, 1)
    f.write_all(&0.0f32.to_le_bytes()).unwrap();
    f.write_all(&0.0f32.to_le_bytes()).unwrap();
    f.write_all(&1.0f32.to_le_bytes()).unwrap();

    // Vertex 1 (0, 0, 0)
    f.write_all(&0.0f32.to_le_bytes()).unwrap();
    f.write_all(&0.0f32.to_le_bytes()).unwrap();
    f.write_all(&0.0f32.to_le_bytes()).unwrap();

    // Vertex 2 (10, 0, 0)
    f.write_all(&10.0f32.to_le_bytes()).unwrap();
    f.write_all(&0.0f32.to_le_bytes()).unwrap();
    f.write_all(&0.0f32.to_le_bytes()).unwrap();

    // Vertex 3 (5, 10, 0)
    f.write_all(&5.0f32.to_le_bytes()).unwrap();
    f.write_all(&10.0f32.to_le_bytes()).unwrap();
    f.write_all(&0.0f32.to_le_bytes()).unwrap();

    // Attribute byte count
    f.write_all(&0u16.to_le_bytes()).unwrap();
}

#[test]
fn render_08_cli_thumbnail_single_output() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("input.stl");
    let out_path = dir.path().join("output.png");

    write_minimal_stl(&stl_path);

    let output = Command::new(cli_bin())
        .args([
            "thumbnail",
            stl_path.to_str().unwrap(),
            "--output",
            out_path.to_str().unwrap(),
            "--resolution",
            "64x64",
        ])
        .output()
        .expect("Failed to run slicecore thumbnail");

    assert!(
        output.status.success(),
        "CLI thumbnail should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_path.exists(), "Output PNG file should exist");

    // Verify PNG magic bytes
    let data = std::fs::read(&out_path).unwrap();
    assert!(data.len() > 8, "PNG should be non-trivial");
    assert_eq!(&data[0..4], &[0x89, 0x50, 0x4E, 0x47], "PNG magic bytes");
}

#[test]
fn render_08_cli_thumbnail_multiple_angles() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("input.stl");

    write_minimal_stl(&stl_path);

    let output = Command::new(cli_bin())
        .args([
            "thumbnail",
            stl_path.to_str().unwrap(),
            "--angles",
            "front,back",
            "--resolution",
            "64x64",
            "--output",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run slicecore thumbnail");

    assert!(
        output.status.success(),
        "CLI thumbnail multi-angle should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check that individual angle files were created
    let front_path = dir.path().join("input_front.png");
    let back_path = dir.path().join("input_back.png");
    assert!(front_path.exists(), "input_front.png should exist");
    assert!(back_path.exists(), "input_back.png should exist");
}

#[test]
fn render_08_cli_thumbnail_help() {
    let output = Command::new(cli_bin())
        .args(["thumbnail", "--help"])
        .output()
        .expect("Failed to run slicecore thumbnail --help");

    assert!(output.status.success(), "thumbnail --help should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("thumbnail") || stdout.contains("Thumbnail"),
        "Help output should mention 'thumbnail'"
    );
}

#[test]
fn cli_thumbnail_jpeg_format_flag() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("input.stl");
    let out_path = dir.path().join("output.jpg");
    write_minimal_stl(&stl_path);

    let output = Command::new(cli_bin())
        .args([
            "thumbnail",
            stl_path.to_str().unwrap(),
            "--format",
            "jpeg",
            "--output",
            out_path.to_str().unwrap(),
            "--resolution",
            "64x64",
        ])
        .output()
        .expect("Failed to run slicecore thumbnail");

    assert!(
        output.status.success(),
        "CLI thumbnail --format jpeg should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_path.exists(), "Output JPEG file should exist");

    let data = std::fs::read(&out_path).unwrap();
    assert!(data.len() > 2, "JPEG should be non-trivial");
    assert_eq!(&data[0..3], &[0xFF, 0xD8, 0xFF], "JPEG magic bytes");
}

#[test]
fn cli_thumbnail_auto_detect_jpeg_from_extension() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("input.stl");
    let out_path = dir.path().join("output.jpg");
    write_minimal_stl(&stl_path);

    let output = Command::new(cli_bin())
        .args([
            "thumbnail",
            stl_path.to_str().unwrap(),
            "--output",
            out_path.to_str().unwrap(),
            "--resolution",
            "64x64",
        ])
        .output()
        .expect("Failed to run slicecore thumbnail");

    assert!(
        output.status.success(),
        "Auto-detect JPEG from .jpg extension: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let data = std::fs::read(&out_path).unwrap();
    assert_eq!(
        &data[0..3],
        &[0xFF, 0xD8, 0xFF],
        "Should be JPEG from extension auto-detect"
    );
}

#[test]
fn cli_thumbnail_jpeg_with_quality() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("input.stl");
    let out_path = dir.path().join("output.jpg");
    write_minimal_stl(&stl_path);

    let output = Command::new(cli_bin())
        .args([
            "thumbnail",
            stl_path.to_str().unwrap(),
            "--format",
            "jpeg",
            "--quality",
            "50",
            "--output",
            out_path.to_str().unwrap(),
            "--resolution",
            "64x64",
        ])
        .output()
        .expect("Failed to run slicecore thumbnail");

    assert!(
        output.status.success(),
        "JPEG with --quality 50 should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_path.exists());
    let data = std::fs::read(&out_path).unwrap();
    assert_eq!(&data[0..3], &[0xFF, 0xD8, 0xFF]);
}

#[test]
fn cli_thumbnail_jpeg_multi_angle_jpg_extension() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("input.stl");
    write_minimal_stl(&stl_path);

    let output = Command::new(cli_bin())
        .args([
            "thumbnail",
            stl_path.to_str().unwrap(),
            "--format",
            "jpeg",
            "--angles",
            "front,back",
            "--resolution",
            "64x64",
            "--output",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run slicecore thumbnail");

    assert!(
        output.status.success(),
        "JPEG multi-angle should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let front_path = dir.path().join("input_front.jpg");
    let back_path = dir.path().join("input_back.jpg");
    assert!(front_path.exists(), "input_front.jpg should exist");
    assert!(back_path.exists(), "input_back.jpg should exist");

    // Verify JPEG magic on at least one
    let data = std::fs::read(&front_path).unwrap();
    assert_eq!(&data[0..3], &[0xFF, 0xD8, 0xFF]);
}

#[test]
fn cli_thumbnail_png_quality_warns() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("input.stl");
    let out_path = dir.path().join("output.png");
    write_minimal_stl(&stl_path);

    let output = Command::new(cli_bin())
        .args([
            "thumbnail",
            stl_path.to_str().unwrap(),
            "--format",
            "png",
            "--quality",
            "50",
            "--output",
            out_path.to_str().unwrap(),
            "--resolution",
            "64x64",
        ])
        .output()
        .expect("Failed to run slicecore thumbnail");

    assert!(
        output.status.success(),
        "PNG with --quality should still succeed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ignored") || stderr.contains("Warning"),
        "Should warn about quality being ignored for PNG, got stderr: {}",
        stderr
    );

    // Output should still be PNG
    let data = std::fs::read(&out_path).unwrap();
    assert_eq!(
        &data[0..4],
        &[0x89, 0x50, 0x4E, 0x47],
        "Should still be PNG"
    );
}

#[test]
fn cli_thumbnail_jpeg_default_output_extension() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("model.stl");
    write_minimal_stl(&stl_path);

    let output = Command::new(cli_bin())
        .current_dir(dir.path())
        .args([
            "thumbnail",
            stl_path.to_str().unwrap(),
            "--format",
            "jpeg",
            "--resolution",
            "64x64",
        ])
        .output()
        .expect("Failed to run slicecore thumbnail");

    assert!(
        output.status.success(),
        "JPEG default output should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Default output should be model.jpg (not model.png)
    let jpg_path = dir.path().join("model.jpg");
    assert!(
        jpg_path.exists(),
        "Default output should be model.jpg, not model.png"
    );
}
