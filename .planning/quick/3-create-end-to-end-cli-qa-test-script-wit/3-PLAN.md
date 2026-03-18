---
phase: quick
plan: 3
type: execute
wave: 1
depends_on: []
files_modified:
  - scripts/qa_tests
autonomous: true
requirements: [QA-E2E]

must_haves:
  truths:
    - "Script runs full build gate (build, fmt, clippy, test)"
    - "Script tests every CLI subcommand with valid inputs"
    - "Script tests error cases (missing files, invalid args)"
    - "Script prints summary with PASS/FAIL/WARN/INFO counts"
    - "Script supports --group filtering and --skip flags"
    - "Script generates runtime fixtures via csg primitive"
    - "Script produces coverage gap report"
  artifacts:
    - path: "scripts/qa_tests"
      provides: "Executable Bash QA test script"
      min_lines: 800
  key_links:
    - from: "scripts/qa_tests"
      to: "target/debug/slicecore"
      via: "cargo build then CLI invocations"
      pattern: "slicecore.*"
---

<objective>
Create a comprehensive end-to-end CLI QA test script in Bash that exercises every `slicecore` subcommand, runs build gates, and reports coverage gaps.

Purpose: Provide a single-command QA pass that validates the entire CLI surface area, catches regressions, and identifies API modules without CLI exposure.
Output: `scripts/qa_tests` -- executable, self-documenting Bash script.
</objective>

<execution_context>
@./.claude/get-shit-done/workflows/execute-plan.md
@./.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@./CLAUDE.md
@./.claude/skills/rust-senior-dev/SKILL.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: Create the qa_tests script with full subcommand coverage</name>
  <files>scripts/qa_tests</files>
  <action>
Create `scripts/` directory and `scripts/qa_tests` as an executable Bash script (chmod +x, shebang `#!/usr/bin/env bash`).

**Script structure and conventions:**
- `set -euo pipefail` at top, but wrap individual tests in functions that capture exit codes (don't let failures abort the script)
- Use color output (with --no-color flag support)
- Track PASS/FAIL/WARN/INFO counters globally
- Each test is a single function that calls `pass "desc"`, `fail "desc" "details"`, `warn "desc"`, or `info "desc"`
- Temp directory created at start (`mktemp -d`), cleaned up on EXIT trap

**Flag parsing (getopts or manual):**
- `--group GROUP` -- run only named group(s), comma-separated. Groups: `build`, `mesh`, `slice`, `gcode`, `csg`, `convert`, `profile`, `arrange`, `thumbnail`, `ai`, `plugin`, `postprocess`, `errors`, `coverage`
- `--skip GROUP` -- skip named group(s)
- `--list-groups` -- print available groups and exit
- `--no-color` -- disable ANSI colors
- `--verbose` -- show stdout/stderr from commands (default: suppress on pass, show on fail)
- `--help` -- usage

**Fixture generation (setup phase):**
Generate all test fixtures at runtime in $TMPDIR using CLI itself:
- `slicecore csg primitive cube --dims 20 20 20 -o $TMPDIR/cube.stl`
- `slicecore csg primitive sphere --dims 10 -o $TMPDIR/sphere.stl`
- `slicecore csg primitive cylinder --dims 5 20 -o $TMPDIR/cylinder.stl`
- Convert cube to 3MF: `slicecore convert $TMPDIR/cube.stl $TMPDIR/cube.3mf`
- Convert cube to OBJ: `slicecore convert $TMPDIR/cube.stl $TMPDIR/cube.obj`
- Slice cube to get gcode: `slicecore slice $TMPDIR/cube.stl -o $TMPDIR/cube.gcode`
- Slice sphere for comparison: `slicecore slice $TMPDIR/sphere.stl -o $TMPDIR/sphere.gcode`
Each fixture generation itself is a test (PASS/FAIL).

**Test groups (implement ALL of these):**

GROUP `build` -- Build gates:
1. `cargo build --workspace` succeeds
2. `cargo fmt --all -- --check` succeeds
3. `cargo clippy --all-features -- -D warnings` succeeds
4. `cargo test --all-features --workspace` succeeds

GROUP `mesh` -- Mesh analysis:
1. `slicecore analyze $TMPDIR/cube.stl` succeeds and output contains "vertices" or "triangles"
2. `slicecore analyze $TMPDIR/cube.3mf` succeeds
3. `slicecore analyze $TMPDIR/cube.obj` succeeds

GROUP `slice` -- Slicing:
1. `slicecore slice $TMPDIR/cube.stl -o $TMPDIR/test_slice.gcode` succeeds and output file exists
2. `slicecore slice $TMPDIR/cube.stl --json` succeeds and output is valid JSON (pipe through python3 -m json.tool or jq)
3. `slicecore slice $TMPDIR/cube.3mf -o $TMPDIR/test_3mf.gcode` succeeds
4. `slicecore slice $TMPDIR/cube.stl --stats-format csv --quiet -o $TMPDIR/csv_test.gcode` succeeds
5. `slicecore slice $TMPDIR/cube.stl --stats-format json --quiet -o $TMPDIR/json_test.gcode` succeeds
6. `slicecore slice $TMPDIR/cube.stl --thumbnails -o $TMPDIR/thumb_slice.gcode` succeeds

GROUP `gcode` -- G-code operations:
1. `slicecore validate $TMPDIR/cube.gcode` succeeds
2. `slicecore analyze-gcode $TMPDIR/cube.gcode` succeeds
3. `slicecore analyze-gcode $TMPDIR/cube.gcode --json` succeeds and output is valid JSON
4. `slicecore analyze-gcode $TMPDIR/cube.gcode --csv` succeeds
5. `slicecore analyze-gcode $TMPDIR/cube.gcode --summary` succeeds
6. `slicecore analyze-gcode $TMPDIR/cube.gcode --no-color` succeeds
7. `slicecore compare-gcode $TMPDIR/cube.gcode $TMPDIR/sphere.gcode` succeeds
8. `slicecore compare-gcode $TMPDIR/cube.gcode $TMPDIR/sphere.gcode --json` succeeds and output is valid JSON
9. `slicecore compare-gcode $TMPDIR/cube.gcode $TMPDIR/sphere.gcode --csv` succeeds

GROUP `csg` -- CSG operations:
1. `slicecore csg union $TMPDIR/cube.stl $TMPDIR/sphere.stl -o $TMPDIR/union.stl` succeeds
2. `slicecore csg difference $TMPDIR/cube.stl $TMPDIR/sphere.stl -o $TMPDIR/diff.stl` succeeds
3. `slicecore csg intersection $TMPDIR/cube.stl $TMPDIR/sphere.stl -o $TMPDIR/inter.stl` succeeds
4. `slicecore csg xor $TMPDIR/cube.stl $TMPDIR/sphere.stl -o $TMPDIR/xor.stl` succeeds
5. `slicecore csg split $TMPDIR/cube.stl --plane 0,0,1,10 -o $TMPDIR/above.stl $TMPDIR/below.stl` succeeds
6. `slicecore csg hollow $TMPDIR/cube.stl --wall 2.0 -o $TMPDIR/hollow.stl` succeeds
7. `slicecore csg hollow $TMPDIR/cube.stl --wall 2.0 --drain-diameter 5.0 -o $TMPDIR/hollow_drain.stl` succeeds
8. `slicecore csg info $TMPDIR/cube.stl` succeeds
9. `slicecore csg info $TMPDIR/cube.stl --json` succeeds and output is valid JSON
10. All CSG boolean ops with `--json` flag succeed
11. All CSG boolean ops with `--verbose` flag succeed
12. Primitive generation: cube, sphere, cylinder, cone, torus, wedge, ngon-prism (each with appropriate --dims)

GROUP `convert` -- File conversion:
1. STL -> 3MF: `slicecore convert $TMPDIR/cube.stl $TMPDIR/conv.3mf` succeeds
2. STL -> OBJ: `slicecore convert $TMPDIR/cube.stl $TMPDIR/conv.obj` succeeds
3. 3MF -> STL: `slicecore convert $TMPDIR/cube.3mf $TMPDIR/conv2.stl` succeeds
4. OBJ -> STL: `slicecore convert $TMPDIR/cube.obj $TMPDIR/conv3.stl` succeeds
5. Output files are non-empty

GROUP `thumbnail` -- Thumbnail generation:
1. `slicecore thumbnail $TMPDIR/cube.stl -o $TMPDIR/thumb.png` succeeds and output file exists
2. `slicecore thumbnail $TMPDIR/cube.stl --angles front,isometric -o $TMPDIR/thumb_dir/` succeeds
3. `slicecore thumbnail $TMPDIR/cube.stl --resolution 640x480 -o $TMPDIR/thumb_hires.png` succeeds
4. `slicecore thumbnail $TMPDIR/cube.stl --background FF0000 --color 00FF00 -o $TMPDIR/thumb_color.png` succeeds

GROUP `arrange` -- Build plate arrangement:
1. `slicecore arrange $TMPDIR/cube.stl $TMPDIR/sphere.stl` succeeds (JSON output)
2. `slicecore arrange $TMPDIR/cube.stl $TMPDIR/sphere.stl --format json` succeeds

GROUP `profile` -- Profile commands (WARN-level since profiles dir may not exist):
1. `slicecore list-profiles --vendors` -- PASS if succeeds, WARN if no profiles dir
2. `slicecore list-profiles` -- PASS if succeeds, WARN if no profiles dir
3. `slicecore search-profiles "PLA"` -- PASS if succeeds, WARN if no profiles dir

GROUP `postprocess` -- Post-processing:
1. `slicecore post-process $TMPDIR/cube.gcode -o $TMPDIR/pp.gcode` succeeds (may need flags -- check what's minimally required)
2. `slicecore post-process $TMPDIR/cube.gcode --timelapse -o $TMPDIR/pp_tl.gcode` succeeds

GROUP `ai` -- AI commands (WARN-level since provider may not be available):
1. `slicecore ai-suggest $TMPDIR/cube.stl` -- WARN if fails (no provider), PASS if succeeds

GROUP `plugin` -- Plugin commands (INFO-level):
1. INFO: "Plugin testing requires compiled WASM plugins -- skipped in automated QA"

GROUP `errors` -- Error handling / edge cases:
1. `slicecore slice nonexistent.stl` fails with non-zero exit
2. `slicecore validate nonexistent.gcode` fails with non-zero exit
3. `slicecore convert nonexistent.stl out.3mf` fails with non-zero exit
4. `slicecore csg union` with no args fails with non-zero exit
5. `slicecore analyze nonexistent.stl` fails with non-zero exit
6. `slicecore csg primitive cube` with no --dims and no -o fails gracefully (non-zero exit, no panic/segfault)
7. `slicecore csg split $TMPDIR/cube.stl --plane invalid` fails gracefully
8. `slicecore` with no subcommand prints help (exit 0 or 2, but no crash)

GROUP `coverage` -- Coverage gap report:
1. List all crate names under `crates/` directory
2. List all CLI subcommands (parse `slicecore --help` output)
3. Map crates to subcommands:
   - slicecore-engine -> slice, analyze
   - slicecore-fileio -> convert, analyze
   - slicecore-gcode-io -> validate, analyze-gcode, compare-gcode
   - slicecore-mesh -> csg
   - slicecore-ai -> ai-suggest
   - slicecore-plugin -> (plugin system, no direct subcommand)
   - slicecore-render -> thumbnail
   - slicecore-slicer -> slice (internal)
   - slicecore-cli -> (the CLI itself)
   - slicecore-arrange -> arrange
   - slicecore-plugin-api -> (plugin API, no direct subcommand)
   - slicecore-geo -> (geometry primitives, no direct subcommand)
   - slicecore-math -> (math utilities, no direct subcommand)
4. Print table: Crate | CLI Exposure | Status
5. Identify any crate with no CLI subcommand exposure and report as INFO
6. Count public modules in each crate (grep for `pub mod` in lib.rs) vs CLI flags that exercise them

**Summary report:**
At end, print a formatted summary:
```
=== QA Test Summary ===
PASS: NN
FAIL: NN
WARN: NN
INFO: NN
Total: NN
```
Exit with code 0 if no FAILs, code 1 if any FAILs.

**Helper functions to implement:**
- `run_test "description" command args...` -- runs command, captures output, records pass/fail
- `run_test_expect_fail "description" command args...` -- expects non-zero exit
- `run_test_json "description" command args...` -- runs command, validates JSON output
- `pass "description"` / `fail "description" "details"` / `warn "description"` / `info "description"`
- `is_json_valid` -- pipes through `python3 -m json.tool` or `jq .` (whichever available)
- `group_header "GROUP_NAME"` -- prints section header
- `should_run_group "name"` -- checks --group/--skip filters

**Important implementation notes:**
- Use `SLICECORE` variable set to `cargo run --release --bin slicecore --` by default, but allow override via `SLICECORE_BIN` env var for pre-built binaries
- Actually, for speed, build first in the build group, then use `target/debug/slicecore` (or `target/release/slicecore` if release) directly for all subsequent tests. Set `SLICECORE="./target/debug/slicecore"` after build succeeds.
- If build group is skipped, check if binary exists, error if not
- All temp files go in the mktemp dir
- Trap EXIT to clean up temp dir
- The script header should contain a usage comment block explaining all groups and flags
  </action>
  <verify>
    <automated>bash scripts/qa_tests --list-groups && bash scripts/qa_tests --group csg,mesh,errors --verbose 2>&1 | tail -20</automated>
  </verify>
  <done>
- `scripts/qa_tests` exists, is executable, and runs without syntax errors
- `--list-groups` prints all available test groups
- `--group` flag filters to specific groups
- `--skip` flag excludes groups
- Running with `--group csg` generates fixtures and runs CSG tests
- Running with `--group errors` tests error handling cases
- Summary report prints at end with PASS/FAIL/WARN/INFO counts
- Exit code is 0 when all tests pass, 1 when any FAIL
- Coverage gap report identifies crates without CLI subcommand exposure
  </done>
</task>

</tasks>

<verification>
1. `bash -n scripts/qa_tests` -- syntax check passes
2. `bash scripts/qa_tests --list-groups` -- lists all groups
3. `bash scripts/qa_tests --group errors` -- error tests produce expected FAILs-that-are-PASSes (expected failures)
4. `bash scripts/qa_tests --group csg,mesh,convert,gcode,thumbnail` -- core functionality tests pass
5. `bash scripts/qa_tests --group coverage` -- coverage gap report prints
6. Full run: `bash scripts/qa_tests` -- completes with summary report
</verification>

<success_criteria>
- Single executable script at `scripts/qa_tests` covering all 18+ CLI subcommands
- Build gate (build, fmt, clippy, test) as first group
- Runtime fixture generation (no committed binaries)
- Group-based filtering with --group and --skip flags
- Error case testing (missing files, invalid args)
- Coverage gap report comparing crates to CLI exposure
- Summary report with PASS/FAIL/WARN/INFO counts and appropriate exit code
</success_criteria>

<output>
After completion, create `.planning/quick/3-create-end-to-end-cli-qa-test-script-wit/3-SUMMARY.md`
</output>
