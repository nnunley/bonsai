use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::NamedTempFile;

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

impl FuzzTarget {
    /// Create a new FuzzTarget. Auto-detects input mode:
    /// if any arg contains "@@", uses ArgReplace; otherwise Stdin.
    pub fn new(command: Vec<String>, timeout: Duration) -> Self {
        let input_mode = if command.iter().any(|a| a.contains("@@")) {
            InputMode::ArgReplace("@@".to_string())
        } else {
            InputMode::Stdin
        };
        Self { command, input_mode, timeout }
    }

    /// Create with explicit input mode.
    pub fn with_input_mode(command: Vec<String>, input_mode: InputMode, timeout: Duration) -> Self {
        Self { command, input_mode, timeout }
    }

    /// Run the target with the given input and return the result.
    pub fn run(&self, input: &[u8]) -> TargetResult {
        match &self.input_mode {
            InputMode::Stdin => self.run_stdin(input),
            InputMode::TempFile => self.run_with_file(input, false),
            InputMode::ArgReplace(_) => self.run_with_file(input, true),
        }
    }

    fn run_stdin(&self, input: &[u8]) -> TargetResult {
        let (program, args) = match self.command.split_first() {
            Some(pair) => pair,
            None => return TargetResult::error(),
        };

        let mut child = match Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return TargetResult::error(),
        };

        // Write input to stdin then close it
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input);
            // stdin is dropped here, closing the pipe
        }

        match child.wait_with_output() {
            Ok(output) => {
                #[cfg(unix)]
                let signal = {
                    use std::os::unix::process::ExitStatusExt;
                    output.status.signal()
                };

                TargetResult {
                    exit_code: output.status.code(),
                    stderr: output.stderr,
                    timed_out: false,
                    #[cfg(unix)]
                    signal,
                }
            }
            Err(_) => TargetResult::error(),
        }
    }

    fn run_with_file(&self, input: &[u8], replace_placeholder: bool) -> TargetResult {
        // Write input to temp file
        let mut tmp = match NamedTempFile::new() {
            Ok(f) => f,
            Err(_) => return TargetResult::error(),
        };
        if tmp.write_all(input).is_err() || tmp.flush().is_err() {
            return TargetResult::error();
        }
        let tmp_path = tmp.path().to_string_lossy().to_string();

        let (program, args) = match self.command.split_first() {
            Some(pair) => pair,
            None => return TargetResult::error(),
        };

        let resolved_args: Vec<String> = if replace_placeholder {
            // Replace @@ with temp file path in all args
            args.iter()
                .map(|a| a.replace("@@", &tmp_path))
                .collect()
        } else {
            // Append temp file path as last argument
            let mut a: Vec<String> = args.to_vec();
            a.push(tmp_path);
            a
        };

        let output = match Command::new(program)
            .args(&resolved_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
        {
            Ok(o) => o,
            Err(_) => return TargetResult::error(),
        };

        #[cfg(unix)]
        let signal = {
            use std::os::unix::process::ExitStatusExt;
            output.status.signal()
        };

        TargetResult {
            exit_code: output.status.code(),
            stderr: output.stderr,
            timed_out: false,
            #[cfg(unix)]
            signal,
        }
    }
}

impl TargetResult {
    fn error() -> Self {
        Self {
            exit_code: None,
            stderr: Vec::new(),
            timed_out: false,
            #[cfg(unix)]
            signal: None,
        }
    }
}

// TODO: implement bonsai_reduce::interest::InterestingnessTest for FuzzTarget once the trait is
// defined in bonsai-reduce. The impl would call self.run(input) and return true for any non-zero
// exit code or signal.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdin_mode() {
        let target = FuzzTarget::new(
            vec!["cat".into()],
            Duration::from_secs(5),
        );
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
        let target = FuzzTarget::new(
            vec!["cat".into()],
            Duration::from_secs(5),
        );
        let result = target.run(b"hello");
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_run_stdin_failure() {
        let target = FuzzTarget::new(
            vec!["false".into()],
            Duration::from_secs(5),
        );
        let result = target.run(b"hello");
        assert_ne!(result.exit_code, Some(0));
    }

    #[test]
    fn test_run_with_arg_replace() {
        // "cat @@" should read the temp file
        let target = FuzzTarget::new(
            vec!["cat".into(), "@@".into()],
            Duration::from_secs(5),
        );
        let result = target.run(b"test content");
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_run_file_mode() {
        let target = FuzzTarget::with_input_mode(
            vec!["cat".into()],
            InputMode::TempFile,
            Duration::from_secs(5),
        );
        let result = target.run(b"test content");
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_empty_command() {
        let target = FuzzTarget::new(vec![], Duration::from_secs(5));
        let result = target.run(b"anything");
        // Should handle gracefully, not panic
        assert!(result.exit_code.is_none() || result.timed_out);
    }
}
