//! Integration tests for profile enable/disable/status CLI commands.
//!
//! Tests exercise the `slicecore profile enable`, `slicecore profile disable`,
//! and `slicecore profile status` commands via the CLI binary.

use std::process::Command;
use tempfile::TempDir;

fn slicecore_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slicecore"))
}

#[test]
fn test_profile_status_no_config() {
    let tmp = TempDir::new().unwrap();
    let output = slicecore_bin()
        .args([
            "profile",
            "status",
            "--profiles-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No profiles enabled"),
        "Expected 'No profiles enabled' hint, got: {stderr}"
    );
    assert!(output.status.success());
}

#[test]
fn test_profile_status_json_with_fixture() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("enabled-profiles.toml");
    let content = r#"[machine]
enabled = ["test/Machine_A", "test/Machine_B"]

[filament]
enabled = ["test/PLA_Basic", "test/PETG_Basic", "test/ABS_Basic"]

[process]
enabled = []
"#;
    std::fs::write(&config_path, content).unwrap();

    let output = slicecore_bin()
        .args([
            "profile",
            "status",
            "--json",
            "--profiles-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "status --json failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("Failed to parse JSON output: {e}\nOutput: {stdout}");
    });

    assert_eq!(parsed["machine_count"], 2);
    assert_eq!(parsed["filament_count"], 3);
    assert_eq!(parsed["process_count"], 0);
    assert_eq!(parsed["machine"].as_array().unwrap().len(), 2);
    assert_eq!(parsed["filament"].as_array().unwrap().len(), 3);
}

#[test]
fn test_profile_enable_no_args_errors() {
    let tmp = TempDir::new().unwrap();
    let output = slicecore_bin()
        .args([
            "profile",
            "enable",
            "--profiles-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Interactive picker not yet implemented"),
        "Expected interactive picker message, got: {stderr}"
    );
    assert!(!output.status.success());
}

#[test]
fn test_profile_disable_no_config() {
    let tmp = TempDir::new().unwrap();
    let output = slicecore_bin()
        .args([
            "profile",
            "disable",
            "some_id",
            "--profiles-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No profiles are enabled"),
        "Expected 'No profiles are enabled' message, got: {stderr}"
    );
    assert!(output.status.success());
}

#[test]
fn test_profile_status_plain_text() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("enabled-profiles.toml");
    let content = r#"[machine]
enabled = ["test/X1C"]

[filament]
enabled = ["test/PLA"]

[process]
enabled = ["test/Standard", "test/Fast"]
"#;
    std::fs::write(&config_path, content).unwrap();

    let output = slicecore_bin()
        .args([
            "profile",
            "status",
            "--profiles-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Machines:  1 enabled"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Filaments: 1 enabled"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Process:   2 enabled"), "stdout: {stdout}");
}
