## 1. Initial Input Validation
- [ ] 1.1 Add initial input check to `reduce()` — call `test()` on source before starting
- [ ] 1.2 On `NotInteresting`: return early with tests_run=1 and original source unchanged
- [ ] 1.3 On `Error(msg)`: return immediately with a fatal error (test infrastructure broken)
- [ ] 1.4 Add test: input that returns `NotInteresting` returns immediately with tests_run=1
- [ ] 1.5 Add test: input that returns `Error` returns immediately with fatal error

## 2. Progress Callback Trait and Reporting
- [ ] 2.1 Define `ProgressCallback` trait in bonsai-core with methods:
  - `on_update(&self, stats: &ProgressStats)` — periodic stats during reduction
  - `on_candidate(&self, kind: &str, accepted: bool)` — per-candidate notification
  - `on_warning(&self, msg: &str)` — warnings (reparse failure, test errors)
- [ ] 2.2 Add `callback: Option<Box<dyn ProgressCallback>>` field to `ReducerConfig`
- [ ] 2.3 Call the callback after each accepted reduction and periodically during candidate testing
- [ ] 2.4 `ProgressReporter` in bonsai-cli implements `ProgressCallback` with interior mutability for rate-limiting
- [ ] 2.5 Remove `ProgressReporter::report_final` — CLI prints final summary directly from `ReducerResult` fields
- [ ] 2.6 Add test: verify callback is invoked during multi-step reduction

## 3. No-Supertype Warning
- [ ] 3.1 Add `log` or `tracing` crate dependency (or use `eprintln!` for simplicity)
- [ ] 3.2 In the CLI, check if `LanguageApiProvider::has_supertypes()` is false and warn
- [ ] 3.3 Add test: grammar with no supertypes produces a warning

## 4. Parse `locals` Field in build.rs
- [ ] 4.1 Add `locals: Option<String>` to `LanguageEntry` struct in build.rs
- [ ] 4.2 Include `locals_scm` (embedded content) in the generated `LanguageInfo` struct
- [ ] 4.3 Add test: verify `LanguageInfo` for JavaScript includes locals content

## 5. Update Specs to Match Implementation
- [ ] 5.1 Update validity spec to document content-based error tracking
- [ ] 5.2 Update validity spec to document replacement bounds validation
- [ ] 5.3 Update reducer spec to document final output re-verification

## 6. TestResult Enum and InterestingnessTest API Change
- [ ] 6.1 Define `TestResult` enum: `Interesting`, `NotInteresting`, `Error(String)`
- [ ] 6.2 Change `InterestingnessTest` trait from `is_interesting(&[u8]) -> bool` to `test(&[u8]) -> TestResult`
- [ ] 6.3 Update `ShellTest::test` to return `Error(msg)` on spawn/write/flush/timeout failures
- [ ] 6.4 Update `ContainsTest` to return `Interesting`/`NotInteresting`
- [ ] 6.5 Update all integration tests and doctests that implement `InterestingnessTest`
- [ ] 6.6 Add test: `ShellTest` returns `Error` on spawn failure (e.g., nonexistent command)

## 7. ShellTest::new Returns Result
- [ ] 7.1 Change `ShellTest::new` signature to return `Result<Self, String>`
- [ ] 7.2 Validate args is non-empty — return `Err` with descriptive message
- [ ] 7.3 Update all call sites (CLI parses shell command, constructs ShellTest, propagates error)
- [ ] 7.4 Add test: empty args returns error

## 8. Error Tolerance in Reducer Loop
- [ ] 8.1 Add `max_test_errors: usize` field to `ReducerConfig` (default 3)
- [ ] 8.2 Expose `--max-test-errors <N>` in CLI
- [ ] 8.3 Implement consecutive error counter in reducer loop:
  - `Interesting` → accept, reset counter
  - `NotInteresting` → skip, reset counter
  - `Error` → increment counter, warn via progress callback, abort when exceeding threshold
- [ ] 8.4 Add test: reducer aborts after N consecutive errors
- [ ] 8.5 Add test: non-consecutive errors do not trigger abort

## 9. InterruptFlag Encapsulation
- [ ] 9.1 Create `InterruptFlag` struct in bonsai-cli encapsulating `Arc<AtomicBool>` and handler registration
- [ ] 9.2 `InterruptFlag::new() -> Result<Self, ctrlc::Error>` — propagates handler registration error
- [ ] 9.3 Replace scattered Arc creation / `ctrlc::set_handler(...).ok()` / config construction in main.rs
- [ ] 9.4 CLI uses `?` or `unwrap_or_else` instead of `.ok()` for handler registration

## 10. Unwrap Removal on Reparse
- [ ] 10.1 Replace `.unwrap()` on `parse()` at reducer.rs:165 and :186 with match/if-let
- [ ] 10.2 On `None`: skip the candidate (do not update current_source/tree)
- [ ] 10.3 Log warning via progress callback
- [ ] 10.4 Add test: reparse failure skips candidate without panicking
