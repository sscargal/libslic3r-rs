//! Integration tests for `--job-dir` CLI behavior.
//!
//! These tests exercise the job directory feature end-to-end by invoking the
//! compiled `slicecore` binary via `std::process::Command`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Creates a minimal binary STL cube (20mm, centered at 100,100 on a 220x220 bed).
fn write_cube_stl(path: &Path) {
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
fn cli_binary() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("slicecore");
    path
}

#[test]
fn test_job_dir_creates_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let job_path = dir.path().join("myjob");

    let output = Command::new(cli_binary())
        .args([
            "slice",
            "--job-dir",
            job_path.to_str().unwrap(),
            "--unsafe-defaults",
            stl_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Job directory should contain the expected artifacts.
    assert!(job_path.exists(), "job directory should exist");
    assert!(
        job_path.join("manifest.json").exists(),
        "manifest.json should exist"
    );
    assert!(
        job_path.join("config.toml").exists(),
        "config.toml should exist"
    );
    assert!(
        job_path.join("slice.log").exists(),
        "slice.log should exist"
    );
    assert!(
        job_path.join("cube.gcode").exists(),
        "cube.gcode should exist"
    );
}

#[test]
fn test_job_dir_auto_creates_uuid_dir() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args([
            "slice",
            "--job-dir",
            "auto",
            "--job-base",
            dir.path().to_str().unwrap(),
            "--unsafe-defaults",
            stl_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let job_path_str = stdout.trim();
    assert!(
        !job_path_str.is_empty(),
        "stdout should contain job directory path"
    );

    let job_path = PathBuf::from(job_path_str);
    assert!(job_path.exists(), "auto-created job directory should exist");

    // The directory name should be a UUID (36 chars: 8-4-4-4-12).
    let dir_name = job_path.file_name().unwrap().to_str().unwrap();
    assert_eq!(dir_name.len(), 36, "directory name should be a UUID");
    assert_eq!(
        dir_name.chars().filter(|c| *c == '-').count(),
        4,
        "UUID should have 4 dashes"
    );

    // Should be under the specified job-base directory.
    assert!(
        job_path.starts_with(dir.path()),
        "job dir should be under job-base"
    );
}

#[test]
fn test_job_dir_conflicts_with_output() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args([
            "slice",
            "--job-dir",
            "./j",
            "--output",
            "out.gcode",
            stl_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        !output.status.success(),
        "should fail with conflicting flags"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflict"),
        "stderr should mention conflict: {stderr}"
    );
}

#[test]
fn test_job_dir_conflicts_with_log_file() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args([
            "slice",
            "--job-dir",
            "./j",
            "--log-file",
            "log.txt",
            stl_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        !output.status.success(),
        "should fail with conflicting flags"
    );
}

#[test]
fn test_job_dir_conflicts_with_save_config() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let output = Command::new(cli_binary())
        .args([
            "slice",
            "--job-dir",
            "./j",
            "--save-config",
            "cfg.toml",
            stl_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        !output.status.success(),
        "should fail with conflicting flags"
    );
}

#[test]
fn test_job_dir_nonempty_fails() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let job_path = dir.path().join("nonempty-job");
    std::fs::create_dir_all(&job_path).unwrap();
    std::fs::write(job_path.join("existing.txt"), "data").unwrap();

    let output = Command::new(cli_binary())
        .args([
            "slice",
            "--job-dir",
            job_path.to_str().unwrap(),
            "--unsafe-defaults",
            stl_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        !output.status.success(),
        "should fail when job dir is non-empty"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not empty"),
        "stderr should mention 'not empty': {stderr}"
    );
}

#[test]
fn test_job_dir_nonempty_with_force() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let job_path = dir.path().join("force-job");
    std::fs::create_dir_all(&job_path).unwrap();
    std::fs::write(job_path.join("existing.txt"), "data").unwrap();

    let output = Command::new(cli_binary())
        .args([
            "slice",
            "--job-dir",
            job_path.to_str().unwrap(),
            "--force",
            "--unsafe-defaults",
            stl_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "should succeed with --force: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_job_dir_manifest_contents() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let job_path = dir.path().join("manifest-test");

    let output = Command::new(cli_binary())
        .args([
            "slice",
            "--job-dir",
            job_path.to_str().unwrap(),
            "--unsafe-defaults",
            stl_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest_str = std::fs::read_to_string(job_path.join("manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_str).unwrap();

    assert_eq!(manifest["schema_version"], 1, "schema_version should be 1");
    assert_eq!(
        manifest["status"], "success",
        "status should be 'success'"
    );
    assert!(
        manifest["checksums"].is_object(),
        "checksums should be an object"
    );
    assert!(
        manifest["environment"].is_object(),
        "environment should be an object"
    );
    assert!(
        manifest["created"].is_string(),
        "created timestamp should be present"
    );
    assert!(
        manifest["completed"].is_string(),
        "completed timestamp should be present"
    );
    assert!(
        manifest["duration_ms"].is_number(),
        "duration_ms should be a number"
    );

    // Statistics must be populated (gap closure for Plan 02 acceptance criteria).
    assert!(
        manifest["statistics"].is_object(),
        "statistics should be an object, got: {:?}",
        manifest["statistics"]
    );
    let stats = &manifest["statistics"];
    assert!(
        stats["layer_count"].as_u64().unwrap_or(0) > 0,
        "statistics.layer_count should be > 0, got: {:?}",
        stats["layer_count"]
    );
    assert!(
        stats["estimated_time_seconds"].as_f64().unwrap_or(0.0) > 0.0,
        "statistics.estimated_time_seconds should be > 0"
    );
    assert!(
        stats["filament_length_mm"].as_f64().is_some(),
        "statistics.filament_length_mm should be a number"
    );
    assert!(
        stats["line_count"].as_u64().unwrap_or(0) > 0,
        "statistics.line_count should be > 0"
    );
}

#[test]
fn test_job_dir_stdout_only_path() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let job_path = dir.path().join("stdout-test");

    let output = Command::new(cli_binary())
        .args([
            "slice",
            "--job-dir",
            job_path.to_str().unwrap(),
            "--unsafe-defaults",
            stl_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stdout_trimmed = stdout.trim();

    // stdout should be exactly the job directory path.
    assert_eq!(
        stdout_trimmed,
        job_path.to_str().unwrap(),
        "stdout should contain only the job directory path"
    );
}

#[test]
fn test_job_base_sets_parent() {
    let dir = tempfile::tempdir().unwrap();
    let stl_path = dir.path().join("cube.stl");
    write_cube_stl(&stl_path);

    let base_dir = dir.path().join("custom-base");
    std::fs::create_dir_all(&base_dir).unwrap();

    let output = Command::new(cli_binary())
        .args([
            "slice",
            "--job-dir",
            "auto",
            "--job-base",
            base_dir.to_str().unwrap(),
            "--unsafe-defaults",
            stl_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run slicecore CLI");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let job_path = PathBuf::from(stdout.trim());

    assert!(
        job_path.starts_with(&base_dir),
        "job directory should be under the specified base directory"
    );
}
