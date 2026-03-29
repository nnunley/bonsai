//! Scope analysis using tree-sitter `locals.scm` queries.
//!
//! Builds a mapping of scopes, definitions, and references from a parse tree
//! using the `@local.scope`, `@local.definition`, and `@local.reference` captures.
//!
//! # Running scope analysis on JavaScript
//!
//! ```
//! use bonsai_core::scope::ScopeAnalysis;
//!
//! let lang = bonsai_core::languages::get_language("javascript").unwrap();
//! let info = bonsai_core::languages::list_languages()
//!     .into_iter()
//!     .find(|l| l.name == "javascript")
//!     .unwrap();
//! let locals_scm = info.locals_scm.unwrap();
//!
//! let source = b"function foo() { let x = 1; return x; }";
//! let tree = bonsai_core::parse::parse(source, &lang).unwrap();
//!
//! let analysis = ScopeAnalysis::from_tree(&tree, source, &lang, locals_scm).unwrap();
//!
//! // Should find the definition of `x` and its reference in `return x`
//! assert!(!analysis.definitions.is_empty());
//! assert!(!analysis.references.is_empty());
//!
//! // Find unreferenced definitions (there are none here — x is used)
//! let dead = analysis.unreferenced_definitions();
//! assert!(dead.iter().all(|d| d.name != "x"));
//! ```

use std::collections::HashMap;
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator, Tree};

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

/// A scope node with its byte range for containment checks.
#[derive(Debug, Clone)]
struct ScopeNode {
    start_byte: usize,
    end_byte: usize,
    node_id: usize,
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
        let mut scope_nodes: Vec<ScopeNode> = Vec::new();

        // Collect all scope nodes first
        let mut cursor = QueryCursor::new();
        let root = tree.root_node();

        // Root node is always a scope
        scope_nodes.push(ScopeNode {
            start_byte: root.start_byte(),
            end_byte: root.end_byte(),
            node_id: root.id(),
        });

        {
            let mut matches = cursor.matches(&query, root, source);
            while let Some(match_) = matches.next() {
                for capture in match_.captures {
                    if Some(capture.index) == scope_idx {
                        let node = capture.node;
                        scope_nodes.push(ScopeNode {
                            start_byte: node.start_byte(),
                            end_byte: node.end_byte(),
                            node_id: node.id(),
                        });
                    }
                }
            }
        }

        // Sort scopes by start byte, then by descending end byte (larger scopes first)
        scope_nodes.sort_by(|a, b| {
            a.start_byte.cmp(&b.start_byte).then(b.end_byte.cmp(&a.end_byte))
        });

        // Now collect definitions and references
        {
            let mut cursor2 = QueryCursor::new();
            let mut matches = cursor2.matches(&query, root, source);
            while let Some(match_) = matches.next() {
                for capture in match_.captures {
                    let node = capture.node;
                    let name = node.utf8_text(source).unwrap_or("").to_string();

                    if Some(capture.index) == def_idx {
                        let scope_id = find_enclosing_scope_by_range(
                            node.start_byte(),
                            node.end_byte(),
                            &scope_nodes,
                            root.id(),
                        );
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

/// Find the innermost scope containing the given byte range.
fn find_enclosing_scope_by_range(
    start: usize,
    end: usize,
    scope_nodes: &[ScopeNode],
    root_id: usize,
) -> usize {
    // Find the innermost (smallest) scope that fully contains [start, end).
    let mut best_id = root_id;
    let mut best_size = usize::MAX;
    for scope in scope_nodes {
        let size = scope.end_byte - scope.start_byte;
        if scope.start_byte <= start && end <= scope.end_byte && size < best_size {
            best_id = scope.node_id;
            best_size = size;
        }
    }
    best_id
}

/// Find a definition by name, walking up the scope chain from the reference's
/// enclosing scope outward through containing scopes.
fn find_definition_in_scope_chain(
    name: &str,
    scope_definitions: &HashMap<usize, Vec<usize>>,
    definitions: &HashMap<usize, Definition>,
    scope_nodes: &[ScopeNode],
    root_id: usize,
    _ref_node_id: usize,
    ref_: &Reference,
) -> Option<usize> {
    // Build a list of scopes that contain the reference, sorted innermost-first
    // (smallest range first).
    let ref_start = ref_.start_byte;
    let ref_end = ref_.end_byte;
    let mut containing_scopes: Vec<&ScopeNode> = scope_nodes
        .iter()
        .filter(|s| s.start_byte <= ref_start && ref_end <= s.end_byte)
        .collect();
    // Sort by scope size ascending (innermost first)
    containing_scopes.sort_by_key(|s| s.end_byte - s.start_byte);

    // Walk from innermost scope outward, looking for a matching definition.
    // Within each scope, pick the nearest preceding definition (largest start_byte
    // that is still <= ref_start) to handle re-bindings/shadowing correctly.
    for scope in &containing_scopes {
        if let Some(found) = find_nearest_def_in_scope(
            scope.node_id,
            name,
            ref_start,
            scope_definitions,
            definitions,
        ) {
            return Some(found);
        }
    }

    // Fallback: check root scope (root may not be in containing_scopes if
    // the reference is at the very end of the file)
    if !containing_scopes.iter().any(|s| s.node_id == root_id) {
        if let Some(found) = find_nearest_def_in_scope(
            root_id,
            name,
            ref_start,
            scope_definitions,
            definitions,
        ) {
            return Some(found);
        }
    }

    None
}

/// Find the nearest preceding definition with the given name in a scope.
/// Returns the definition with the largest start_byte <= ref_start (closest to reference).
fn find_nearest_def_in_scope(
    scope_id: usize,
    name: &str,
    ref_start: usize,
    scope_definitions: &HashMap<usize, Vec<usize>>,
    definitions: &HashMap<usize, Definition>,
) -> Option<usize> {
    let def_ids = scope_definitions.get(&scope_id)?;
    let mut best: Option<(usize, usize)> = None; // (def_id, start_byte)
    for &def_id in def_ids {
        if let Some(def) = definitions.get(&def_id) {
            if def.name == name && def.start_byte <= ref_start
                && best.is_none_or(|(_, best_start)| def.start_byte > best_start) {
                    best = Some((def_id, def.start_byte));
                }
        }
    }
    best.map(|(id, _)| id)
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
        let locals_content = info.locals_scm.expect("JS should have locals.scm");

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
        let locals_content = info.locals_scm.unwrap();

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
    fn test_scope_analysis_sibling_scopes_do_not_leak() {
        let lang = languages::get_language("javascript").unwrap();
        let info = languages::list_languages()
            .iter()
            .find(|l| l.name == "javascript")
            .unwrap();
        let locals_content = info.locals_scm.unwrap();

        // x is defined in function a() but referenced in function b() — they're siblings,
        // so b's reference to x should NOT resolve to a's definition.
        let source = b"function a() { let x = 1; } function b() { return x; }";
        let tree = parse::parse(source, &lang).unwrap();
        let analysis = ScopeAnalysis::from_tree(&tree, source, &lang, locals_content).unwrap();

        // x in a() should be unreferenced (b's x doesn't resolve to it)
        let unreferenced = analysis.unreferenced_definitions();
        let unreferenced_names: Vec<&str> = unreferenced.iter().map(|d| d.name.as_str()).collect();
        assert!(
            unreferenced_names.contains(&"x"),
            "x in a() should be unreferenced since b() is a sibling scope. Got: {:?}",
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
