## Why

Code review identified several gaps between the reducer spec and implementation:
1. Reducer does not verify the initial input passes the interestingness test before starting
2. Progress reporting exists as a module but is not called during the reduction loop (only final summary)
3. No warning logged when a grammar has no supertypes
4. `locals` field in grammars.toml is not parsed by build.rs
5. Replacement bounds validation was missing (now fixed, spec should document it)
6. Error tracking in lenient mode used byte positions (now fixed to content+kind, spec should reflect)

## What Changes

- Add initial input validation to reducer spec
- Add progress callback mechanism to reducer
- Add no-supertype warning requirement
- Add `locals` field parsing to build.rs grammar entry
- Update validity spec to document bounds checking and content-based error tracking
- Replace `InterestingnessTest::is_interesting` with `test() -> TestResult` enum (`Interesting`, `NotInteresting`, `Error(String)`)
- Add error tolerance to reducer loop via `max_test_errors` config
- `ShellTest::new` returns `Result<Self, String>` with arg validation
- Remove `ProgressReporter::report_final` — CLI handles final summary directly
- Encapsulate ctrlc handler in `InterruptFlag` struct that propagates registration errors
- Remove `.unwrap()` on reparse — skip candidate on `None`

## Impact

- **Breaking change**: `InterestingnessTest` trait changes from `is_interesting(&[u8]) -> bool` to `test(&[u8]) -> TestResult` — all implementors (`ShellTest`, `ContainsTest`, integration tests, doctests) must update
- Reducer behavior change: fails fast if initial input is not interesting
- Reducer behavior change: consecutive test errors trigger abort instead of silent `false`
- Progress: users see periodic updates during long reductions, not just final summary
- Build: `locals` field is parsed (no runtime behavior change yet — ScopeAnalysis is a separate change)
