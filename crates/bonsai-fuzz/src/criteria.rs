use regex::Regex;

/// Criteria for determining whether a target execution result is "interesting."
/// Multiple criteria can be combined — any match makes the result interesting.
///
/// ```
/// use bonsai_fuzz::criteria::InterestCriteria;
/// use bonsai_fuzz::target::TargetResult;
///
/// // Build custom criteria with the builder pattern
/// let criteria = InterestCriteria::none()
///     .with_any_nonzero_exit()
///     .with_timeout();
///
/// let timeout_result = TargetResult {
///     exit_code: None,
///     stderr: Vec::new(),
///     timed_out: true,
///     #[cfg(unix)]
///     signal: None,
/// };
/// assert!(criteria.is_interesting(&timeout_result));
/// ```
#[derive(Clone)]
pub struct InterestCriteria {
    checks: Vec<InterestCheck>,
}

#[derive(Clone)]
enum InterestCheck {
    /// Any non-zero exit code.
    AnyNonZeroExit,
    /// Specific exit code.
    ExitCode(i32),
    /// Target was killed by any signal (Unix only).
    AnySignal,
    /// Stderr matches this regex.
    StderrPattern(Regex),
    /// Target timed out.
    Timeout,
}

impl InterestCriteria {
    /// Create criteria that matches any non-zero exit code or signal.
    pub fn any_crash() -> Self {
        Self {
            checks: vec![InterestCheck::AnyNonZeroExit, InterestCheck::AnySignal],
        }
    }

    /// Create empty criteria (nothing is interesting). Use builder methods to add checks.
    pub fn none() -> Self {
        Self { checks: vec![] }
    }

    /// Add: any non-zero exit code is interesting.
    pub fn with_any_nonzero_exit(mut self) -> Self {
        self.checks.push(InterestCheck::AnyNonZeroExit);
        self
    }

    /// Add: specific exit code is interesting.
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.checks.push(InterestCheck::ExitCode(code));
        self
    }

    /// Add: any signal kill is interesting (Unix).
    pub fn with_any_signal(mut self) -> Self {
        self.checks.push(InterestCheck::AnySignal);
        self
    }

    /// Add: stderr matching this regex is interesting.
    pub fn with_stderr_pattern(mut self, pattern: Regex) -> Self {
        self.checks.push(InterestCheck::StderrPattern(pattern));
        self
    }

    /// Add: timeout is interesting.
    pub fn with_timeout(mut self) -> Self {
        self.checks.push(InterestCheck::Timeout);
        self
    }

    /// Check whether a target result matches any of the criteria.
    pub fn is_interesting(&self, result: &super::target::TargetResult) -> bool {
        self.checks.iter().any(|check| match check {
            InterestCheck::AnyNonZeroExit => {
                matches!(result.exit_code, Some(code) if code != 0)
            }
            InterestCheck::ExitCode(expected) => result.exit_code == Some(*expected),
            InterestCheck::AnySignal => {
                #[cfg(unix)]
                {
                    result.signal.is_some()
                }
                #[cfg(not(unix))]
                {
                    false
                }
            }
            InterestCheck::StderrPattern(pattern) => {
                let stderr_str = String::from_utf8_lossy(&result.stderr);
                pattern.is_match(&stderr_str)
            }
            InterestCheck::Timeout => result.timed_out,
        })
    }

    /// Returns true if no checks are configured.
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::TargetResult;

    fn make_result(exit_code: Option<i32>, stderr: &[u8], timed_out: bool) -> TargetResult {
        TargetResult {
            exit_code,
            stderr: stderr.to_vec(),
            timed_out,
            #[cfg(unix)]
            signal: None,
        }
    }

    #[cfg(unix)]
    fn make_signal_result(signal: i32) -> TargetResult {
        TargetResult {
            exit_code: None,
            stderr: Vec::new(),
            timed_out: false,
            signal: Some(signal),
        }
    }

    #[test]
    fn test_any_nonzero_exit() {
        let criteria = InterestCriteria::none().with_any_nonzero_exit();
        assert!(!criteria.is_interesting(&make_result(Some(0), b"", false)));
        assert!(criteria.is_interesting(&make_result(Some(1), b"", false)));
        assert!(criteria.is_interesting(&make_result(Some(139), b"", false)));
    }

    #[test]
    fn test_specific_exit_code() {
        let criteria = InterestCriteria::none().with_exit_code(139);
        assert!(!criteria.is_interesting(&make_result(Some(0), b"", false)));
        assert!(!criteria.is_interesting(&make_result(Some(1), b"", false)));
        assert!(criteria.is_interesting(&make_result(Some(139), b"", false)));
    }

    #[cfg(unix)]
    #[test]
    fn test_any_signal() {
        let criteria = InterestCriteria::none().with_any_signal();
        assert!(!criteria.is_interesting(&make_result(Some(0), b"", false)));
        assert!(criteria.is_interesting(&make_signal_result(11))); // SIGSEGV
    }

    #[test]
    fn test_stderr_pattern() {
        let criteria = InterestCriteria::none().with_stderr_pattern(Regex::new("panic").unwrap());
        assert!(!criteria.is_interesting(&make_result(Some(1), b"error occurred", false)));
        assert!(criteria.is_interesting(&make_result(Some(1), b"thread panicked", false)));
    }

    #[test]
    fn test_timeout() {
        let criteria = InterestCriteria::none().with_timeout();
        assert!(!criteria.is_interesting(&make_result(Some(0), b"", false)));
        assert!(criteria.is_interesting(&make_result(None, b"", true)));
    }

    #[test]
    fn test_combined_criteria() {
        let criteria = InterestCriteria::none()
            .with_any_nonzero_exit()
            .with_stderr_pattern(Regex::new("panic").unwrap());

        // Matches exit code criterion
        assert!(criteria.is_interesting(&make_result(Some(1), b"normal error", false)));
        // Matches stderr criterion
        assert!(criteria.is_interesting(&make_result(Some(0), b"thread panicked", false)));
        // Matches neither
        assert!(!criteria.is_interesting(&make_result(Some(0), b"all good", false)));
    }

    #[test]
    fn test_any_crash_preset() {
        let criteria = InterestCriteria::any_crash();
        assert!(criteria.is_interesting(&make_result(Some(1), b"", false)));
        assert!(!criteria.is_interesting(&make_result(Some(0), b"", false)));
    }

    #[test]
    fn test_empty_criteria() {
        let criteria = InterestCriteria::none();
        assert!(criteria.is_empty());
        assert!(!criteria.is_interesting(&make_result(Some(1), b"panic", true)));
    }
}
