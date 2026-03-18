---
phase: quick
plan: 260318-mtf
type: execute
wave: 1
depends_on: []
files_modified:
  - scripts/qa_tests
autonomous: true
requirements: []
must_haves:
  truths:
    - "All CLI subcommands from phases 25-35 are exercised by the QA test script"
    - "calibrate, schema, convert-profile, show-profile subcommands have dedicated test groups"
    - "Coverage report CRATE_MAP includes all current workspace crates"
    - "Running scripts/qa_tests --group calibrate,schema,profiles passes"
  artifacts:
    - path: "scripts/qa_tests"
      provides: "Complete QA test coverage for all CLI subcommands"
  key_links:
    - from: "scripts/qa_tests"
      to: "target/debug/slicecore"
      via: "CLI invocations"
      pattern: "SLICECORE.*calibrate|schema|convert-profile|show-profile"
---

<objective>
Add QA test coverage for CLI subcommands introduced in phases 25-35 that are currently untested.

Purpose: The qa_tests script was written before phases 25-35 shipped. It is missing test groups for:
1. `calibrate` (Phase 31) - temp-tower, retraction, flow, first-layer, list
2. `schema` (Phase 35) - JSON Schema output, flat JSON, tier/category/search filtering
3. `convert-profile` (Phase 30-34) - JSON profile to TOML conversion
4. `show-profile` (Phase 30) - profile detail display
5. Coverage report CRATE_MAP is stale (missing slicecore-config-schema, slicecore-config-derive crates)

Output: Updated scripts/qa_tests with 4 new test groups and updated coverage map
</objective>

<execution_context>
@/home/steve/libslic3r-rs/.claude/get-shit-done/workflows/execute-plan.md
@/home/steve/libslic3r-rs/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@scripts/qa_tests
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add calibrate, schema, convert-profile, and show-profile test groups</name>
  <files>scripts/qa_tests</files>
  <action>
Add 4 new test groups to scripts/qa_tests. Insert them after the existing groups (before `group_errors`) and add their names to `ALL_GROUPS`. Also add them to the `needs_fixtures` check if they need fixture files, and call them from `main()`.

**Group: calibrate** (insert after group_postprocess)
```
group_calibrate() {
    should_run_group "calibrate" || return 0
    group_header "Calibration Commands"

    run_test "calibrate list" \
        "$SLICECORE" calibrate list

    run_test "calibrate temp-tower" \
        "$SLICECORE" calibrate temp-tower -o "$TMPDIR_QA/temp_tower.gcode"

    run_test_file_exists "temp tower gcode exists" "$TMPDIR_QA/temp_tower.gcode"

    run_test "calibrate retraction" \
        "$SLICECORE" calibrate retraction -o "$TMPDIR_QA/retraction.gcode"

    run_test_file_exists "retraction gcode exists" "$TMPDIR_QA/retraction.gcode"

    run_test "calibrate flow" \
        "$SLICECORE" calibrate flow -o "$TMPDIR_QA/flow.gcode"

    run_test_file_exists "flow gcode exists" "$TMPDIR_QA/flow.gcode"

    run_test "calibrate first-layer" \
        "$SLICECORE" calibrate first-layer -o "$TMPDIR_QA/first_layer.gcode"

    run_test_file_exists "first layer gcode exists" "$TMPDIR_QA/first_layer.gcode"

    # Validate generated gcode is valid
    run_test "validate calibration gcode" \
        "$SLICECORE" validate "$TMPDIR_QA/temp_tower.gcode"
}
```

**Group: schema** (insert after group_calibrate)
```
group_schema() {
    should_run_group "schema" || return 0
    group_header "Schema Commands"

    run_test_json "schema default (json-schema)" \
        "$SLICECORE" schema

    run_test_json "schema --format json" \
        "$SLICECORE" schema --format json

    run_test_json "schema --tier simple" \
        "$SLICECORE" schema --tier simple

    run_test_json "schema --tier advanced" \
        "$SLICECORE" schema --tier advanced

    run_test_json "schema --category quality" \
        "$SLICECORE" schema --category quality

    run_test_json "schema --search layer_height" \
        "$SLICECORE" schema --format json --search layer_height

    run_test_json "schema combined filters" \
        "$SLICECORE" schema --format json --tier intermediate --category speed
}
```

**Expand group_profile** to include convert-profile and show-profile tests. Add these to the existing group_profile function:
```
    # convert-profile needs a JSON fixture -- create a minimal one
    cat > "$TMPDIR_QA/test_profile.json" << 'JSONEOF'
    {
      "layer_height": "0.2",
      "wall_loops": "3",
      "sparse_infill_density": "15%",
      "nozzle_temperature": ["220"],
      "hot_plate_temp": ["60"]
    }
JSONEOF

    run_test "convert-profile JSON to TOML" \
        "$SLICECORE" convert-profile "$TMPDIR_QA/test_profile.json" -o "$TMPDIR_QA/converted.toml"

    run_test_file_exists "converted TOML exists" "$TMPDIR_QA/converted.toml"

    run_test "convert-profile verbose" \
        "$SLICECORE" convert-profile "$TMPDIR_QA/test_profile.json" -v -o "$TMPDIR_QA/converted_v.toml"
```

Update `ALL_GROUPS` to include: `calibrate schema` (profiles group already covers convert-profile/show-profile).

Update `main()` to call `group_calibrate` and `group_schema` in the right order (after group_postprocess, before group_ai).

Add `calibrate` to the `needs_fixtures` loop (it does NOT need fixtures, so skip it).
  </action>
  <verify>
    <automated>bash -n scripts/qa_tests && echo "Syntax OK"</automated>
  </verify>
  <done>
    - ALL_GROUPS includes calibrate and schema
    - group_calibrate tests all 4 calibration subcommands plus list
    - group_schema tests json-schema, json, tier, category, and search filters
    - group_profile expanded with convert-profile tests
    - main() calls both new groups
    - Script passes bash -n syntax check
  </done>
</task>

<task type="auto">
  <name>Task 2: Update CRATE_MAP in coverage group and add error-case tests for new commands</name>
  <files>scripts/qa_tests</files>
  <action>
1. In the `group_coverage()` function, update the `CRATE_MAP` associative array to include the new crates added since phase 25:
   - `CRATE_MAP["slicecore-config-schema"]="schema"`
   - `CRATE_MAP["slicecore-config-derive"]="(proc-macro, no direct subcommand)"`
   Check if there are any other crates in the workspace not yet in the map by running `ls crates/` and comparing.

2. In the `group_errors()` function, add error-case tests for the new commands:
```
    run_test_expect_fail "calibrate temp-tower with invalid temp range" \
        "$SLICECORE" calibrate temp-tower --start-temp 300 --end-temp 100

    run_test_expect_fail "schema with invalid tier" \
        "$SLICECORE" schema --tier nonexistent

    run_test_expect_fail "schema with invalid category" \
        "$SLICECORE" schema --category nonexistent

    run_test_expect_fail "convert-profile nonexistent file" \
        "$SLICECORE" convert-profile nonexistent.json
```

3. Verify the CRATE_MAP matches the actual workspace crates by checking `ls crates/` output against the map keys. Add any missing crates.
  </action>
  <verify>
    <automated>bash -n scripts/qa_tests && grep -c 'config-schema\|config-derive' scripts/qa_tests</automated>
  </verify>
  <done>
    - CRATE_MAP has entries for all crates in crates/ directory
    - Error cases exist for calibrate, schema, and convert-profile
    - Script passes syntax check
  </done>
</task>

<task type="auto">
  <name>Task 3: Run the new test groups and fix any failures</name>
  <files>scripts/qa_tests</files>
  <action>
Run the new test groups individually to verify they pass:
1. `scripts/qa_tests --group build` (builds the binary)
2. `scripts/qa_tests --group calibrate`
3. `scripts/qa_tests --group schema`
4. `scripts/qa_tests --group profile` (includes new convert-profile tests)
5. `scripts/qa_tests --group errors` (includes new error cases)

Fix any test failures by adjusting command arguments or expected behaviors. Some calibrate subcommands may require specific flags -- check `--help` for each and adjust tests accordingly. If `convert-profile` or `show-profile` require a profiles directory, use warn() instead of fail() (matching the existing profile group pattern).

After individual groups pass, run the full suite: `scripts/qa_tests --skip build` (assuming binary is already built).
  </action>
  <verify>
    <automated>scripts/qa_tests --group calibrate,schema --verbose 2>&1 | tail -20</automated>
  </verify>
  <done>
    - All new test groups (calibrate, schema) pass with 0 failures
    - Expanded profile group passes
    - New error cases pass
    - Full suite runs with no regressions in existing groups
  </done>
</task>

</tasks>

<verification>
1. `bash -n scripts/qa_tests` passes (syntax valid)
2. `scripts/qa_tests --list-groups` shows calibrate and schema in the list
3. `scripts/qa_tests --group calibrate,schema,profile,errors` passes with 0 FAIL
4. Coverage group CRATE_MAP has no UNMAPPED entries for workspace crates
</verification>

<success_criteria>
- Every CLI subcommand listed in `slicecore --help` has at least one QA test exercising it
- calibrate group: tests list + all 4 calibration types + validates generated gcode
- schema group: tests json-schema output, json output, tier/category/search filtering
- profile group: tests convert-profile with JSON input
- error group: tests invalid inputs for calibrate, schema, convert-profile
- CRATE_MAP complete for all workspace crates
- Zero regressions in existing test groups
</success_criteria>

<output>
After completion, create `.planning/quick/260318-mtf-review-qa-tests-and-add-coverage-for-rec/260318-mtf-SUMMARY.md`
</output>
