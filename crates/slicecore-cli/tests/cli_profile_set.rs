//! Integration tests for `slicecore profile set` subcommands.
//!
//! Tests verify the profile set management CLI commands and the renamed
//! `profile setting` command, as well as the `--profile-set` flag on slice.

use std::process::Command;

fn slicecore_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slicecore"))
}

#[test]
fn test_set_help_shows_subcommands() {
    let output = slicecore_bin()
        .args(["profile", "set", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Expected success, stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("create"), "Expected 'create' in help output: {stdout}");
    assert!(stdout.contains("delete"), "Expected 'delete' in help output: {stdout}");
    assert!(stdout.contains("list"), "Expected 'list' in help output: {stdout}");
    assert!(stdout.contains("show"), "Expected 'show' in help output: {stdout}");
    assert!(stdout.contains("default"), "Expected 'default' in help output: {stdout}");
}

#[test]
fn test_set_create_requires_all_args() {
    let output = slicecore_bin()
        .args(["profile", "set", "create", "test-set"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "Expected failure when missing --machine, --filament, --process"
    );
}

#[test]
fn test_set_list_empty() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = slicecore_bin()
        .args([
            "profile",
            "set",
            "list",
            "--profiles-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected success for empty list, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No saved profile sets"),
        "Expected 'No saved profile sets' in stderr: {stderr}"
    );
}

#[test]
fn test_set_show_nonexistent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = slicecore_bin()
        .args([
            "profile",
            "set",
            "show",
            "nonexistent-set",
            "--profiles-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "Expected failure for nonexistent set"
    );
}

#[test]
fn test_setting_command_exists() {
    let output = slicecore_bin()
        .args(["profile", "setting", "--help"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Expected 'profile setting' help to succeed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("setting") || stdout.contains("Set a single setting"),
        "Expected setting help text: {stdout}"
    );
}

#[test]
fn test_slice_profile_set_flag_exists() {
    let output = slicecore_bin()
        .args(["slice", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success(), "Expected slice --help to succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--profile-set"),
        "Expected --profile-set in slice help: {stdout}"
    );
}

#[test]
fn test_set_create_and_list_roundtrip() {
    let tmp = tempfile::TempDir::new().unwrap();

    // Create a profile set
    let output = slicecore_bin()
        .args([
            "profile",
            "set",
            "create",
            "test-combo",
            "--machine",
            "BBL/X1C",
            "--filament",
            "PLA_Basic",
            "--process",
            "0.20mm_Standard",
            "--profiles-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected create to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Created profile set 'test-combo'"),
        "Expected creation message: {stdout}"
    );

    // List should now show the set
    let output = slicecore_bin()
        .args([
            "profile",
            "set",
            "list",
            "--profiles-dir",
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("test-combo"),
        "Expected set name in list: {stdout}"
    );
    assert!(
        stdout.contains("BBL/X1C"),
        "Expected machine in list: {stdout}"
    );
}
