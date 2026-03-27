## 1. FuzzTarget Fixes (F-2, F-3)
- [ ] 1.1 Add `wait-timeout` crate as a dependency of `bonsai-fuzz`
- [ ] 1.2 Replace `child.wait_with_output()` in `run_stdin()` with `wait_timeout(self.timeout)`; on timeout, kill child and set `timed_out: true`
- [ ] 1.3 Replace `Command::output()` in `run_with_file()` with spawned child + `wait_timeout(self.timeout)`; same timeout handling
- [ ] 1.4 Add tests: verify timeout triggers kill and sets `timed_out: true`
- [ ] 1.5 Change `FuzzTarget::run` return type from `TargetResult` to `Result<TargetResult, TargetError>`
- [ ] 1.6 Remove `TargetResult::error()` sentinel constructor
- [ ] 1.7 Convert all spawn/write failures in `run_stdin()` and `run_with_file()` to `Err(TargetError { message })`
- [ ] 1.8 Update `target.rs` tests to unwrap the `Result`
- [ ] 1.9 Verify `criteria.rs` callers are unaffected (they take `&TargetResult` — caller unwraps first)

## 2. Dead Code Cleanup (F-11, F-12, F-15)
- [ ] 2.1 Remove from `bonsai-core/Cargo.toml` `[dependencies]`: `thiserror`, `serde` (both only used in build.rs via `[build-dependencies]`)
- [ ] 2.2 Remove from `bonsai-reduce/Cargo.toml`: `thiserror`, `libc`
- [ ] 2.3 Remove from `bonsai-fuzz/Cargo.toml`: `bonsai-core`, `bonsai-reduce`, `xxhash-rust`, `serde`, `thiserror`
- [ ] 2.4 Remove from workspace `Cargo.toml` `[workspace.dependencies]`: `thiserror`, `libc`
- [ ] 2.5 Run `cargo check --workspace` to verify no breakage
- [ ] 2.6 Remove `supertypes: Option<String>` from `LanguageEntry` in `build.rs`
- [ ] 2.7 Remove `supertypes_scm: Option<&'static str>` from generated `LanguageInfo` struct
- [ ] 2.8 Remove the codegen lines that produce `supertypes_scm`
- [ ] 2.9 Remove `pub fn is_interrupted(flag: &AtomicBool) -> bool` from `output.rs`
- [ ] 2.10 Run `cargo check --workspace` and `cargo test --workspace` to verify

## 3. API Cleanup (F-17)
- [ ] 3.1 Remove the `language: &Language` parameter from `compatible_replacements` in `compat.rs`
- [ ] 3.2 Remove the `let _ = language;` line
- [ ] 3.3 Update all callers in transform modules (`delete.rs`, `unwrap.rs`, etc.)
- [ ] 3.4 Update all test call sites
- [ ] 3.5 Run `cargo test --workspace` to verify

## 4. Test Helper Dedup (F-14)
- [ ] 4.1 Add `#[cfg(test)] pub mod test_utils` in `bonsai-core/src/lib.rs` with shared `visit_all` function
- [ ] 4.2 Replace `visit_all` in `compat.rs` tests with import from `test_utils`
- [ ] 4.3 Replace `visit_all` in `delete.rs` tests with import from `test_utils`
- [ ] 4.4 Replace `visit_all` in `unwrap.rs` tests with import from `test_utils`
- [ ] 4.5 Replace `visit_all` in `edge_cases.rs` tests with import from `test_utils`
- [ ] 4.6 Run `cargo test --workspace` to verify

## 5. Documentation Fixes (F-10, F-13)
- [ ] 5.1 Update `bonsai-fuzz/src/lib.rs:1` doc comment to describe subprocess target execution harness and crash interest criteria
- [ ] 5.2 Fix AGENTS.md Key Design Decision 5: note InterestingnessTest bridge is planned for when fuzzer is built
- [ ] 5.3 Fix AGENTS.md SupertypeProvider description: describe as Language API + fallback, with ChainProvider for future extension
- [ ] 5.4 Fix AGENTS.md grammars.toml header: describe as runtime via `Language::supertypes()`
