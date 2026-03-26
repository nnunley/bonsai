## 1. Initial Input Validation
- [ ] 1.1 Add initial input check to `reduce()` — verify source passes the test before starting
- [ ] 1.2 Return early with a clear error/result if initial input is not interesting
- [ ] 1.3 Add test: input that fails the test returns immediately with tests_run=1

## 2. Progress During Reduction Loop
- [ ] 2.1 Add a `on_progress` callback field to `ReducerConfig` (optional `Box<dyn FnMut(ProgressUpdate)>`)
- [ ] 2.2 Call the callback after each accepted reduction and periodically during candidate testing
- [ ] 2.3 Wire the CLI's `ProgressReporter` into the callback
- [ ] 2.4 Add test: verify callback is invoked during multi-step reduction

## 3. No-Supertype Warning
- [ ] 3.1 Add `log` or `tracing` crate dependency (or use `eprintln!` for simplicity)
- [ ] 3.2 In the CLI, check if `LanguageApiProvider::has_supertypes()` is false and warn
- [ ] 3.3 Add test: grammar with no supertypes produces a warning

## 4. Parse `locals` Field in build.rs
- [ ] 4.1 Add `locals: Option<String>` to `LanguageEntry` struct in build.rs
- [ ] 4.2 Include `locals_scm` in the generated `LanguageInfo` struct
- [ ] 4.3 Add test: verify `LanguageInfo` for JavaScript includes locals path

## 5. Update Specs to Match Implementation
- [ ] 5.1 Update validity spec to document content-based error tracking
- [ ] 5.2 Update validity spec to document replacement bounds validation
- [ ] 5.3 Update reducer spec to document final output re-verification
