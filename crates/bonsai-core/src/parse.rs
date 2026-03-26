use tree_sitter::{InputEdit, Language, Parser, Tree};

/// Parse source code with the given tree-sitter language.
/// Returns None if parsing fails.
pub fn parse(source: &[u8], language: &Language) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(language).ok()?;
    parser.parse(source, None)
}

/// Reparse source code incrementally after an edit.
/// The caller must provide the old tree and the edit that was applied.
/// Returns None if parsing fails.
pub fn reparse(source: &[u8], old_tree: &mut Tree, edit: &InputEdit) -> Option<Tree> {
    old_tree.edit(edit);
    let mut parser = Parser::new();
    parser.set_language(&old_tree.language()).ok()?;
    parser.parse(source, Some(old_tree))
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
