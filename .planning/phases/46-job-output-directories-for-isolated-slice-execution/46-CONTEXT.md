# Phase 46: Job Output Directories for Isolated Slice Execution - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement `--job-dir` flag for the `slice` command that creates a structured output directory containing G-code, logs, config snapshot, thumbnail, and manifest. Includes `--job-dir auto` for auto-generated UUID directory names, `--job-base` flag for custom parent directory, `SLICECORE_JOB_DIR` env var for default base directory (SaaS/farm use), and a config file option for persistent base directory configuration. Enables isolated artifact management for parallel slicing, batch workflows, and future daemon/farm/SaaS features.

**Not in scope:** Job management CLI subcommands (inspect, list, clean, reslice), input model copying into job dir, batch slicing, 3MF project output.

</domain>

<decisions>
## Implementation Decisions

### Directory Structure
- Flat layout — all files at top level of job directory
- G-code filename derived from input model name (benchy.stl → benchy.gcode)
- Plate mode produces single combined G-code file named `plate.gcode`
- All artifacts always written when --job-dir is used — config.toml, slice.log, thumbnail.png, manifest.json are automatic (no extra flags needed)
- Fixed artifact filenames: `manifest.json`, `config.toml`, `slice.log`, `thumbnail.png`, plus model-derived `.gcode`

### Manifest Format
- JSON format (`manifest.json`)
- Integer `schema_version` field (starting at 1) for forward compatibility
- Manifest written twice: initially with `"status": "running"` at job start, overwritten with final status + stats on completion/failure
- On failure: `"status": "failed"` with error details, partial artifacts remain (config, partial log)

**Manifest fields:**
- Core: schema_version, slicecore_version, status, created, started, completed, duration_ms
- Files: input (array of model paths), output (gcode filename), config, log, thumbnail
- Reproduce command: full CLI invocation to re-slice identically
- Print statistics: filament usage (length, weight, cost), estimated print time, layer count, line count
- File checksums: SHA256 of each artifact (G-code, config, thumbnail) for integrity verification
- Profile provenance: which profiles were used (machine, filament, process), source paths, checksums
- Input model metadata: triangle count, bounding box dimensions, file size, repair status
- Warnings summary: non-fatal warnings emitted during slicing
- Environment info: OS, architecture, hostname
- Config diff from defaults: which settings differ from base profile (compact non-default view)
- Per-object stats in plate mode

### --job-dir Flag Behavior
- `--job-dir <path>` creates directory with `mkdir -p` semantics (auto-creates parents)
- Error if target directory is non-empty (use `--force` to override and overwrite)
- `--job-dir` is mutually exclusive with `--output`, `--log-file`, `--save-config` — error if combined
- On success, prints the job directory path to stdout (for script consumption: `JOB=$(slicecore slice ... --job-dir ./job)`)
- `--job-dir auto` generates UUID-named directory
- `--job-base <path>` sets parent directory for auto-generated job dirs (default: CWD)
- `SLICECORE_JOB_DIR` env var sets default base directory for auto jobs (SaaS/farm use)
- Priority: `--job-base` flag > `SLICECORE_JOB_DIR` env var > CWD
- Config file option (`job_base_dir`) for persistent base directory configuration

### Isolation & Safety
- File lock (`.lock` file with PID) in job directory to prevent concurrent writes
- Second process attempting same job dir errors with "job dir locked by PID X"
- Lock released on completion (success or failure)
- On failure: artifacts written directly to final dir, manifest updated to `"status": "failed"` with error details
- Partial artifacts (config, log) remain for debugging — no cleanup on failure

### Claude's Discretion
- Lock file implementation details (flock vs advisory vs PID file)
- Config file format and location for `job_base_dir` setting
- Exact manifest JSON structure and field ordering
- How config diff from defaults is computed and represented
- Thumbnail format/size selection within job dir context
- Internal code organization (separate module vs inline)
- Error message wording

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### CLI and Profile System
- `crates/slicecore-cli/src/main.rs` — Current `cmd_slice()` and `cmd_slice_plate()` implementations, existing flag handling
- `.planning/phases/30-cli-profile-composition-and-slice-workflow/30-CONTEXT.md` — Profile composition decisions, provenance tracking, log file behavior, --save-config

### Requirements
- `.planning/REQUIREMENTS.md` — [API-02] requirement mapped to this phase

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cmd_slice()` (main.rs:1197): current slice command with output_path, log_file, save_config, thumbnails parameters — these become job dir artifacts
- `gcode_gen.rs:364`: already generates reproduce command for G-code header — reuse for manifest
- Profile provenance system from Phase 30: FieldSource tracking with source type, file path, override chain
- Thumbnail rendering pipeline: existing --thumbnails, --thumbnail-format, --thumbnail-quality flags
- `PrintConfig::from_file()`: auto-detects TOML vs JSON — reuse for config snapshot writing

### Established Patterns
- clap derive macros for CLI argument parsing with conflict groups
- stderr for progress/warnings, stdout for output data
- `process::exit()` with structured exit codes (0=success, 1=general, 2=config, 3=mesh, 4=safety)
- indicatif for progress bars (Phase 40)

### Integration Points
- `cmd_slice()` parameter list: --job-dir flag needs to be added, then controls output routing
- Mutually exclusive flag group: --job-dir conflicts with --output, --log-file, --save-config
- G-code writer: already embeds config in header comments — config.toml snapshot is the file version
- Stats output system: already computes filament usage, time estimates, layer counts — feed into manifest

</code_context>

<specifics>
## Specific Ideas

- Job directory is the "everything in one place" mode — when you use --job-dir, you get the complete picture without needing to remember individual flags
- For SaaS and print farms, the base directory config (env var + config file + --job-base flag) means the daemon sets SLICECORE_JOB_DIR once and every `--job-dir auto` call creates an isolated UUID directory under it
- Manifest as a lifecycle document: "running" → "success"/"failed" — external monitoring tools can poll manifest.json to track job status
- The file lock + non-empty check provides a belt-and-suspenders approach to preventing concurrent writes

</specifics>

<deferred>
## Deferred Ideas

- **Job management CLI subcommands** — `slicecore job inspect/list/clean/reslice` for managing job directories. Natural follow-up phase.
- **Input model copy (--include-model)** — Copy source STL/3MF into job dir for fully self-contained reproducibility.
- **Model + timestamp naming** — Alternative to UUID for auto-generated directory names (e.g., benchy-2026-03-24T120000/).
- **Model + short hash naming** — Another auto-naming alternative (e.g., benchy-a1b2c3/).

</deferred>

---

*Phase: 46-job-output-directories-for-isolated-slice-execution*
*Context gathered: 2026-03-24*
