use std::collections::HashMap;
use tree_sitter::Tree;

use crate::parse;

/// A proposed replacement: replace bytes [start_byte..end_byte] with new_bytes.
#[derive(Debug, Clone)]
pub struct Replacement {
    pub start_byte: usize,
    pub end_byte: usize,
    pub new_bytes: Vec<u8>,
}

/// Apply a replacement to source bytes, returning the new source.
///
/// ```
/// use bonsai_core::validity::{Replacement, apply_replacement};
///
/// let source = b"hello world";
/// let r = Replacement { start_byte: 5, end_byte: 11, new_bytes: b" rust".to_vec() };
/// assert_eq!(apply_replacement(source, &r), b"hello rust");
///
/// // Deletion: replace with empty bytes
/// let r = Replacement { start_byte: 5, end_byte: 11, new_bytes: vec![] };
/// assert_eq!(apply_replacement(source, &r), b"hello");
/// ```
pub fn apply_replacement(source: &[u8], replacement: &Replacement) -> Vec<u8> {
    let mut result = Vec::with_capacity(
        source.len() - (replacement.end_byte - replacement.start_byte)
            + replacement.new_bytes.len(),
    );
    result.extend_from_slice(&source[..replacement.start_byte]);
    result.extend_from_slice(&replacement.new_bytes);
    result.extend_from_slice(&source[replacement.end_byte..]);
    result
}

/// A multiset of error signatures in a parse tree.
/// Used to distinguish pre-existing errors from new ones.
/// Errors are identified by (node_kind, source_text) rather than byte position,
/// so they remain stable when byte offsets shift due to earlier deletions.
/// Uses counts to detect when the same error signature appears more times than before.
#[derive(Debug, Clone)]
pub struct ErrorSet {
    /// Count of each (node_kind, source_text) error signature.
    errors: HashMap<(String, Vec<u8>), usize>,
}

impl ErrorSet {
    /// Collect all ERROR and MISSING nodes from a tree.
    pub fn from_tree(tree: &Tree, source: &[u8]) -> Self {
        let mut errors = HashMap::new();
        let mut cursor = tree.root_node().walk();
        collect_errors_recursive(&mut cursor, source, &mut errors);
        Self { errors }
    }

    /// Check if this set contains any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check if a tree has any NEW errors not in this set.
    /// An error is "new" if its (kind, text) signature appears more times
    /// in the new tree than in this set.
    pub fn has_new_errors(&self, tree: &Tree, source: &[u8]) -> bool {
        let mut new_errors = HashMap::new();
        let mut cursor = tree.root_node().walk();
        collect_errors_recursive(&mut cursor, source, &mut new_errors);

        for (key, &new_count) in &new_errors {
            let known_count = self.errors.get(key).copied().unwrap_or(0);
            if new_count > known_count {
                return true;
            }
        }
        false
    }

    /// Number of distinct error signatures.
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

fn collect_errors_recursive(
    cursor: &mut tree_sitter::TreeCursor,
    source: &[u8],
    errors: &mut HashMap<(String, Vec<u8>), usize>,
) {
    let node = cursor.node();
    if node.is_error() || node.is_missing() {
        let text = source
            .get(node.start_byte()..node.end_byte())
            .unwrap_or_default()
            .to_vec();
        *errors.entry((node.kind().to_string(), text)).or_insert(0) += 1;
    }
    if cursor.goto_first_child() {
        loop {
            collect_errors_recursive(cursor, source, errors);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

/// Check if a tree has any ERROR or MISSING nodes at all.
pub fn tree_has_errors(tree: &Tree) -> bool {
    let mut cursor = tree.root_node().walk();
    has_error_recursive(&mut cursor)
}

fn has_error_recursive(cursor: &mut tree_sitter::TreeCursor) -> bool {
    let node = cursor.node();
    if node.is_error() || node.is_missing() {
        return true;
    }
    if cursor.goto_first_child() {
        loop {
            if has_error_recursive(cursor) {
                cursor.goto_parent();
                return true;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    false
}

/// Check if applying a replacement produces a valid result.
///
/// - If `initial_errors` is None (strict mode), rejects ANY error in the result.
/// - If `initial_errors` is Some, only rejects NEW errors not in the initial set.
///
/// Returns Some(new_source) if valid, None if invalid.
///
/// ```
/// use bonsai_core::validity::{Replacement, try_replacement};
///
/// let lang = bonsai_core::languages::get_language("python").unwrap();
/// let source = b"x = 1\ny = 2\nz = 3";
///
/// // Valid: delete "y = 2\n" — remaining code is still valid Python
/// let r = Replacement { start_byte: 6, end_byte: 12, new_bytes: vec![] };
/// assert!(try_replacement(source, &r, &lang, None).is_some());
///
/// // Invalid: delete the condition from "if True:" — produces a parse error
/// let source2 = b"if True:\n  pass";
/// let r = Replacement { start_byte: 3, end_byte: 7, new_bytes: vec![] };
/// assert!(try_replacement(source2, &r, &lang, None).is_none());
/// ```
pub fn try_replacement(
    source: &[u8],
    replacement: &Replacement,
    language: &tree_sitter::Language,
    initial_errors: Option<&ErrorSet>,
) -> Option<Vec<u8>> {
    if replacement.start_byte > replacement.end_byte || replacement.end_byte > source.len() {
        return None;
    }
    let new_source = apply_replacement(source, replacement);
    let tree = parse::parse(&new_source, language)?;

    match initial_errors {
        None => {
            // Strict mode: no errors at all
            if tree_has_errors(&tree) {
                None
            } else {
                Some(new_source)
            }
        }
        Some(known) => {
            // Lenient mode: only reject new errors
            if known.has_new_errors(&tree, &new_source) {
                None
            } else {
                Some(new_source)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages;

    #[test]
    fn test_apply_replacement_middle() {
        let source = b"hello world";
        let replacement = Replacement {
            start_byte: 5,
            end_byte: 11,
            new_bytes: b" rust".to_vec(),
        };
        assert_eq!(apply_replacement(source, &replacement), b"hello rust");
    }

    #[test]
    fn test_apply_replacement_delete() {
        let source = b"hello world";
        let replacement = Replacement {
            start_byte: 5,
            end_byte: 11,
            new_bytes: vec![],
        };
        assert_eq!(apply_replacement(source, &replacement), b"hello");
    }

    #[test]
    fn test_apply_replacement_insert() {
        let source = b"hello";
        let replacement = Replacement {
            start_byte: 5,
            end_byte: 5,
            new_bytes: b" world".to_vec(),
        };
        assert_eq!(apply_replacement(source, &replacement), b"hello world");
    }

    #[test]
    fn test_tree_has_errors_clean() {
        let lang = languages::get_language("python").unwrap();
        let tree = crate::parse::parse(b"x = 1", &lang).unwrap();
        assert!(!tree_has_errors(&tree));
    }

    #[test]
    fn test_tree_has_errors_broken() {
        let lang = languages::get_language("python").unwrap();
        let tree = crate::parse::parse(b"def )", &lang).unwrap();
        assert!(tree_has_errors(&tree));
    }

    #[test]
    fn test_error_set_from_clean_tree() {
        let lang = languages::get_language("python").unwrap();
        let source = b"x = 1";
        let tree = crate::parse::parse(source, &lang).unwrap();
        let errors = ErrorSet::from_tree(&tree, source);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_error_set_from_broken_tree() {
        let lang = languages::get_language("python").unwrap();
        let source = b"def )";
        let tree = crate::parse::parse(source, &lang).unwrap();
        let errors = ErrorSet::from_tree(&tree, source);
        assert!(errors.has_errors());
    }

    #[test]
    fn test_error_set_detects_new_errors() {
        let lang = languages::get_language("python").unwrap();
        // Start with a file that has an error
        let source1 = b"def )\nx = 1";
        let tree1 = crate::parse::parse(source1, &lang).unwrap();
        let initial = ErrorSet::from_tree(&tree1, source1);
        assert!(initial.has_errors());

        // Parse a different broken version — should have NEW errors with different content
        let source2 = b"def )\nx = )";
        let tree2 = crate::parse::parse(source2, &lang).unwrap();
        assert!(initial.has_new_errors(&tree2, source2));
    }

    #[test]
    fn test_try_replacement_valid() {
        let lang = languages::get_language("python").unwrap();
        let source = b"x = 1\ny = 2";
        // Delete "y = 2" (the second line)
        let replacement = Replacement {
            start_byte: 6,
            end_byte: 11,
            new_bytes: vec![],
        };
        let result = try_replacement(source, &replacement, &lang, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), b"x = 1\n");
    }

    #[test]
    fn test_try_replacement_invalid() {
        let lang = languages::get_language("python").unwrap();
        let source = b"if True:\n  pass";
        // Delete "True" — should produce syntax error
        let replacement = Replacement {
            start_byte: 3,
            end_byte: 7,
            new_bytes: vec![],
        };
        let result = try_replacement(source, &replacement, &lang, None);
        assert!(
            result.is_none(),
            "Removing condition from if should be invalid"
        );
    }

    #[test]
    fn test_try_replacement_lenient_with_preexisting_errors() {
        let lang = languages::get_language("python").unwrap();
        // Source already has an error on line 1; lines 2-3 are valid
        let source = b"x = )\ny = 1\nz = 2";
        let tree = crate::parse::parse(source, &lang).unwrap();
        let initial = ErrorSet::from_tree(&tree, source);
        assert!(initial.has_errors());

        // Now parse the source with "z = 2" removed — the error on line 1 should remain
        let new_source = b"x = )\ny = 1\n";
        let new_tree = crate::parse::parse(new_source, &lang).unwrap();
        // Verify no new errors were introduced (the error has same content)
        assert!(
            !initial.has_new_errors(&new_tree, new_source),
            "Removing a trailing line should not introduce new errors"
        );

        // Now test via try_replacement
        let replacement = Replacement {
            start_byte: 12,
            end_byte: 17,
            new_bytes: vec![],
        };
        let result = try_replacement(source, &replacement, &lang, Some(&initial));
        assert!(
            result.is_some(),
            "Should accept removal that doesn't add new errors"
        );
    }

    #[test]
    fn test_try_replacement_strict_rejects_preexisting_errors() {
        let lang = languages::get_language("python").unwrap();
        let source = b"def )\nx = 1";
        // Even removing valid code, strict mode rejects because errors exist
        let replacement = Replacement {
            start_byte: 6,
            end_byte: 11,
            new_bytes: vec![],
        };
        let result = try_replacement(source, &replacement, &lang, None);
        assert!(
            result.is_none(),
            "Strict mode should reject when errors exist"
        );
    }

    #[test]
    fn test_try_replacement_invalid_range() {
        let lang = languages::get_language("python").unwrap();
        let source = b"x = 1";

        // end_byte > source.len()
        let r = Replacement {
            start_byte: 0,
            end_byte: 100,
            new_bytes: vec![],
        };
        assert!(try_replacement(source, &r, &lang, None).is_none());

        // start_byte > end_byte
        let r = Replacement {
            start_byte: 4,
            end_byte: 2,
            new_bytes: vec![],
        };
        assert!(try_replacement(source, &r, &lang, None).is_none());
    }

    #[test]
    fn test_missing_nodes_are_errors() {
        let lang = languages::get_language("python").unwrap();
        // "def foo(" — missing closing paren and body creates MISSING nodes
        let tree = crate::parse::parse(b"def foo(", &lang).unwrap();
        assert!(
            tree_has_errors(&tree),
            "Incomplete code should have errors/missing nodes"
        );
    }

    #[test]
    fn test_lenient_mode_with_error_before_deletion() {
        let lang = languages::get_language("python").unwrap();
        // Error at "x = )" — then valid code after
        let source = b"x = )\ny = 1\nz = 2";
        let tree = crate::parse::parse(source, &lang).unwrap();
        let initial = ErrorSet::from_tree(&tree, source);
        assert!(initial.has_errors());

        // Delete "y = 1\n" (bytes 6..12) — this is AFTER the error, should work
        let r = Replacement {
            start_byte: 6,
            end_byte: 12,
            new_bytes: vec![],
        };
        let result = try_replacement(source, &r, &lang, Some(&initial));
        assert!(result.is_some(), "Deleting after error should be accepted");
    }

    #[test]
    fn test_lenient_mode_deletion_before_error_no_false_reject() {
        let lang = languages::get_language("python").unwrap();
        // Valid code, then error
        let source = b"y = 1\nx = )\nz = 2";
        let tree = crate::parse::parse(source, &lang).unwrap();
        let initial = ErrorSet::from_tree(&tree, source);
        assert!(initial.has_errors());

        // Delete "y = 1\n" (bytes 0..6) — this is BEFORE the error, shifts it
        let r = Replacement {
            start_byte: 0,
            end_byte: 6,
            new_bytes: vec![],
        };
        let result = try_replacement(source, &r, &lang, Some(&initial));
        // After fix: the error "x = )" still exists with same content, just shifted
        // So this should be accepted (no NEW errors)
        assert!(
            result.is_some(),
            "Deleting before error should be accepted (error just shifted, same content)"
        );
    }
}
