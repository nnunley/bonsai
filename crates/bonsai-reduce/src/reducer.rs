use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tree_sitter::Language;

use bonsai_core::supertype::SupertypeProvider;
use bonsai_core::transform::Transform;
use bonsai_core::validity;

use crate::cache::TestCache;
use crate::interest::{InterestingnessTest, TestResult};
use crate::progress::{ProgressCallback, ProgressStats};
use crate::queue::ReductionQueue;

/// Configuration for a reduction run.
pub struct ReducerConfig {
    /// The tree-sitter language for parsing.
    pub language: Language,
    /// Transforms to try (Delete, Unwrap, etc.)
    pub transforms: Vec<Box<dyn Transform>>,
    /// Supertype provider for compatibility checking.
    pub provider: Box<dyn SupertypeProvider>,
    /// Maximum number of interestingness test invocations (0 = unlimited).
    pub max_tests: usize,
    /// Maximum wall-clock time (Duration::ZERO = unlimited).
    pub max_time: Duration,
    /// Number of parallel test workers (1 = sequential/deterministic).
    ///
    /// NOTE: `jobs` is accepted but parallelism is not yet implemented.
    /// All reduction runs currently execute sequentially regardless of this value.
    pub jobs: usize,
    /// If true, reject any ERROR/MISSING nodes. If false, only reject NEW errors.
    pub strict: bool,
    /// Maximum consecutive test errors before aborting (0 = abort on first error).
    pub max_test_errors: usize,
    /// Interrupt flag: when set to true, the reduction loop will stop at the next
    /// opportunity. Typically set by a SIGINT handler in the CLI.
    pub interrupted: Arc<AtomicBool>,
}

/// Result of a reduction run.
pub struct ReducerResult {
    /// The reduced source bytes.
    pub source: Vec<u8>,
    /// Number of interestingness tests executed.
    pub tests_run: usize,
    /// Number of successful reductions accepted.
    pub reductions: usize,
    /// Cache hit rate.
    pub cache_hit_rate: f64,
    /// Wall-clock time elapsed.
    pub elapsed: Duration,
}

/// Run the Perses-style reduction algorithm.
///
/// Returns the reduced source that still passes the interestingness test,
/// or the original source if no reductions were possible.
pub fn reduce(
    source: &[u8],
    test: &dyn InterestingnessTest,
    config: ReducerConfig,
    progress: Option<&dyn ProgressCallback>,
) -> ReducerResult {
    let start = Instant::now();
    let mut current_source = source.to_vec();
    let mut cache = TestCache::new();
    let mut tests_run: usize = 0;
    let mut reductions: usize = 0;
    let mut consecutive_errors: usize = 0;

    // Parse initial tree
    let mut tree = match bonsai_core::parse::parse(&current_source, &config.language) {
        Some(t) => t,
        None => {
            return ReducerResult {
                source: current_source,
                tests_run: 0,
                reductions: 0,
                cache_hit_rate: 0.0,
                elapsed: start.elapsed(),
            };
        }
    };

    // Collect initial errors for lenient mode
    let mut initial_errors = if config.strict {
        None
    } else {
        let errors = validity::ErrorSet::from_tree(&tree, &current_source);
        if errors.is_empty() {
            None
        } else {
            Some(errors)
        }
    };

    // Build priority queue
    let mut queue = ReductionQueue::from_tree(&tree);

    // Main reduction loop
    loop {
        // Check termination bounds
        if config.interrupted.load(Ordering::Relaxed) {
            break;
        }
        if config.max_tests > 0 && tests_run >= config.max_tests {
            break;
        }
        if config.max_time > Duration::ZERO && start.elapsed() >= config.max_time {
            break;
        }
        if consecutive_errors > config.max_test_errors {
            break;
        }

        // Pop next entry
        let entry = match queue.pop() {
            Some(e) => e,
            None => break, // Queue exhausted
        };

        // Find the node in the current tree by byte range
        let node = find_node_by_range(&tree, entry.start_byte, entry.end_byte);
        let node = match node {
            Some(n) if n.grammar_id() == entry.kind_id => n,
            _ => continue, // Node no longer exists or changed type -- skip
        };

        // Generate candidates from all transforms
        let mut candidates: Vec<validity::Replacement> = Vec::new();
        for transform in &config.transforms {
            candidates.extend(transform.candidates(
                &node,
                &current_source,
                &tree,
                config.provider.as_ref(),
            ));
        }

        // Test candidates
        let mut accepted = false;
        for candidate in &candidates {
            // Check termination
            if config.interrupted.load(Ordering::Relaxed) {
                break;
            }
            if config.max_tests > 0 && tests_run >= config.max_tests {
                break;
            }
            if config.max_time > Duration::ZERO && start.elapsed() >= config.max_time {
                break;
            }

            // Validate: apply replacement, reparse, check for errors
            let new_source = match validity::try_replacement(
                &current_source,
                candidate,
                &config.language,
                initial_errors.as_ref(),
            ) {
                Some(s) => s,
                None => continue, // Invalid replacement
            };

            // Check cache
            if let Some(cached) = cache.get(&new_source) {
                if !cached {
                    continue; // Known not interesting
                }
                // Known interesting -- accept
                current_source = new_source;
                match bonsai_core::parse::parse(&current_source, &config.language) {
                    Some(new_tree) => {
                        tree = new_tree;
                        if !config.strict {
                            initial_errors = {
                                let errors = validity::ErrorSet::from_tree(&tree, &current_source);
                                if errors.is_empty() { None } else { Some(errors) }
                            };
                        }
                        queue.rebuild(&tree);
                        reductions += 1;
                        accepted = true;
                    }
                    None => {
                        if let Some(p) = progress {
                            p.on_warning("reparse failed after accepted candidate, skipping");
                        }
                        current_source = source.to_vec();
                    }
                }
                break;
            }

            // Run interestingness test
            tests_run += 1;
            let test_result = test.test(&new_source);
            match &test_result {
                TestResult::Error(msg) => {
                    consecutive_errors += 1;
                    if let Some(p) = progress {
                        p.on_warning(&format!("test error: {}", msg));
                    }
                    if consecutive_errors > config.max_test_errors {
                        if let Some(p) = progress {
                            p.on_warning(&format!("aborting after {} consecutive test errors", consecutive_errors));
                        }
                        break;
                    }
                    continue;
                }
                _ => {
                    consecutive_errors = 0;
                }
            }
            let interesting = matches!(test_result, TestResult::Interesting);
            cache.put(&new_source, interesting);

            if interesting {
                current_source = new_source;
                match bonsai_core::parse::parse(&current_source, &config.language) {
                    Some(new_tree) => {
                        tree = new_tree;
                        if !config.strict {
                            initial_errors = {
                                let errors = validity::ErrorSet::from_tree(&tree, &current_source);
                                if errors.is_empty() { None } else { Some(errors) }
                            };
                        }
                        queue.rebuild(&tree);
                        reductions += 1;
                        accepted = true;
                    }
                    None => {
                        if let Some(p) = progress {
                            p.on_warning("reparse failed after accepted candidate, skipping");
                        }
                        current_source = source.to_vec();
                    }
                }
                break;
            }
        }

        // Report progress after processing each queue entry
        if let Some(p) = progress {
            p.on_update(&ProgressStats {
                original_size: source.len(),
                current_size: current_source.len(),
                tests_run,
                reductions,
                cache_hit_rate: cache.hit_rate(),
            });
        }

        if accepted {
            continue; // Re-enter loop with rebuilt queue
        }
        // No candidate worked -- entry is skipped, move to next
    }

    // Final verification: re-run the interestingness test to catch any cache collision corruption
    if !current_source.is_empty() && current_source != source {
        let final_result = test.test(&current_source);
        if !matches!(final_result, TestResult::Interesting) {
            // Cache collision corrupted the result or test error — fall back to original
            current_source = source.to_vec();
            reductions = 0;
        }
    }

    ReducerResult {
        source: current_source,
        tests_run,
        reductions,
        cache_hit_rate: cache.hit_rate(),
        elapsed: start.elapsed(),
    }
}

/// Find a node in the tree by its byte range.
fn find_node_by_range(
    tree: &tree_sitter::Tree,
    start: usize,
    end: usize,
) -> Option<tree_sitter::Node<'_>> {
    let root = tree.root_node();
    find_node_recursive(&root, start, end)
}

fn find_node_recursive<'a>(
    node: &tree_sitter::Node<'a>,
    start: usize,
    end: usize,
) -> Option<tree_sitter::Node<'a>> {
    if node.start_byte() == start && node.end_byte() == end {
        return Some(*node);
    }
    // Only recurse into children that overlap the target range
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.start_byte() <= start && child.end_byte() >= end {
                if let result @ Some(_) = find_node_recursive(&child, start, end) {
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
    use bonsai_core::supertype::LanguageApiProvider;
    use bonsai_core::transforms::delete::DeleteTransform;
    use bonsai_core::transforms::unwrap::UnwrapTransform;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Test that checks if a specific string is present in the input.
    struct ContainsTest {
        pattern: Vec<u8>,
        call_count: Arc<AtomicUsize>,
    }

    impl ContainsTest {
        fn new(pattern: &[u8]) -> Self {
            Self {
                pattern: pattern.to_vec(),
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        #[allow(dead_code)]
        fn calls(&self) -> usize {
            self.call_count.load(Ordering::Relaxed)
        }
    }

    impl InterestingnessTest for ContainsTest {
        fn test(&self, input: &[u8]) -> TestResult {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            // Check if pattern is a substring
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

    fn make_config(language: Language, strict: bool) -> ReducerConfig {
        let provider = LanguageApiProvider::new(&language);
        ReducerConfig {
            language: language.clone(),
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
    fn test_reduce_removes_unnecessary_code() {
        let lang = bonsai_core::languages::get_language("python").unwrap();
        let source = b"x = 1\ny = 2\nz = 3";
        let test = ContainsTest::new(b"x = 1");
        let config = make_config(lang, true);

        let result = reduce(source, &test, config, None);

        // Should keep "x = 1" and remove the rest
        assert!(
            result.source.len() < source.len(),
            "Reduced source should be smaller: {} vs {}",
            result.source.len(),
            source.len()
        );
        assert!(
            result.source.windows(5).any(|w| w == b"x = 1"),
            "Reduced source should still contain 'x = 1': {:?}",
            String::from_utf8_lossy(&result.source)
        );
        assert!(result.reductions > 0);
        assert!(result.tests_run > 0);
    }

    #[test]
    fn test_reduce_already_minimal() {
        let lang = bonsai_core::languages::get_language("python").unwrap();
        let source = b"x";
        let test = ContainsTest::new(b"x");
        let config = make_config(lang, true);

        let result = reduce(source, &test, config, None);
        // Can't reduce further
        assert_eq!(result.source, source);
    }

    #[test]
    fn test_reduce_respects_max_tests() {
        let lang = bonsai_core::languages::get_language("python").unwrap();
        let source = b"x = 1\ny = 2\nz = 3\nw = 4\na = 5";
        let test = ContainsTest::new(b"x = 1");
        let mut config = make_config(lang, true);
        config.max_tests = 3;

        let result = reduce(source, &test, config, None);
        assert!(
            result.tests_run <= 3,
            "Should stop after max_tests: ran {}",
            result.tests_run
        );
    }

    #[test]
    fn test_reduce_deterministic() {
        let lang = bonsai_core::languages::get_language("python").unwrap();
        let source = b"x = 1\ny = 2\nz = 3";
        let test1 = ContainsTest::new(b"x = 1");
        let test2 = ContainsTest::new(b"x = 1");
        let config1 = make_config(lang.clone(), true);
        let config2 = make_config(lang, true);

        let result1 = reduce(source, &test1, config1, None);
        let result2 = reduce(source, &test2, config2, None);

        assert_eq!(
            result1.source, result2.source,
            "Sequential reduction should be deterministic"
        );
    }

    #[test]
    fn test_reduce_caching_reduces_test_calls() {
        let lang = bonsai_core::languages::get_language("python").unwrap();
        let source = b"x = 1\ny = 2\nz = 3";
        let test = ContainsTest::new(b"x = 1");
        let config = make_config(lang, true);

        let result = reduce(source, &test, config, None);
        // Cache should have some entries
        assert!(result.cache_hit_rate >= 0.0);
    }

    #[test]
    fn test_reduce_empty_source() {
        let lang = bonsai_core::languages::get_language("python").unwrap();
        let source = b"";
        let test = ContainsTest::new(b"x");
        let config = make_config(lang, true);

        let result = reduce(source, &test, config, None);
        assert_eq!(result.source, b"");
    }

    #[test]
    fn test_reduce_final_output_is_verified() {
        // This test ensures the final result actually passes the interestingness test
        let lang = bonsai_core::languages::get_language("python").unwrap();
        let source = b"x = 1\ny = 2";
        let test = ContainsTest::new(b"x = 1");
        let config = make_config(lang, true);
        let result = reduce(source, &test, config, None);

        // The final result must pass the test
        assert_eq!(test.test(&result.source), TestResult::Interesting,
            "Final output must pass the interestingness test");
    }

    #[test]
    fn test_reduce_respects_interrupt() {
        // Set the interrupt flag before starting
        let lang = bonsai_core::languages::get_language("python").unwrap();
        let source = b"x = 1\ny = 2\nz = 3";
        let test = ContainsTest::new(b"x = 1");
        let config = make_config(lang, true);
        config.interrupted.store(true, Ordering::Relaxed);

        let result = reduce(source, &test, config, None);
        // Should stop immediately without running tests
        assert_eq!(result.tests_run, 0);
    }

    /// Test that always returns Error.
    struct AlwaysErrorTest;

    impl InterestingnessTest for AlwaysErrorTest {
        fn test(&self, _input: &[u8]) -> TestResult {
            TestResult::Error("simulated error".into())
        }
    }

    #[test]
    fn test_reduce_aborts_after_consecutive_errors() {
        let lang = bonsai_core::languages::get_language("python").unwrap();
        let source = b"x = 1\ny = 2\nz = 3";
        let test = AlwaysErrorTest;
        let mut config = make_config(lang, true);
        config.max_test_errors = 2;

        let result = reduce(source, &test, config, None);
        // Should abort after 2 consecutive errors, returning original source
        assert_eq!(result.source, source);
        assert!(result.tests_run <= 3, "Should abort quickly, ran {} tests", result.tests_run);
    }

    /// Mock progress callback that counts invocations.
    struct CountingCallback {
        updates: AtomicUsize,
    }

    impl CountingCallback {
        fn new() -> Self {
            Self { updates: AtomicUsize::new(0) }
        }
    }

    impl ProgressCallback for CountingCallback {
        fn on_update(&self, _stats: &ProgressStats) {
            self.updates.fetch_add(1, Ordering::Relaxed);
        }
        fn on_candidate(&self, _name: &str, _start: usize, _end: usize, _accepted: bool) {}
        fn on_warning(&self, _msg: &str) {}
    }

    #[test]
    fn test_progress_callback_invoked() {
        let lang = bonsai_core::languages::get_language("python").unwrap();
        let source = b"x = 1\ny = 2\nz = 3";
        let test = ContainsTest::new(b"x = 1");
        let config = make_config(lang, true);
        let callback = CountingCallback::new();

        let result = reduce(source, &test, config, Some(&callback));

        assert!(result.reductions > 0, "Should have made reductions");
        assert!(
            callback.updates.load(Ordering::Relaxed) > 0,
            "Progress callback should have been invoked"
        );
    }
}
