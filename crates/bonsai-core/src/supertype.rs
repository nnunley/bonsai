//! Supertype/subtype relationships for tree-sitter grammars.
//!
//! Provides a pluggable system for determining node type compatibility —
//! the core of how bonsai knows what can replace what in a parse tree.
//!
//! # Using the runtime API provider
//!
//! ```
//! use bonsai_core::supertype::{LanguageApiProvider, SupertypeProvider};
//!
//! let lang = bonsai_core::languages::get_language("python").unwrap();
//! let provider = LanguageApiProvider::new(&lang);
//!
//! // Python grammar has supertypes (e.g., _expression, _statement)
//! assert!(provider.has_supertypes());
//!
//! // An identifier is a subtype of some expression supertype
//! let id_kind = lang.id_for_node_kind("identifier", true);
//! let supertypes = provider.supertypes_for(id_kind);
//! assert!(!supertypes.is_empty());
//! ```
//!
//! # Chaining providers for fallback
//!
//! ```
//! use bonsai_core::supertype::{ChainProvider, LanguageApiProvider, NodeTypesProvider, SupertypeProvider};
//!
//! let lang = bonsai_core::languages::get_language("python").unwrap();
//! let chain = ChainProvider::new(vec![
//!     Box::new(LanguageApiProvider::new(&lang)),
//!     Box::new(NodeTypesProvider::new(&lang, "python")),
//! ]);
//!
//! let id_kind = lang.id_for_node_kind("identifier", true);
//! let supertypes = chain.supertypes_for(id_kind);
//! assert!(!supertypes.is_empty());
//! ```
//!
//! # EmptyProvider as a no-op fallback
//!
//! ```
//! use bonsai_core::supertype::{EmptyProvider, SupertypeProvider};
//!
//! let provider = EmptyProvider;
//! assert!(provider.supertypes_for(42).is_empty());
//! assert!(provider.subtypes_for(42).is_empty());
//! // Same type is always compatible, different types are not
//! assert!(provider.is_compatible(5, 5));
//! assert!(!provider.is_compatible(5, 10));
//! ```

use std::collections::HashMap;
use tree_sitter::Language;

/// Provides supertype/subtype relationships for a tree-sitter grammar.
/// Used to determine which node types can replace which positions in the tree.
pub trait SupertypeProvider: Send + Sync {
    /// Return the supertype IDs that this node kind belongs to.
    fn supertypes_for(&self, kind_id: u16) -> Vec<u16>;

    /// Return all subtype IDs for a given supertype.
    fn subtypes_for(&self, supertype_id: u16) -> Vec<u16>;

    /// Check if a node kind is a valid replacement for a position expecting the given type.
    fn is_compatible(&self, candidate_kind: u16, expected_kind: u16) -> bool {
        if candidate_kind == expected_kind {
            return true;
        }
        // Check if both are subtypes of the same supertype
        let candidate_supers = self.supertypes_for(candidate_kind);
        let expected_supers = self.supertypes_for(expected_kind);
        candidate_supers.iter().any(|s| expected_supers.contains(s))
    }
}

/// Wraps tree-sitter's `Language::supertypes()` and `Language::subtypes_for_supertype()`.
///
/// Eagerly builds lookup maps at construction time for fast queries.
pub struct LanguageApiProvider {
    /// kind_id -> list of supertype IDs this kind belongs to
    kind_to_supertypes: HashMap<u16, Vec<u16>>,
    /// supertype_id -> list of subtype IDs
    supertype_to_subtypes: HashMap<u16, Vec<u16>>,
}

impl LanguageApiProvider {
    pub fn new(language: &Language) -> Self {
        let mut kind_to_supertypes: HashMap<u16, Vec<u16>> = HashMap::new();
        let mut supertype_to_subtypes: HashMap<u16, Vec<u16>> = HashMap::new();

        for &supertype_id in language.supertypes() {
            let subtypes: Vec<u16> = language.subtypes_for_supertype(supertype_id).to_vec();
            supertype_to_subtypes.insert(supertype_id, subtypes.clone());
            for &subtype_id in &subtypes {
                kind_to_supertypes
                    .entry(subtype_id)
                    .or_default()
                    .push(supertype_id);
            }
        }

        Self {
            kind_to_supertypes,
            supertype_to_subtypes,
        }
    }

    /// Returns true if this provider has any supertype information.
    pub fn has_supertypes(&self) -> bool {
        !self.supertype_to_subtypes.is_empty()
    }
}

impl SupertypeProvider for LanguageApiProvider {
    fn supertypes_for(&self, kind_id: u16) -> Vec<u16> {
        self.kind_to_supertypes
            .get(&kind_id)
            .cloned()
            .unwrap_or_default()
    }

    fn subtypes_for(&self, supertype_id: u16) -> Vec<u16> {
        self.supertype_to_subtypes
            .get(&supertype_id)
            .cloned()
            .unwrap_or_default()
    }
}

/// Tries multiple providers in order, merging results (deduplicating).
pub struct ChainProvider {
    providers: Vec<Box<dyn SupertypeProvider>>,
}

impl ChainProvider {
    pub fn new(providers: Vec<Box<dyn SupertypeProvider>>) -> Self {
        Self { providers }
    }
}

impl SupertypeProvider for ChainProvider {
    fn supertypes_for(&self, kind_id: u16) -> Vec<u16> {
        let mut result = Vec::new();
        for provider in &self.providers {
            for id in provider.supertypes_for(kind_id) {
                if !result.contains(&id) {
                    result.push(id);
                }
            }
        }
        result
    }

    fn subtypes_for(&self, supertype_id: u16) -> Vec<u16> {
        let mut result = Vec::new();
        for provider in &self.providers {
            for id in provider.subtypes_for(supertype_id) {
                if !result.contains(&id) {
                    result.push(id);
                }
            }
        }
        result
    }
}

/// Fallback provider for grammars with no supertype information.
pub struct EmptyProvider;

impl SupertypeProvider for EmptyProvider {
    fn supertypes_for(&self, _kind_id: u16) -> Vec<u16> {
        Vec::new()
    }

    fn subtypes_for(&self, _supertype_id: u16) -> Vec<u16> {
        Vec::new()
    }
}

/// Provides supertype relationships parsed from `node-types.json` at build time.
///
/// This fills in supertype information for grammars where `Language::supertypes()`
/// returns empty (older grammar ABIs). Since every grammar ships `node-types.json`,
/// this provider works for all registered grammars.
pub struct NodeTypesProvider {
    kind_to_supertypes: HashMap<u16, Vec<u16>>,
    supertype_to_subtypes: HashMap<u16, Vec<u16>>,
}

impl NodeTypesProvider {
    /// Build from the static mappings generated by build.rs.
    /// Resolves type names to kind IDs using the Language.
    pub fn new(language: &Language, lang_name: &str) -> Self {
        let mappings = crate::languages::get_node_types_supertypes(lang_name);
        let mut kind_to_supertypes: HashMap<u16, Vec<u16>> = HashMap::new();
        let mut supertype_to_subtypes: HashMap<u16, Vec<u16>> = HashMap::new();

        for &(supertype_name, subtype_names) in mappings {
            let supertype_id = language.id_for_node_kind(supertype_name, true);
            if supertype_id == 0 {
                continue; // Unknown type name — skip
            }

            let mut subtypes = Vec::new();
            for &subtype_name in subtype_names {
                let subtype_id = language.id_for_node_kind(subtype_name, true);
                if subtype_id == 0 {
                    continue;
                }
                subtypes.push(subtype_id);
                kind_to_supertypes
                    .entry(subtype_id)
                    .or_default()
                    .push(supertype_id);
            }

            if !subtypes.is_empty() {
                supertype_to_subtypes.insert(supertype_id, subtypes);
            }
        }

        Self {
            kind_to_supertypes,
            supertype_to_subtypes,
        }
    }

    pub fn has_supertypes(&self) -> bool {
        !self.supertype_to_subtypes.is_empty()
    }
}

impl SupertypeProvider for NodeTypesProvider {
    fn supertypes_for(&self, kind_id: u16) -> Vec<u16> {
        self.kind_to_supertypes
            .get(&kind_id)
            .cloned()
            .unwrap_or_default()
    }

    fn subtypes_for(&self, supertype_id: u16) -> Vec<u16> {
        self.supertype_to_subtypes
            .get(&supertype_id)
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages;

    #[test]
    fn test_language_api_provider_python_has_supertypes() {
        let lang = languages::get_language("python").unwrap();
        let provider = LanguageApiProvider::new(&lang);
        assert!(
            provider.has_supertypes(),
            "Python grammar should have supertypes"
        );
    }

    #[test]
    fn test_language_api_provider_python_expression_subtypes() {
        let lang = languages::get_language("python").unwrap();
        let provider = LanguageApiProvider::new(&lang);

        // Find the _expression supertype by checking supertypes list
        let supertypes = lang.supertypes();
        let expr_supertype = supertypes.iter().find(|&&id| {
            lang.node_kind_for_id(id)
                .is_some_and(|name| name.contains("expression"))
        });

        if let Some(&expr_id) = expr_supertype {
            let subtypes = provider.subtypes_for(expr_id);
            assert!(
                !subtypes.is_empty(),
                "Expression supertype should have subtypes"
            );

            // identifier should be a subtype of expression
            let identifier_id = lang.id_for_node_kind("identifier", true);
            if identifier_id != 0 {
                let id_supers = provider.supertypes_for(identifier_id);
                assert!(!id_supers.is_empty(), "identifier should have supertypes");
            }
        }
    }

    #[test]
    fn test_empty_provider() {
        let provider = EmptyProvider;
        assert!(provider.supertypes_for(42).is_empty());
        assert!(provider.subtypes_for(42).is_empty());
        assert!(!provider.is_compatible(1, 2));
        assert!(provider.is_compatible(1, 1)); // same type is always compatible
    }

    #[test]
    fn test_chain_provider_merges() {
        // Create two providers with different data
        let lang = languages::get_language("python").unwrap();
        let api_provider = LanguageApiProvider::new(&lang);
        let empty_provider = EmptyProvider;

        let chain = ChainProvider::new(vec![Box::new(api_provider), Box::new(empty_provider)]);

        // Chain should return same results as api_provider alone
        // (empty adds nothing)
        let supertypes = lang.supertypes();
        if let Some(&first_super) = supertypes.first() {
            let subtypes = chain.subtypes_for(first_super);
            assert!(!subtypes.is_empty());
        }
    }

    #[test]
    fn test_is_compatible_same_type() {
        let provider = EmptyProvider;
        assert!(provider.is_compatible(5, 5));
    }

    #[test]
    fn test_is_compatible_different_type_no_supertypes() {
        let provider = EmptyProvider;
        assert!(!provider.is_compatible(5, 10));
    }

    #[test]
    fn test_node_types_provider_python_has_supertypes() {
        let lang = languages::get_language("python").unwrap();
        let provider = NodeTypesProvider::new(&lang, "python");
        assert!(
            provider.has_supertypes(),
            "NodeTypesProvider should have supertypes for Python"
        );
    }

    #[test]
    fn test_node_types_provider_matches_language_api() {
        // For Python, NodeTypesProvider and LanguageApiProvider should agree
        // on identifier being a subtype of some expression supertype
        let lang = languages::get_language("python").unwrap();
        let api = LanguageApiProvider::new(&lang);
        let ntp = NodeTypesProvider::new(&lang, "python");

        let identifier_id = lang.id_for_node_kind("identifier", true);
        if identifier_id != 0 {
            let api_supers = api.supertypes_for(identifier_id);
            let ntp_supers = ntp.supertypes_for(identifier_id);

            // Both should find supertypes for identifier
            // (They may differ in exact IDs depending on whether the grammar
            // exposes runtime supertypes, but both should be non-empty)
            if !api_supers.is_empty() {
                assert!(
                    !ntp_supers.is_empty(),
                    "If LanguageApiProvider finds supertypes for identifier, NodeTypesProvider should too"
                );
            }
        }
    }

    #[test]
    fn test_node_types_provider_rust() {
        let lang = languages::get_language("rust").unwrap();
        let provider = NodeTypesProvider::new(&lang, "rust");
        // Rust grammar should have supertypes in node-types.json
        assert!(
            provider.has_supertypes(),
            "NodeTypesProvider should have supertypes for Rust"
        );
    }

    #[test]
    fn test_node_types_provider_unknown_language() {
        let lang = languages::get_language("python").unwrap();
        let provider = NodeTypesProvider::new(&lang, "nonexistent");
        assert!(
            !provider.has_supertypes(),
            "Unknown language should have no supertypes"
        );
    }

    #[test]
    fn test_language_api_provider_compatible_expressions() {
        let lang = languages::get_language("python").unwrap();
        let provider = LanguageApiProvider::new(&lang);

        let identifier_id = lang.id_for_node_kind("identifier", true);
        let string_id = lang.id_for_node_kind("string", true);

        if identifier_id != 0 && string_id != 0 {
            // Both identifier and string are expressions in Python
            // They should be compatible (both subtypes of _expression or similar)
            let id_supers = provider.supertypes_for(identifier_id);
            let str_supers = provider.supertypes_for(string_id);

            // They share at least one supertype if both are expressions
            let shared = id_supers.iter().any(|s| str_supers.contains(s));
            if shared {
                assert!(provider.is_compatible(identifier_id, string_id));
            }
        }
    }
}
