use tree_sitter::{Node, Tree};
use crate::supertype::SupertypeProvider;
use crate::transform::Transform;
use crate::validity::Replacement;

/// Proposes deleting a named node by replacing its byte range with empty bytes.
/// Does NOT check validity — the caller must validate via reparsing.
pub struct DeleteTransform;

impl Transform for DeleteTransform {
    fn candidates(
        &self,
        node: &Node,
        _source: &[u8],
        _tree: &Tree,
        _provider: &dyn SupertypeProvider,
    ) -> Vec<Replacement> {
        // Only propose deletion for named nodes
        if !node.is_named() {
            return vec![];
        }

        // Propose replacing the node's byte range with empty
        vec![Replacement {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            new_bytes: vec![],
        }]
    }

    fn name(&self) -> &str {
        "delete"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{languages, parse, supertype::EmptyProvider, validity};

    #[test]
    fn test_delete_named_node_proposes_empty() {
        let lang = languages::get_language("python").unwrap();
        let source = b"if True:\n  pass\nelse:\n  pass";
        let tree = parse::parse(source, &lang).unwrap();
        let provider = EmptyProvider;

        // Find the else_clause
        let root = tree.root_node();
        let if_stmt = root.child(0).unwrap(); // if_statement
        let else_clause = if_stmt.children(&mut if_stmt.walk())
            .find(|c| c.kind() == "else_clause");

        if let Some(else_node) = else_clause {
            let transform = DeleteTransform;
            let candidates = transform.candidates(&else_node, source, &tree, &provider);
            assert_eq!(candidates.len(), 1);
            assert!(candidates[0].new_bytes.is_empty());
            assert_eq!(candidates[0].start_byte, else_node.start_byte());
            assert_eq!(candidates[0].end_byte, else_node.end_byte());
        }
    }

    #[test]
    fn test_delete_valid_removal() {
        let lang = languages::get_language("python").unwrap();
        let source = b"x = 1\ny = 2";
        let tree = parse::parse(source, &lang).unwrap();
        let provider = EmptyProvider;
        let transform = DeleteTransform;

        // Try deleting the second expression_statement (y = 2)
        let root = tree.root_node();
        let second_stmt = root.child(1).unwrap();
        let candidates = transform.candidates(&second_stmt, source, &tree, &provider);
        assert_eq!(candidates.len(), 1);

        // Validate the replacement produces valid Python
        let result = validity::try_replacement(source, &candidates[0], &lang, None);
        assert!(result.is_some(), "Deleting second statement should be valid");
    }

    #[test]
    fn test_delete_anonymous_node_skipped() {
        let lang = languages::get_language("python").unwrap();
        let source = b"x = 1";
        let tree = parse::parse(source, &lang).unwrap();
        let provider = EmptyProvider;
        let transform = DeleteTransform;

        // Find an anonymous node (like "=" operator)
        let root = tree.root_node();
        let mut found_anonymous = false;
        let mut cursor = root.walk();
        visit_all(&mut cursor, &mut |node: Node| {
            if !node.is_named() {
                let candidates = transform.candidates(&node, source, &tree, &provider);
                assert!(candidates.is_empty(), "Anonymous nodes should not get delete candidates");
                found_anonymous = true;
            }
        });
        assert!(found_anonymous, "Should have found at least one anonymous node");
    }

    use crate::test_utils::visit_all;
}
