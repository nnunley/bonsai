use std::io::{self, Write};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Stats passed to the progress callback after each queue entry.
pub struct ProgressStats {
    pub original_size: usize,
    pub current_size: usize,
    pub tests_run: usize,
    pub reductions: usize,
    pub cache_hit_rate: f64,
}

/// Callback trait for receiving progress updates during reduction.
///
/// All methods take `&self` — implementors must use interior mutability
/// for any mutable state (e.g., rate-limiting timestamps).
pub trait ProgressCallback: Send + Sync {
    /// Called after each queue entry is processed.
    fn on_update(&self, stats: &ProgressStats);

    /// Called for each candidate tested (verbose detail).
    fn on_candidate(&self, transform_name: &str, start: usize, end: usize, accepted: bool);

    /// Called when a non-fatal warning occurs (e.g., reparse failure, test error).
    fn on_warning(&self, msg: &str);
}

/// Verbosity level for progress reporting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verbosity {
    /// No progress output.
    Quiet,
    /// Default: periodic size/percentage updates.
    Normal,
    /// Per-candidate detail.
    Verbose,
}

/// Reports reduction progress to stderr. Implements `ProgressCallback`.
pub struct ProgressReporter {
    verbosity: Verbosity,
    original_size: usize,
    last_report: Mutex<Instant>,
    min_interval: Duration,
}

impl ProgressReporter {
    pub fn new(verbosity: Verbosity, original_size: usize) -> Self {
        Self {
            verbosity,
            original_size,
            last_report: Mutex::new(Instant::now() - Duration::from_secs(10)),
            min_interval: Duration::from_secs(1),
        }
    }
}

impl ProgressCallback for ProgressReporter {
    fn on_update(&self, stats: &ProgressStats) {
        if self.verbosity == Verbosity::Quiet {
            return;
        }

        let now = Instant::now();
        {
            let mut last = self.last_report.lock().unwrap();
            if self.verbosity == Verbosity::Normal && now.duration_since(*last) < self.min_interval {
                return;
            }
            *last = now;
        }

        let percentage = if self.original_size > 0 {
            100.0 * (1.0 - stats.current_size as f64 / self.original_size as f64)
        } else {
            0.0
        };

        let _ = writeln!(
            io::stderr(),
            "bonsai: {} -> {} bytes ({:.1}% reduced) | tests: {} | reductions: {} | cache: {:.1}%",
            self.original_size,
            stats.current_size,
            percentage,
            stats.tests_run,
            stats.reductions,
            stats.cache_hit_rate * 100.0,
        );
    }

    fn on_candidate(&self, transform_name: &str, start: usize, end: usize, accepted: bool) {
        if self.verbosity != Verbosity::Verbose {
            return;
        }
        let status = if accepted { "INTERESTING" } else { "skip" };
        let _ = writeln!(
            io::stderr(),
            "  {} [{}-{}]: {}",
            transform_name, start, end, status,
        );
    }

    fn on_warning(&self, msg: &str) {
        if self.verbosity != Verbosity::Quiet {
            let _ = writeln!(io::stderr(), "bonsai: warning: {}", msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quiet_produces_no_output() {
        let reporter = ProgressReporter::new(Verbosity::Quiet, 100);
        reporter.on_update(&ProgressStats {
            original_size: 100,
            current_size: 50,
            tests_run: 10,
            reductions: 2,
            cache_hit_rate: 0.5,
        });
    }

    #[test]
    fn test_normal_mode_reports() {
        let reporter = ProgressReporter::new(Verbosity::Normal, 100);
        reporter.on_update(&ProgressStats {
            original_size: 100,
            current_size: 80,
            tests_run: 5,
            reductions: 1,
            cache_hit_rate: 0.3,
        });
    }

    #[test]
    fn test_verbose_reports_candidates() {
        let reporter = ProgressReporter::new(Verbosity::Verbose, 100);
        reporter.on_candidate("delete", 10, 20, false);
        reporter.on_candidate("delete", 10, 20, true);
    }

    #[test]
    fn test_zero_original_size() {
        let reporter = ProgressReporter::new(Verbosity::Normal, 0);
        reporter.on_update(&ProgressStats {
            original_size: 0,
            current_size: 0,
            tests_run: 0,
            reductions: 0,
            cache_hit_rate: 0.0,
        });
    }

    #[test]
    fn test_on_warning() {
        let reporter = ProgressReporter::new(Verbosity::Normal, 100);
        reporter.on_warning("test warning");
    }
}
