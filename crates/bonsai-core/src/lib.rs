//! `bonsai-core` — tree manipulation, compatibility checks, and transforms.

pub mod compat;
pub mod parse;
pub mod supertype;
pub mod transform;
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
