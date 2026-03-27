//! Transform that removes definitions with no references, using scope analysis.

use std::collections::HashSet;
use tree_sitter::{Language, Node, Tree};

use crate::scope::ScopeAnalysis;
use crate::supertype::SupertypeProvider;
use crate::transform::Transform;
use crate::validity::Replacement;

/// Proposes deleting statements that contain definitions with zero references.
///
/// Requires a `ScopeAnalysis` built from a `locals.scm` query. When no scope
/// analysis is available, this transform produces no candidates.
///
/// Implements `on_reduction` to recompute dead ranges after each accepted
/// reduction, so newly-dead definitions are caught as the source shrinks.
pub struct DeadDefinitionTransform {
    /// Byte ranges of unreferenced definition statements.
    /// Recomputed after each accepted reduction via `on_reduction`.
    dead_ranges: HashSet<(usize, usize)>,
    /// locals.scm query string, stored for rebuilding scope analysis.
    locals_query: Option<String>,
}

impl DeadDefinitionTransform {
    /// Build from a ScopeAnalysis. Precomputes which statement byte ranges
    /// contain unreferenced definitions.
    pub fn from_analysis(analysis: &ScopeAnalysis, tree: &Tree, locals_query: &str) -> Self {
        let dead_ranges = compute_dead_ranges(analysis, tree);
        Self {
            dead_ranges,
            locals_query: Some(locals_query.to_string()),
        }
    }

    /// Build an empty transform (no scope analysis available).
    pub fn empty() -> Self {
        Self {
            dead_ranges: HashSet::new(),
            locals_query: None,
        }
    }
}

/// Compute byte ranges of statements containing unreferenced definitions.
fn compute_dead_ranges(analysis: &ScopeAnalysis, tree: &Tree) -> HashSet<(usize, usize)> {
    let mut dead_ranges = HashSet::new();
    for def in analysis.unreferenced_definitions() {
        if let Some(stmt_node) = find_containing_statement(tree, def.start_byte, def.end_byte) {
            dead_ranges.insert((stmt_node.start_byte(), stmt_node.end_byte()));
        }
    }
    dead_ranges
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

    fn on_reduction(&mut self, tree: &Tree, source: &[u8], language: &Language) {
        if let Some(ref query) = self.locals_query {
            if let Some(analysis) = ScopeAnalysis::from_tree(tree, source, language, query) {
                self.dead_ranges = compute_dead_ranges(&analysis, tree);
            }
        }
    }
}

/// Walk up from a byte range to find the containing statement node.
fn find_containing_statement(tree: &Tree, start: usize, end: usize) -> Option<Node<'_>> {
    let root = tree.root_node();
    let node = crate::parse::find_node_at(root, start, end)?;

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

        let transform = DeadDefinitionTransform::from_analysis(&analysis, &tree, locals_content);

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

        let transform = DeadDefinitionTransform::from_analysis(&analysis, &tree, locals_content);

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
