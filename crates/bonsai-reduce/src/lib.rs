//! `bonsai-reduce` — reduction algorithm for minimizing tree-sitter parse trees.

pub mod cache;
pub mod interest;

pub use cache::TestCache;
pub use interest::{InterestingnessTest, ShellTest};
