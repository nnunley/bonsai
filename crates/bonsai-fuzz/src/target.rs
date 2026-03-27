use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::NamedTempFile;
use wait_timeout::ChildExt;

/// How to pass generated input to the target program.
#[derive(Debug, Clone)]
pub enum InputMode {
    /// Pipe input to the target's stdin (default when @@ not in command).
    Stdin,
    /// Write input to a temp file, pass path as last argument.
    TempFile,
    /// Replace "@@" in the command with the temp file path.
    ArgReplace(String),
}

/// Configuration for a fuzz target program.
#[derive(Debug, Clone)]
pub struct FuzzTarget {
    /// Command and arguments. Uses Command::new — no shell interpolation.
    pub command: Vec<String>,
    /// How to pass input to the target.
    pub input_mode: InputMode,
    /// Maximum time the target is allowed to run per execution.
    pub timeout: Duration,
}

/// Result of running the target on an input.
pub struct TargetResult {
    pub exit_code: Option<i32>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
    #[cfg(unix)]
    pub signal: Option<i32>,
}

/// Error from running the target (infrastructure failure, not a test result).
#[derive(Debug)]
pub struct TargetError {
    pub message: String,
}

impl FuzzTarget {
    /// Create a new FuzzTarget. Auto-detects input mode:
    /// if any arg contains "@@", uses ArgReplace; otherwise Stdin.
    pub fn new(command: Vec<String>, timeout: Duration) -> Self {
        let input_mode = if command.iter().any(|a| a.contains("@@")) {
            InputMode::ArgReplace("@@".to_string())
        } else {
            InputMode::Stdin
        };
        Self {
            command,
            input_mode,
            timeout,
        }
    }

    /// Create with explicit input mode.
    pub fn with_input_mode(command: Vec<String>, input_mode: InputMode, timeout: Duration) -> Self {
        Self {
            command,
            input_mode,
            timeout,
        }
    }

    /// Run the target with the given input and return the result.
    pub fn run(&self, input: &[u8]) -> Result<TargetResult, TargetError> {
        match &self.input_mode {
            InputMode::Stdin => self.run_stdin(input),
            InputMode::TempFile => self.run_with_file(input, false),
            InputMode::ArgReplace(_) => self.run_with_file(input, true),
        }
    }

    fn run_stdin(&self, input: &[u8]) -> Result<TargetResult, TargetError> {
        let (program, args) = match self.command.split_first() {
            Some(pair) => pair,
            None => {
                return Err(TargetError {
                    message: "command is empty".into(),
                })
            }
        };

        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| TargetError {
                message: format!("failed to spawn '{}': {e}", program),
            })?;

        // Write input to stdin then close it
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input);
        }

        match child.wait_timeout(self.timeout) {
            Ok(Some(status)) => {
                let mut stderr = Vec::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = std::io::Read::read_to_end(&mut err, &mut stderr);
                }

                #[cfg(unix)]
                let signal = {
                    use std::os::unix::process::ExitStatusExt;
                    status.signal()
                };

                Ok(TargetResult {
                    exit_code: status.code(),
                    stderr,
                    timed_out: false,
                    #[cfg(unix)]
                    signal,
                })
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                Ok(TargetResult {
                    exit_code: None,
                    stderr: Vec::new(),
                    timed_out: true,
                    #[cfg(unix)]
                    signal: None,
                })
            }
            Err(e) => Err(TargetError {
                message: format!("failed to wait on process: {e}"),
            }),
        }
    }

    fn run_with_file(
        &self,
        input: &[u8],
        replace_placeholder: bool,
    ) -> Result<TargetResult, TargetError> {
        let mut tmp = NamedTempFile::new().map_err(|e| TargetError {
            message: format!("failed to create temp file: {e}"),
        })?;
        tmp.write_all(input).map_err(|e| TargetError {
            message: format!("failed to write temp file: {e}"),
        })?;
        tmp.flush().map_err(|e| TargetError {
            message: format!("failed to flush temp file: {e}"),
        })?;
        let tmp_path = tmp.path().to_string_lossy().to_string();

        let (program, args) = match self.command.split_first() {
            Some(pair) => pair,
            None => {
                return Err(TargetError {
                    message: "command is empty".into(),
                })
            }
        };

        let resolved_args: Vec<String> = if replace_placeholder {
            args.iter().map(|a| a.replace("@@", &tmp_path)).collect()
        } else {
            let mut a: Vec<String> = args.to_vec();
            a.push(tmp_path);
            a
        };

        let mut child = Command::new(program)
            .args(&resolved_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| TargetError {
                message: format!("failed to spawn '{}': {e}", program),
            })?;

        match child.wait_timeout(self.timeout) {
            Ok(Some(status)) => {
                let mut stderr = Vec::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = std::io::Read::read_to_end(&mut err, &mut stderr);
                }

                #[cfg(unix)]
                let signal = {
                    use std::os::unix::process::ExitStatusExt;
                    status.signal()
                };

                Ok(TargetResult {
                    exit_code: status.code(),
                    stderr,
                    timed_out: false,
                    #[cfg(unix)]
                    signal,
                })
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                Ok(TargetResult {
                    exit_code: None,
                    stderr: Vec::new(),
                    timed_out: true,
                    #[cfg(unix)]
                    signal: None,
                })
            }
            Err(e) => Err(TargetError {
                message: format!("failed to wait on process: {e}"),
            }),
        }
    }
}

impl std::fmt::Display for TargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdin_mode() {
        let target = FuzzTarget::new(vec!["cat".into()], Duration::from_secs(5));
        assert!(matches!(target.input_mode, InputMode::Stdin));
    }

    #[test]
    fn test_arg_replace_auto_detect() {
        let target = FuzzTarget::new(
            vec!["./my-compiler".into(), "@@".into()],
            Duration::from_secs(5),
        );
        assert!(matches!(target.input_mode, InputMode::ArgReplace(_)));
    }

    #[test]
    fn test_run_stdin_success() {
        let target = FuzzTarget::new(vec!["cat".into()], Duration::from_secs(5));
        let result = target.run(b"hello").unwrap();
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_run_stdin_failure() {
        let target = FuzzTarget::new(vec!["false".into()], Duration::from_secs(5));
        let result = target.run(b"hello").unwrap();
        assert_ne!(result.exit_code, Some(0));
    }

    #[test]
    fn test_run_with_arg_replace() {
        let target = FuzzTarget::new(vec!["cat".into(), "@@".into()], Duration::from_secs(5));
        let result = target.run(b"test content").unwrap();
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_run_file_mode() {
        let target = FuzzTarget::with_input_mode(
            vec!["cat".into()],
            InputMode::TempFile,
            Duration::from_secs(5),
        );
        let result = target.run(b"test content").unwrap();
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_empty_command() {
        let target = FuzzTarget::new(vec![], Duration::from_secs(5));
        let result = target.run(b"anything");
        assert!(result.is_err(), "Empty command should return Err");
    }

    #[test]
    fn test_spawn_error() {
        let target = FuzzTarget::new(vec!["/nonexistent/command".into()], Duration::from_secs(5));
        let result = target.run(b"anything");
        assert!(result.is_err(), "Nonexistent command should return Err");
    }

    #[test]
    fn test_stdin_timeout() {
        let target = FuzzTarget::new(vec!["sleep".into(), "60".into()], Duration::from_secs(1));
        let start = std::time::Instant::now();
        let result = target.run(b"anything").unwrap();
        let elapsed = start.elapsed();
        assert!(result.timed_out, "Should have timed out");
        assert!(
            elapsed < Duration::from_secs(3),
            "Should timeout quickly, took {:?}",
            elapsed
        );
    }

    #[test]
    fn test_file_mode_timeout() {
        let target = FuzzTarget::with_input_mode(
            vec!["sh".into(), "-c".into(), "sleep 60".into(), "@@".into()],
            InputMode::ArgReplace("@@".to_string()),
            Duration::from_secs(1),
        );
        let start = std::time::Instant::now();
        let result = target.run(b"anything").unwrap();
        let elapsed = start.elapsed();
        assert!(result.timed_out, "Should have timed out");
        assert!(
            elapsed < Duration::from_secs(3),
            "Should timeout quickly, took {:?}",
            elapsed
        );
    }
}
