//! Perses-style syntax-guided test case reducer for tree-sitter grammars.
//!
//! # Reducing a source file programmatically
//!
//! ```
//! use std::sync::atomic::AtomicBool;
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! use bonsai_core::supertype::LanguageApiProvider;
//! use bonsai_core::transforms::delete::DeleteTransform;
//! use bonsai_core::transforms::unwrap::UnwrapTransform;
//! use bonsai_reduce::reducer::{reduce, ReducerConfig};
//! use bonsai_reduce::{InterestingnessTest, TestResult};
//!
//! // Define what makes an input "interesting" (still triggers the bug)
//! struct ContainsPattern(Vec<u8>);
//!
//! impl InterestingnessTest for ContainsPattern {
//!     fn test(&self, input: &[u8]) -> TestResult {
//!         if input.windows(self.0.len()).any(|w| w == self.0.as_slice()) {
//!             TestResult::Interesting
//!         } else {
//!             TestResult::NotInteresting
//!         }
//!     }
//! }
//!
//! let lang = bonsai_core::languages::get_language("python").unwrap();
//! let source = b"x = 1\ny = 2\nz = 3\nw = 4\n";
//!
//! let config = ReducerConfig {
//!     language: lang.clone(),
//!     transforms: vec![Box::new(DeleteTransform), Box::new(UnwrapTransform)],
//!     provider: Box::new(LanguageApiProvider::new(&lang)),
//!     max_tests: 0,       // unlimited
//!     max_time: Duration::ZERO, // unlimited
//!     jobs: 1,
//!     strict: true,
//!     interrupted: Arc::new(AtomicBool::new(false)),
//! };
//!
//! let test = ContainsPattern(b"x = 1".to_vec());
//! let result = reduce(source, &test, config);
//!
//! // The reducer removed everything except what's needed for "x = 1"
//! assert!(result.source.len() < source.len());
//! assert!(String::from_utf8_lossy(&result.source).contains("x = 1"));
//! assert!(result.reductions > 0);
//! ```
//!
//! # Using ShellTest for CLI-style interestingness tests
//!
//! ```no_run
//! use std::time::Duration;
//! use bonsai_reduce::{ShellTest, InterestingnessTest, TestResult};
//!
//! // "grep -q 'error'" exits 0 when the input contains "error"
//! let test = ShellTest::new(
//!     vec!["grep".into(), "-q".into(), "error".into()],
//!     Duration::from_secs(10),
//! );
//!
//! // The test writes input to a temp file and passes the path as an argument
//! assert_eq!(test.test(b"an error occurred\n"), TestResult::Interesting);
//! assert_eq!(test.test(b"all good\n"), TestResult::NotInteresting);
//! ```

pub mod cache;
pub mod interest;
pub mod output;
pub mod progress;
pub mod queue;
pub mod reducer;

pub use cache::TestCache;
pub use interest::{InterestingnessTest, ShellTest, TestResult};
pub use output::{write_output, OutputTarget};
