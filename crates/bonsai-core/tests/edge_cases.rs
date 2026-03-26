//! Edge case and multi-grammar integration tests for bonsai-core.

use bonsai_core::transform::Transform;
use bonsai_core::validity::Replacement;
use bonsai_core::{languages, parse, supertype, transforms, validity};

#[test]
fn test_parse_empty_source_all_grammars() {
    for info in languages::list_languages() {
        let lang = languages::get_language(info.name).unwrap();
        let tree = parse::parse(b"", &lang);
        assert!(tree.is_some(), "Empty source should parse for {}", info.name);
    }
}

#[test]
fn test_single_token_source() {
    let lang = languages::get_language("python").unwrap();
    let tree = parse::parse(b"x", &lang).unwrap();
    let root = tree.root_node();
    assert!(
        root.child_count() > 0,
        "Single token should produce at least one child node"
    );
}

#[test]
fn test_deeply_nested_tree() {
    let lang = languages::get_language("python").unwrap();
    // Deeply nested parenthesized expressions
    let source = b"x = ((((((((1))))))))";
    let tree = parse::parse(source, &lang).unwrap();
    assert!(!validity::tree_has_errors(&tree));
}

#[test]
fn test_delete_transform_all_grammars() {
    let sources: &[(&str, &[u8])] = &[
        ("python", b"x = 1\ny = 2\nz = 3"),
        ("javascript", b"let x = 1;\nlet y = 2;\nlet z = 3;"),
        ("rust", b"fn main() {}\nfn foo() {}\nfn bar() {}"),
    ];

    let delete = transforms::delete::DeleteTransform;

    for (lang_name, source) in sources {
        let lang = languages::get_language(lang_name).unwrap();
        let tree = parse::parse(*source, &lang).unwrap();
        let provider = supertype::EmptyProvider;

        // At least some named nodes should have delete candidates
        let root = tree.root_node();
        let mut has_candidates = false;
        let mut cursor = root.walk();
        visit_all(&mut cursor, &mut |node| {
            if node.is_named() {
                let candidates = delete.candidates(&node, source, &tree, &provider);
                if !candidates.is_empty() {
                    has_candidates = true;
                }
            }
        });
        assert!(
            has_candidates,
            "Should find delete candidates for {}",
            lang_name
        );
    }
}

#[test]
fn test_grammar_with_no_supertypes_still_works() {
    // Use EmptyProvider to simulate a grammar with no supertypes
    let lang = languages::get_language("python").unwrap();
    let source = b"x = 1\ny = 2";
    let tree = parse::parse(source, &lang).unwrap();
    let provider = supertype::EmptyProvider;
    let delete = transforms::delete::DeleteTransform;

    // Delete transform still works (doesn't need supertypes)
    let root = tree.root_node();
    let second_stmt = root.child(1).unwrap();
    let candidates = delete.candidates(&second_stmt, source, &tree, &provider);
    assert!(!candidates.is_empty(), "Delete should work without supertypes");

    // And the deletion should produce valid code
    let result = validity::try_replacement(source, &candidates[0], &lang, None);
    assert!(result.is_some(), "Deletion should produce valid code");
}

#[test]
fn test_supertype_provider_javascript() {
    let lang = languages::get_language("javascript").unwrap();
    let provider = supertype::LanguageApiProvider::new(&lang);
    // JavaScript should have supertypes (expression, statement, etc.)
    // Just verify it doesn't panic and returns something
    let _has = provider.has_supertypes();
}

#[test]
fn test_supertype_provider_rust() {
    let lang = languages::get_language("rust").unwrap();
    let provider = supertype::LanguageApiProvider::new(&lang);
    let _has = provider.has_supertypes();
}

#[test]
fn test_replacement_at_boundaries() {
    // Replace at start of source
    let source = b"abc";
    let r = Replacement {
        start_byte: 0,
        end_byte: 1,
        new_bytes: b"X".to_vec(),
    };
    assert_eq!(validity::apply_replacement(source, &r), b"Xbc");

    // Replace at end of source
    let r = Replacement {
        start_byte: 2,
        end_byte: 3,
        new_bytes: b"Z".to_vec(),
    };
    assert_eq!(validity::apply_replacement(source, &r), b"abZ");

    // Replace entire source
    let r = Replacement {
        start_byte: 0,
        end_byte: 3,
        new_bytes: b"XYZ".to_vec(),
    };
    assert_eq!(validity::apply_replacement(source, &r), b"XYZ");

    // Delete entire source
    let r = Replacement {
        start_byte: 0,
        end_byte: 3,
        new_bytes: vec![],
    };
    assert_eq!(validity::apply_replacement(source, &r), b"");
}

#[test]
fn test_error_set_empty_tree() {
    let lang = languages::get_language("python").unwrap();
    let tree = parse::parse(b"", &lang).unwrap();
    let source = b"";
    let errors = validity::ErrorSet::from_tree(&tree, source);
    assert!(errors.is_empty());
}

fn visit_all(cursor: &mut tree_sitter::TreeCursor, f: &mut dyn FnMut(tree_sitter::Node)) {
    f(cursor.node());
    if cursor.goto_first_child() {
        loop {
            visit_all(cursor, f);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}
