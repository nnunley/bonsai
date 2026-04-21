//! Property-based tests for bonsai-reduce using Hegel.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bonsai_core::supertype::LanguageApiProvider;
use bonsai_core::transforms::delete::DeleteTransform;
use bonsai_core::transforms::unwrap::UnwrapTransform;
use bonsai_reduce::cache::TestCache;
use bonsai_reduce::interest::{InterestingnessTest, TestResult};
use bonsai_reduce::queue::ReductionQueue;
use bonsai_reduce::reducer::{reduce, ReducerConfig};
use hegel::generators::integers;
use hegel::TestCase;

// --- Generators ---

/// Generate valid Python source from a pool of statements.
#[hegel::composite]
fn python_source(tc: TestCase) -> Vec<u8> {
    let pool: &[&[u8]] = &[
        b"x = 1",
        b"y = 2",
        b"z = 3",
        b"w = 4",
        b"if True:\n  pass",
        b"def f():\n  return 1",
        b"class C:\n  pass",
        b"print(\"hello\")",
    ];
    let count = tc.draw(integers::<usize>().min_value(2).max_value(8));
    let mut lines = Vec::new();
    for _ in 0..count {
        let idx = tc.draw(integers::<usize>().min_value(0).max_value(pool.len() - 1));
        lines.push(pool[idx]);
    }
    lines.join(&b'\n')
}

/// Generate valid Rust source from a pool of items.
#[hegel::composite]
fn rust_source(tc: TestCase) -> Vec<u8> {
    let pool: &[&[u8]] = &[
        b"fn main() {}",
        b"fn foo() {}",
        b"fn bar() -> i32 { 0 }",
        b"struct S;",
        b"const X: i32 = 1;",
        b"type T = i32;",
        b"static Y: i32 = 2;",
    ];
    let count = tc.draw(integers::<usize>().min_value(2).max_value(6));
    let mut items = Vec::new();
    for _ in 0..count {
        let idx = tc.draw(integers::<usize>().min_value(0).max_value(pool.len() - 1));
        items.push(pool[idx]);
    }
    items.join(&b'\n')
}

// --- Test helpers ---

/// In-process interestingness test: checks if a pattern is present.
struct ContainsTest {
    pattern: Vec<u8>,
    calls: Arc<AtomicUsize>,
}

impl ContainsTest {
    fn new(pattern: &[u8]) -> Self {
        Self {
            pattern: pattern.to_vec(),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

}

impl InterestingnessTest for ContainsTest {
    fn test(&self, input: &[u8]) -> TestResult {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.pattern.is_empty() {
            return TestResult::Interesting;
        }
        if input
            .windows(self.pattern.len())
            .any(|w| w == self.pattern.as_slice())
        {
            TestResult::Interesting
        } else {
            TestResult::NotInteresting
        }
    }
}

fn make_config(lang_name: &str, strict: bool) -> ReducerConfig {
    let language = bonsai_core::languages::get_language(lang_name).unwrap();
    let provider = LanguageApiProvider::new(&language);
    ReducerConfig {
        language,
        transforms: vec![Box::new(DeleteTransform), Box::new(UnwrapTransform)],
        provider: Box::new(provider),
        max_tests: 500, // cap to keep property tests fast
        max_time: Duration::from_secs(5),
        jobs: 1,
        strict,
        max_test_errors: 0,
        interrupted: Arc::new(AtomicBool::new(false)),
    }
}

// --- Reducer property: output never larger than input ---

#[hegel::test]
fn prop_reducer_monotonic_python(tc: TestCase) {
    let source = tc.draw(python_source());
    // Pick a line from the source to use as the interestingness pattern
    let lines: Vec<&[u8]> = source.split(|&b| b == b'\n').collect();
    if lines.is_empty() {
        return;
    }
    let idx = tc.draw(integers::<usize>().min_value(0).max_value(lines.len() - 1));
    let pattern = lines[idx];
    if pattern.is_empty() {
        return;
    }

    let test = ContainsTest::new(pattern);
    let config = make_config("python", true);
    let result = reduce(&source, &test, config, None);

    assert!(
        result.source.len() <= source.len(),
        "Reducer output ({}) should not be larger than input ({})",
        result.source.len(),
        source.len()
    );
}

#[hegel::test]
fn prop_reducer_monotonic_rust(tc: TestCase) {
    let source = tc.draw(rust_source());
    let lines: Vec<&[u8]> = source.split(|&b| b == b'\n').collect();
    if lines.is_empty() {
        return;
    }
    let idx = tc.draw(integers::<usize>().min_value(0).max_value(lines.len() - 1));
    let pattern = lines[idx];
    if pattern.is_empty() {
        return;
    }

    let test = ContainsTest::new(pattern);
    let config = make_config("rust", true);
    let result = reduce(&source, &test, config, None);

    assert!(
        result.source.len() <= source.len(),
        "Reducer output ({}) should not be larger than input ({})",
        result.source.len(),
        source.len()
    );
}

// --- Reducer property: output preserves interestingness ---

#[hegel::test]
fn prop_reducer_preserves_interest_python(tc: TestCase) {
    let source = tc.draw(python_source());
    let lines: Vec<&[u8]> = source.split(|&b| b == b'\n').collect();
    if lines.is_empty() {
        return;
    }
    let idx = tc.draw(integers::<usize>().min_value(0).max_value(lines.len() - 1));
    let pattern = lines[idx];
    if pattern.is_empty() {
        return;
    }

    let test = ContainsTest::new(pattern);
    let config = make_config("python", true);
    let result = reduce(&source, &test, config, None);

    // The output must still be interesting
    assert_eq!(
        test.test(&result.source),
        TestResult::Interesting,
        "Reduced output should still be interesting. Pattern: {:?}, Output: {:?}",
        String::from_utf8_lossy(pattern),
        String::from_utf8_lossy(&result.source)
    );
}

#[hegel::test]
fn prop_reducer_preserves_interest_rust(tc: TestCase) {
    let source = tc.draw(rust_source());
    let lines: Vec<&[u8]> = source.split(|&b| b == b'\n').collect();
    if lines.is_empty() {
        return;
    }
    let idx = tc.draw(integers::<usize>().min_value(0).max_value(lines.len() - 1));
    let pattern = lines[idx];
    if pattern.is_empty() {
        return;
    }

    let test = ContainsTest::new(pattern);
    let config = make_config("rust", true);
    let result = reduce(&source, &test, config, None);

    assert_eq!(
        test.test(&result.source),
        TestResult::Interesting,
        "Reduced output should still be interesting. Pattern: {:?}, Output: {:?}",
        String::from_utf8_lossy(pattern),
        String::from_utf8_lossy(&result.source)
    );
}

// --- Reducer property: parallel mode also monotonic and preserving ---

#[hegel::test]
fn prop_reducer_parallel_monotonic_python(tc: TestCase) {
    let source = tc.draw(python_source());
    let lines: Vec<&[u8]> = source.split(|&b| b == b'\n').collect();
    if lines.is_empty() {
        return;
    }
    let idx = tc.draw(integers::<usize>().min_value(0).max_value(lines.len() - 1));
    let pattern = lines[idx];
    if pattern.is_empty() {
        return;
    }

    let test = ContainsTest::new(pattern);
    let mut config = make_config("python", true);
    config.jobs = 2;
    let result = reduce(&source, &test, config, None);

    assert!(
        result.source.len() <= source.len(),
        "Parallel reducer output ({}) should not be larger than input ({})",
        result.source.len(),
        source.len()
    );

    assert_eq!(
        test.test(&result.source),
        TestResult::Interesting,
        "Parallel reduced output should still be interesting"
    );
}

// --- Cache property: get-after-put returns the stored value ---

#[hegel::test]
fn prop_cache_get_after_put(tc: TestCase) {
    let mut cache = TestCache::new();

    let key: Vec<u8> = tc.draw(hegel::generators::vecs(integers::<u8>()).min_size(1).max_size(100));
    let value = tc.draw(hegel::generators::booleans());

    cache.put(&key, value);
    let result = cache.get(&key);

    assert_eq!(
        result,
        Some(value),
        "Cache should return the stored value"
    );
}

#[hegel::test]
fn prop_cache_overwrite(tc: TestCase) {
    let mut cache = TestCache::new();

    let key: Vec<u8> = tc.draw(hegel::generators::vecs(integers::<u8>()).min_size(1).max_size(100));

    cache.put(&key, true);
    cache.put(&key, false);
    let result = cache.get(&key);

    assert_eq!(
        result,
        Some(false),
        "Cache should return the last stored value after overwrite"
    );
}

// --- Queue property: entries pop in non-increasing token count order ---

#[hegel::test]
fn prop_queue_ordering_python(tc: TestCase) {
    let source = tc.draw(python_source());
    let lang = bonsai_core::languages::get_language("python").unwrap();
    let tree = match bonsai_core::parse::parse(&source, &lang) {
        Some(t) => t,
        None => return,
    };

    let mut queue = ReductionQueue::from_tree(&tree);
    let mut prev_count = usize::MAX;

    while let Some(entry) = queue.pop() {
        assert!(
            entry.token_count <= prev_count,
            "Queue should pop in non-increasing token count order: got {} after {}",
            entry.token_count,
            prev_count
        );
        prev_count = entry.token_count;
    }
}

#[hegel::test]
fn prop_queue_ordering_rust(tc: TestCase) {
    let source = tc.draw(rust_source());
    let lang = bonsai_core::languages::get_language("rust").unwrap();
    let tree = match bonsai_core::parse::parse(&source, &lang) {
        Some(t) => t,
        None => return,
    };

    let mut queue = ReductionQueue::from_tree(&tree);
    let mut prev_count = usize::MAX;

    while let Some(entry) = queue.pop() {
        assert!(
            entry.token_count <= prev_count,
            "Queue should pop in non-increasing token count order: got {} after {}",
            entry.token_count,
            prev_count
        );
        prev_count = entry.token_count;
    }
}
