//! Integration test: bonsai reduces a Python program with an off-by-one bug
//! to its minimal form while preserving the wrong output.
//!
//! The bug: process_items uses range(len(items) - 1), so for input [1, 2, 3, 4, 5]
//! it produces [2, 4, 6, 8] instead of [2, 4, 6, 8, 10] — the last item is skipped.
//!
//! The interestingness test checks that the program outputs "[2, 4, 6, 8]" (the wrong
//! answer). Bonsai must preserve the buggy computation to maintain this output, so the
//! reduced program contains the root cause.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use bonsai_core::languages;
use bonsai_core::supertype::LanguageApiProvider;
use bonsai_core::transforms::delete::DeleteTransform;
use bonsai_core::transforms::unwrap::UnwrapTransform;
use bonsai_reduce::interest::{InterestingnessTest, TestResult};
use bonsai_reduce::reducer::{reduce, ReducerConfig};

/// Interestingness test: the program must produce exactly "[2, 4, 6, 8]" on stdout.
/// This is the wrong output from the off-by-one bug — [1,2,3,4,5] doubled should be
/// [2, 4, 6, 8, 10], but the bug skips the last element.
struct ProducesWrongOutput;

impl InterestingnessTest for ProducesWrongOutput {
    fn test(&self, input: &[u8]) -> TestResult {
        use std::process::{Command, Stdio};

        let mut child = match Command::new("python3")
            .arg("-c")
            .arg(String::from_utf8_lossy(input).as_ref())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return TestResult::Error(format!("failed to spawn python3: {e}")),
        };

        match child.wait_with_output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // The wrong output: 4 items instead of 5
                if stdout.trim() == "[2, 4, 6, 8]" {
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
    // A Python program with an off-by-one bug buried in surrounding code.
    // process_items uses range(len(items) - 1), skipping the last item.
    // The program prints the wrong result: [2, 4, 6, 8] instead of [2, 4, 6, 8, 10].
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

def format_output(data):
    return str(data)

def log(msg):
    if DEBUG:
        print(f"LOG: {msg}", file=sys.stderr)

def main():
    setup()
    log("starting")
    data = [1, 2, 3, 4, 5]
    result = process_items(data)
    log(f"processed {len(result)} items")
    output = format_output(result)
    print(output)
    log("done")
    teardown()

if __name__ == "__main__":
    main()
"#;

    let test = ProducesWrongOutput;
    let config = make_config();

    // Verify the original produces the wrong output
    assert_eq!(
        test.test(source),
        TestResult::Interesting,
        "Original program should produce '[2, 4, 6, 8]'"
    );

    let result = reduce(source, &test, config, None);

    // The reduced output should still produce the wrong answer
    assert_eq!(
        test.test(&result.source),
        TestResult::Interesting,
        "Reduced program should still produce '[2, 4, 6, 8]'"
    );

    let output = String::from_utf8_lossy(&result.source);

    // Should have reduced significantly — removed imports, setup, teardown, log, etc.
    assert!(
        result.source.len() < source.len() / 2,
        "Should reduce by at least 50%: {} -> {} bytes.\nReduced:\n{}",
        source.len(),
        result.source.len(),
        output
    );

    // The reduced code must still contain the core bug: the off-by-one in range()
    // and the data that triggers it
    assert!(
        output.contains("range"),
        "Reduced output should preserve the range() call (the bug site):\n{}",
        output
    );

    eprintln!(
        "\n=== Reduction result ({} -> {} bytes, {:.1}% reduced) ===",
        source.len(),
        result.source.len(),
        100.0 * (1.0 - result.source.len() as f64 / source.len() as f64)
    );
    eprintln!("{}", output);
    eprintln!(
        "=== {} tests run, {} reductions ===",
        result.tests_run, result.reductions
    );
}
