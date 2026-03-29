//! `bonsai-fuzz` — subprocess target execution harness and crash interest criteria for fuzz testing.
//!
//! # Defining crash criteria
//!
//! ```
//! use bonsai_fuzz::criteria::InterestCriteria;
//! use bonsai_fuzz::target::TargetResult;
//!
//! // Match any crash (non-zero exit or signal)
//! let criteria = InterestCriteria::any_crash();
//!
//! let crash = TargetResult {
//!     exit_code: Some(139),
//!     stderr: Vec::new(),
//!     timed_out: false,
//!     #[cfg(unix)]
//!     signal: None,
//! };
//! assert!(criteria.is_interesting(&crash));
//!
//! let success = TargetResult {
//!     exit_code: Some(0),
//!     stderr: Vec::new(),
//!     timed_out: false,
//!     #[cfg(unix)]
//!     signal: None,
//! };
//! assert!(!criteria.is_interesting(&success));
//! ```

pub mod criteria;
pub mod target;
