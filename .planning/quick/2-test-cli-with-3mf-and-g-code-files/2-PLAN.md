---
phase: quick
plan: 2
type: execute
wave: 1
depends_on: []
files_modified: []
autonomous: true
requirements: [QUICK-02]

must_haves:
  truths:
    - "CLI can open and parse all three 3MF files into triangle meshes"
    - "CLI can slice at least one 3MF file to G-code successfully"
    - "CLI can analyze all three G-code files with structured metrics"
    - "CLI validate subcommand accepts all three G-code files"
  artifacts: []
  key_links:
    - from: "slicecore CLI"
      to: "lib3mf-core"
      via: "load_mesh -> detect_format -> ThreeMf -> threemf::parse"
      pattern: "load_mesh"
---

<objective>
Test the slicecore CLI against real 3MF model files and Bambu-generated G-code files to validate end-to-end functionality.

Purpose: Verify that the 3MF parsing (via lib3mf-core v0.3.0), mesh loading, slicing pipeline, G-code validation, and G-code analysis all work with real-world files. This is a read-only validation task -- NO code changes.

Output: A SUMMARY documenting what works, what fails, and any issues discovered.
</objective>

<execution_context>
@./.claude/get-shit-done/workflows/execute-plan.md
@./.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md

Test files available:
- tmp/models/Cube_PLA.3mf (simple cube model)
- tmp/models/3DBenchy_PLA.3mf (complex benchmark model)
- tmp/models/SimplePyramid.3mf (simple pyramid model)
- tmp/gcode-bambu/Cube_PLA.gcode (Bambu-generated cube G-code)
- tmp/gcode-bambu/3DBenchy_PLA.gcode (Bambu-generated benchy G-code)
- tmp/gcode-bambu/SimplePyramid.gcode (Bambu-generated pyramid G-code)

CLI subcommands to test:
- `slicecore analyze <INPUT>` -- analyzes a mesh file (accepts any format via load_mesh auto-detect)
- `slicecore slice <INPUT> -o <OUTPUT>` -- slices a mesh to G-code (accepts any format via load_mesh)
- `slicecore validate <INPUT>` -- validates a G-code file
- `slicecore analyze-gcode <INPUT> --summary` -- analyzes G-code with structured metrics
</context>

<tasks>

<task type="auto">
  <name>Task 1: Test 3MF file loading and slicing</name>
  <files></files>
  <action>
Run the following CLI commands and capture all output (stdout and stderr). No code changes -- just run and observe.

Step 1 - Analyze each 3MF file to verify mesh loading works:
```
cargo run -- analyze tmp/models/Cube_PLA.3mf
cargo run -- analyze tmp/models/3DBenchy_PLA.3mf
cargo run -- analyze tmp/models/SimplePyramid.3mf
```

Step 2 - Slice each 3MF file to G-code to verify full pipeline:
```
cargo run -- slice tmp/models/Cube_PLA.3mf -o /tmp/test-cube.gcode
cargo run -- slice tmp/models/3DBenchy_PLA.3mf -o /tmp/test-benchy.gcode
cargo run -- slice tmp/models/SimplePyramid.3mf -o /tmp/test-pyramid.gcode
```

For each command, record:
- Exit code (0 = success, non-zero = failure)
- Key output (mesh stats, layer count, time estimate)
- Any errors or warnings

If a command fails, capture the full error message. Do NOT attempt to fix code -- just document what happened.
  </action>
  <verify>
All 6 commands run to completion. At minimum, the `analyze` commands should succeed (proving 3MF parsing works). Slice commands producing G-code output files is ideal but failures are acceptable findings.
  </verify>
  <done>All 3 analyze commands and all 3 slice commands have been run. Results documented with exit codes and output.</done>
</task>

<task type="auto">
  <name>Task 2: Test G-code validation and analysis on Bambu-generated files</name>
  <files></files>
  <action>
Run the following CLI commands against the Bambu-generated G-code files and capture all output. No code changes.

Step 1 - Validate each G-code file:
```
cargo run -- validate tmp/gcode-bambu/Cube_PLA.gcode
cargo run -- validate tmp/gcode-bambu/3DBenchy_PLA.gcode
cargo run -- validate tmp/gcode-bambu/SimplePyramid.gcode
```

Step 2 - Analyze each G-code file with summary metrics:
```
cargo run -- analyze-gcode tmp/gcode-bambu/Cube_PLA.gcode --summary --no-color
cargo run -- analyze-gcode tmp/gcode-bambu/3DBenchy_PLA.gcode --summary --no-color
cargo run -- analyze-gcode tmp/gcode-bambu/SimplePyramid.gcode --summary --no-color
```

Step 3 - Compare the Bambu G-code files against each other (optional, if compare-gcode works):
```
cargo run -- compare-gcode tmp/gcode-bambu/Cube_PLA.gcode tmp/gcode-bambu/3DBenchy_PLA.gcode --no-color
```

For each command, record:
- Exit code
- Key metrics reported (estimated time, filament usage, layer count)
- Any validation warnings or errors
- Whether Bambu-specific G-code commands are recognized or flagged

Do NOT attempt to fix code -- just document findings.
  </action>
  <verify>
All validate and analyze-gcode commands run to completion. Results show meaningful metrics extracted from the G-code files. Any unrecognized commands or validation failures are documented as findings.
  </verify>
  <done>All 6 validate/analyze-gcode commands have been run plus the comparison. Results documented with metrics and any issues found.</done>
</task>

</tasks>

<verification>
- All 3MF files can be opened and parsed (analyze succeeds)
- Slicing pipeline works end-to-end on at least one 3MF file
- G-code validation and analysis produce meaningful output
- All findings documented in SUMMARY
</verification>

<success_criteria>
- Every CLI command has been executed and its output captured
- A clear report of what works vs what fails across all 6 model/gcode files
- No code changes made -- this is purely observational testing
</success_criteria>

<output>
After completion, create `.planning/quick/2-test-cli-with-3mf-and-g-code-files/2-SUMMARY.md` with:
- Table of all commands run, their exit codes, and key output
- Issues discovered (if any)
- Recommendations for follow-up work (if any)
</output>
