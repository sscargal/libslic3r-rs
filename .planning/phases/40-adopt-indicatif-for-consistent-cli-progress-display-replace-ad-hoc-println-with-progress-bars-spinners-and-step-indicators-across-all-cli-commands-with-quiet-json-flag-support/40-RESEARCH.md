# Phase 40: Adopt indicatif for consistent CLI progress display - Research

**Researched:** 2026-03-19
**Domain:** CLI progress display, indicatif, terminal UX
**Confidence:** HIGH

## Summary

Phase 40 replaces ad-hoc `println!`/`eprintln!` progress output across all CLI commands with a unified `CliOutput` abstraction built on indicatif. The existing `SliceProgress` in `progress.rs` (83 lines) already wraps indicatif with TTY detection and provides a solid foundation to expand into `CliOutput`. The indicatif crate (0.17.11) is already a dependency with its `console` (0.15.11) transitive dependency providing color detection, `NO_COLOR` support, and terminal styling.

The main work involves: (1) expanding `SliceProgress` into a full `CliOutput` type with spinner/progress-bar/step-indicator modes and quiet/json awareness, (2) adding global `--quiet`, `--color` flags to the top-level CLI struct, (3) migrating the slice command's ~76 `eprintln!` calls to step-based CliOutput calls, (4) adding spinners to medium-duration commands (calibrate, csg, convert-profile, etc.), and (5) ensuring `--json` implies quiet for progress output.

**Primary recommendation:** Expand the existing `SliceProgress` into `CliOutput` using indicatif's `MultiProgress` for nested step + progress bar rendering, and the transitive `console` crate for color/styling. No new crate dependencies are needed.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Extend existing `SliceProgress` (in `progress.rs`) into a shared `CliOutput` type -- rename file to `cli_output.rs`
- `CliOutput` lives in `slicecore-cli` crate -- it's a CLI concern, not engine
- Supports three modes: spinner (indeterminate), progress bar (measurable), step indicator (multi-phase)
- Constructed with `CliOutput::new(quiet, json)` -- quiet and json awareness built in
- Handles progress/status AND warn/error messages (not data output)
- `warn()` suppressed in --quiet mode; `error_msg()` always shown regardless of --quiet
- TTY detection built in: styled output for terminals, plain text fallback for pipes/CI
- Data output (JSON results, tables) stays as direct `println!`/`serde_json` -- CliOutput doesn't manage that
- Progress bar commands: `slice` (per-layer), `import-profiles` (per-file with current filename in message)
- Spinner commands: `convert-profile`, `analyze-gcode`, `ai-suggest`, `calibrate *`, `csg`, `compare-gcode`
- Step indicator: `slice` workflow gets steps -- [1/N] Resolve profiles, Validate config, Load mesh, Slice (with nested progress bar), Write G-code
- No indicator (instant): `list-profiles`, `search-profiles`, `show-profile`, `schema`, `validate`, `diff-profiles`, `plugins`
- `--quiet`/`-q` added as a global flag on the top-level CLI struct (`#[arg(short, long, global = true)]`)
- Existing per-command `--quiet` on slice migrated to global flag with hidden deprecated alias for backwards compatibility
- `--quiet` suppresses ALL human output (progress, warnings, informational messages) -- only errors and data output remain
- `--json` implies `--quiet` for progress output -- no spinners/bars when JSON active, errors still go to stderr
- Add `--json` to commands: `convert-profile`, `import-profiles` (summary), `calibrate *` (output paths), `csg` (output info)
- Clean minimal style: Unicode checkmarks and braille dot spinners, subtle colors
- Green checkmarks for completed steps, braille dot spinner for active operations
- Cyan progress bar, yellow for warnings, red for errors
- Step timings shown by default in TTY mode (right-aligned elapsed time per step)
- Non-TTY plain text fallback: `[step] Operation... done` / `[progress] N/M`
- `--color always/never/auto` as global flag -- auto-detect TTY by default
- Respects `NO_COLOR` environment variable
- Migrate Phase 38's per-command `--no-color`/`--color` on diff-profiles to the global flag

### Claude's Discretion
- Exact indicatif `ProgressStyle` templates and progress characters
- How to handle `MultiProgress` for nested step + bar rendering
- Internal CliOutput state management (active spinner tracking, step count)
- Whether to use `console` crate for color/style or stick with indicatif's built-in styling
- Non-TTY plain text format details
- Deprecated --quiet alias implementation approach on slice subcommand
- Progress bar update frequency / redraw throttling

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| indicatif | 0.17.11 | Progress bars, spinners, multi-progress | Already a dependency; the de facto Rust progress display crate |
| console | 0.15.11 | Color detection, NO_COLOR, terminal styling | Transitive dependency of indicatif; already available at no cost |
| clap | 4.5.x | CLI argument parsing with global flags | Already a dependency; derive macros with `global = true` for shared flags |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::io::IsTerminal | stable | TTY detection | Already used in `progress.rs`; use for fallback decisions |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| console (transitive) | owo-colors, colored | Would add a new dependency; console is already available via indicatif |
| indicatif MultiProgress | Manual terminal control | MultiProgress handles cursor management, redraws, thread safety |

**No new dependencies needed.** indicatif 0.17.11 and its transitive `console` 0.15.11 provide everything required.

## Architecture Patterns

### Recommended Project Structure
```
crates/slicecore-cli/src/
  cli_output.rs          # CliOutput (renamed from progress.rs)
  main.rs                # Global --quiet, --color, --json flags on Cli struct
  slice_workflow.rs       # Migrated to use CliOutput steps
  calibrate/             # Each subcommand gets spinner via CliOutput
  csg_command.rs         # Spinner via CliOutput
  diff_profiles_command.rs  # Migrate per-command --color to global
```

### Pattern 1: CliOutput Construction and Mode Selection
**What:** A single `CliOutput` struct encapsulating all output modes with quiet/json/color awareness.
**When to use:** Every command handler receives a `CliOutput` instance constructed from global flags.
**Example:**
```rust
use console::Style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

/// Output mode for CLI commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Always,
    Never,
    Auto,
}

/// Unified CLI output handler with quiet/json/color awareness.
pub struct CliOutput {
    multi: MultiProgress,
    quiet: bool,
    json: bool,
    is_tty: bool,
    color_enabled: bool,
}

impl CliOutput {
    pub fn new(quiet: bool, json: bool, color: ColorMode) -> Self {
        let is_tty = std::io::stderr().is_terminal();
        let effective_quiet = quiet || json;
        let color_enabled = match color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => is_tty && std::env::var("NO_COLOR").is_err(),
        };

        // If quiet or json, use hidden draw target to suppress all progress
        let multi = if effective_quiet {
            MultiProgress::with_draw_target(indicatif::ProgressDrawTarget::hidden())
        } else {
            MultiProgress::new() // defaults to stderr
        };

        // Set console crate global color state to match our decision
        console::set_colors_enabled_stderr(color_enabled);
        console::set_colors_enabled(color_enabled);

        Self { multi, quiet: effective_quiet, json, is_tty, color_enabled }
    }
}
```

### Pattern 2: Step Indicator with Nested Progress Bar
**What:** `MultiProgress` manages a step line (spinner) and a nested progress bar beneath it.
**When to use:** The `slice` command workflow where steps complete sequentially and slicing has a measurable progress bar.
**Example:**
```rust
impl CliOutput {
    /// Start a numbered step (e.g., "[1/5] Resolve profiles").
    pub fn start_step(&self, step: usize, total: usize, message: &str) -> ProgressBar {
        if !self.is_tty && !self.quiet {
            eprintln!("[{step}/{total}] {message}...");
            return ProgressBar::hidden();
        }
        let spinner = self.multi.add(ProgressBar::new_spinner());
        let style = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{prefix}] {msg}"
        ).expect("valid template")
         .tick_strings(&["\u{28CB}", "\u{28D9}", "\u{28F9}", "\u{28F8}", "\u{28FC}", "\u{28F4}", "\u{2713}"]);
        // braille spinner chars: ⣋ ⣙ ⣹ ⣸ ⣼ ⣴  then checkmark ✓ as final
        spinner.set_style(style);
        spinner.set_prefix(format!("{step}/{total}"));
        spinner.set_message(message.to_string());
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        spinner
    }

    /// Finish a step with a checkmark and elapsed time.
    pub fn finish_step(&self, spinner: &ProgressBar) {
        // The final tick_string is the checkmark
        spinner.finish();
    }

    /// Create a progress bar nested under the current MultiProgress.
    pub fn add_progress_bar(&self, total: u64) -> ProgressBar {
        if !self.is_tty && !self.quiet {
            return ProgressBar::hidden(); // non-TTY gets text updates via println
        }
        let bar = self.multi.add(ProgressBar::new(total));
        let style = ProgressStyle::with_template(
            "  {bar:40.cyan/blue} {pos}/{len} {msg}"
        ).expect("valid template")
         .progress_chars("=>-");
        bar.set_style(style);
        bar
    }
}
```

### Pattern 3: Spinner for Medium-Duration Commands
**What:** A simple spinner for commands like `convert-profile`, `csg`, `calibrate *`.
**When to use:** Any command that takes more than ~0.5s but doesn't have measurable progress.
**Example:**
```rust
impl CliOutput {
    /// Start a spinner with a message.
    pub fn spinner(&self, message: &str) -> ProgressBar {
        if !self.is_tty && !self.quiet {
            eprintln!("{message}...");
            return ProgressBar::hidden();
        }
        let spinner = self.multi.add(ProgressBar::new_spinner());
        let style = ProgressStyle::with_template("{spinner:.green} {msg}")
            .expect("valid template")
            .tick_strings(&["\u{28CB}", "\u{28D9}", "\u{28F9}", "\u{28F8}", "\u{28FC}", "\u{28F4}", "\u{2713}"]);
        spinner.set_style(style);
        spinner.set_message(message.to_string());
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        spinner
    }

    /// Finish spinner with a success message.
    pub fn finish_spinner(&self, spinner: &ProgressBar, message: &str) {
        spinner.finish_with_message(message.to_string());
    }
}
```

### Pattern 4: Warning and Error Output
**What:** Centralized warn/error methods that respect quiet mode.
**When to use:** All commands that currently use `eprintln!("Warning: ...")` or `eprintln!("Error: ...")`.
**Example:**
```rust
impl CliOutput {
    /// Print a warning (suppressed in --quiet mode).
    pub fn warn(&self, msg: &str) {
        if self.quiet { return; }
        let styled = if self.color_enabled {
            format!("{}", console::style(format!("Warning: {msg}")).yellow())
        } else {
            format!("Warning: {msg}")
        };
        // Use multi.println to avoid clobbering active progress bars
        let _ = self.multi.println(&styled);
    }

    /// Print an error (ALWAYS shown, even in --quiet mode).
    pub fn error_msg(&self, msg: &str) {
        let styled = if self.color_enabled {
            format!("{}", console::style(format!("Error: {msg}")).red().bold())
        } else {
            format!("Error: {msg}")
        };
        let _ = self.multi.println(&styled);
    }

    /// Print an informational message (suppressed in --quiet mode).
    pub fn info(&self, msg: &str) {
        if self.quiet { return; }
        let _ = self.multi.println(msg);
    }
}
```

### Pattern 5: Global Flags on Cli Struct
**What:** Add `--quiet`, `--color` as global flags to the top-level struct.
**When to use:** Single point of definition; all subcommands inherit automatically.
**Example:**
```rust
#[derive(Parser)]
#[command(name = "slicecore", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Plugin directory (overrides config plugin_dir)
    #[arg(long, global = true)]
    plugin_dir: Option<PathBuf>,

    /// Suppress progress output, warnings, and informational messages
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Color output mode
    #[arg(long, global = true, default_value = "auto", value_parser = ["always", "never", "auto"])]
    color: String,
}
```

### Pattern 6: Deprecated Alias for Slice --quiet
**What:** Keep backwards compatibility for `slicecore slice --quiet` by using a hidden per-command flag.
**When to use:** Migration period for existing scripts.
**Example:**
```rust
Commands::Slice {
    // ... existing fields ...

    /// [DEPRECATED] Use global -q/--quiet instead.
    #[arg(long, hide = true)]
    quiet: bool,  // rename to quiet_deprecated or use a different approach
}
// In dispatch: let effective_quiet = cli.quiet || slice_quiet_deprecated;
```

### Anti-Patterns to Avoid
- **Direct `eprintln!` for progress**: Always route through `CliOutput` so quiet/json/TTY logic is centralized.
- **Creating ProgressBar without MultiProgress**: Individual bars outside MultiProgress will conflict with step indicators; always use `multi.add()`.
- **Checking `is_terminal()` in individual commands**: TTY detection belongs in `CliOutput::new()` only.
- **Using `println!` for warnings/progress**: All non-data output must go to stderr (already the convention).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-line progress rendering | Manual cursor manipulation | `indicatif::MultiProgress` | Handles cursor movement, redraws, thread safety |
| Terminal color detection | Manual `$TERM` / `$NO_COLOR` checks | `console::colors_enabled_stderr()` | Handles CLICOLOR, NO_COLOR, TERM=dumb, Windows |
| Spinner animation | Manual `\r` + character cycling | `ProgressBar::new_spinner()` + `enable_steady_tick()` | Background thread handles animation timing |
| TTY detection | `libc::isatty()` | `std::io::IsTerminal` | Standard library, cross-platform |

**Key insight:** indicatif + console handle all the terminal edge cases (Windows terminals, pipe detection, cursor save/restore, line clearing). Building custom terminal rendering is a rabbit hole of platform-specific bugs.

## Common Pitfalls

### Pitfall 1: MultiProgress and Hidden Bars Interaction
**What goes wrong:** Creating a `MultiProgress` and adding hidden bars still causes empty line flickering.
**Why it happens:** `MultiProgress::new()` draws to stderr even if individual bars are hidden.
**How to avoid:** When quiet/json mode, create `MultiProgress::with_draw_target(ProgressDrawTarget::hidden())` so the entire multi-progress is suppressed.
**Warning signs:** Blank lines appearing in `--quiet` or `--json` mode.

### Pitfall 2: Spinner Not Animating
**What goes wrong:** Spinner appears frozen at the first tick character.
**Why it happens:** Forgot to call `enable_steady_tick()` -- without it, the spinner only updates when `inc()` or `set_message()` is called.
**How to avoid:** Always call `spinner.enable_steady_tick(Duration::from_millis(80))` immediately after creation.
**Warning signs:** Static spinner character that never changes.

### Pitfall 3: Progress Bar Under MultiProgress Not Removed
**What goes wrong:** Completed progress bars leave visual artifacts (blank lines or stale content).
**Why it happens:** Using `finish_and_clear()` on a bar under `MultiProgress` clears the bar but doesn't remove its slot.
**How to avoid:** After `finish_and_clear()`, call `multi.remove(&bar)` to reclaim the line. Alternatively, use `finish()` or `finish_with_message()` and leave the completed state visible (which is the desired UX for step indicators).
**Warning signs:** Growing number of blank lines between active indicators.

### Pitfall 4: Global Flag Conflict with Per-Command Flag
**What goes wrong:** Both `Cli.quiet` (global) and `Commands::Slice { quiet }` (per-command) exist, causing clap to error on ambiguity.
**Why it happens:** clap's `global = true` propagates the flag to all subcommands, conflicting with a subcommand-level flag of the same name.
**How to avoid:** Remove the per-command `--quiet` from `Slice` and add a hidden `--quiet` alias (or rename to `--quiet-legacy`) with `#[arg(long = "quiet-compat", hide = true)]`. Or simply remove it since the global flag replaces it.
**Warning signs:** clap panics or returns `error: the argument '--quiet' cannot be used multiple times`.

### Pitfall 5: console::set_colors_enabled Affects Global State
**What goes wrong:** Calling `set_colors_enabled(false)` disables color for ALL subsequent `console::style()` calls, including in library code.
**Why it happens:** `console` uses a global atomic for color state.
**How to avoid:** Call `set_colors_enabled`/`set_colors_enabled_stderr` once at startup in `CliOutput::new()`, before any other output. This is fine since CliOutput is constructed once in `main()`.
**Warning signs:** Colors inconsistently applied across different output paths.

### Pitfall 6: Non-TTY Fallback Still Needs Output
**What goes wrong:** Non-TTY mode (piped output, CI) gets complete silence because all progress bars are hidden.
**Why it happens:** Using `ProgressBar::hidden()` suppresses everything, but CI users still want status updates.
**How to avoid:** In non-TTY mode, don't create hidden bars; instead, use plain text `eprintln!` in the `start_step`/`finish_step` methods. Only use hidden bars for `--quiet` mode.
**Warning signs:** CI logs show no progress information at all.

## Code Examples

### Example 1: Full CliOutput Construction in main()
```rust
fn main() {
    let cli = Cli::parse();

    let color_mode = match cli.color.as_str() {
        "always" => ColorMode::Always,
        "never" => ColorMode::Never,
        _ => ColorMode::Auto,
    };

    let output = CliOutput::new(cli.quiet, false, color_mode);
    // json is per-command, resolved in each match arm

    match cli.command {
        Commands::Slice { json, .. } => {
            // Override CliOutput if json is set on this command
            let output = if json { CliOutput::new(cli.quiet, true, color_mode) } else { output };
            cmd_slice(&output, ...);
        }
        Commands::Csg(cmd) => {
            let spinner = output.spinner("Running CSG operation");
            if let Err(e) = csg_command::run_csg(cmd) {
                output.error_msg(&format!("{e}"));
                process::exit(1);
            }
            output.finish_spinner(&spinner, "CSG operation complete");
        }
        // ...
    }
}
```

### Example 2: Slice Workflow Step Migration
```rust
// BEFORE (current code in cmd_slice):
eprintln!("Note: Mesh repaired ({} degenerates removed, ...)", report.degenerate_removed, ...);
eprintln!("Profile: {expected_type} = {} ({})", resolved.name, resolved.source);

// AFTER:
let step1 = output.start_step(1, 5, "Resolve profiles");
// ... profile resolution ...
output.info(&format!("Profile: {expected_type} = {} ({})", resolved.name, resolved.source));
output.finish_step(&step1);

let step2 = output.start_step(2, 5, "Validate config");
// ... validation ...
for w in &warnings {
    output.warn(&format!("{}: {}", w.field, w.message));
}
output.finish_step(&step2);

let step3 = output.start_step(3, 5, "Load mesh");
// ... mesh loading + repair ...
if !report.was_already_clean {
    output.info(&format!("Mesh repaired ({} degenerates removed, ...)", ...));
}
output.finish_step(&step3);

let step4 = output.start_step(4, 5, "Slice");
let bar = output.add_progress_bar(total_layers as u64);
for layer in 0..total_layers {
    // ... slice layer ...
    bar.inc(1);
}
bar.finish_and_clear();
output.finish_step(&step4);

let step5 = output.start_step(5, 5, "Write G-code");
// ... write output ...
output.finish_step(&step5);
```

### Example 3: Progress Bar Style Templates (Discretion Area)
```rust
// Step indicator (spinner with prefix for step numbering)
const STEP_TEMPLATE: &str = "{spinner:.green} [{prefix}] {msg} {elapsed:.dim}";

// Completed step (shows checkmark and elapsed time)
// Achieved by tick_strings final entry being "✓"

// Progress bar (nested under step)
const BAR_TEMPLATE: &str = "  {bar:40.cyan/blue} {pos}/{len} {msg}";

// Simple spinner (for non-step commands)
const SPINNER_TEMPLATE: &str = "{spinner:.green} {msg}";

// Braille dot spinner characters
const BRAILLE_TICKS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✓"];
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `indicatif::ProgressBar` standalone | `MultiProgress` for composable indicators | indicatif 0.16+ | Enables step + nested bar rendering |
| Manual `\r` terminal writes | `enable_steady_tick()` background thread | indicatif 0.15+ | No manual animation loop needed |
| `atty` crate for TTY detection | `std::io::IsTerminal` (stable in 1.70) | Rust 1.70 (June 2023) | No dependency needed for TTY check |
| `colored` / `ansi_term` | `console` crate (indicatif transitive) | console 0.15 | Already available, no extra dep |

**Deprecated/outdated:**
- `atty` crate: replaced by `std::io::IsTerminal` (already used in existing code)
- `ansi_term`: unmaintained, superseded by `console`
- `colored`: works but adds a dependency when `console` is already available

## Open Questions

1. **Exact `--quiet` deprecation strategy for slice subcommand**
   - What we know: Global `--quiet` must replace per-command `--quiet` on slice
   - What's unclear: Whether clap allows both a global and a per-command flag with the same `--quiet` long name (likely not -- would need different field names)
   - Recommendation: Remove per-command `--quiet` from `Slice` entirely. Since global flags propagate via `global = true`, `slicecore slice --quiet model.stl` will still work. If exact same position is needed, no action required -- clap global flags work transparently.

2. **Progress bar update frequency for slicing**
   - What we know: indicatif defaults to max 15 redraws/sec on MultiProgress
   - What's unclear: Whether per-layer updates on large models (1000+ layers) cause overhead
   - Recommendation: The 15 Hz default is fine. indicatif internally rate-limits. No manual throttling needed.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (standard) |
| Config file | workspace Cargo.toml |
| Quick run command | `cargo test -p slicecore-cli` |
| Full suite command | `cargo test --all-features --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| N/A-01 | CliOutput::new respects quiet flag | unit | `cargo test -p slicecore-cli --lib -- cli_output::tests` | Wave 0 |
| N/A-02 | CliOutput::new respects json->quiet | unit | `cargo test -p slicecore-cli --lib -- cli_output::tests` | Wave 0 |
| N/A-03 | ColorMode::Auto respects NO_COLOR | unit | `cargo test -p slicecore-cli --lib -- cli_output::tests` | Wave 0 |
| N/A-04 | Global --quiet flag parsed by clap | unit | `cargo test -p slicecore-cli --lib -- cli_tests` | Wave 0 |
| N/A-05 | Slice command still works with quiet | integration | `cargo test -p slicecore-cli -- slice` | Existing tests cover |
| N/A-06 | Non-TTY plain text fallback | unit | `cargo test -p slicecore-cli --lib -- cli_output::tests` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-cli`
- **Per wave merge:** `cargo test --all-features --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/slicecore-cli/src/cli_output.rs` -- unit tests for CliOutput modes, quiet/json logic, color mode selection
- [ ] Verify existing slice integration tests pass after migration

## Sources

### Primary (HIGH confidence)
- [indicatif 0.17.11 docs](https://docs.rs/indicatif/0.17.11/) - MultiProgress, ProgressBar, ProgressStyle API
- [console 0.15.11 source](https://docs.rs/console/0.15.11/) - Color detection, NO_COLOR support (verified in source: `unix_term.rs` line 29)
- Existing codebase: `crates/slicecore-cli/src/progress.rs` (83 lines, SliceProgress with TTY detection)
- Existing codebase: `crates/slicecore-cli/src/main.rs` (~1100 lines, full CLI structure with 245 println/eprintln calls)
- Existing codebase: `crates/slicecore-cli/Cargo.toml` (indicatif 0.17 already in dependencies)

### Secondary (MEDIUM confidence)
- [Rain's Rust CLI recommendations](https://rust-cli-recommendations.sunshowers.io/managing-colors-in-rust.html) - Color management best practices
- [indicatif GitHub examples](https://github.com/console-rs/indicatif/blob/main/examples/multi.rs) - MultiProgress usage patterns

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - indicatif 0.17.11 already a dependency, API verified via docs.rs
- Architecture: HIGH - CliOutput pattern is straightforward expansion of existing SliceProgress; MultiProgress API well-documented
- Pitfalls: HIGH - Common issues verified against indicatif docs and prior experience with terminal rendering

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable crate, unlikely to change)
