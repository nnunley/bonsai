//! Core library for bonsai — tree-sitter parse tree manipulation, node type
//! compatibility, and syntax-guided transforms.
//!
//! # Parsing and inspecting a tree
//!
//! ```
//! let lang = bonsai_core::languages::get_language("python").unwrap();
//! let tree = bonsai_core::parse::parse(b"x = 1\ny = 2", &lang).unwrap();
//!
//! let root = tree.root_node();
//! assert_eq!(root.kind(), "module");
//! assert_eq!(root.named_child_count(), 2); // two expression statements
//! ```
//!
//! # Applying a replacement and validating
//!
//! ```
//! use bonsai_core::validity::{Replacement, apply_replacement, try_replacement};
//!
//! let lang = bonsai_core::languages::get_language("python").unwrap();
//! let source = b"x = 1\ny = 2\nz = 3";
//!
//! // Delete the second line (bytes 6..11 = "y = 2")
//! let replacement = Replacement {
//!     start_byte: 6,
//!     end_byte: 11,
//!     new_bytes: vec![],
//! };
//!
//! // apply_replacement just does byte surgery
//! let new_source = apply_replacement(source, &replacement);
//! assert_eq!(new_source, b"x = 1\n\nz = 3");
//!
//! // try_replacement also reparses and validates (no ERROR/MISSING nodes)
//! let valid = try_replacement(source, &replacement, &lang, None);
//! assert!(valid.is_some());
//! ```
//!
//! # Using transforms to generate candidates
//!
//! ```
//! use bonsai_core::transform::Transform;
//! use bonsai_core::transforms::delete::DeleteTransform;
//! use bonsai_core::supertype::EmptyProvider;
//!
//! let lang = bonsai_core::languages::get_language("python").unwrap();
//! let source = b"x = 1\ny = 2";
//! let tree = bonsai_core::parse::parse(source, &lang).unwrap();
//! let provider = EmptyProvider;
//!
//! let delete = DeleteTransform;
//! let root = tree.root_node();
//!
//! // Get the second statement
//! let second_stmt = root.named_child(1).unwrap();
//! let candidates = delete.candidates(&second_stmt, source, &tree, &provider);
//!
//! assert_eq!(candidates.len(), 1);
//! assert!(candidates[0].new_bytes.is_empty()); // deletion = empty replacement
//! ```
//!
//! # Querying supertype compatibility
//!
//! ```
//! use bonsai_core::supertype::{LanguageApiProvider, SupertypeProvider};
//!
//! let lang = bonsai_core::languages::get_language("python").unwrap();
//! let provider = LanguageApiProvider::new(&lang);
//!
//! // Check if the grammar has supertypes (Python does)
//! assert!(provider.has_supertypes());
//!
//! // Find what can replace an identifier
//! let id_kind = lang.id_for_node_kind("identifier", true);
//! let supertypes = provider.supertypes_for(id_kind);
//! assert!(!supertypes.is_empty(), "identifier belongs to expression supertypes");
//! ```

pub mod compat;
pub mod parse;
pub mod supertype;
pub mod transform;
pub mod transforms;
pub mod validity;

pub mod languages {
    include!(concat!(env!("OUT_DIR"), "/languages.rs"));
}

#[cfg(test)]
mod tests {
    use super::languages;

    #[test]
    fn test_get_language_python() {
        let lang = languages::get_language("python");
        assert!(lang.is_some(), "Python language should be available");
    }

    #[test]
    fn test_get_language_by_extension() {
        let result = languages::get_language_by_extension("py");
        assert!(result.is_some());
        let (name, _lang) = result.unwrap();
        assert_eq!(name, "python");
    }

    #[test]
    fn test_get_language_unknown() {
        assert!(languages::get_language("unknown").is_none());
    }

    #[test]
    fn test_list_languages() {
        let langs = languages::list_languages();
        assert!(langs.len() >= 3);
        assert!(langs.iter().any(|l| l.name == "python"));
        assert!(langs.iter().any(|l| l.name == "javascript"));
        assert!(langs.iter().any(|l| l.name == "rust"));
    }

    #[test]
    fn test_python_parses_code() {
        let lang = languages::get_language("python").unwrap();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse("def foo(): pass", None).unwrap();
        assert_eq!(tree.root_node().kind(), "module");
    }
}
