//! Integration tests for `slicecore profile compat` command.
//!
//! Tests exercise the CLI binary to verify that the compat command accepts
//! expected arguments, requires an ID, and handles missing profiles gracefully.

use std::process::Command;

fn slicecore_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slicecore"))
}

#[test]
fn test_compat_help_shows_id_and_json() {
    let output = slicecore_bin()
        .args(["profile", "compat", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--json"),
        "Expected --json flag in help, got: {stdout}"
    );
    assert!(
        stdout.contains("<ID>") || stdout.contains("id"),
        "Expected id argument in help, got: {stdout}"
    );
}

#[test]
fn test_compat_requires_id_argument() {
    let output = slicecore_bin()
        .args(["profile", "compat"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "Expected failure when ID argument is missing"
    );
}

#[test]
fn test_compat_nonexistent_profile() {
    let output = slicecore_bin()
        .args([
            "profile",
            "compat",
            "nonexistent-profile-id",
            "--profiles-dir",
            "/tmp/nonexistent_slicecore_test",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "Expected failure for nonexistent profile"
    );
}
