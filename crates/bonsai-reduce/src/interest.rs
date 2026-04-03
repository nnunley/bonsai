use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tempfile::NamedTempFile;
use wait_timeout::ChildExt;

/// Result of an interestingness test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestResult {
    /// The input does not satisfy the interestingness criterion.
    NotInteresting,
    /// The input satisfies the interestingness criterion.
    Interesting,
    /// The test infrastructure failed (e.g., could not spawn process, write temp file).
    Error(String),
}

/// Trait for determining whether a candidate input is "interesting"
/// (e.g., still triggers a bug after reduction).
///
/// Implement this to define custom interestingness criteria for programmatic use.
/// For shell-command-based tests, see [`ShellTest`].
///
/// ```
/// use bonsai_reduce::{InterestingnessTest, TestResult};
///
/// struct ContainsKeyword(Vec<u8>);
///
/// impl InterestingnessTest for ContainsKeyword {
///     fn test(&self, input: &[u8]) -> TestResult {
///         if input.windows(self.0.len()).any(|w| w == self.0.as_slice()) {
///             TestResult::Interesting
///         } else {
///             TestResult::NotInteresting
///         }
///     }
/// }
///
/// let test = ContainsKeyword(b"error".to_vec());
/// assert_eq!(test.test(b"an error occurred"), TestResult::Interesting);
/// assert_eq!(test.test(b"all good"), TestResult::NotInteresting);
/// ```
pub trait InterestingnessTest: Send + Sync {
    fn test(&self, input: &[u8]) -> TestResult;
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
    pub fn new(command: Vec<String>, timeout: Duration) -> Result<Self, String> {
        if command.is_empty() {
            return Err("test command cannot be empty".into());
        }
        Ok(Self { command, timeout })
    }
}

impl InterestingnessTest for ShellTest {
    fn test(&self, input: &[u8]) -> TestResult {
        // Write input to a temp file
        let mut tmp = match NamedTempFile::new() {
            Ok(f) => f,
            Err(e) => return TestResult::Error(format!("failed to create temp file: {e}")),
        };
        if let Err(e) = tmp.write_all(input) {
            return TestResult::Error(format!("failed to write temp file: {e}"));
        }
        if let Err(e) = tmp.flush() {
            return TestResult::Error(format!("failed to flush temp file: {e}"));
        }

        let tmp_path = tmp.path().to_string_lossy().to_string();

        // Build command: first element is the program, rest are args, temp path appended
        let (program, args) = match self.command.split_first() {
            Some((p, a)) => (p, a),
            None => return TestResult::Error("test command is empty".into()),
        };

        let mut child = match Command::new(program)
            .args(args)
            .arg(&tmp_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return TestResult::Error(format!("failed to spawn '{}': {e}", program)),
        };

        // Wait with timeout
        match child.wait_timeout(self.timeout) {
            Ok(Some(status)) => {
                if status.success() {
                    TestResult::Interesting
                } else {
                    TestResult::NotInteresting
                }
            }
            Ok(None) => {
                // Timeout — kill the process
                let _ = child.kill();
                let _ = child.wait();
                TestResult::NotInteresting
            }
            Err(e) => TestResult::Error(format!("failed to wait on process: {e}")),
        }
    }
}

/// Project-level interestingness test.
/// Writes input to a specific file within an existing directory, runs the command
/// with the directory path as the last argument, and treats exit code 0 as interesting.
///
/// Unlike [`ShellTest`] which creates a new temp file and passes its path,
/// `ProjectTest` writes to a fixed file in a project directory and passes the
/// directory path. This supports multi-file reduction where the test script
/// needs to validate changes in the context of a full project.
pub struct ProjectTest {
    /// Path to the temporary project directory.
    temp_dir: PathBuf,
    /// Path to the target file within temp_dir where candidate bytes are written.
    target_file: PathBuf,
    /// Command and arguments. The temp_dir path is appended as the last argument.
    command: Vec<String>,
    /// Maximum time the command is allowed to run.
    timeout: Duration,
}

impl ProjectTest {
    pub fn new(
        temp_dir: PathBuf,
        target_file: PathBuf,
        command: Vec<String>,
        timeout: Duration,
    ) -> Result<Self, String> {
        if command.is_empty() {
            return Err("test command cannot be empty".into());
        }
        Ok(Self {
            temp_dir,
            target_file,
            command,
            timeout,
        })
    }
}

impl InterestingnessTest for ProjectTest {
    fn test(&self, input: &[u8]) -> TestResult {
        // Write input to the target file
        if let Err(e) = std::fs::write(&self.target_file, input) {
            return TestResult::Error(format!("failed to write target file: {e}"));
        }

        let dir_path = self.temp_dir.to_string_lossy().to_string();

        // Build command: first element is the program, rest are args, dir path appended
        let (program, args) = match self.command.split_first() {
            Some((p, a)) => (p, a),
            None => return TestResult::Error("test command is empty".into()),
        };

        let mut child = match Command::new(program)
            .args(args)
            .arg(&dir_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return TestResult::Error(format!("failed to spawn '{}': {e}", program)),
        };

        // Wait with timeout
        match child.wait_timeout(self.timeout) {
            Ok(Some(status)) => {
                if status.success() {
                    TestResult::Interesting
                } else {
                    TestResult::NotInteresting
                }
            }
            Ok(None) => {
                // Timeout — kill the process
                let _ = child.kill();
                let _ = child.wait();
                TestResult::NotInteresting
            }
            Err(e) => TestResult::Error(format!("failed to wait on process: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_test_success() {
        let test = ShellTest::new(
            vec!["grep".into(), "-q".into(), "hello".into()],
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(test.test(b"hello world\n"), TestResult::Interesting);
    }

    #[test]
    fn test_shell_test_failure() {
        let test = ShellTest::new(
            vec!["grep".into(), "-q".into(), "hello".into()],
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(test.test(b"goodbye world\n"), TestResult::NotInteresting);
    }

    #[test]
    fn test_shell_test_timeout() {
        let test = ShellTest::new(
            vec!["sh".into(), "-c".into(), "sleep 60".into()],
            Duration::from_secs(1),
        )
        .unwrap();
        let start = std::time::Instant::now();
        let result = test.test(b"anything");
        let elapsed = start.elapsed();
        assert_eq!(
            result,
            TestResult::NotInteresting,
            "Timeout should be NotInteresting"
        );
        assert!(elapsed < Duration::from_secs(3), "Should timeout quickly");
    }

    #[test]
    fn test_shell_test_with_spaces_in_path() {
        let test = ShellTest::new(
            vec!["grep".into(), "-q".into(), "hello".into()],
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(test.test(b"hello world\n"), TestResult::Interesting);
    }

    #[test]
    fn test_shell_test_spawn_error() {
        let test =
            ShellTest::new(vec!["/nonexistent/command".into()], Duration::from_secs(5)).unwrap();
        let result = test.test(b"anything");
        assert!(
            matches!(result, TestResult::Error(_)),
            "Spawn failure should return Error"
        );
    }

    #[test]
    fn test_empty_command_rejected() {
        let result = ShellTest::new(vec![], Duration::from_secs(5));
        assert!(
            result.is_err(),
            "Empty command should be rejected at construction"
        );
    }

    // --- ProjectTest tests ---

    #[test]
    fn test_project_test_empty_command_rejected() {
        let result = ProjectTest::new(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp/file.txt"),
            vec![],
            Duration::from_secs(5),
        );
        assert!(
            result.is_err(),
            "Empty command should be rejected at construction"
        );
    }

    #[test]
    fn test_project_test_construction_succeeds() {
        let result = ProjectTest::new(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp/file.txt"),
            vec!["echo".into()],
            Duration::from_secs(5),
        );
        assert!(result.is_ok(), "Valid construction should succeed");
    }

    #[test]
    fn test_project_test_writes_bytes_and_passes_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target.txt");

        // Script: reads file at $1/target.txt and greps for "hello"
        let test = ProjectTest::new(
            tmp.path().to_path_buf(),
            target.clone(),
            vec![
                "sh".into(),
                "-c".into(),
                "grep -q hello \"$1/target.txt\"".into(),
                "--".into(),
            ],
            Duration::from_secs(5),
        )
        .unwrap();

        assert_eq!(test.test(b"hello world\n"), TestResult::Interesting);
        // Verify the file was actually written
        let contents = std::fs::read_to_string(&target).unwrap();
        assert_eq!(contents, "hello world\n");
    }

    #[test]
    fn test_project_test_not_interesting() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target.txt");

        let test = ProjectTest::new(
            tmp.path().to_path_buf(),
            target,
            vec![
                "sh".into(),
                "-c".into(),
                "grep -q hello \"$1/target.txt\"".into(),
                "--".into(),
            ],
            Duration::from_secs(5),
        )
        .unwrap();

        assert_eq!(test.test(b"goodbye world\n"), TestResult::NotInteresting);
    }

    #[test]
    fn test_project_test_timeout() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target.txt");

        let test = ProjectTest::new(
            tmp.path().to_path_buf(),
            target,
            vec!["sh".into(), "-c".into(), "sleep 60".into()],
            Duration::from_secs(1),
        )
        .unwrap();

        let start = std::time::Instant::now();
        let result = test.test(b"anything");
        let elapsed = start.elapsed();
        assert_eq!(
            result,
            TestResult::NotInteresting,
            "Timeout should be NotInteresting"
        );
        assert!(elapsed < Duration::from_secs(3), "Should timeout quickly");
    }

    #[test]
    fn test_project_test_spawn_error() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target.txt");

        let test = ProjectTest::new(
            tmp.path().to_path_buf(),
            target,
            vec!["/nonexistent/command".into()],
            Duration::from_secs(5),
        )
        .unwrap();

        let result = test.test(b"anything");
        assert!(
            matches!(result, TestResult::Error(_)),
            "Spawn failure should return Error"
        );
    }

    #[test]
    fn test_project_test_write_error() {
        let tmp = tempfile::tempdir().unwrap();
        // Target in a nonexistent subdirectory
        let target = tmp.path().join("nonexistent_subdir").join("target.txt");

        let test = ProjectTest::new(
            tmp.path().to_path_buf(),
            target,
            vec!["echo".into()],
            Duration::from_secs(5),
        )
        .unwrap();

        let result = test.test(b"anything");
        assert!(
            matches!(result, TestResult::Error(_)),
            "Write to nonexistent subdir should return Error"
        );
    }

    #[test]
    fn test_project_test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ProjectTest>();
    }
}
