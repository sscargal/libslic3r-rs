---
phase: 35-configschema-system-with-setting-metadata-and-json-schema-generation
plan: 02
subsystem: config
tags: [proc-macro, derive, syn, quote, setting-schema]

requires:
  - phase: 35-01
    provides: "Runtime types (SettingDefinition, HasSettingSchema trait, ValueType, Tier, etc.)"
provides:
  - "#[derive(SettingSchema)] proc-macro for structs and enums"
  - "Attribute parsing for all #[setting(...)] field/struct/variant attributes"
  - "Code generation producing HasSettingSchema trait implementations"
affects: [35-03, 35-04, 35-05, 35-06, 35-07]

tech-stack:
  added: [syn 2, quote 1, proc-macro2 1]
  patterns: [proc-macro derive with attribute parsing, fully-qualified path generation, type inference from syn::Type]

key-files:
  created:
    - crates/slicecore-config-derive/Cargo.toml
    - crates/slicecore-config-derive/src/lib.rs
    - crates/slicecore-config-derive/src/parse.rs
    - crates/slicecore-config-derive/src/codegen.rs
    - crates/slicecore-config-derive/tests/derive_test.rs
  modified: []

key-decisions:
  - "Used clippy::all instead of clippy::pedantic for proc-macro crate (pedantic is noisy in proc-macro code)"
  - "Enum setting_definitions returns single definition with ValueType::Enum containing all variants"
  - "Struct fields with unknown types default to ValueType::String"
  - "Generated code uses fully-qualified ::slicecore_config_schema:: paths to avoid import conflicts"

patterns-established:
  - "Attribute parsing pattern: parse_nested_meta with ident matching for #[setting(...)]"
  - "Type inference: match on last path segment ident string for f64/bool/String/Vec/Option"
  - "Compile-time validation: min < max check at macro expansion time"

requirements-completed: []

duration: 3min
completed: 2026-03-18
---

# Phase 35 Plan 02: Derive Macro Summary

**#[derive(SettingSchema)] proc-macro with syn-based attribute parsing, struct/enum codegen, and type inference for 14+ attribute types**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-18T00:14:10Z
- **Completed:** 2026-03-18T00:17:10Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Proc-macro crate with full #[setting(...)] attribute parsing (tier, description, display_name, category, units, min, max, affects, depends_on, tags, since_version, deprecated, skip, flatten, prefix)
- Code generation for struct and enum types producing HasSettingSchema implementations
- ValueType inference from Rust types: f64->Float, bool->Bool, String->String, Vec<f64>->FloatVec, integers->Int, Option<T> unwraps
- 9 integration tests proving derive macro works end-to-end

## Task Commits

1. **Task 1: Create proc-macro crate with attribute parsing** - `40c5262` (feat)
2. **Task 2: Implement code generation for structs and enums** - `cd1abf3` (feat)
3. **Task 3: Integration test proving derive macro works end-to-end** - `2fba060` (test)

## Files Created/Modified
- `crates/slicecore-config-derive/Cargo.toml` - Proc-macro crate definition with syn/quote deps
- `crates/slicecore-config-derive/src/lib.rs` - #[proc_macro_derive(SettingSchema)] entry point
- `crates/slicecore-config-derive/src/parse.rs` - SettingAttrs, StructAttrs, EnumVariantAttrs parsing
- `crates/slicecore-config-derive/src/codegen.rs` - HasSettingSchema impl generation for structs/enums
- `crates/slicecore-config-derive/tests/derive_test.rs` - 9 integration tests

## Decisions Made
- Used `clippy::all` instead of `clippy::pedantic` for the proc-macro crate since pedantic lints are noisy in macro code
- Enum `setting_definitions` returns a single definition with `ValueType::Enum` containing all variant metadata
- Unknown/custom types in struct fields default to `ValueType::String` (enums register their variants separately)
- All generated code uses fully-qualified `::slicecore_config_schema::` paths to avoid name conflicts

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed temporary borrow lifetime in flatten codegen**
- **Found during:** Task 2
- **Issue:** `field_name.to_string()` created a temporary that was dropped while still borrowed
- **Fix:** Extracted to a named binding before borrowing
- **Files modified:** crates/slicecore-config-derive/src/codegen.rs
- **Verification:** cargo check passes cleanly
- **Committed in:** cd1abf3

**2. [Rule 1 - Bug] Fixed parse_terminated function signature mismatch**
- **Found during:** Task 1
- **Issue:** `syn::LitStr::parse` is a method, not a function; `parse_terminated` expects a function
- **Fix:** Used closure `|input| input.parse::<syn::LitStr>()` instead
- **Files modified:** crates/slicecore-config-derive/src/parse.rs
- **Verification:** cargo check passes cleanly
- **Committed in:** 40c5262

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes were minor Rust borrow/API issues. No scope creep.

## Issues Encountered
None beyond the auto-fixed items above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Derive macro is ready for use by downstream crates (35-03 re-export, 35-04 JSON Schema)
- All 14+ attribute types are parsed and code-generated
- Integration tests confirm struct (basic, skip, flatten, unannotated) and enum derive works

---
*Phase: 35-configschema-system-with-setting-metadata-and-json-schema-generation*
*Completed: 2026-03-18*
