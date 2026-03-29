use crate::supertype::SupertypeProvider;
use crate::validity::Replacement;
use tree_sitter::{Language, Node, Tree};

/// A transform proposes candidate replacements for tree nodes.
/// Each candidate is a [`Replacement`] that the caller validates via reparsing.
///
/// # Implementing a custom transform
///
/// ```
/// use bonsai_core::transform::Transform;
/// use bonsai_core::validity::Replacement;
/// use bonsai_core::supertype::SupertypeProvider;
/// use tree_sitter::{Node, Tree};
///
/// /// A transform that proposes deleting any node containing "unused" in its text.
/// struct RemoveUnusedTransform;
///
/// impl Transform for RemoveUnusedTransform {
///     fn candidates(
///         &self, node: &Node, source: &[u8], _tree: &Tree,
///         _provider: &dyn SupertypeProvider,
///     ) -> Vec<Replacement> {
///         let text = &source[node.start_byte()..node.end_byte()];
///         if text.windows(6).any(|w| w == b"unused") {
///             vec![Replacement {
///                 start_byte: node.start_byte(),
///                 end_byte: node.end_byte(),
///                 new_bytes: vec![],
///             }]
///         } else {
///             vec![]
///         }
///     }
///     fn name(&self) -> &str { "remove_unused" }
/// }
///
/// // Transforms are object-safe and can be boxed
/// let transform: Box<dyn Transform> = Box::new(RemoveUnusedTransform);
/// assert_eq!(transform.name(), "remove_unused");
/// ```
///
/// [`Replacement`]: crate::validity::Replacement
pub trait Transform: Send + Sync {
    /// Propose candidate replacements for the given node.
    /// Returns an empty vec if no replacements are applicable.
    fn candidates(
        &self,
        node: &Node,
        source: &[u8],
        tree: &Tree,
        provider: &dyn SupertypeProvider,
    ) -> Vec<Replacement>;

    /// Human-readable name of this transform (for logging/progress).
    fn name(&self) -> &str;

    /// Called after each accepted reduction so transforms can update internal state.
    /// Default implementation does nothing.
    fn on_reduction(&mut self, _tree: &Tree, _source: &[u8], _language: &Language) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTransform;

    impl Transform for MockTransform {
        fn candidates(
            &self,
            _node: &Node,
            _source: &[u8],
            _tree: &Tree,
            _provider: &dyn SupertypeProvider,
        ) -> Vec<Replacement> {
            vec![Replacement {
                start_byte: 0,
                end_byte: 1,
                new_bytes: vec![],
            }]
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_trait_is_object_safe() {
        let transform: Box<dyn Transform> = Box::new(MockTransform);
        assert_eq!(transform.name(), "mock");
    }

    #[test]
    fn test_mock_transform_returns_candidates() {
        let transform = MockTransform;
        let lang = crate::languages::get_language("python").unwrap();
        let tree = crate::parse::parse(b"x = 1", &lang).unwrap();
        let provider = crate::supertype::EmptyProvider;
        let root = tree.root_node();

        let candidates = transform.candidates(&root, b"x = 1", &tree, &provider);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].start_byte, 0);
    }
}
