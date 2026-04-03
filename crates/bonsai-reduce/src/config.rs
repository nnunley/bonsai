use serde::Deserialize;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = "bonsai.toml";

// ---------------------------------------------------------------------------
// Config structs (parsed from TOML)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default, Clone, PartialEq)]
pub struct BonsaiConfig {
    #[serde(default)]
    pub reduce: ReduceConfig,
}

#[derive(Debug, Deserialize, Default, Clone, PartialEq)]
pub struct ReduceConfig {
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub test: Option<String>,
    #[serde(default)]
    pub dependencies: DependenciesConfig,
}

#[derive(Debug, Deserialize, Default, Clone, PartialEq)]
pub struct DependenciesConfig {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

// ---------------------------------------------------------------------------
// ConfigError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Parse(toml::de::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "config I/O error: {e}"),
            ConfigError::Parse(e) => write!(f, "config parse error: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(e: io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        ConfigError::Parse(e)
    }
}

// ---------------------------------------------------------------------------
// CLI merge types
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct CliOverrides {
    pub roots: Option<Vec<String>>,
    pub test: Option<String>,
    pub deps: Option<Vec<String>>,
    pub exclude_deps: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergedConfig {
    pub roots: Vec<String>,
    pub test: Option<String>,
    pub dependencies: DependencyConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DependencyConfig {
    pub paths: Vec<String>,
    pub exclude: Vec<String>,
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/// Parse a TOML string into a `BonsaiConfig`.
pub fn parse(content: &str) -> Result<BonsaiConfig, toml::de::Error> {
    toml::from_str(content)
}

/// Load a `BonsaiConfig` from the given file path.
pub fn load(path: &Path) -> Result<BonsaiConfig, ConfigError> {
    let content = fs::read_to_string(path)?;
    Ok(parse(&content)?)
}

/// Walk up from `start_path` looking for `bonsai.toml`.
///
/// If `start_path` is a file, searching begins in its parent directory.
pub fn discover(start_path: &Path) -> Option<PathBuf> {
    let dir = if start_path.is_file() {
        start_path.parent()?
    } else {
        start_path
    };

    let mut current = dir;
    loop {
        let candidate = current.join(CONFIG_FILENAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        current = current.parent()?;
    }
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

impl BonsaiConfig {
    /// Merge config file values with CLI overrides. CLI takes precedence.
    pub fn merge(&self, cli: &CliOverrides) -> MergedConfig {
        MergedConfig {
            roots: cli
                .roots
                .clone()
                .unwrap_or_else(|| self.reduce.roots.clone()),
            test: cli.test.clone().or_else(|| self.reduce.test.clone()),
            dependencies: DependencyConfig {
                paths: cli
                    .deps
                    .clone()
                    .unwrap_or_else(|| self.reduce.dependencies.paths.clone()),
                exclude: cli
                    .exclude_deps
                    .clone()
                    .unwrap_or_else(|| self.reduce.dependencies.exclude.clone()),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // -- parse tests --------------------------------------------------------

    #[test]
    fn parse_full_config() {
        let toml = r#"
[reduce]
roots = ["src/main.py"]
test = "./check.sh"

[reduce.dependencies]
paths = ["src/utils/"]
exclude = ["tests/"]
"#;
        let cfg = parse(toml).unwrap();
        assert_eq!(cfg.reduce.roots, vec!["src/main.py"]);
        assert_eq!(cfg.reduce.test, Some("./check.sh".to_string()));
        assert_eq!(cfg.reduce.dependencies.paths, vec!["src/utils/"]);
        assert_eq!(cfg.reduce.dependencies.exclude, vec!["tests/"]);
    }

    #[test]
    fn parse_empty_config() {
        let cfg = parse("").unwrap();
        assert_eq!(cfg, BonsaiConfig::default());
        assert!(cfg.reduce.roots.is_empty());
        assert!(cfg.reduce.test.is_none());
        assert!(cfg.reduce.dependencies.paths.is_empty());
        assert!(cfg.reduce.dependencies.exclude.is_empty());
    }

    #[test]
    fn parse_partial_config_roots_only() {
        let toml = r#"
[reduce]
roots = ["a.py", "b.py"]
"#;
        let cfg = parse(toml).unwrap();
        assert_eq!(cfg.reduce.roots, vec!["a.py", "b.py"]);
        assert!(cfg.reduce.test.is_none());
        assert!(cfg.reduce.dependencies.paths.is_empty());
    }

    #[test]
    fn parse_invalid_toml() {
        let result = parse("[reduce\nroots = ???");
        assert!(result.is_err());
    }

    #[test]
    fn parse_wrong_types() {
        // roots should be an array of strings, not a bare string
        let toml = r#"
[reduce]
roots = "not_an_array"
"#;
        let result = parse(toml);
        assert!(result.is_err());
    }

    // -- load tests ---------------------------------------------------------

    #[test]
    fn load_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bonsai.toml");
        fs::write(
            &path,
            r#"
[reduce]
roots = ["main.rs"]
test = "cargo test"
"#,
        )
        .unwrap();

        let cfg = load(&path).unwrap();
        assert_eq!(cfg.reduce.roots, vec!["main.rs"]);
        assert_eq!(cfg.reduce.test, Some("cargo test".to_string()));
    }

    #[test]
    fn load_missing_file() {
        let result = load(Path::new("/nonexistent/bonsai.toml"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::Io(_)));
    }

    // -- discover tests -----------------------------------------------------

    #[test]
    fn discover_in_current_dir() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILENAME);
        fs::write(&config_path, "").unwrap();

        let found = discover(dir.path()).unwrap();
        assert_eq!(found, config_path);
    }

    #[test]
    fn discover_in_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILENAME);
        fs::write(&config_path, "").unwrap();

        let child = dir.path().join("subdir");
        fs::create_dir(&child).unwrap();

        let found = discover(&child).unwrap();
        assert_eq!(found, config_path);
    }

    #[test]
    fn discover_in_grandparent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILENAME);
        fs::write(&config_path, "").unwrap();

        let grandchild = dir.path().join("a").join("b");
        fs::create_dir_all(&grandchild).unwrap();

        let found = discover(&grandchild).unwrap();
        assert_eq!(found, config_path);
    }

    #[test]
    fn discover_from_file_path() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILENAME);
        fs::write(&config_path, "").unwrap();

        let file_path = dir.path().join("src").join("main.py");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "").unwrap();

        let found = discover(&file_path).unwrap();
        assert_eq!(found, config_path);
    }

    // -- merge tests --------------------------------------------------------

    #[test]
    fn merge_cli_overrides_all() {
        let cfg = BonsaiConfig {
            reduce: ReduceConfig {
                roots: vec!["old.py".into()],
                test: Some("old.sh".into()),
                dependencies: DependenciesConfig {
                    paths: vec!["old/".into()],
                    exclude: vec!["old_exc/".into()],
                },
            },
        };

        let cli = CliOverrides {
            roots: Some(vec!["new.py".into()]),
            test: Some("new.sh".into()),
            deps: Some(vec!["new/".into()]),
            exclude_deps: Some(vec!["new_exc/".into()]),
        };

        let merged = cfg.merge(&cli);
        assert_eq!(merged.roots, vec!["new.py"]);
        assert_eq!(merged.test, Some("new.sh".into()));
        assert_eq!(merged.dependencies.paths, vec!["new/"]);
        assert_eq!(merged.dependencies.exclude, vec!["new_exc/"]);
    }

    #[test]
    fn merge_config_used_when_no_cli() {
        let cfg = BonsaiConfig {
            reduce: ReduceConfig {
                roots: vec!["main.py".into()],
                test: Some("test.sh".into()),
                dependencies: DependenciesConfig {
                    paths: vec!["lib/".into()],
                    exclude: vec!["vendor/".into()],
                },
            },
        };

        let cli = CliOverrides::default();
        let merged = cfg.merge(&cli);

        assert_eq!(merged.roots, vec!["main.py"]);
        assert_eq!(merged.test, Some("test.sh".into()));
        assert_eq!(merged.dependencies.paths, vec!["lib/"]);
        assert_eq!(merged.dependencies.exclude, vec!["vendor/"]);
    }

    #[test]
    fn merge_empty_config_empty_cli() {
        let cfg = BonsaiConfig::default();
        let cli = CliOverrides::default();
        let merged = cfg.merge(&cli);

        assert_eq!(
            merged,
            MergedConfig {
                roots: vec![],
                test: None,
                dependencies: DependencyConfig {
                    paths: vec![],
                    exclude: vec![],
                },
            }
        );
    }

    #[test]
    fn merge_partial_cli_overrides() {
        let cfg = BonsaiConfig {
            reduce: ReduceConfig {
                roots: vec!["config_root.py".into()],
                test: Some("config_test.sh".into()),
                dependencies: DependenciesConfig {
                    paths: vec!["config_path/".into()],
                    exclude: vec!["config_exc/".into()],
                },
            },
        };

        // Only override roots and exclude_deps
        let cli = CliOverrides {
            roots: Some(vec!["cli_root.py".into()]),
            test: None,
            deps: None,
            exclude_deps: Some(vec!["cli_exc/".into()]),
        };

        let merged = cfg.merge(&cli);
        assert_eq!(merged.roots, vec!["cli_root.py"]);
        assert_eq!(merged.test, Some("config_test.sh".into()));
        assert_eq!(merged.dependencies.paths, vec!["config_path/"]);
        assert_eq!(merged.dependencies.exclude, vec!["cli_exc/"]);
    }
}
