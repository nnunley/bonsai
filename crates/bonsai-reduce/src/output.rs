use std::io::{self, Write};
use std::path::Path;

/// Where to write the reduced output.
pub enum OutputTarget {
    /// Write to stdout.
    Stdout,
    /// Write to a file at this path.
    File(String),
}

/// Write the reduced source to the configured target.
pub fn write_output(source: &[u8], target: &OutputTarget) -> io::Result<()> {
    match target {
        OutputTarget::Stdout => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(source)?;
            handle.flush()?;
            Ok(())
        }
        OutputTarget::File(path) => {
            std::fs::write(Path::new(path), source)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_output_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.py");
        let target = OutputTarget::File(path.to_string_lossy().to_string());
        write_output(b"x = 1\n", &target).unwrap();
        let content = std::fs::read(&path).unwrap();
        assert_eq!(content, b"x = 1\n");
    }

    #[test]
    fn test_write_output_to_stdout() {
        // Just verify it doesn't panic
        let target = OutputTarget::Stdout;
        write_output(b"", &target).unwrap();
    }
}
