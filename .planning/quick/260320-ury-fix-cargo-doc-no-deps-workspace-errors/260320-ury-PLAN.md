---
phase: quick
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/slicecore-cli/src/profile_command.rs
autonomous: true
requirements: []
must_haves:
  truths:
    - "cargo doc --no-deps --workspace produces zero warnings"
    - "RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features exits 0"
  artifacts:
    - path: "crates/slicecore-cli/src/profile_command.rs"
      provides: "Fixed doc comment with escaped brackets"
  key_links: []
---

<objective>
Fix all doc warnings/errors emitted by `cargo doc --no-deps --workspace`.

Purpose: Clean doc build is a toolchain gate per project skill (rust-senior-dev).
Output: Zero-warning doc build across entire workspace.
</objective>

<execution_context>
@/home/steve/libslic3r-rs/.claude/get-shit-done/workflows/execute-plan.md
@/home/steve/libslic3r-rs/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.claude/skills/rust-senior-dev/SKILL.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: Capture and fix all cargo doc warnings</name>
  <files>crates/slicecore-cli/src/profile_command.rs</files>
  <action>
1. Run `cargo doc --no-deps --workspace 2>&1` to capture all warnings.

2. Currently known: one warning in `crates/slicecore-cli/src/profile_command.rs:34` — unresolved link to `metadata`. The doc comment reads `[metadata] section recording the clone lineage.` but `metadata` is not an item in scope.

3. Fix by escaping the brackets: change `[metadata]` to `\[metadata\]` so rustdoc treats it as literal text rather than an intra-doc link. Alternatively, if `metadata` refers to a real type/field that should be linked, use the correct path (e.g., `[MetadataStruct]`). Given the context ("section recording the clone lineage"), this is prose referring to a TOML/config section name, so escaping is correct.

4. After fixing, run `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` to confirm zero warnings with warnings-as-errors (the project toolchain gate from rust-senior-dev skill).
  </action>
  <verify>
    <automated>RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features 2>&1; echo "EXIT: $?"</automated>
  </verify>
  <done>cargo doc --no-deps --workspace and RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features both complete with zero warnings/errors (exit code 0).</done>
</task>

</tasks>

<verification>
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features exits 0 with no warnings.
</verification>

<success_criteria>
- Zero warnings from `cargo doc --no-deps --workspace`
- Zero errors from `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
</success_criteria>

<output>
After completion, create `.planning/quick/260320-ury-fix-cargo-doc-no-deps-workspace-errors/260320-ury-SUMMARY.md`
</output>
