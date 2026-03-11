//! Parallel processing infrastructure for the slicing pipeline.
//!
//! This module provides conditional parallelism via the `parallel` feature flag.
//! When enabled, operations use rayon's parallel iterators; when disabled,
//! they fall back to sequential iterators. This ensures WASM compatibility
//! (which cannot use rayon) while maximizing native performance.

#[cfg(feature = "parallel")]
#[allow(unused_imports, reason = "used by maybe_par_iter macro expansion")]
use rayon::prelude::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Dispatches to `par_iter()` when the `parallel` feature is enabled,
/// or `iter()` when it is disabled.
///
/// # Example
/// ```ignore
/// use crate::parallel::maybe_par_iter;
/// let data: Vec<i32> = vec![1, 2, 3];
/// let sum: i32 = maybe_par_iter!(data).map(|x| x * 2).sum();
/// ```
macro_rules! maybe_par_iter {
    ($slice:expr) => {{
        #[cfg(feature = "parallel")]
        {
            $slice.par_iter()
        }
        #[cfg(not(feature = "parallel"))]
        {
            $slice.iter()
        }
    }};
}
pub(crate) use maybe_par_iter;

/// Dispatches to `par_iter_mut()` when the `parallel` feature is enabled,
/// or `iter_mut()` when it is disabled.
#[allow(unused_macros, reason = "utility macro available for future use")]
macro_rules! maybe_par_iter_mut {
    ($slice:expr) => {{
        #[cfg(feature = "parallel")]
        {
            $slice.par_iter_mut()
        }
        #[cfg(not(feature = "parallel"))]
        {
            $slice.iter_mut()
        }
    }};
}
#[allow(unused_imports, reason = "utility macro available for future use")]
pub(crate) use maybe_par_iter_mut;

/// Initialize the global rayon thread pool with a specific thread count.
///
/// When the `parallel` feature is enabled and `thread_count` is `Some(n)`,
/// configures rayon to use `n` threads. If `None`, rayon uses its default
/// (number of logical CPUs). When the `parallel` feature is disabled,
/// this is a no-op.
///
/// Safe to call multiple times -- if the pool is already initialized,
/// subsequent calls are silently ignored.
pub fn init_thread_pool(thread_count: Option<usize>) {
    #[cfg(feature = "parallel")]
    {
        if let Some(count) = thread_count {
            // build_global returns Err if already initialized -- that's fine.
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(count)
                .build_global();
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = thread_count;
    }
}

/// Thread-safe progress tracker for parallel layer processing.
///
/// Uses atomic operations so multiple threads can increment the counter
/// concurrently without locks.
#[derive(Debug, Clone)]
#[allow(dead_code, reason = "utility struct; total and percent() used in tests and future parallel features")]
pub struct AtomicProgress {
    current: Arc<AtomicUsize>,
    total: usize,
}

impl AtomicProgress {
    /// Create a new progress tracker with the given total count.
    pub fn new(total: usize) -> Self {
        Self {
            current: Arc::new(AtomicUsize::new(0)),
            total,
        }
    }

    /// Increment the progress counter by one.
    pub fn increment(&self) {
        self.current.fetch_add(1, Ordering::Relaxed);
    }

    /// Return the current progress as a percentage (0.0 to 100.0).
    #[allow(dead_code, reason = "utility method; used in tests and future parallel features")]
    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            return 100.0;
        }
        let current = self.current.load(Ordering::Relaxed);
        (current as f64 / self.total as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maybe_par_iter_sums_correctly() {
        let data: Vec<i32> = (1..=100).collect();
        let sum: i32 = maybe_par_iter!(data).copied().sum();
        assert_eq!(sum, 5050);
    }

    #[test]
    fn maybe_par_iter_mut_doubles() {
        let mut data: Vec<i32> = vec![1, 2, 3];
        maybe_par_iter_mut!(data).for_each(|x| *x *= 2);
        assert_eq!(data, vec![2, 4, 6]);
    }

    #[test]
    fn init_thread_pool_no_panic() {
        // Should not panic even if called multiple times.
        init_thread_pool(None);
        init_thread_pool(Some(2));
    }

    #[test]
    fn atomic_progress_tracking() {
        let progress = AtomicProgress::new(10);
        assert_eq!(progress.percent(), 0.0);

        for _ in 0..5 {
            progress.increment();
        }
        assert!((progress.percent() - 50.0).abs() < f64::EPSILON);

        for _ in 0..5 {
            progress.increment();
        }
        assert!((progress.percent() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn atomic_progress_zero_total() {
        let progress = AtomicProgress::new(0);
        assert!((progress.percent() - 100.0).abs() < f64::EPSILON);
    }
}
