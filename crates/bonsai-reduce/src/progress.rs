use std::io::{self, Write};
use std::time::{Duration, Instant};

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

/// Reports reduction progress to stderr.
pub struct ProgressReporter {
    verbosity: Verbosity,
    original_size: usize,
    last_report: Instant,
    min_interval: Duration,
}

impl ProgressReporter {
    pub fn new(verbosity: Verbosity, original_size: usize) -> Self {
        Self {
            verbosity,
            original_size,
            last_report: Instant::now() - Duration::from_secs(10), // ensure first report happens
            min_interval: Duration::from_secs(1),
        }
    }

    /// Report current progress. Rate-limited to ~1/sec in Normal mode.
    pub fn report(&mut self, current_size: usize, tests_run: usize, reductions: usize, cache_hit_rate: f64) {
        if self.verbosity == Verbosity::Quiet {
            return;
        }

        let now = Instant::now();
        if self.verbosity == Verbosity::Normal && now.duration_since(self.last_report) < self.min_interval {
            return;
        }
        self.last_report = now;

        let percentage = if self.original_size > 0 {
            100.0 * (1.0 - current_size as f64 / self.original_size as f64)
        } else {
            0.0
        };

        let _ = writeln!(
            io::stderr(),
            "bonsai: {} -> {} bytes ({:.1}% reduced) | tests: {} | reductions: {} | cache: {:.1}%",
            self.original_size,
            current_size,
            percentage,
            tests_run,
            reductions,
            cache_hit_rate * 100.0,
        );
    }

    /// Report a single candidate being tested (verbose mode only).
    pub fn report_candidate(&self, transform_name: &str, start_byte: usize, end_byte: usize, interesting: bool) {
        if self.verbosity != Verbosity::Verbose {
            return;
        }
        let status = if interesting { "INTERESTING" } else { "skip" };
        let _ = writeln!(
            io::stderr(),
            "  {} [{}-{}]: {}",
            transform_name, start_byte, end_byte, status,
        );
    }

    /// Report final result.
    pub fn report_final(&self, result: &crate::reducer::ReducerResult) {
        if self.verbosity == Verbosity::Quiet {
            return;
        }

        let percentage = if self.original_size > 0 {
            100.0 * (1.0 - result.source.len() as f64 / self.original_size as f64)
        } else {
            0.0
        };

        let _ = writeln!(
            io::stderr(),
            "bonsai: done. {} -> {} bytes ({:.1}% reduced) in {:.1}s | tests: {} | reductions: {} | cache: {:.1}%",
            self.original_size,
            result.source.len(),
            percentage,
            result.elapsed.as_secs_f64(),
            result.tests_run,
            result.reductions,
            result.cache_hit_rate * 100.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quiet_produces_no_output() {
        let mut reporter = ProgressReporter::new(Verbosity::Quiet, 100);
        // Should not panic, and should produce no output
        reporter.report(50, 10, 2, 0.5);
    }

    #[test]
    fn test_normal_mode_reports() {
        let mut reporter = ProgressReporter::new(Verbosity::Normal, 100);
        // First report should always go through
        reporter.report(80, 5, 1, 0.3);
        // This is mostly a smoke test — hard to capture stderr in tests
    }

    #[test]
    fn test_verbose_reports_candidates() {
        let reporter = ProgressReporter::new(Verbosity::Verbose, 100);
        reporter.report_candidate("delete", 10, 20, false);
        reporter.report_candidate("delete", 10, 20, true);
    }

    #[test]
    fn test_percentage_calculation() {
        let mut reporter = ProgressReporter::new(Verbosity::Normal, 200);
        // 100 bytes = 50% reduced — just verify it doesn't panic
        reporter.report(100, 5, 1, 0.0);
    }

    #[test]
    fn test_zero_original_size() {
        let mut reporter = ProgressReporter::new(Verbosity::Normal, 0);
        // Should not divide by zero
        reporter.report(0, 0, 0, 0.0);
    }
}
