use std::collections::HashSet;
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
        source.len() - (replacement.end_byte - replacement.start_byte) + replacement.new_bytes.len(),
    );
    result.extend_from_slice(&source[..replacement.start_byte]);
    result.extend_from_slice(&replacement.new_bytes);
    result.extend_from_slice(&source[replacement.end_byte..]);
    result
}

/// A set of error locations in a parse tree.
/// Used to distinguish pre-existing errors from new ones.
#[derive(Debug, Clone)]
pub struct ErrorSet {
    /// Set of (start_byte, end_byte) for ERROR and MISSING nodes.
    errors: HashSet<(usize, usize)>,
}

impl ErrorSet {
    /// Collect all ERROR and MISSING nodes from a tree.
    pub fn from_tree(tree: &Tree) -> Self {
        let mut errors = HashSet::new();
        let mut cursor = tree.root_node().walk();
        collect_errors_recursive(&mut cursor, &mut errors);
        Self { errors }
    }

    /// Check if this set contains any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check if a tree has any NEW errors not in this set.
    pub fn has_new_errors(&self, tree: &Tree) -> bool {
        let mut cursor = tree.root_node().walk();
        has_new_error_recursive(&mut cursor, &self.errors)
    }

    /// Number of errors.
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

fn collect_errors_recursive(
    cursor: &mut tree_sitter::TreeCursor,
    errors: &mut HashSet<(usize, usize)>,
) {
    let node = cursor.node();
    if node.is_error() || node.is_missing() {
        errors.insert((node.start_byte(), node.end_byte()));
    }
    if cursor.goto_first_child() {
        loop {
            collect_errors_recursive(cursor, errors);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn has_new_error_recursive(
    cursor: &mut tree_sitter::TreeCursor,
    known_errors: &HashSet<(usize, usize)>,
) -> bool {
    let node = cursor.node();
    if (node.is_error() || node.is_missing())
        && !known_errors.contains(&(node.start_byte(), node.end_byte()))
    {
        return true; // Found a new error — early return
    }
    if cursor.goto_first_child() {
        loop {
            if has_new_error_recursive(cursor, known_errors) {
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
            if known.has_new_errors(&tree) {
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
        let tree = crate::parse::parse(b"x = 1", &lang).unwrap();
        let errors = ErrorSet::from_tree(&tree);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_error_set_from_broken_tree() {
        let lang = languages::get_language("python").unwrap();
        let tree = crate::parse::parse(b"def )", &lang).unwrap();
        let errors = ErrorSet::from_tree(&tree);
        assert!(errors.has_errors());
    }

    #[test]
    fn test_error_set_detects_new_errors() {
        let lang = languages::get_language("python").unwrap();
        // Start with a file that has an error
        let tree1 = crate::parse::parse(b"def )\nx = 1", &lang).unwrap();
        let initial = ErrorSet::from_tree(&tree1);
        assert!(initial.has_errors());

        // Parse a different broken version — should have NEW errors at different positions
        let tree2 = crate::parse::parse(b"def )\nx = )", &lang).unwrap();
        assert!(initial.has_new_errors(&tree2));
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
        assert!(result.is_none(), "Removing condition from if should be invalid");
    }

    #[test]
    fn test_try_replacement_lenient_with_preexisting_errors() {
        let lang = languages::get_language("python").unwrap();
        // Source already has an error on line 1; lines 2-3 are valid
        let source = b"x = )\ny = 1\nz = 2";
        let tree = crate::parse::parse(source, &lang).unwrap();
        let initial = ErrorSet::from_tree(&tree);
        assert!(initial.has_errors());

        // Now parse the source with "z = 2" removed — the error on line 1 should remain at same position
        let new_source = b"x = )\ny = 1\n";
        let new_tree = crate::parse::parse(new_source, &lang).unwrap();
        // Verify no new errors were introduced (the error at ")" should be at same byte offset)
        assert!(
            !initial.has_new_errors(&new_tree),
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
    fn test_missing_nodes_are_errors() {
        let lang = languages::get_language("python").unwrap();
        // "def foo(" — missing closing paren and body creates MISSING nodes
        let tree = crate::parse::parse(b"def foo(", &lang).unwrap();
        assert!(
            tree_has_errors(&tree),
            "Incomplete code should have errors/missing nodes"
        );
    }
}
