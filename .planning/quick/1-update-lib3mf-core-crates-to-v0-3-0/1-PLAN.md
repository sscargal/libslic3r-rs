---
phase: quick-update-lib3mf-core
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/slicecore-fileio/Cargo.toml
  - Cargo.lock
autonomous: true
requirements: ["UPDATE-LIB3MF-CORE-V0.3"]

must_haves:
  truths:
    - "lib3mf-core dependency is pinned to v0.3 in all Cargo.toml files"
    - "Cargo.lock resolves lib3mf-core to exactly 0.3.x"
    - "All existing tests pass without modification"
    - "WASM build continues to compile"
  artifacts:
    - path: "crates/slicecore-fileio/Cargo.toml"
      provides: "lib3mf-core v0.3 dependency declaration"
      contains: 'lib3mf-core = { version = "0.3"'
    - path: "Cargo.lock"
      provides: "Resolved lib3mf-core 0.3.x"
      contains: 'name = "lib3mf-core"'
  key_links:
    - from: "crates/slicecore-fileio/Cargo.toml"
      to: "Cargo.lock"
      via: "cargo dependency resolution"
      pattern: 'version = "0\.3'
---

<objective>
Update lib3mf-core from v0.2 to v0.3.0 across the project.

Purpose: Keep the lib3mf-core ecosystem dependency current at the latest published version (0.3.0).
Output: Updated Cargo.toml and Cargo.lock with lib3mf-core 0.3.0, all tests passing.
</objective>

<execution_context>
@./.claude/get-shit-done/workflows/execute-plan.md
@./.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@crates/slicecore-fileio/Cargo.toml
@crates/slicecore-fileio/src/threemf.rs
@crates/slicecore-fileio/tests/wasm_3mf_test.rs

The only crate in this project that depends on lib3mf-core is `slicecore-fileio`.
Current version spec: `lib3mf-core = { version = "0.2", default-features = false }`
Current resolved version in Cargo.lock: 0.2.0

The project uses the following lib3mf-core APIs:
- `lib3mf_core::archive::{find_model_path, ArchiveReader, ZipArchiver}`
- `lib3mf_core::parser::parse_model`
- `lib3mf_core::model::{BuildItem, Geometry, Mesh, Model, Object, ObjectType, ResourceId}`
- `lib3mf_core::Model` (default, write, resources)

docs.rs confirms these modules and types are still present in v0.3.0.
</context>

<tasks>

<task type="auto">
  <name>Task 1: Update lib3mf-core version and verify</name>
  <files>crates/slicecore-fileio/Cargo.toml, Cargo.lock</files>
  <action>
1. In `crates/slicecore-fileio/Cargo.toml`, change line 16 from:
   `lib3mf-core = { version = "0.2", default-features = false }`
   to:
   `lib3mf-core = { version = "0.3", default-features = false }`

2. Run `cargo update -p lib3mf-core` to update Cargo.lock to resolve 0.3.x.

3. Run `cargo check --workspace` to verify compilation.

4. Run `cargo test -p slicecore-fileio` to verify all fileio tests pass (threemf unit tests + wasm_3mf_test integration tests).

5. Run `cargo test --workspace` to verify no regressions across the full project.

6. If any test fails due to API changes in v0.3.0, inspect the compiler errors and adapt the usage in `threemf.rs`, `lib.rs`, and `wasm_3mf_test.rs` accordingly. The core API (ZipArchiver, find_model_path, parse_model, Model, Mesh, Object, BuildItem, Geometry) is expected to remain stable, but field additions or signature changes should be handled.
  </action>
  <verify>
    `cargo test -p slicecore-fileio` passes (0 failures).
    `cargo test --workspace` passes (0 failures).
    `grep 'version = "0.3"' crates/slicecore-fileio/Cargo.toml` matches lib3mf-core line.
    `grep -A1 'name = "lib3mf-core"' Cargo.lock` shows version 0.3.x.
  </verify>
  <done>
    lib3mf-core dependency updated to v0.3 in Cargo.toml, Cargo.lock resolves to 0.3.x, all workspace tests pass with zero failures.
  </done>
</task>

</tasks>

<verification>
- `grep 'lib3mf-core' crates/slicecore-fileio/Cargo.toml` shows version "0.3"
- `grep -A1 'name = "lib3mf-core"' Cargo.lock` shows version "0.3.0" (or 0.3.x)
- `cargo test -p slicecore-fileio` -- all tests pass
- `cargo test --workspace` -- no regressions
</verification>

<success_criteria>
- lib3mf-core version spec is "0.3" in crates/slicecore-fileio/Cargo.toml
- Cargo.lock resolves lib3mf-core to 0.3.0+
- All slicecore-fileio tests pass (threemf parsing, round-trip, multi-object, error handling)
- Full workspace test suite passes with no regressions
</success_criteria>

<output>
After completion, create `.planning/quick/1-update-lib3mf-core-crates-to-v0-3-0/1-SUMMARY.md`
</output>
