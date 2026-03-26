//! `bonsai-reduce` — reduction algorithm for minimizing tree-sitter parse trees.

pub mod cache;
pub mod interest;
pub mod queue;
pub mod reducer;

pub use cache::TestCache;
pub use interest::{InterestingnessTest, ShellTest};
