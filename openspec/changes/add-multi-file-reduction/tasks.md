## 1. ProjectFileSet
- [ ] 1.1 Create `crates/bonsai-reduce/src/project.rs` with `ProjectFileSet` struct (owns `TempDir` handle, `HashMap<PathBuf, Vec<u8>>`, tracks root vs dependency files)
- [ ] 1.2 Define `FileCommand` trait with `apply()` and `undo()` methods. Implement `ModifyCommand` and `ExcludeCommand`. `ProjectFileSet` maintains a per-file command stack for rollback.
- [ ] 1.3 Implement `from_directory(path, roots)` — recursively load files, skip `.git/`, `target/`, `node_modules/`, `__pycache__/`, skip symlinks with warning, create persistent temp dir, copy all files, classify as root or dependency
- [ ] 1.4 Implement `update_file(path, contents)` — push `ModifyCommand`, write to temp dir
- [ ] 1.5 Implement `undo_last(path)` — pop top command from file's stack, call `undo()`
- [ ] 1.6 Implement `exclude_file(path)` — push `ExcludeCommand`, remove from temp dir. Error if path is a root file.
- [ ] 1.7 Implement `root_files()` — return root files
- [ ] 1.8 Implement `dependency_files()` — return dependency files sorted by size descending
- [ ] 1.9 Implement `temp_dir_path() -> &Path`
- [ ] 1.10 Add tests: construction, modification, undo modification, exclusion, undo exclusion, multi-step undo, root protection (exclude root fails), sorting

## 2. Configuration
- [ ] 2.1 Define `BonsaiConfig` struct parsed from `bonsai.toml` with `[reduce]` (roots, test) and `[reduce.dependencies]` (paths, exclude)
- [ ] 2.2 Implement `BonsaiConfig::discover(start_path)` — walk up parent dirs looking for `bonsai.toml`, stop at filesystem root
- [ ] 2.3 Implement `BonsaiConfig::load(path)` — parse toml file
- [ ] 2.4 Implement CLI flag merging: `--roots`, `--deps`, `--exclude-deps`, `--config` override config values
- [ ] 2.5 Implement `DependencyConfig` from merged config: local paths
- [ ] 2.6 Add tests: config discovery, explicit config, CLI overrides, defaults when no config

## 3. ProjectTest Adapter
- [ ] 3.1 Implement `ProjectTest` struct with owned `PathBuf` for temp dir and target file path, plus command `Vec<String>` and timeout `Duration`
- [ ] 3.2 Implement `InterestingnessTest` for `ProjectTest`: write candidate bytes to target file in temp dir, run test command with temp dir path as last arg, return TestResult
- [ ] 3.3 Add test: ProjectTest correctly writes file and runs command

## 4. ProjectReducerConfig and Result
- [ ] 4.1 Define `ProjectReducerConfig` wrapping project-level settings: `max_tests`, `max_time`, `jobs`, `strict`, `max_test_errors`, `interrupted`, `test_command`, `test_timeout`
- [ ] 4.2 Define `ProjectReducerResult` with per-phase stats (Phase 0 `InlineStats`, Phase 1 files excluded, Phase 2 per-file stats) and overall stats (total bytes removed, total test invocations, elapsed time)

## 5. reduce_project() Orchestrator
- [ ] 5.1 Implement initial validation: run interestingness test on unmodified project, return error if it fails
- [ ] 5.2 Implement Phase 0 language dispatch: group root files by language, construct `CallInlineTransform` per language with `DependencyIndex` from config, call `inline_root()` per root. Track per-language `InlineStats`, subtract cumulative `test_invocations` from remaining budget.
- [ ] 5.3 Implement Phase 0 skip: if no root language has `inlines.scm`, proceed directly to Phase 1
- [ ] 5.4 Implement Phase 1: iterate dependency files (largest first), try excluding each from temp dir, keep exclusion if still interesting, restore if not. Never exclude root files.
- [ ] 5.5 Implement Phase 2: for each root file, detect language by extension, build `ReducerConfig` with remaining budget, create `ProjectTest` adapter, call `reduce()`, update `ProjectFileSet` with result
- [ ] 5.6 Implement progress reporting for all phases via `ProgressCallback::on_warning`
- [ ] 5.7 Handle edge case: all dependencies excluded in Phase 1 (Phase 2 proceeds with roots only)
- [ ] 5.8 Handle edge case: budget exhausted in Phase 0 (Phase 1/2 receive remaining budget)
- [ ] 5.9 Add test: Phase 0 inlines dependency code into root file
- [ ] 5.10 Add test: Phase 0 rollback on failed interestingness test restores root file
- [ ] 5.11 Add test: Phase 0 budget exhaustion stops inlining, Phase 1 continues
- [ ] 5.12 Add test: Phase 1 excludes unneeded dependency files
- [ ] 5.13 Add test: Phase 1 never excludes root files
- [ ] 5.14 Add test: Phase 2 reduces only root files
- [ ] 5.15 Add test: non-candidate files are preserved
- [ ] 5.16 Add test: initial validation fails returns early
- [ ] 5.17 Add test: budget shared across all three phases cumulatively
- [ ] 5.18 Add test: mixed-language roots get correct language-specific inliner
- [ ] 5.19 Add test: dependency file exclusion + restoration round-trips correctly
- [ ] 5.20 Add test: inlined code has provenance annotation

## 6. CLI Integration
- [ ] 6.1 In `cmd_reduce`, detect if input path is a directory
- [ ] 6.2 Add `--roots`, `--deps`, `--exclude-deps`, `--config` flags
- [ ] 6.3 If directory: require `--output`, error if omitted or output dir already exists
- [ ] 6.4 If directory: load/merge config, build `ProjectReducerConfig` from CLI flags + config
- [ ] 6.5 If directory: call `reduce_project()`, write result to output directory
- [ ] 6.6 If file with `inlines.scm`: construct lightweight project context (file as root, containing dir as deps), run three-phase reduction
- [ ] 6.7 If file without `inlines.scm`: existing single-file behavior unchanged
- [ ] 6.8 Add integration test: reduce a multi-file Python project via CLI
- [ ] 6.9 Add integration test: `bonsai.toml` config discovery and CLI override
- [ ] 6.10 Add integration test: single-file input with inlining support constructs project context

## 7. Module Wiring
- [ ] 7.1 Add `pub mod project;` and `pub mod config;` to `crates/bonsai-reduce/src/lib.rs`
- [ ] 7.2 Re-export key types: `ProjectFileSet`, `reduce_project`, `ProjectReducerConfig`, `ProjectReducerResult`, `BonsaiConfig`
- [ ] 7.3 Update README with multi-file usage example and `bonsai.toml` format
