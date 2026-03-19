# Phase 40: Adopt indicatif for consistent CLI progress display - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace ad-hoc `println!`/`eprintln!` progress output across all CLI commands with a unified `CliOutput` abstraction built on indicatif. Add consistent `--quiet` and `--json` flag support globally. Does NOT include new CLI commands, new output formats, or TUI features.

</domain>

<decisions>
## Implementation Decisions

### Output Abstraction
- Extend existing `SliceProgress` (in `progress.rs`) into a shared `CliOutput` type — rename file to `cli_output.rs`
- `CliOutput` lives in `slicecore-cli` crate — it's a CLI concern, not engine
- Supports three modes: spinner (indeterminate), progress bar (measurable), step indicator (multi-phase)
- Constructed with `CliOutput::new(quiet, json)` — quiet and json awareness built in
- Handles progress/status AND warn/error messages (not data output)
- `warn()` suppressed in --quiet mode; `error_msg()` always shown regardless of --quiet
- TTY detection built in: styled output for terminals, plain text fallback for pipes/CI
- Data output (JSON results, tables) stays as direct `println!`/`serde_json` — CliOutput doesn't manage that

### Command Coverage (Tiered by Duration)
- **Progress bar** (long, measurable): `slice` (per-layer), `import-profiles` (per-file with current filename in message)
- **Spinner** (medium, indeterminate): `convert-profile`, `analyze-gcode`, `ai-suggest`, `calibrate *`, `csg`, `compare-gcode`
- **Step indicator** (multi-phase): `slice` workflow gets steps — [1/N] Resolve profiles, Validate config, Load mesh, Slice (with nested progress bar), Write G-code
- **No indicator** (instant): `list-profiles`, `search-profiles`, `show-profile`, `schema`, `validate`, `diff-profiles`, `plugins`

### Slice Command Workflow
- Replace ad-hoc eprintln! messages with step indicators + nested progress bar
- Steps show as completed checkmarks with elapsed time when done
- Active step shows spinner, slicing step shows progress bar nested under step
- Summary line at end: Output path, layers, estimated time, filament usage

### Quiet/JSON Consistency
- `--quiet`/`-q` added as a **global flag** on the top-level CLI struct (`#[arg(short, long, global = true)]`)
- Existing per-command `--quiet` on slice migrated to global flag with hidden deprecated alias for backwards compatibility
- `--quiet` suppresses ALL human output (progress, warnings, informational messages) — only errors and data output remain
- `--json` implies `--quiet` for progress output — no spinners/bars when JSON active, errors still go to stderr
- Add `--json` to commands that produce useful structured data: `convert-profile`, `import-profiles` (summary), `calibrate *` (output paths), `csg` (output info)
- Commands that already have `--json`: `slice`, `analyze-gcode`, `list-profiles`, `search-profiles`, `diff-profiles`, `schema`

### Visual Style
- Clean minimal style — Unicode checkmarks and braille dot spinners, subtle colors
- Green checkmarks (✓) for completed steps, braille dot spinner (⠋⠙⠹⠸⠼⠴) for active operations
- Cyan progress bar, yellow for warnings, red for errors
- Step timings shown by default in TTY mode (right-aligned elapsed time per step)
- Non-TTY plain text fallback: `[step] Operation... done` / `[progress] N/M`
- `--color always/never/auto` as global flag — auto-detect TTY by default
- Respects `NO_COLOR` environment variable
- Migrate Phase 38's per-command `--no-color` / `--color` on diff-profiles to the global flag

### Claude's Discretion
- Exact indicatif `ProgressStyle` templates and progress characters
- How to handle `MultiProgress` for nested step + bar rendering
- Internal CliOutput state management (active spinner tracking, step count)
- Whether to use `console` crate for color/style or stick with indicatif's built-in styling
- Non-TTY plain text format details
- Deprecated --quiet alias implementation approach on slice subcommand
- Progress bar update frequency / redraw throttling

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing progress code
- `crates/slicecore-cli/src/progress.rs` — Current `SliceProgress` wrapper with TTY detection, indicatif usage, and basic progress bar/spinner support
- `crates/slicecore-cli/Cargo.toml` — Current indicatif dependency version

### CLI structure
- `crates/slicecore-cli/src/main.rs` — Commands enum, all subcommand definitions, existing --quiet/--json flags, main dispatch
- `crates/slicecore-cli/src/slice_workflow.rs` — Slice workflow with ~76 eprintln!/println! calls to migrate
- `crates/slicecore-cli/src/calibrate/` — Calibrate subcommands with ad-hoc output

### Prior phase decisions
- `.planning/phases/23-progress-cancellation-api/23-CONTEXT.md` — EventBus progress events that CliOutput should consume during slicing
- `.planning/phases/30-cli-profile-composition-and-slice-workflow/30-CONTEXT.md` — Slice workflow steps, profile resolution, existing progress bar decision
- `.planning/phases/38-profile-diff-command-to-compare-presets-side-by-side-implement-slicecore-profile-diff-cli-subcommand-with-settings-comparison-category-grouping-impact-hints-and-multiple-output-formats/38-CONTEXT.md` — `--color always/never/auto` pattern to migrate to global flag

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `SliceProgress` (`progress.rs`): already wraps indicatif with TTY detection — direct foundation for CliOutput
- `indicatif` already in Cargo.toml dependency — no new dependency needed
- `std::io::IsTerminal` already used for TTY detection

### Established Patterns
- clap derive macros with `#[arg(short, long, global = true)]` for global flags
- stderr for progress/warnings, stdout for data (JSON, G-code paths)
- `--json` flag pattern across multiple commands
- Serde serialization for structured JSON output

### Integration Points
- `progress.rs` — rename to `cli_output.rs`, expand `SliceProgress` into `CliOutput`
- `main.rs` CLI struct — add global `--quiet`, `--color` flags
- `slice_workflow.rs` — heaviest migration (76 println!/eprintln! calls → CliOutput step/progress calls)
- `calibrate/*.rs` — add spinners to each calibrate subcommand
- `csg_command.rs` — add spinner for boolean operations
- All command handlers in `main.rs` — pass CliOutput through

</code_context>

<specifics>
## Specific Ideas

- The slice workflow mockup shows the target UX: checkmarks with timings for completed steps, spinner/bar for active step, clean summary at the end
- "Like cargo or rustup" — clean, professional, no decoration overload
- Global flags (`--quiet`, `--color`) go on the top-level struct, before subcommand — `slicecore -q slice model.stl`
- `--json` implying `--quiet` for progress is key for clean piping — stderr gets errors only, stdout gets JSON

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 40-adopt-indicatif-for-consistent-cli-progress-display-replace-ad-hoc-println-with-progress-bars-spinners-and-step-indicators-across-all-cli-commands-with-quiet-json-flag-support*
*Context gathered: 2026-03-19*
