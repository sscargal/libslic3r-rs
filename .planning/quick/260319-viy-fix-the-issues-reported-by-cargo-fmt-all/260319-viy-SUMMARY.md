# Quick Task 260319-viy: Fix cargo fmt issues

**Date:** 2026-03-19
**Commit:** 3f25cd2

## What was done
Ran `cargo fmt --all` to fix formatting issues in 9 files under `crates/slicecore-cli/src/`. These were introduced during phase 40 (CLI progress migration) where function signatures with `&CliOutput` parameters exceeded line length limits.

## Files modified
- `crates/slicecore-cli/src/calibrate/first_layer.rs`
- `crates/slicecore-cli/src/calibrate/flow.rs`
- `crates/slicecore-cli/src/calibrate/mod.rs`
- `crates/slicecore-cli/src/calibrate/retraction.rs`
- `crates/slicecore-cli/src/calibrate/temp_tower.rs`
- `crates/slicecore-cli/src/cli_output.rs`
- `crates/slicecore-cli/src/csg_command.rs`
- `crates/slicecore-cli/src/main.rs`
- `crates/slicecore-cli/src/slice_workflow.rs`
