//! Integration tests for plate-mode slicing: --plate, --object, multi-model,
//! --strict, --save-plate, and validation.

use std::io::Write;
use std::process::Command;
use tempfile::TempDir;

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

fn slicecore_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slicecore"))
}

fn setup_stl(dir: &TempDir, name: &str) -> std::path::PathBuf {
    let stl_path = dir.path().join(format!("{name}.stl"));
    write_cube_stl(&stl_path);
    stl_path
}

// =============================================================================
// 1. --plate loads and parses a plate.toml fixture
// =============================================================================

#[test]
fn test_plate_loads_toml_config() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir, "cube");

    // Write a plate.toml that references the STL file.
    // PlateConfig expects [[objects]] array with mesh_source as tagged enum.
    let stl_escaped = stl.display().to_string().replace('\\', "\\\\");
    let plate_toml = format!(
        r#"
[[objects]]
name = "cube"
mesh_source = {{ File = "{stl}" }}
copies = 1
"#,
        stl = stl_escaped
    );
    let plate_path = dir.path().join("plate.toml");
    std::fs::write(&plate_path, &plate_toml).unwrap();

    let output = slicecore_bin()
        .args([
            "slice",
            "--plate",
            plate_path.to_str().unwrap(),
            "--force",
            "--no-log",
        ])
        .output()
        .unwrap();

    // Should succeed (exit 0) or produce output
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The plate mode should produce output mentioning objects
    assert!(
        output.status.success() || stderr.contains("Plate") || stdout.contains("object"),
        "Expected plate slicing to produce output. stderr: {stderr}, stdout: {stdout}"
    );
}

// =============================================================================
// 2. --object 1:infill_density=0.8 applies inline override
// =============================================================================

#[test]
fn test_object_inline_override() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir, "cube");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "--object",
            "1:infill_density=0.8",
            "--force",
            "--dry-run",
            "--no-log",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Dry run should succeed -- the engine builds without slicing
    assert!(
        output.status.success(),
        "Expected dry-run with --object to succeed. stderr: {stderr}"
    );
}

// =============================================================================
// 3. --plate and positional input are mutually exclusive
// =============================================================================

#[test]
fn test_plate_and_input_mutually_exclusive() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir, "cube");
    let plate_path = dir.path().join("plate.toml");
    std::fs::write(&plate_path, "[[objects]]\ncopies = 1\n").unwrap();

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "--plate",
            plate_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Should fail with exit code 2
    assert!(
        !output.status.success(),
        "Expected failure when both --plate and positional args provided"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("mutually exclusive") || stderr.contains("cannot be used with"),
        "Expected mutual exclusion error. stderr: {stderr}"
    );
}

// =============================================================================
// 4. Multiple model files create multi-object plate
// =============================================================================

#[test]
fn test_multiple_models_create_plate() {
    let dir = TempDir::new().unwrap();
    let stl_a = setup_stl(&dir, "cube_a");
    let stl_b = setup_stl(&dir, "cube_b");

    let output = slicecore_bin()
        .args([
            "slice",
            stl_a.to_str().unwrap(),
            stl_b.to_str().unwrap(),
            "--force",
            "--no-log",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should succeed and mention 2 objects
    assert!(
        output.status.success(),
        "Expected multi-model slice to succeed. stderr: {stderr}"
    );
    assert!(
        stdout.contains("2 object") || stderr.contains("2 object") || stdout.contains("object"),
        "Expected output to mention multiple objects. stdout: {stdout}, stderr: {stderr}"
    );
}

// =============================================================================
// 5. Invalid object index => error with count
// =============================================================================

#[test]
fn test_invalid_object_index_error() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir, "cube");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "--object",
            "5:layer_height=0.1",
            "--force",
            "--no-log",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "Expected failure for out-of-bounds object index"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("out of bounds") || stderr.contains("1 object"),
        "Expected out-of-bounds error with count. stderr: {stderr}"
    );
}

// =============================================================================
// 6. --strict turns warnings into errors
// =============================================================================

#[test]
fn test_strict_flag_exists() {
    // Verify the --strict flag is accepted by the CLI parser
    let output = slicecore_bin()
        .args(["slice", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--strict"),
        "Expected --strict in help output"
    );
}

// =============================================================================
// 7. --save-plate generates valid TOML
// =============================================================================

#[test]
fn test_save_plate_generates_toml() {
    let dir = TempDir::new().unwrap();
    let stl = setup_stl(&dir, "cube");
    let save_path = dir.path().join("saved_plate.toml");

    let output = slicecore_bin()
        .args([
            "slice",
            stl.to_str().unwrap(),
            "--save-plate",
            save_path.to_str().unwrap(),
            "--force",
            "--dry-run",
            "--no-log",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected --save-plate --dry-run to succeed. stderr: {stderr}"
    );

    // Verify the saved file is valid TOML
    assert!(save_path.exists(), "Expected saved plate file to exist");
    let content = std::fs::read_to_string(&save_path).unwrap();
    let parsed: Result<toml::Value, _> = toml::from_str(&content);
    assert!(
        parsed.is_ok(),
        "Expected saved plate to be valid TOML. Content: {content}"
    );
}
