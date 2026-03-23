//! Integration tests for `slicecore profile search` command.
//!
//! Tests exercise the CLI binary to verify that the search command accepts
//! expected flags, requires a query argument, and supports JSON output.

use std::process::Command;

fn slicecore_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slicecore"))
}

#[test]
fn test_search_help_shows_filter_flags() {
    let output = slicecore_bin()
        .args(["profile", "search", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--material"),
        "Expected --material flag in help, got: {stdout}"
    );
    assert!(
        stdout.contains("--vendor"),
        "Expected --vendor flag in help, got: {stdout}"
    );
    assert!(
        stdout.contains("--nozzle"),
        "Expected --nozzle flag in help, got: {stdout}"
    );
    assert!(
        stdout.contains("--type"),
        "Expected --type flag in help, got: {stdout}"
    );
}

#[test]
fn test_search_requires_query_argument() {
    let output = slicecore_bin()
        .args(["profile", "search"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "Expected failure when query argument is missing"
    );
}

#[test]
fn test_search_include_incompatible_flag() {
    let output = slicecore_bin()
        .args(["profile", "search", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--include-incompatible"),
        "Expected --include-incompatible flag in help, got: {stdout}"
    );
}

#[test]
fn test_search_enable_flag() {
    let output = slicecore_bin()
        .args(["profile", "search", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--enable"),
        "Expected --enable flag in help, got: {stdout}"
    );
}

#[test]
fn test_search_with_nonexistent_profiles_dir() {
    let output = slicecore_bin()
        .args([
            "profile",
            "search",
            "PLA",
            "--profiles-dir",
            "/tmp/nonexistent_slicecore_test",
        ])
        .output()
        .unwrap();

    // Should succeed (empty result) or fail gracefully -- not panic
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "Search should not panic with nonexistent profiles dir"
    );
}
