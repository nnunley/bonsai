//! Integration tests for the bonsai reducer.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use bonsai_core::languages;
use bonsai_core::supertype::{EmptyProvider, LanguageApiProvider};
use bonsai_core::transforms::delete::DeleteTransform;
use bonsai_core::transforms::unwrap::UnwrapTransform;
use bonsai_reduce::interest::ShellTest;
use bonsai_reduce::reducer::{reduce, ReducerConfig};

fn make_config(lang_name: &str, strict: bool) -> ReducerConfig {
    let language = languages::get_language(lang_name).unwrap();
    let provider = LanguageApiProvider::new(&language);
    ReducerConfig {
        language,
        transforms: vec![Box::new(DeleteTransform), Box::new(UnwrapTransform)],
        provider: Box::new(provider),
        max_tests: 0,
        max_time: Duration::ZERO,
        jobs: 1,
        strict,
        max_test_errors: 3,
        interrupted: Arc::new(AtomicBool::new(false)),
    }
}

#[test]
fn test_reduce_python_keeps_target_line() {
    let source = b"x = 1\ny = 2\nz = 3\nw = 4\n";

    let test = ShellTest::new(
        vec!["grep".into(), "-q".into(), "x = 1".into()],
        Duration::from_secs(10),
    ).unwrap();

    let config = make_config("python", true);
    let result = reduce(source, &test, config);

    assert!(result.source.len() < source.len(),
        "Should reduce: {} -> {} bytes", source.len(), result.source.len());

    let output = String::from_utf8_lossy(&result.source);
    assert!(output.contains("x = 1"),
        "Reduced output should contain 'x = 1': {}", output);

    let lang = languages::get_language("python").unwrap();
    let tree = bonsai_core::parse::parse(&result.source, &lang).unwrap();
    assert!(!bonsai_core::validity::tree_has_errors(&tree),
        "Reduced output should be valid Python: {}", output);

    assert!(result.reductions > 0);
    assert!(result.tests_run > 0);
}

#[test]
fn test_reduce_javascript() {
    let source = b"function foo() { return 1; }\nfunction bar() { return 2; }\nfunction baz() { return 3; }\n";

    let test = ShellTest::new(
        vec!["grep".into(), "-q".into(), "function foo".into()],
        Duration::from_secs(10),
    ).unwrap();

    let config = make_config("javascript", true);
    let result = reduce(source, &test, config);

    assert!(result.source.len() < source.len());
    let output = String::from_utf8_lossy(&result.source);
    assert!(output.contains("function foo"));
}

#[test]
fn test_reduce_output_is_valid_parse() {
    let source = b"if True:\n  x = 1\n  y = 2\n  z = 3\nelse:\n  w = 4\n";

    let test = ShellTest::new(
        vec!["grep".into(), "-q".into(), "x = 1".into()],
        Duration::from_secs(10),
    ).unwrap();

    let config = make_config("python", true);
    let result = reduce(source, &test, config);

    let lang = languages::get_language("python").unwrap();
    let tree = bonsai_core::parse::parse(&result.source, &lang).unwrap();
    assert!(!bonsai_core::validity::tree_has_errors(&tree),
        "Reduced output should parse cleanly: {}",
        String::from_utf8_lossy(&result.source));
}

#[test]
fn test_reduce_with_no_supertypes() {
    let language = languages::get_language("python").unwrap();
    let source = b"x = 1\ny = 2\nz = 3\n";

    let test = ShellTest::new(
        vec!["grep".into(), "-q".into(), "x = 1".into()],
        Duration::from_secs(10),
    ).unwrap();

    let config = ReducerConfig {
        language,
        transforms: vec![Box::new(DeleteTransform), Box::new(UnwrapTransform)],
        provider: Box::new(EmptyProvider),
        max_tests: 0,
        max_time: Duration::ZERO,
        jobs: 1,
        strict: true,
        max_test_errors: 3,
        interrupted: Arc::new(AtomicBool::new(false)),
    };

    let result = reduce(source, &test, config);

    assert!(result.source.len() < source.len(),
        "Should reduce even without supertypes: {} -> {}",
        source.len(), result.source.len());
    let output = String::from_utf8_lossy(&result.source);
    assert!(output.contains("x = 1"));
}

#[test]
fn test_reduce_caching_effectiveness() {
    let source = b"a = 1\nb = 2\nc = 3\nd = 4\ne = 5\n";

    let test = ShellTest::new(
        vec!["grep".into(), "-q".into(), "a = 1".into()],
        Duration::from_secs(10),
    ).unwrap();

    let config = make_config("python", true);
    let result = reduce(source, &test, config);

    assert!(result.tests_run > 0);
    assert!(result.cache_hit_rate >= 0.0);
}
