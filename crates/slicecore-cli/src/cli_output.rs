//! Unified CLI output abstraction with progress bars, spinners, and colored messages.
//!
//! Provides [`CliOutput`], the single entry point for all user-facing output in the
//! slicecore CLI. It respects `--quiet`, `--json`, `--color`, TTY detection, and the
//! `NO_COLOR` environment variable.

use std::io::IsTerminal as _;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

// ---------------------------------------------------------------------------
// ColorMode
// ---------------------------------------------------------------------------

/// Color output mode for the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Always emit ANSI color codes.
    Always,
    /// Never emit ANSI color codes.
    Never,
    /// Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset.
    Auto,
}

// ---------------------------------------------------------------------------
// CliOutput
// ---------------------------------------------------------------------------

/// Unified output handler for the slicecore CLI.
///
/// All user-facing messages (progress spinners, step indicators, warnings,
/// errors, informational text) go through this type. It respects the global
/// `--quiet`, `--json`, and `--color` flags and adapts to TTY vs non-TTY
/// environments.
pub struct CliOutput {
    /// Manages all progress bars and spinners on stderr.
    multi: MultiProgress,
    /// Whether the user asked for quiet mode.
    quiet: bool,
    /// Whether JSON output is active (implies effective quiet for progress).
    json: bool,
    /// Whether stderr is an interactive terminal.
    is_tty: bool,
    /// Resolved color setting.
    pub(crate) color_enabled: bool,
}

impl std::fmt::Debug for CliOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliOutput")
            .field("quiet", &self.quiet)
            .field("json", &self.json)
            .field("is_tty", &self.is_tty)
            .field("color_enabled", &self.color_enabled)
            .finish_non_exhaustive()
    }
}

impl CliOutput {
    /// Creates a new `CliOutput` that respects the given flags.
    ///
    /// When `quiet` or `json` is true, all progress output is suppressed
    /// (drawn to a hidden target). `error_msg` is never suppressed.
    #[must_use]
    pub fn new(quiet: bool, json: bool, color: ColorMode) -> Self {
        let is_tty = std::io::stderr().is_terminal();
        let effective_quiet = quiet || json;

        let color_enabled = match color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => is_tty && std::env::var("NO_COLOR").is_err(),
        };

        let multi = if effective_quiet {
            MultiProgress::with_draw_target(ProgressDrawTarget::hidden())
        } else {
            MultiProgress::new()
        };

        // Configure the console crate's color handling globally.
        console::set_colors_enabled_stderr(color_enabled);
        console::set_colors_enabled(color_enabled);

        Self {
            multi,
            quiet,
            json,
            is_tty,
            color_enabled,
        }
    }

    // -- Step indicators ---------------------------------------------------

    /// Creates a numbered step spinner like `[1/5] Resolve profiles`.
    ///
    /// In non-TTY non-quiet mode, prints a plain text line and returns a
    /// hidden progress bar. In TTY mode, returns a live spinner managed by
    /// the [`MultiProgress`].
    #[must_use]
    pub fn start_step(&self, step: usize, total: usize, message: &str) -> ProgressBar {
        if self.effective_quiet() {
            return ProgressBar::hidden();
        }

        if !self.is_tty {
            eprintln!("[{step}/{total}] {message}...");
            return ProgressBar::hidden();
        }

        let spinner = self.multi.add(ProgressBar::new_spinner());
        let style = ProgressStyle::with_template("{spinner:.green} [{prefix}] {msg} {elapsed:.dim}")
            .expect("valid spinner template")
            .tick_strings(&[
                "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}",
                "\u{2826}", "\u{2827}", "\u{2807}", "\u{280f}", "\u{2713}",
            ]);
        spinner.set_style(style);
        spinner.set_prefix(format!("{step}/{total}"));
        spinner.set_message(message.to_string());
        spinner.enable_steady_tick(Duration::from_millis(80));
        spinner
    }

    /// Finishes a step spinner with a completion message.
    pub fn finish_step(&self, spinner: &ProgressBar, message: &str) {
        if !self.is_tty && !self.effective_quiet() {
            eprintln!("[done] {message}");
        } else {
            spinner.finish_with_message(message.to_string());
        }
    }

    // -- Progress bars -----------------------------------------------------

    /// Creates a nested progress bar (e.g., for layer progress within a step).
    ///
    /// Returns a hidden bar if output is suppressed or not a TTY.
    #[must_use]
    pub fn add_progress_bar(&self, total: u64) -> ProgressBar {
        if self.effective_quiet() || !self.is_tty {
            return ProgressBar::hidden();
        }

        let bar = self.multi.add(ProgressBar::new(total));
        let style = ProgressStyle::with_template("  {bar:40.cyan/blue} {pos}/{len} {msg}")
            .expect("valid bar template")
            .progress_chars("=>-");
        bar.set_style(style);
        bar
    }

    // -- Simple spinners ---------------------------------------------------

    /// Creates a simple spinner for medium-duration operations.
    #[must_use]
    pub fn spinner(&self, message: &str) -> ProgressBar {
        if self.effective_quiet() {
            return ProgressBar::hidden();
        }

        if !self.is_tty {
            eprintln!("{message}...");
            return ProgressBar::hidden();
        }

        let spinner = self.multi.add(ProgressBar::new_spinner());
        let style = ProgressStyle::with_template("{spinner:.green} {msg}")
            .expect("valid spinner template")
            .tick_strings(&[
                "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}",
                "\u{2826}", "\u{2827}", "\u{2807}", "\u{280f}", "\u{2713}",
            ]);
        spinner.set_style(style);
        spinner.set_message(message.to_string());
        spinner.enable_steady_tick(Duration::from_millis(80));
        spinner
    }

    /// Finishes a spinner with a completion message.
    pub fn finish_spinner(&self, spinner: &ProgressBar, message: &str) {
        spinner.finish_with_message(message.to_string());
    }

    // -- Messages ----------------------------------------------------------

    /// Prints a warning message. Suppressed in quiet mode.
    pub fn warn(&self, msg: &str) {
        if self.effective_quiet() {
            return;
        }
        let text = if self.color_enabled {
            format!("{}", console::style(format!("Warning: {msg}")).yellow())
        } else {
            format!("Warning: {msg}")
        };
        if self.is_tty {
            let _ = self.multi.println(&text);
        } else {
            eprintln!("{text}");
        }
    }

    /// Prints an error message. **Never** suppressed, even in quiet mode.
    ///
    /// Uses `eprintln!` directly to guarantee the message reaches stderr
    /// even when no progress bars are active (where `multi.println` may
    /// silently discard the output).
    pub fn error_msg(&self, msg: &str) {
        let text = if self.color_enabled {
            format!(
                "{}",
                console::style(format!("Error: {msg}")).red().bold()
            )
        } else {
            format!("Error: {msg}")
        };
        eprintln!("{text}");
    }

    /// Prints an informational message. Suppressed in quiet mode.
    pub fn info(&self, msg: &str) {
        if self.effective_quiet() {
            return;
        }
        if self.is_tty {
            let _ = self.multi.println(msg);
        } else {
            eprintln!("{msg}");
        }
    }

    // -- Accessors ---------------------------------------------------------

    /// Returns `true` if JSON output mode is active.
    #[must_use]
    pub fn is_json(&self) -> bool {
        self.json
    }

    /// Returns `true` if quiet mode was requested.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    // -- Internal ----------------------------------------------------------

    /// Effective quiet: true when either `--quiet` or `--json` is set.
    fn effective_quiet(&self) -> bool {
        self.quiet || self.json
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quiet_mode_suppresses_output() {
        let out = CliOutput::new(true, false, ColorMode::Auto);
        assert!(out.is_quiet());
    }

    #[test]
    fn test_json_implies_quiet() {
        let out = CliOutput::new(false, true, ColorMode::Auto);
        assert!(out.is_json());
        // effective_quiet is true, but is_quiet() returns the raw flag
        assert!(!out.is_quiet());
        // JSON mode suppresses progress via effective_quiet
        assert!(out.effective_quiet());
    }

    #[test]
    fn test_color_mode_never() {
        let out = CliOutput::new(false, false, ColorMode::Never);
        assert!(!out.color_enabled);
    }

    #[test]
    fn test_color_mode_always() {
        let out = CliOutput::new(false, false, ColorMode::Always);
        assert!(out.color_enabled);
    }
}
