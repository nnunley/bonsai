//! Integration test: bonsai reduces a Python program with an off-by-one bug
//! to its minimal form while preserving the bug-triggering code.
//!
//! Demonstrates bonsai's core value: given a large program that triggers a bug,
//! automatically produce the smallest program that still triggers it.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use bonsai_core::languages;
use bonsai_core::supertype::LanguageApiProvider;
use bonsai_core::transforms::delete::DeleteTransform;
use bonsai_core::transforms::unwrap::UnwrapTransform;
use bonsai_reduce::interest::{InterestingnessTest, TestResult};
use bonsai_reduce::reducer::{reduce, ReducerConfig};

/// Interestingness test: the source must be valid Python that, when executed,
/// outputs "off by one" in its stderr (from the ValueError). This is tighter
/// than just checking for any ValueError — it requires the specific message
/// from the validate() function catching the process_items() bug.
struct RaisesOffByOne;

impl InterestingnessTest for RaisesOffByOne {
    fn test(&self, input: &[u8]) -> TestResult {
        use std::process::{Command, Stdio};

        let mut child = match Command::new("python3")
            .arg("-c")
            .arg(String::from_utf8_lossy(input).as_ref())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return TestResult::Error(format!("failed to spawn python3: {e}")),
        };

        match child.wait_with_output() {
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Check for the specific error message from our validate() function
                if stderr.contains("off by one") {
                    TestResult::Interesting
                } else {
                    TestResult::NotInteresting
                }
            }
            Err(e) => TestResult::Error(format!("failed to wait: {e}")),
        }
    }
}

fn make_config() -> ReducerConfig {
    let language = languages::get_language("python").unwrap();
    let provider = LanguageApiProvider::new(&language);
    ReducerConfig {
        language,
        transforms: vec![Box::new(DeleteTransform), Box::new(UnwrapTransform)],
        provider: Box::new(provider),
        max_tests: 0,
        max_time: Duration::from_secs(30),
        jobs: 1,
        strict: true,
        max_test_errors: 3,
        interrupted: Arc::new(AtomicBool::new(false)),
    }
}

#[test]
fn test_reduces_off_by_one_program() {
    // A Python program with a deliberate off-by-one bug buried in surrounding code.
    // The bug is in process_items: range(len(items) - 1) skips the last item.
    // validate() catches this and raises ValueError.
    let source = br#"
import sys
import os

DEBUG = False
VERSION = "1.0.0"

def setup():
    pass

def teardown():
    pass

def helper(x):
    return x * 2

def process_items(items):
    results = []
    for i in range(len(items) - 1):
        results.append(items[i] * 2)
    return results

def validate(items):
    processed = process_items(items)
    if len(processed) != len(items):
        raise ValueError("off by one")
    return processed

def format_output(data):
    return str(data)

def main():
    setup()
    data = [1, 2, 3]
    result = validate(data)
    output = format_output(result)
    print(output)
    teardown()

if __name__ == "__main__":
    main()
"#;

    let test = RaisesOffByOne;
    let config = make_config();

    // Verify the original triggers the bug
    assert_eq!(test.test(source), TestResult::Interesting,
        "Original program should trigger 'off by one' ValueError");

    let result = reduce(source, &test, config, None);

    // Should have reduced significantly
    assert!(
        result.source.len() < source.len(),
        "Should reduce: {} -> {} bytes",
        source.len(),
        result.source.len()
    );

    // The reduced output should still trigger the bug
    assert_eq!(
        test.test(&result.source),
        TestResult::Interesting,
        "Reduced program should still trigger 'off by one' ValueError"
    );

    // Should have removed the irrelevant code (imports, setup, teardown, etc.)
    let output = String::from_utf8_lossy(&result.source);
    assert!(
        result.source.len() < source.len() / 2,
        "Should reduce by at least 50%: {} -> {} bytes.\nReduced:\n{}",
        source.len(),
        result.source.len(),
        output
    );

    // The essential error message must remain in the source
    assert!(
        output.contains("off by one"),
        "Reduced output should contain the error message:\n{}",
        output
    );

    eprintln!("\n=== Reduction result ({} -> {} bytes, {:.1}% reduced) ===",
        source.len(), result.source.len(),
        100.0 * (1.0 - result.source.len() as f64 / source.len() as f64));
    eprintln!("{}", output);
    eprintln!("=== {} tests run, {} reductions ===", result.tests_run, result.reductions);
}
