//! Scope analysis using tree-sitter `locals.scm` queries.
//!
//! Builds a mapping of scopes, definitions, and references from a parse tree
//! using the `@local.scope`, `@local.definition`, and `@local.reference` captures.

use std::collections::HashMap;
use tree_sitter::{Language, Node, Query, QueryCursor, StreamingIterator, Tree};

/// A definition found by scope analysis.
#[derive(Debug, Clone)]
pub struct Definition {
    /// The node ID of the definition.
    pub node_id: usize,
    /// The name of the defined identifier.
    pub name: String,
    /// Byte range of the definition node.
    pub start_byte: usize,
    pub end_byte: usize,
    /// The node ID of the scope containing this definition.
    pub scope_node_id: usize,
}

/// A reference found by scope analysis.
#[derive(Debug, Clone)]
pub struct Reference {
    /// The node ID of the reference.
    pub node_id: usize,
    /// The name being referenced.
    pub name: String,
    /// Byte range of the reference node.
    pub start_byte: usize,
    pub end_byte: usize,
    /// The definition this reference resolves to (if found).
    pub definition_node_id: Option<usize>,
}

/// Result of running scope analysis on a parse tree.
#[derive(Debug)]
pub struct ScopeAnalysis {
    /// All definitions found, keyed by node ID.
    pub definitions: HashMap<usize, Definition>,
    /// All references found, keyed by node ID.
    pub references: HashMap<usize, Reference>,
    /// Definitions within each scope, keyed by scope node ID.
    pub scope_definitions: HashMap<usize, Vec<usize>>,
}

impl ScopeAnalysis {
    /// Build scope analysis from a parse tree using a locals.scm query string.
    ///
    /// Returns `None` if the query string is invalid for the language.
    pub fn from_tree(
        tree: &Tree,
        source: &[u8],
        language: &Language,
        locals_query_str: &str,
    ) -> Option<Self> {
        let query = Query::new(language, locals_query_str).ok()?;

        // Find capture indices for @local.scope, @local.definition, @local.reference
        let scope_idx = query.capture_index_for_name("local.scope");
        let def_idx = query.capture_index_for_name("local.definition");
        let ref_idx = query.capture_index_for_name("local.reference");

        let mut definitions: HashMap<usize, Definition> = HashMap::new();
        let mut references: HashMap<usize, Reference> = HashMap::new();
        let mut scope_definitions: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut scope_nodes: Vec<(usize, usize)> = Vec::new(); // (start_byte, node_id) for scope lookup

        // Collect all scope nodes first
        let mut cursor = QueryCursor::new();
        let root = tree.root_node();

        // Root node is always a scope
        scope_nodes.push((root.start_byte(), root.id()));

        {
            let mut matches = cursor.matches(&query, root, source);
            while let Some(match_) = matches.next() {
                for capture in match_.captures {
                    if Some(capture.index) == scope_idx {
                        let node = capture.node;
                        scope_nodes.push((node.start_byte(), node.id()));
                    }
                }
            }
        }

        // Sort scopes by start byte for binary search
        scope_nodes.sort_by_key(|&(start, _)| start);

        // Now collect definitions and references
        {
            let mut cursor2 = QueryCursor::new();
            let mut matches = cursor2.matches(&query, root, source);
            while let Some(match_) = matches.next() {
                for capture in match_.captures {
                    let node = capture.node;
                    let name = node.utf8_text(source).unwrap_or("").to_string();

                    if Some(capture.index) == def_idx {
                        let scope_id = find_enclosing_scope(&node, &scope_nodes, root.id());
                        let def = Definition {
                            node_id: node.id(),
                            name: name.clone(),
                            start_byte: node.start_byte(),
                            end_byte: node.end_byte(),
                            scope_node_id: scope_id,
                        };
                        definitions.insert(node.id(), def);
                        scope_definitions.entry(scope_id).or_default().push(node.id());
                    } else if Some(capture.index) == ref_idx {
                        let ref_ = Reference {
                            node_id: node.id(),
                            name,
                            start_byte: node.start_byte(),
                            end_byte: node.end_byte(),
                            definition_node_id: None, // resolved below
                        };
                        references.insert(node.id(), ref_);
                    }
                }
            }
        }

        // Resolve references to definitions by walking the scope chain
        let mut resolved_refs: HashMap<usize, Reference> = HashMap::new();
        for (node_id, mut ref_) in references {
            // Skip references that are also definitions (the same identifier node)
            if definitions.contains_key(&node_id) {
                continue;
            }
            // Find the definition with matching name in the enclosing scopes
            ref_.definition_node_id = find_definition_in_scope_chain(
                &ref_.name,
                &scope_definitions,
                &definitions,
                &scope_nodes,
                root.id(),
                node_id,
                &ref_,
            );
            resolved_refs.insert(node_id, ref_);
        }

        Some(Self {
            definitions,
            references: resolved_refs,
            scope_definitions,
        })
    }

    /// Get definitions that have no references pointing to them.
    pub fn unreferenced_definitions(&self) -> Vec<&Definition> {
        let referenced_def_ids: std::collections::HashSet<usize> = self
            .references
            .values()
            .filter_map(|r| r.definition_node_id)
            .collect();

        self.definitions
            .values()
            .filter(|d| !referenced_def_ids.contains(&d.node_id))
            .collect()
    }

    /// Count references to a given definition.
    pub fn reference_count(&self, def_node_id: usize) -> usize {
        self.references
            .values()
            .filter(|r| r.definition_node_id == Some(def_node_id))
            .count()
    }
}

/// Find the enclosing scope for a node by walking up the tree.
fn find_enclosing_scope(node: &Node, scope_nodes: &[(usize, usize)], root_id: usize) -> usize {
    // Walk up the tree from the node to find the nearest scope
    let mut current = node.parent();
    while let Some(parent) = current {
        if scope_nodes.iter().any(|&(_, id)| id == parent.id()) {
            return parent.id();
        }
        current = parent.parent();
    }
    root_id
}

/// Find a definition by name, walking up the scope chain.
fn find_definition_in_scope_chain(
    name: &str,
    scope_definitions: &HashMap<usize, Vec<usize>>,
    definitions: &HashMap<usize, Definition>,
    scope_nodes: &[(usize, usize)],
    root_id: usize,
    _ref_node_id: usize,
    ref_: &Reference,
) -> Option<usize> {
    // Start from the reference's position and find its enclosing scope
    // Then walk up scopes looking for a definition with the matching name
    // For simplicity, we check all scopes that contain the reference position

    // Find scopes that contain this reference (by byte range containment)
    let ref_byte = ref_.start_byte;

    // Collect candidate scopes — those whose range contains the reference
    // We don't have end_byte for scopes in scope_nodes, so we use a simpler approach:
    // check all scope definitions for a matching name
    for &(_start, scope_id) in scope_nodes.iter().rev() {
        if let Some(def_ids) = scope_definitions.get(&scope_id) {
            for &def_id in def_ids {
                if let Some(def) = definitions.get(&def_id) {
                    if def.name == name && def.start_byte <= ref_byte {
                        return Some(def_id);
                    }
                }
            }
        }
    }

    // Check root scope
    if let Some(def_ids) = scope_definitions.get(&root_id) {
        for &def_id in def_ids {
            if let Some(def) = definitions.get(&def_id) {
                if def.name == name {
                    return Some(def_id);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{languages, parse};

    #[test]
    fn test_scope_analysis_javascript() {
        let lang = languages::get_language("javascript").unwrap();
        let info = languages::list_languages()
            .iter()
            .find(|l| l.name == "javascript")
            .unwrap();
        let locals_content = info.locals_scm_content.expect("JS should have locals.scm");

        let source = b"function foo() { let x = 1; return x; }";
        let tree = parse::parse(source, &lang).unwrap();

        let analysis = ScopeAnalysis::from_tree(&tree, source, &lang, locals_content)
            .expect("Should parse locals.scm");

        // Should find at least one definition (x) and one reference (x in return)
        assert!(
            !analysis.definitions.is_empty(),
            "Should find definitions in JS code"
        );
        assert!(
            !analysis.references.is_empty(),
            "Should find references in JS code"
        );

        // x should be referenced
        let x_def = analysis
            .definitions
            .values()
            .find(|d| d.name == "x");
        assert!(x_def.is_some(), "Should find definition of 'x'");

        if let Some(def) = x_def {
            let ref_count = analysis.reference_count(def.node_id);
            assert!(
                ref_count > 0,
                "x should have at least one reference, got {}",
                ref_count
            );
        }
    }

    #[test]
    fn test_scope_analysis_unreferenced_definition() {
        let lang = languages::get_language("javascript").unwrap();
        let info = languages::list_languages()
            .iter()
            .find(|l| l.name == "javascript")
            .unwrap();
        let locals_content = info.locals_scm_content.unwrap();

        let source = b"function foo() { let unused = 1; let used = 2; return used; }";
        let tree = parse::parse(source, &lang).unwrap();

        let analysis = ScopeAnalysis::from_tree(&tree, source, &lang, locals_content).unwrap();

        let unreferenced = analysis.unreferenced_definitions();
        let unreferenced_names: Vec<&str> = unreferenced.iter().map(|d| d.name.as_str()).collect();

        assert!(
            unreferenced_names.contains(&"unused"),
            "Should find 'unused' as unreferenced. Got: {:?}",
            unreferenced_names
        );
    }

    #[test]
    fn test_scope_analysis_no_locals() {
        let lang = languages::get_language("python").unwrap();
        // Python doesn't have locals.scm configured
        let result = ScopeAnalysis::from_tree(
            &parse::parse(b"x = 1", &lang).unwrap(),
            b"x = 1",
            &lang,
            "invalid query that won't parse",
        );
        assert!(
            result.is_none(),
            "Invalid query should return None"
        );
    }
}
