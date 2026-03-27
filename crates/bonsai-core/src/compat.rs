//! Node type compatibility checking for tree reduction.
//!
//! Determines which node types can legally replace a given node in the tree,
//! using supertype relationships from a [`SupertypeProvider`].

use tree_sitter::Node;

use crate::supertype::SupertypeProvider;

/// Find all node kind IDs that could be valid replacements for the given node,
/// based on supertype relationships.
///
/// Returns a list of kind IDs (not including the node's own kind).
/// An empty list means only exact-type replacement or deletion is possible.
pub fn compatible_replacements(node: &Node, provider: &dyn SupertypeProvider) -> Vec<u16> {
    let kind_id = node.grammar_id();
    let mut result = Vec::new();

    // Get all supertypes this node belongs to
    let supertypes = provider.supertypes_for(kind_id);

    // For each supertype, collect all sibling subtypes
    for supertype_id in &supertypes {
        for subtype_id in provider.subtypes_for(*supertype_id) {
            if subtype_id != kind_id && !result.contains(&subtype_id) {
                result.push(subtype_id);
            }
        }
    }

    result
}

/// Check if a candidate node kind can replace a position that expects the given kind.
pub fn is_compatible_replacement(
    candidate_kind: u16,
    expected_kind: u16,
    provider: &dyn SupertypeProvider,
) -> bool {
    provider.is_compatible(candidate_kind, expected_kind)
}

/// Check if a node is a named node (as opposed to anonymous punctuation/keywords).
/// Only named nodes are typically candidates for reduction.
pub fn is_named_node(node: &Node) -> bool {
    node.is_named()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{languages, parse};

    #[test]
    fn test_compatible_replacements_for_expression() {
        let lang = languages::get_language("python").unwrap();
        let provider = crate::supertype::LanguageApiProvider::new(&lang);

        let source = b"x = 1 + 2";
        let tree = parse::parse(source, &lang).unwrap();
        let root = tree.root_node();

        let mut found_compatible = false;
        let mut cursor = root.walk();
        visit_all(&mut cursor, &mut |node: Node| {
            if node.is_named() {
                let replacements = compatible_replacements(&node, &provider);
                if !replacements.is_empty() {
                    found_compatible = true;
                }
            }
        });
        assert!(
            found_compatible,
            "Should find at least one node with compatible replacements in Python code"
        );
    }

    #[test]
    fn test_compatible_replacements_empty_provider() {
        let lang = languages::get_language("python").unwrap();
        let provider = crate::supertype::EmptyProvider;

        let source = b"x = 1";
        let tree = parse::parse(source, &lang).unwrap();
        let root = tree.root_node();

        let mut cursor = root.walk();
        visit_all(&mut cursor, &mut |node: Node| {
            if node.is_named() {
                let replacements = compatible_replacements(&node, &provider);
                assert!(
                    replacements.is_empty(),
                    "Empty provider should yield no replacements"
                );
            }
        });
    }

    #[test]
    fn test_is_named_node() {
        let lang = languages::get_language("python").unwrap();
        let tree = parse::parse(b"x = 1", &lang).unwrap();
        let root = tree.root_node();
        assert!(is_named_node(&root)); // "module" is named
    }

    use crate::test_utils::visit_all;
}
