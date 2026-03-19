---
phase: quick
plan: 260319-qfm
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/slicecore-cli/src/main.rs
autonomous: true
requirements: []
must_haves:
  truths:
    - "cargo clippy --workspace -- -D warnings passes with zero errors"
  artifacts:
    - path: "crates/slicecore-cli/src/main.rs"
      provides: "Clippy-clean CLI binary"
  key_links: []
---

<objective>
Fix all clippy warnings in the workspace so that `cargo clippy --workspace -- -D warnings` passes cleanly.

Purpose: Enforce idiomatic Rust and maintain CI-ready code quality.
Output: Zero clippy errors across the entire workspace.
</objective>

<execution_context>
@/home/steve/libslic3r-rs/.claude/get-shit-done/workflows/execute-plan.md
@/home/steve/libslic3r-rs/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
All 3 errors are in `crates/slicecore-cli/src/main.rs`:

1. **Line 1223** — `unnecessary_map_or`: `output_path.map_or(false, |p| {` should use `.is_some_and(|p| {`
2. **Line 1226** — `unnecessary_map_or`: `.map_or(false, |e| e.eq_ignore_ascii_case("3mf"))` should use `.is_some_and(|e| ...)`
3. **Line 2998** — `manual_range_contains`: `q < 1 || q > 100` should use `!(1..=100).contains(&q)`
</context>

<tasks>

<task type="auto">
  <name>Task 1: Fix all clippy warnings in slicecore-cli</name>
  <files>crates/slicecore-cli/src/main.rs</files>
  <action>
Apply three fixes in `crates/slicecore-cli/src/main.rs`:

1. Line 1223: Replace `output_path.map_or(false, |p| {` with `output_path.is_some_and(|p| {`
2. Line 1226: Replace `.map_or(false, |e| e.eq_ignore_ascii_case("3mf"))` with `.is_some_and(|e| e.eq_ignore_ascii_case("3mf"))`
3. Line 2998: Replace `if q < 1 || q > 100 {` with `if !(1..=100).contains(&q) {`

These are mechanical replacements suggested by clippy. No behavior changes.
  </action>
  <verify>
    <automated>cd /home/steve/libslic3r-rs && cargo clippy --workspace -- -D warnings 2>&1 | tail -5</automated>
  </verify>
  <done>cargo clippy --workspace -- -D warnings exits 0 with no errors or warnings</done>
</task>

</tasks>

<verification>
cargo clippy --workspace -- -D warnings exits with code 0 and produces no error output.
</verification>

<success_criteria>
- Zero clippy warnings across all workspace crates
- No behavioral changes to existing code
- All existing tests still pass
</success_criteria>

<output>
After completion, create `.planning/quick/260319-qfm-run-cargo-clippy-workspace-d-warnings-an/260319-qfm-SUMMARY.md`
</output>
