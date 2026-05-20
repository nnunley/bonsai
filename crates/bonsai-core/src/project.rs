//! Per-project configuration loaded from `bonsai.toml`.
//!
//! Bonsai's built-in `locals.scm` queries and `grammars.toml` supertype
//! declarations are designed to work for *any* project using a given
//! language. But individual projects often define their own
//! defining-form macros — `defcreature`, `defcomponent`, `defroute`,
//! etc. — that the built-in scope analysis doesn't recognize.
//!
//! Rather than forcing projects to fork bonsai's queries (or maintain
//! ad-hoc patches), they can drop a `bonsai.toml` in their project
//! root that extends the built-in analysis with project-specific
//! knowledge.
//!
//! # Discovery
//!
//! When bonsai is asked to reduce a file at `path/to/file.ext`, it
//! walks upward from `path/to/` looking for a `bonsai.toml`. The
//! first one found wins. Walking stops at the filesystem root.
//!
//! # Configuration shape
//!
//! ```toml
//! # bonsai.toml at project root
//!
//! [clojure]
//! # Treat these as defining forms in scope analysis. Each rule says:
//! #   head-pattern    regex matched against the first symbol of a list_lit
//! #   name-position   0-indexed position of the defined symbol within the form
//! #   introduces-scope whether the form opens a new lexical scope
//! defining-forms = [
//!   { head-pattern = "^(defcreature|defitem|deftrait)$",
//!     name-position = 1,
//!     introduces-scope = true },
//! ]
//!
//! # Optional: add supertypes on top of grammars.toml's declarations.
//! # Same soundness contract — see ConfigSupertypeProvider docs.
//! [clojure.supertypes]
//! # _form = ["list_lit", "vec_lit"]
//! ```
//!
//! Top-level table keys are language names matching bonsai's
//! registered languages (`clojure`, `python`, `rust`, etc.). Each
//! section is optional.
//!
//! # What this module provides
//!
//! Loading + parsing only. The actual application of project config
//! to scope analysis / supertype lookup lives in the consumer modules
//! (e.g., the CLI wires the project's extra defining-forms into the
//! locals.scm passed to ScopeAnalysis).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The name of the config file bonsai looks for.
pub const PROJECT_CONFIG_FILENAME: &str = "bonsai.toml";

/// A parsed `bonsai.toml`. The top-level structure is a map from
/// language-name to per-language config.
#[derive(Debug, Default, Deserialize)]
pub struct ProjectConfig {
    /// Per-language sections, keyed by language name (e.g. "clojure").
    /// Languages without a section in the file get `LanguageConfig::default()`.
    #[serde(flatten)]
    pub languages: BTreeMap<String, LanguageConfig>,
}

/// Per-language extensions to bonsai's built-in analysis.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LanguageConfig {
    /// Extra defining-form rules. These get translated into additional
    /// tree-sitter query patterns that augment the language's built-in
    /// `locals.scm`.
    #[serde(default)]
    pub defining_forms: Vec<DefiningFormRule>,

    /// Extra supertype declarations layered on top of `grammars.toml`.
    /// Same soundness contract as `[language.supertypes]` in
    /// grammars.toml: only declare supertypes whose subtypes are
    /// grammatically interchangeable in every syntactic position.
    #[serde(default)]
    pub supertypes: BTreeMap<String, Vec<String>>,
}

/// A defining-form rule: tells bonsai to recognize `(head-pattern
/// name body)` as a definition of `name` (and optionally a scope).
///
/// Currently shaped for Clojure-style list-headed forms. The pattern
/// language could be extended later for non-Lisp grammars.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DefiningFormRule {
    /// Regex matched against the form's head symbol name. Use anchors
    /// (`^...$`) to avoid matching substrings.
    pub head_pattern: String,
    /// 0-indexed position of the named symbol within the form's
    /// children. For `(defn foo [args] body)`, position 1 names `foo`.
    pub name_position: usize,
    /// If true, the matched form is also treated as a new lexical
    /// scope. Use for forms whose subsequent siblings should be able
    /// to reference the defined name (function bodies, etc.).
    #[serde(default)]
    pub introduces_scope: bool,
}

/// Discover and load a `bonsai.toml` by walking upward from `start_dir`.
///
/// Returns `Ok(None)` if no config file is found (this is normal —
/// not every project wants project-level customization). Returns
/// `Err` only on filesystem or parse errors.
///
/// Walking stops at filesystem root. Symlinks are followed.
pub fn discover_and_load(
    start_dir: &Path,
) -> Result<Option<(PathBuf, ProjectConfig)>, ConfigError> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join(PROJECT_CONFIG_FILENAME);
        if candidate.exists() {
            let config = load(&candidate)?;
            return Ok(Some((candidate, config)));
        }
        match dir.parent() {
            Some(parent) => {
                let parent = parent.to_path_buf();
                if parent == dir {
                    // We're at the root.
                    return Ok(None);
                }
                dir = parent;
            }
            None => return Ok(None),
        }
    }
}

/// Load a `bonsai.toml` from an explicit path.
pub fn load(path: &Path) -> Result<ProjectConfig, ConfigError> {
    let content = fs::read_to_string(path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    toml::from_str(&content).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

/// Errors from loading project configuration. We carry the file path
/// in every variant so callers can produce useful error messages.
#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io { path, source } => {
                write!(f, "failed to read {}: {}", path.display(), source)
            }
            ConfigError::Parse { path, message } => {
                write!(f, "failed to parse {}: {}", path.display(), message)
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io { source, .. } => Some(source),
            ConfigError::Parse { .. } => None,
        }
    }
}

/// Translate a `DefiningFormRule` into a tree-sitter query fragment
/// suitable for appending to a Clojure-style `locals.scm`.
///
/// The generated fragment uses tree-sitter-clojure's `list_lit` /
/// `sym_lit` / `sym_name` node shape. For other Lisp grammars (or
/// non-Lisp grammars), the caller is responsible for using a different
/// translator — or for keeping defining-forms empty.
///
/// Returns the SCM source text. The text is appended verbatim to the
/// language's embedded `locals.scm` before being passed to
/// `ScopeAnalysis::from_tree`.
///
/// Example output for `{head_pattern: "^def(creature|item)$",
/// name_position: 1, introduces_scope: true}`:
///
/// ```text
/// ;; bonsai.toml defining-form rule
/// ((list_lit
///    .
///    (sym_lit (sym_name) @_bonsai-head)
///    .
///    (sym_lit (sym_name) @local.definition))
///  (#match? @_bonsai-head "^def(creature|item)$"))
///
/// ((list_lit
///    .
///    (sym_lit (sym_name) @_bonsai-head)
///    .
///    _*)
///  (#match? @_bonsai-head "^def(creature|item)$"))
///  @local.scope
/// ```
///
/// (Two patterns: one captures the definition, one introduces the scope
/// when `introduces_scope` is true. The scope pattern is omitted
/// otherwise.)
pub fn rule_to_clojure_scm(rule: &DefiningFormRule) -> String {
    let mut out = String::new();
    out.push_str(";; bonsai.toml defining-form rule\n");

    // Definition pattern: target the symbol at name_position.
    out.push_str("((list_lit\n");
    out.push_str("   .\n");
    out.push_str("   (sym_lit (sym_name) @_bonsai-head)\n");
    // Skip name_position - 1 anchor positions before the captured
    // name. name_position == 0 means the head IS the name (unusual for
    // a defining form; this fall-through still emits a sensible pattern).
    for _ in 0..rule.name_position.saturating_sub(1) {
        out.push_str("   .\n");
        out.push_str("   _\n");
    }
    out.push_str("   .\n");
    out.push_str("   (sym_lit (sym_name) @local.definition))\n");
    out.push_str(&format!(
        " (#match? @_bonsai-head {:?}))\n\n",
        rule.head_pattern
    ));

    if rule.introduces_scope {
        // Scope pattern: any form whose head matches the rule.
        out.push_str("((list_lit\n");
        out.push_str("   .\n");
        out.push_str("   (sym_lit (sym_name) @_bonsai-head-scope)\n");
        out.push_str("   _*)\n");
        out.push_str(&format!(
            " (#match? @_bonsai-head-scope {:?}))\n",
            rule.head_pattern
        ));
        out.push_str(" @local.scope\n\n");
    }

    out
}

/// Build a merged locals.scm by appending project-config-derived
/// patterns to the language's embedded one. Returns `None` if the
/// project config contributes nothing.
pub fn merge_locals_scm(base_locals: &str, rules: &[DefiningFormRule]) -> Option<String> {
    if rules.is_empty() {
        return None;
    }
    let mut merged = base_locals.to_string();
    if !merged.ends_with('\n') {
        merged.push('\n');
    }
    merged.push_str("\n;; ---- bonsai.toml extensions ----\n");
    for rule in rules {
        merged.push_str(&rule_to_clojure_scm(rule));
    }
    Some(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_toml(dir: &Path, content: &str) -> PathBuf {
        let path = dir.join(PROJECT_CONFIG_FILENAME);
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn load_empty_config() {
        let dir = tempdir();
        let path = write_temp_toml(&dir, "");
        let config = load(&path).unwrap();
        assert!(config.languages.is_empty());
    }

    #[test]
    fn load_clojure_defining_forms() {
        let dir = tempdir();
        let path = write_temp_toml(
            &dir,
            r#"
[clojure]
defining-forms = [
  { head-pattern = "^(defcreature|defitem)$", name-position = 1, introduces-scope = true },
]
"#,
        );
        let config = load(&path).unwrap();
        let clojure = config.languages.get("clojure").expect("clojure section");
        assert_eq!(clojure.defining_forms.len(), 1);
        let rule = &clojure.defining_forms[0];
        assert_eq!(rule.head_pattern, "^(defcreature|defitem)$");
        assert_eq!(rule.name_position, 1);
        assert!(rule.introduces_scope);
    }

    #[test]
    fn load_clojure_supertypes() {
        let dir = tempdir();
        let path = write_temp_toml(
            &dir,
            r#"
[clojure.supertypes]
_atom = ["sym_lit", "kwd_lit"]
"#,
        );
        let config = load(&path).unwrap();
        let clojure = config.languages.get("clojure").expect("clojure section");
        assert_eq!(
            clojure.supertypes.get("_atom").unwrap(),
            &vec!["sym_lit".to_string(), "kwd_lit".to_string()]
        );
    }

    #[test]
    fn discover_walks_upward() {
        let root = tempdir();
        // Put the toml at root, ask from a deep subdir.
        write_temp_toml(&root, "[clojure]\n");
        let deep = root.join("a").join("b").join("c");
        fs::create_dir_all(&deep).unwrap();

        let result = discover_and_load(&deep).unwrap();
        let (found_path, _config) = result.expect("should find config");
        assert_eq!(found_path, root.join(PROJECT_CONFIG_FILENAME));
    }

    #[test]
    fn discover_returns_none_when_no_config() {
        let root = tempdir();
        let result = discover_and_load(&root).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn parse_error_reports_path() {
        let dir = tempdir();
        let path = write_temp_toml(&dir, "this is = = not valid toml [[[");
        let err = load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        assert!(format!("{}", err).contains(PROJECT_CONFIG_FILENAME));
    }

    #[test]
    fn rule_to_clojure_scm_definition_pattern() {
        let rule = DefiningFormRule {
            head_pattern: "^defthing$".to_string(),
            name_position: 1,
            introduces_scope: false,
        };
        let scm = rule_to_clojure_scm(&rule);
        // The definition pattern must capture @local.definition.
        assert!(scm.contains("@local.definition"));
        // The head pattern must appear in a #match? predicate.
        assert!(scm.contains("#match?"));
        assert!(scm.contains("^defthing$"));
        // No scope pattern when introduces_scope is false.
        assert!(!scm.contains("@local.scope"));
    }

    #[test]
    fn rule_to_clojure_scm_includes_scope_when_requested() {
        let rule = DefiningFormRule {
            head_pattern: "^defscope$".to_string(),
            name_position: 1,
            introduces_scope: true,
        };
        let scm = rule_to_clojure_scm(&rule);
        assert!(scm.contains("@local.definition"));
        assert!(scm.contains("@local.scope"));
    }

    #[test]
    fn merge_locals_scm_appends_when_rules_present() {
        let base = ";; base locals\n(sym_lit) @local.reference\n";
        let rules = vec![DefiningFormRule {
            head_pattern: "^defx$".to_string(),
            name_position: 1,
            introduces_scope: false,
        }];
        let merged = merge_locals_scm(base, &rules).expect("should produce merged scm");
        assert!(merged.starts_with(";; base locals"));
        assert!(merged.contains("bonsai.toml extensions"));
        assert!(merged.contains("^defx$"));
    }

    #[test]
    fn merge_locals_scm_returns_none_when_no_rules() {
        let merged = merge_locals_scm(";; base\n", &[]);
        assert!(merged.is_none());
    }

    // Test helper: create a temp dir. Returning PathBuf rather than
    // tempfile::TempDir means we don't get auto-cleanup, but tests
    // use unique enough paths that it's not a worry for this suite.
    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("bonsai-project-test-{}-{}", pid, n));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
