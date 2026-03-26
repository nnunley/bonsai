use std::io::Write;
use std::process::Command;
use std::time::Duration;
use tempfile::NamedTempFile;
use wait_timeout::ChildExt;

/// Trait for determining whether a candidate input is "interesting"
/// (e.g., still triggers a bug after reduction).
pub trait InterestingnessTest: Send + Sync {
    fn is_interesting(&self, input: &[u8]) -> bool;
}

/// Shell-command-based interestingness test.
/// Writes input to a temp file, runs the command with the file path as an argument,
/// and treats exit code 0 as interesting.
///
/// Uses Command::new with args array — no shell interpolation.
pub struct ShellTest {
    /// Command and arguments. The temp file path is appended as the last argument.
    command: Vec<String>,
    /// Maximum time the command is allowed to run.
    timeout: Duration,
}

impl ShellTest {
    pub fn new(command: Vec<String>, timeout: Duration) -> Self {
        Self { command, timeout }
    }
}

impl InterestingnessTest for ShellTest {
    fn is_interesting(&self, input: &[u8]) -> bool {
        // Write input to a temp file
        let mut tmp = match NamedTempFile::new() {
            Ok(f) => f,
            Err(_) => return false,
        };
        if tmp.write_all(input).is_err() {
            return false;
        }
        if tmp.flush().is_err() {
            return false;
        }

        let tmp_path = tmp.path().to_string_lossy().to_string();

        // Build command: first element is the program, rest are args, temp path appended
        let (program, args) = match self.command.split_first() {
            Some((p, a)) => (p, a),
            None => return false,
        };

        let result = Command::new(program)
            .args(args)
            .arg(&tmp_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();

        let mut child = match result {
            Ok(c) => c,
            Err(_) => return false,
        };

        // Wait with timeout
        match child.wait_timeout(self.timeout) {
            Ok(Some(status)) => status.success(),
            Ok(None) => {
                // Timeout — kill the process
                let _ = child.kill();
                let _ = child.wait();
                false
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_test_success() {
        // "grep -q hello" should exit 0 when input contains "hello"
        let test = ShellTest::new(
            vec!["grep".into(), "-q".into(), "hello".into()],
            Duration::from_secs(5),
        );
        assert!(test.is_interesting(b"hello world\n"));
    }

    #[test]
    fn test_shell_test_failure() {
        // "grep -q hello" should exit non-zero when input doesn't contain "hello"
        let test = ShellTest::new(
            vec!["grep".into(), "-q".into(), "hello".into()],
            Duration::from_secs(5),
        );
        assert!(!test.is_interesting(b"goodbye world\n"));
    }

    #[test]
    fn test_shell_test_timeout() {
        // sh -c "sleep 60" <temppath> — sh treats the temp path as $0, sleep still runs
        let test = ShellTest::new(
            vec!["sh".into(), "-c".into(), "sleep 60".into()],
            Duration::from_secs(1),
        );
        let start = std::time::Instant::now();
        let result = test.is_interesting(b"anything");
        let elapsed = start.elapsed();
        assert!(!result, "Should not be interesting (timeout)");
        assert!(elapsed < Duration::from_secs(3), "Should timeout quickly");
    }

    #[test]
    fn test_shell_test_with_spaces_in_path() {
        // Verify Command::new is used (not shell interpolation)
        let test = ShellTest::new(
            vec!["grep".into(), "-q".into(), "hello".into()],
            Duration::from_secs(5),
        );
        assert!(test.is_interesting(b"hello world\n"));
    }

    #[test]
    fn test_empty_command() {
        let test = ShellTest::new(vec![], Duration::from_secs(5));
        assert!(!test.is_interesting(b"anything"));
    }
}
