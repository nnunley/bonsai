//! Transform that removes definitions with no references, using scope analysis.

use tree_sitter::{Node, Tree};

use crate::scope::ScopeAnalysis;
use crate::supertype::SupertypeProvider;
use crate::transform::Transform;
use crate::validity::Replacement;

/// Proposes deleting statements that contain definitions with zero references.
///
/// Requires a `ScopeAnalysis` built from a `locals.scm` query. When no scope
/// analysis is available, this transform produces no candidates.
pub struct DeadDefinitionTransform {
    /// Byte ranges of unreferenced definition statements.
    /// Precomputed from ScopeAnalysis so candidates() is fast.
    dead_ranges: Vec<(usize, usize)>,
}

impl DeadDefinitionTransform {
    /// Build from a ScopeAnalysis. Precomputes which statement byte ranges
    /// contain unreferenced definitions.
    pub fn from_analysis(analysis: &ScopeAnalysis, tree: &Tree) -> Self {
        let mut dead_ranges = Vec::new();

        for def in analysis.unreferenced_definitions() {
            // Find the containing statement — walk up from the definition node
            // to find a statement-level node to delete
            if let Some(stmt_node) = find_containing_statement(tree, def.start_byte, def.end_byte)
            {
                let range = (stmt_node.start_byte(), stmt_node.end_byte());
                if !dead_ranges.contains(&range) {
                    dead_ranges.push(range);
                }
            }
        }

        Self { dead_ranges }
    }

    /// Build an empty transform (no scope analysis available).
    pub fn empty() -> Self {
        Self {
            dead_ranges: Vec::new(),
        }
    }
}

impl Transform for DeadDefinitionTransform {
    fn candidates(
        &self,
        node: &Node,
        _source: &[u8],
        _tree: &Tree,
        _provider: &dyn SupertypeProvider,
    ) -> Vec<Replacement> {
        // Check if this node's byte range matches a dead definition statement
        let range = (node.start_byte(), node.end_byte());
        if self.dead_ranges.contains(&range) {
            vec![Replacement {
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                new_bytes: vec![],
            }]
        } else {
            vec![]
        }
    }

    fn name(&self) -> &str {
        "dead_definition"
    }
}

/// Walk up from a byte range to find the containing statement node.
fn find_containing_statement(tree: &Tree, start: usize, end: usize) -> Option<Node<'_>> {
    let root = tree.root_node();
    let node = find_node_at(root, start, end)?;

    // Walk up to find a statement-level node
    let mut current = node;
    loop {
        let parent = current.parent()?;
        // If the parent is the root (module), current is a top-level statement
        if parent.parent().is_none() {
            return Some(current);
        }
        // If the parent is a block/body, current is a statement within it
        let kind = parent.kind();
        if kind.contains("block")
            || kind.contains("body")
            || kind == "module"
            || kind == "program"
            || kind == "source_file"
        {
            return Some(current);
        }
        current = parent;
    }
}

/// Find a node at the given byte range.
fn find_node_at(node: Node<'_>, start: usize, end: usize) -> Option<Node<'_>> {
    if node.start_byte() == start && node.end_byte() == end {
        return Some(node);
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.start_byte() <= start && child.end_byte() >= end {
                if let result @ Some(_) = find_node_at(child, start, end) {
                    return result;
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::ScopeAnalysis;
    use crate::{languages, parse};

    #[test]
    fn test_dead_definition_proposes_deletion() {
        let lang = languages::get_language("javascript").unwrap();
        let info = languages::list_languages()
            .iter()
            .find(|l| l.name == "javascript")
            .unwrap();
        let locals_content = info.locals_scm_content.unwrap();

        let source = b"function foo() { let unused = 1; let used = 2; return used; }";
        let tree = parse::parse(source, &lang).unwrap();
        let analysis = ScopeAnalysis::from_tree(&tree, source, &lang, locals_content).unwrap();

        let transform = DeadDefinitionTransform::from_analysis(&analysis, &tree);

        // Walk the tree and check if the transform proposes deleting the unused variable
        let provider = crate::supertype::EmptyProvider;
        let root = tree.root_node();
        let mut found_dead = false;
        let mut cursor = root.walk();
        crate::test_utils::visit_all(&mut cursor, &mut |node| {
            let candidates = transform.candidates(&node, source, &tree, &provider);
            if !candidates.is_empty() {
                // The deletion should cover the "let unused = 1;" statement
                let deleted_text =
                    String::from_utf8_lossy(&source[candidates[0].start_byte..candidates[0].end_byte]);
                if deleted_text.contains("unused") {
                    found_dead = true;
                }
            }
        });

        assert!(
            found_dead,
            "Should propose deleting the 'unused' definition"
        );
    }

    #[test]
    fn test_dead_definition_does_not_delete_used() {
        let lang = languages::get_language("javascript").unwrap();
        let info = languages::list_languages()
            .iter()
            .find(|l| l.name == "javascript")
            .unwrap();
        let locals_content = info.locals_scm_content.unwrap();

        let source = b"let x = 1; console.log(x);";
        let tree = parse::parse(source, &lang).unwrap();
        let analysis = ScopeAnalysis::from_tree(&tree, source, &lang, locals_content).unwrap();

        let transform = DeadDefinitionTransform::from_analysis(&analysis, &tree);

        // x is used — should NOT be proposed for deletion
        let provider = crate::supertype::EmptyProvider;
        let root = tree.root_node();
        let mut cursor = root.walk();
        crate::test_utils::visit_all(&mut cursor, &mut |node| {
            let candidates = transform.candidates(&node, source, &tree, &provider);
            for c in &candidates {
                let text = String::from_utf8_lossy(&source[c.start_byte..c.end_byte]);
                assert!(
                    !text.contains("let x"),
                    "Should NOT propose deleting used definition: {}",
                    text
                );
            }
        });
    }

    #[test]
    fn test_dead_definition_empty_produces_nothing() {
        let transform = DeadDefinitionTransform::empty();
        let lang = languages::get_language("javascript").unwrap();
        let source = b"let x = 1;";
        let tree = parse::parse(source, &lang).unwrap();
        let provider = crate::supertype::EmptyProvider;
        let root = tree.root_node();

        let candidates = transform.candidates(&root, source, &tree, &provider);
        assert!(candidates.is_empty());
    }
}
