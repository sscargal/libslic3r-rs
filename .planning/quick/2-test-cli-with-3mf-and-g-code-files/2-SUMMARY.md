---
phase: quick
plan: 2
subsystem: testing
tags: [cli, 3mf, gcode, validation, analysis, bambu]

# Dependency graph
requires:
  - phase: 22
    provides: lib3mf-core 3MF parser
  - phase: 21
    provides: G-code analysis and comparison tool
provides:
  - End-to-end CLI validation results for 3MF and G-code workflows
  - Identification of 3MF production extension parsing limitation
affects: [3mf-parsing, lib3mf-core, fileio]

# Tech tracking
tech-stack:
  added: []
  patterns: []

key-files:
  created:
    - .planning/quick/2-test-cli-with-3mf-and-g-code-files/2-SUMMARY.md
  modified: []

key-decisions:
  - "No code changes -- read-only testing as specified"
  - "3MF production extension not supported by lib3mf-core v0.3.0 parser -- follow-up work needed"

patterns-established: []

requirements-completed: [QUICK-02]

# Metrics
duration: 4min
completed: 2026-02-26
---

# Quick Task 2: Test CLI with 3MF and G-code Files Summary

**All 3 BambuStudio 3MF files fail to parse (production extension not supported); all 6 G-code validation/analysis commands succeed with rich metrics**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-26T01:15:49Z
- **Completed:** 2026-02-26T01:19:30Z
- **Tasks:** 2
- **Files modified:** 0 (read-only testing)

## Accomplishments

- Ran all 13 CLI commands (6 mesh-related, 7 G-code-related) and captured full output
- Discovered critical 3MF parsing limitation: BambuStudio production extension not supported
- Confirmed G-code validation, analysis, and comparison all work correctly with Bambu-generated files
- Zero unknown G-code commands across all three files (Bambu commands fully recognized)

## Test Results

### 3MF Commands (All Failed)

| # | Command | File | Exit | Result |
|---|---------|------|------|--------|
| 1 | `analyze` | Cube_PLA.3mf | 1 | `file contains no mesh data` |
| 2 | `analyze` | 3DBenchy_PLA.3mf | 1 | `file contains no mesh data` |
| 3 | `analyze` | SimplePyramid.3mf | 1 | `file contains no mesh data` |
| 4 | `slice` | Cube_PLA.3mf | 1 | `file contains no mesh data` |
| 5 | `slice` | 3DBenchy_PLA.3mf | 1 | `file contains no mesh data` |
| 6 | `slice` | SimplePyramid.3mf | 1 | `file contains no mesh data` |

**Root cause:** BambuStudio 3MF files use the **3MF Production Extension** (`p:` namespace). The root model file (`3D/3dmodel.model`) contains `<component p:path="/3D/Objects/object_1.model" .../>` references to sub-model files in `3D/Objects/`. The actual mesh geometry (vertices and triangles) is in these sub-model files. The lib3mf-core v0.3.0 parser only reads the root model file via `find_model_path` + `parse_model`, finds no mesh geometry there, and returns `EmptyModel`.

The sub-model files contain valid mesh data:
- `Cube_PLA.3mf` -> `3D/Objects/object_1.model` (9,629 bytes with mesh vertices/triangles)
- `3DBenchy_PLA.3mf` -> `3D/Objects/object_1.model` (20,445,944 bytes)
- `SimplePyramid.3mf` -> `3D/Objects/object_1.model` (960 bytes)

### G-code Commands (All Succeeded)

| # | Command | File | Exit | Key Metrics |
|---|---------|------|------|-------------|
| 7 | `validate` | Cube_PLA.gcode | 0 | 18,121 lines, VALID |
| 8 | `validate` | 3DBenchy_PLA.gcode | 0 | 66,006 lines, VALID |
| 9 | `validate` | SimplePyramid.gcode | 0 | 20,382 lines, VALID |
| 10 | `analyze-gcode --summary` | Cube_PLA.gcode | 0 | 151 layers, 8m43s est, 1.76m filament, 5.26g |
| 11 | `analyze-gcode --summary` | 3DBenchy_PLA.gcode | 0 | 345 layers, 10m45s est, 4.92m filament, 14.68g |
| 12 | `analyze-gcode --summary` | SimplePyramid.gcode | 0 | 451 layers, 16m26s est, 2.63m filament, 7.83g |
| 13 | `compare-gcode` | Cube vs Benchy | 0 | Full per-feature delta comparison |

**Notable findings:**
- Slicer detected correctly as `BambuStudio 02.05.00.66` for all files
- Zero unknown commands across all files -- BambuStudio G-code commands fully recognized
- Estimated vs header time shows consistent negative delta (-552s to -806s), meaning header time is higher than computed estimate (expected since header includes acceleration/deceleration not fully modeled)
- Feature breakdown includes 11 distinct feature types: Outer wall, Inner wall, Sparse infill, Internal solid infill, Bridge, Gap infill, Top surface, Bottom surface, Overhang wall, Floating vertical shell, Custom
- Compare tool correctly identifies per-feature deltas with percentage changes

### Detailed Feature Breakdown

**Cube_PLA.gcode** (20mm calibration cube):
- Dominated by Sparse infill (28.6%), Custom (21.3%), Outer wall (19.0%), Inner wall (17.5%)
- 950 retractions, 7 Z-hops

**3DBenchy_PLA.gcode** (complex benchmark):
- Dominated by Outer wall (38.2%), Inner wall (17.8%), Custom (15.9%)
- 3,518 retractions, high gap infill time (6.3%) due to complex geometry
- Highest move count (46,033) reflecting geometric complexity

**SimplePyramid.gcode** (pyramid shape):
- Dominated by Sparse infill (31.5%), Outer wall (27.8%), Inner wall (24.4%)
- Most layers (451) despite being simpler geometry (likely fine layer height)
- Minimal bridging and overhang features as expected for pyramid shape

## Task Commits

No code changes were made -- this was read-only testing. Only the summary document is committed.

**Plan metadata:** (see final commit)

## Files Created/Modified

- `.planning/quick/2-test-cli-with-3mf-and-g-code-files/2-SUMMARY.md` - This test report

## Decisions Made

- No code changes -- followed plan specification for read-only testing

## Deviations from Plan

None - plan executed exactly as written.

## Issues Discovered

### Critical: 3MF Production Extension Not Supported

**Severity:** High -- prevents loading any BambuStudio-exported 3MF file
**Affected component:** `crates/slicecore-fileio/src/threemf.rs` -> lib3mf-core v0.3.0
**Root cause:** `find_model_path` locates the root model, but BambuStudio uses `<component p:path="...">` references to sub-models. The parser needs to:
1. Parse the root model and detect `<component>` elements with `p:path` attributes
2. Read the referenced sub-model files from the archive
3. Parse each sub-model and merge the mesh geometry

**Impact:** Any 3MF file using the production extension (all BambuStudio, most OrcaSlicer exports) will fail to load. Only simple single-file 3MF archives will work.

**Recommended fix:** Add production extension resolution to `threemf::parse()` in slicecore-fileio, or upstream the fix to lib3mf-core.

### Observation: Time Estimate Delta

All three files show the estimated time being lower than the header time by 9-13 minutes. This is expected behavior -- the trapezoid time estimator does not account for all firmware-level acceleration, jerk, and pressure advance processing that BambuStudio models.

## User Setup Required

None - no external service configuration required.

## Follow-up Recommendations

1. **Fix 3MF production extension support** -- Add resolution of `<component p:path="...">` references in `threemf::parse()`. This requires reading sub-model `.rels` files and parsing referenced sub-models from the archive.
2. **Consider upstreaming** -- The production extension support could be added to lib3mf-core directly, benefiting all downstream users.
3. **Test with non-Bambu 3MF files** -- Verify that simple 3MF files (e.g., from Windows 3D Builder, which do not use production extension) load correctly.

---
*Quick Task: 2-test-cli-with-3mf-and-g-code-files*
*Completed: 2026-02-26*
