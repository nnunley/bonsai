## 1. ProjectFileSet
- [ ] 1.1 Create `crates/bonsai-reduce/src/project.rs` with `ProjectFileSet` struct (owns `TempDir` handle, `HashMap<PathBuf, Vec<u8>>`)
- [ ] 1.2 Implement `from_directory(path)` — recursively load files, skip `.git/`, `target/`, `node_modules/`, `__pycache__/`, skip symlinks with warning, create persistent temp dir, copy all files
- [ ] 1.3 Implement `delete_file(path)` — remove from HashMap and temp dir
- [ ] 1.4 Implement `restore_file(path, contents)` — add back to HashMap and write to temp dir
- [ ] 1.5 Implement `update_file(path, contents)` — update HashMap and write to temp dir
- [ ] 1.6 Implement `candidate_files()` — return files with recognized extensions, sorted by size descending
- [ ] 1.7 Implement `temp_dir_path() -> &Path` — return path to the persistent temp directory
- [ ] 1.8 Add tests: construction, deletion, restoration, update, candidate sorting, empty dir error, no-candidate warning

## 2. ProjectTest Adapter
- [ ] 2.1 Implement `ProjectTest` struct with owned `PathBuf` for temp dir and target file path, plus command `Vec<String>` and timeout `Duration`
- [ ] 2.2 Implement `InterestingnessTest` for `ProjectTest`: write candidate bytes to target file in temp dir, run test command with temp dir path as last arg, return TestResult
- [ ] 2.3 Add test: ProjectTest correctly writes file and runs command

## 3. ProjectReducerConfig and Result
- [ ] 3.1 Define `ProjectReducerConfig` wrapping per-file settings: `max_tests`, `max_time`, `jobs`, `strict`, `max_test_errors`, `interrupted`, `test_command`, `test_timeout`
- [ ] 3.2 Define `ProjectReducerResult` with per-file stats and overall stats (total files deleted, total bytes removed, elapsed time)

## 4. reduce_project() Orchestrator
- [ ] 4.1 Implement initial validation: run interestingness test on unmodified project, return error if it fails
- [ ] 4.2 Implement Phase 1: iterate candidate files largest first, try deleting each, keep if still interesting
- [ ] 4.3 Implement Phase 2: for each surviving candidate, detect language by extension, build `ReducerConfig` from `ProjectReducerConfig`, create `ProjectTest` adapter, call `reduce()`, then call `file_set.update_file()` with the result to sync state
- [ ] 4.4 Implement progress reporting for both phases via `ProgressCallback::on_warning`
- [ ] 4.5 Handle edge case: all candidates deleted in Phase 1 (Phase 2 is no-op, return success)
- [ ] 4.6 Add test: Phase 1 deletes unnecessary files from a multi-file project
- [ ] 4.7 Add test: Phase 2 reduces surviving files
- [ ] 4.8 Add test: non-candidate files are preserved
- [ ] 4.9 Add test: initial validation fails returns early

## 5. CLI Integration
- [ ] 5.1 In `cmd_reduce`, detect if input path is a directory
- [ ] 5.2 If directory: require `--output`, error if omitted or output dir already exists
- [ ] 5.3 If directory: ignore `--lang`, build `ProjectReducerConfig` from CLI flags
- [ ] 5.4 If directory: call `reduce_project()`, write result to output directory
- [ ] 5.5 Add integration test: reduce a multi-file Python project via CLI

## 6. Module Wiring
- [ ] 6.1 Add `pub mod project;` to `crates/bonsai-reduce/src/lib.rs`
- [ ] 6.2 Re-export key types: `ProjectFileSet`, `reduce_project`, `ProjectReducerConfig`, `ProjectReducerResult`
- [ ] 6.3 Update README with multi-file usage example
