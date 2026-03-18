//! Progress bar wrapper with TTY detection.
//!
//! Provides a [`SliceProgress`] struct that wraps `indicatif::ProgressBar` with
//! automatic terminal detection: interactive terminals get a visual progress bar,
//! while non-TTY environments (pipes, CI) get plain text line output.

use std::io::IsTerminal as _;

use indicatif::{ProgressBar, ProgressStyle};

/// Progress bar for slice operations with automatic TTY detection.
///
/// When running in an interactive terminal, displays a styled progress bar
/// with spinner, elapsed time, and phase messages. In non-TTY environments,
/// emits plain text lines to stderr instead.
#[derive(Debug)]
pub struct SliceProgress {
    /// The underlying indicatif progress bar.
    bar: ProgressBar,
    /// Whether we are running in a TTY (interactive terminal).
    is_tty: bool,
}

impl SliceProgress {
    /// Creates a new progress bar with the given total step count.
    ///
    /// Automatically detects whether stderr is a terminal. If so, creates a
    /// styled progress bar. Otherwise, creates a hidden bar and uses text
    /// fallback for phase updates.
    #[must_use]
    pub fn new(total_steps: u64) -> Self {
        let is_tty = std::io::stderr().is_terminal();

        let bar = if is_tty {
            let pb = ProgressBar::new(total_steps);
            let style = ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
            )
            .expect("valid progress bar template")
            .progress_chars("=>-");
            pb.set_style(style);
            pb
        } else {
            ProgressBar::hidden()
        };

        Self { bar, is_tty }
    }

    /// Updates the current phase message.
    ///
    /// In TTY mode, updates the progress bar message. In non-TTY mode,
    /// emits a plain text line to stderr.
    pub fn set_phase(&self, phase: &str) {
        if self.is_tty {
            self.bar.set_message(phase.to_string());
        } else {
            eprintln!("[progress] {phase}");
        }
    }

    /// Increments the progress bar by `n` steps.
    pub fn inc(&self, n: u64) {
        self.bar.inc(n);
    }

    /// Finishes and clears the progress bar, printing a completion message.
    pub fn finish(&self) {
        if self.is_tty {
            self.bar.finish_and_clear();
        }
        eprintln!("Slicing complete.");
    }

    /// Prints a message above the progress bar (TTY) or directly to stderr (non-TTY).
    pub fn println(&self, msg: &str) {
        if self.is_tty {
            self.bar.println(msg);
        } else {
            eprintln!("{msg}");
        }
    }
}

/// Convenience function to create a new [`SliceProgress`].
#[must_use]
pub fn create_progress(total: u64) -> SliceProgress {
    SliceProgress::new(total)
}
