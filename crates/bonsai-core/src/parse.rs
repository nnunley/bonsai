use tree_sitter::{InputEdit, Language, Node, Parser, Tree};

/// Parse source code with the given tree-sitter language.
/// Returns None if parsing fails.
///
/// ```
/// let lang = bonsai_core::languages::get_language("python").unwrap();
/// let tree = bonsai_core::parse::parse(b"def foo(): pass", &lang).unwrap();
/// assert_eq!(tree.root_node().kind(), "module");
/// assert!(!tree.root_node().has_error());
/// ```
pub fn parse(source: &[u8], language: &Language) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(language).ok()?;
    parser.parse(source, None)
}

/// Reparse source code incrementally after an edit.
/// The caller must provide the old tree and the edit that was applied.
/// Returns None if parsing fails.
///
/// ```
/// use tree_sitter::{InputEdit, Point};
///
/// let lang = bonsai_core::languages::get_language("python").unwrap();
/// let mut tree = bonsai_core::parse::parse(b"x = 1", &lang).unwrap();
///
/// // Replace "1" with "42" (byte 4..5 becomes byte 4..6)
/// let edit = InputEdit {
///     start_byte: 4,
///     old_end_byte: 5,
///     new_end_byte: 6,
///     start_position: Point { row: 0, column: 4 },
///     old_end_position: Point { row: 0, column: 5 },
///     new_end_position: Point { row: 0, column: 6 },
/// };
/// let new_tree = bonsai_core::parse::reparse(b"x = 42", &mut tree, &edit).unwrap();
/// assert!(!new_tree.root_node().has_error());
/// ```
pub fn reparse(source: &[u8], old_tree: &mut Tree, edit: &InputEdit) -> Option<Tree> {
    old_tree.edit(edit);
    let mut parser = Parser::new();
    parser.set_language(&old_tree.language()).ok()?;
    parser.parse(source, Some(old_tree))
}

/// Find a node in the tree by exact byte range.
pub fn find_node_at<'a>(node: Node<'a>, start: usize, end: usize) -> Option<Node<'a>> {
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
    use crate::languages;
    use tree_sitter::Point;

    #[test]
    fn test_parse_python() {
        let lang = languages::get_language("python").unwrap();
        let tree = parse(b"def foo(): pass", &lang).unwrap();
        assert_eq!(tree.root_node().kind(), "module");
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn test_parse_empty_source() {
        let lang = languages::get_language("python").unwrap();
        let tree = parse(b"", &lang).unwrap();
        assert_eq!(tree.root_node().kind(), "module");
        assert_eq!(tree.root_node().child_count(), 0);
    }

    #[test]
    fn test_reparse_after_edit() {
        let lang = languages::get_language("python").unwrap();
        let original = b"x = 1";
        let mut tree = parse(original, &lang).unwrap();

        // Replace "1" with "2" (byte 4 to 5)
        let new_source = b"x = 2";
        let edit = InputEdit {
            start_byte: 4,
            old_end_byte: 5,
            new_end_byte: 5,
            start_position: Point { row: 0, column: 4 },
            old_end_position: Point { row: 0, column: 5 },
            new_end_position: Point { row: 0, column: 5 },
        };

        let new_tree = reparse(new_source, &mut tree, &edit).unwrap();
        assert_eq!(new_tree.root_node().kind(), "module");
        assert!(!new_tree.root_node().has_error());
    }

    #[test]
    fn test_parse_javascript() {
        let lang = languages::get_language("javascript").unwrap();
        let tree = parse(b"function foo() { return 1; }", &lang).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn test_parse_rust_code() {
        let lang = languages::get_language("rust").unwrap();
        let tree = parse(b"fn main() {}", &lang).unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
        assert!(!tree.root_node().has_error());
    }
}
