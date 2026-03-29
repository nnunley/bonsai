## ADDED Requirements

### Requirement: Project File Set

The system SHALL maintain an in-memory file set (`HashMap<PathBuf, Vec<u8>>`) as the source of truth for project contents, backed by a persistent temp directory (`TempDir` handle owned by `ProjectFileSet`, kept alive for the duration of the reduction) for test execution.

#### Scenario: Construction from directory
- **WHEN** a directory path is provided as input
- **THEN** all files are recursively loaded into the file set and copied to a persistent temp directory
- **AND** hidden directories (`.git/`, `.hg/`) and common build artifacts (`target/`, `node_modules/`, `__pycache__/`) are excluded
- **AND** symlinks are not followed (skipped with a warning)

#### Scenario: Command pattern for mutations
- **WHEN** any mutation is applied to the file set (modification, exclusion)
- **THEN** the mutation is recorded as a command on a per-file stack
- **AND** each command supports `apply()` and `undo()` operations
- **NOTE** this enables multi-step rollback and consistent undo semantics across all mutation types

#### Scenario: File modification
- **WHEN** a file's contents are updated in the file set
- **THEN** a modification command is pushed to the file's stack
- **AND** the updated contents are written to the temp directory at the same relative path

#### Scenario: File exclusion from temp dir
- **WHEN** a dependency file is excluded during Phase 1
- **THEN** an exclusion command is pushed to the file's stack
- **AND** the file is removed from the temp directory
- **NOTE** root files are never candidates for exclusion

#### Scenario: Undo last mutation
- **WHEN** a mutation is reverted (test failed)
- **THEN** the top command is popped from the file's stack and its `undo()` is called
- **AND** the file set and temp directory are restored to the previous state

#### Scenario: Empty directory
- **WHEN** the input directory is empty or contains no files
- **THEN** the system reports an error and exits

#### Scenario: No candidate files
- **WHEN** the directory contains only files with unrecognized extensions
- **THEN** the system warns that no candidate files were found and returns the project as-is

### Requirement: Root and Dependency Boundaries

The system SHALL distinguish between root files (under reduction) and dependency files (source material for inlining). The user declares these boundaries explicitly.

#### Scenario: Roots from CLI
- **WHEN** `--roots` is provided
- **THEN** only the specified files are treated as roots
- **AND** all other project files are treated as dependencies

#### Scenario: Single-file root
- **WHEN** a single file is provided as input (not a directory)
- **THEN** that file is the root
- **AND** its containing directory provides dependencies

#### Scenario: Roots from config
- **WHEN** `bonsai.toml` specifies `[reduce].roots`
- **THEN** those files are treated as roots
- **AND** CLI `--roots` overrides config roots

#### Scenario: Dependency paths
- **WHEN** `[reduce.dependencies].paths` is configured (or `--deps` CLI)
- **THEN** only files under those paths are scanned for definitions
- **AND** `[reduce.dependencies].exclude` (or `--exclude-deps`) removes paths from scanning

#### Scenario: Default dependencies
- **WHEN** no dependency paths are configured
- **THEN** all project files except roots are treated as dependencies

#### Scenario: Cargo dependencies (future, not V1)
- **NOTE** cargo dependency walking is planned as a priority follow-up after V1 local-path inlining works. V1 supports only local file paths as dependencies.

### Requirement: Configuration File

The system SHALL support an optional `bonsai.toml` configuration file.

#### Scenario: Config discovery
- **WHEN** no `--config` flag is provided
- **THEN** the system walks up parent directories from the input path looking for `bonsai.toml`
- **AND** stops at the filesystem root

#### Scenario: Explicit config
- **WHEN** `--config path/to/bonsai.toml` is provided
- **THEN** that config file is used, skipping discovery

#### Scenario: CLI overrides config
- **WHEN** both CLI flags and config file specify the same setting
- **THEN** CLI flags take precedence

#### Scenario: No config found
- **WHEN** no config file is found and no `--config` is provided
- **THEN** defaults are used: root = CLI positional arg, deps = project directory minus roots, no cargo deps

### Requirement: Three-Phase Reduction

The system SHALL reduce a project in three phases: call inlining pre-pass, dependency file exclusion, and per-file root reduction. All phases operate on the same `ProjectFileSet` and share the project-level test budget.

#### Scenario: Initial project validation
- **WHEN** multi-file reduction begins
- **THEN** the interestingness test is run on the unmodified project first
- **AND** if it fails, the system reports an error and returns immediately

#### Scenario: Phase 0 — Call inlining (pre-pass)
- **WHEN** initial validation passes
- **THEN** the orchestrator groups root files by language (using `get_language_by_extension`)
- **AND** for each language that has `inlines.scm` available, a `CallInlineTransform` is constructed with a `DependencyIndex` built from declared dependencies
- **AND** the orchestrator iterates root files; for each root whose language supports inlining, it delegates to the corresponding `CallInlineTransform::inline_root()`
- **AND** root files in languages without `inlines.scm` are skipped
- **AND** dependency files are read-only — never mutated by Phase 0
- **AND** each tentative inline modifies only the root file in `ProjectFileSet`
- **AND** the interestingness test is run against the full project after each tentative inline
- **AND** if the test fails, the root file is restored and that inline is skipped
- **AND** if the test passes, the inline is accepted and the root is reparsed for the next candidate
- **AND** test invocations count against `max_tests`; Phase 0 respects `max_time`
- **AND** if budget is exhausted, Phase 0 stops with whatever inlines have been accepted
- **NOTE** cross-language inlining is not supported; each language's inliner only resolves calls within its own language
- **NOTE** inlined code is annotated with `# inlined: <file>:<name>` provenance comments

#### Scenario: Phase 0 skipped
- **WHEN** no root files are in languages with `inlines.scm` available
- **THEN** Phase 0 is a no-op and reduction proceeds directly to Phase 1

#### Scenario: Phase 0 progress
- **WHEN** a call inlining attempt is made
- **THEN** progress reports which root file, language, and call site are being tried via `on_warning`
- **AND** reports per-language `InlineStats` when Phase 0 completes

#### Scenario: Phase 1 — Dependency file exclusion (largest first)
- **WHEN** Phase 0 completes (or is skipped)
- **THEN** dependency files (not roots) are sorted by size in descending order
- **AND** each dependency is tried for exclusion: remove from temp dir, run test, keep exclusion if still interesting, restore if not
- **NOTE** root files are never candidates for exclusion

#### Scenario: All dependencies excluded in Phase 1
- **WHEN** Phase 1 excludes every dependency file
- **THEN** Phase 2 proceeds with root files only in the temp dir

#### Scenario: Phase 2 — Per-file reduction (roots only)
- **WHEN** Phase 1 completes
- **THEN** each root file is reduced individually using the existing `reduce()` function
- **AND** the interestingness test runs against the full project temp directory after each candidate replacement
- **NOTE** dependency files are not reduced — they are support material

#### Scenario: Phase 2 state synchronization
- **WHEN** `reduce()` returns a `ReducerResult` for a root file
- **THEN** the orchestrator updates the `ProjectFileSet` with `result.source`

#### Scenario: Non-candidate files preserved
- **WHEN** files have unrecognized extensions (configs, docs, binaries)
- **THEN** they are copied to the temp directory but never modified or excluded

#### Scenario: Budget shared across phases
- **WHEN** `max_tests` or `max_time` is configured
- **THEN** test invocations and elapsed time accumulate across Phase 0, Phase 1, and Phase 2
- **AND** within Phase 2, the budget is shared cumulatively across root files (not reset per file)
- **AND** if the budget is exhausted in any phase, subsequent phases receive the remaining budget

### Requirement: Project Interestingness Test Adapter

The system SHALL provide a `ProjectTest` adapter that implements `InterestingnessTest` for per-file reduction within a project context. The adapter holds owned `PathBuf` values (not references) for the temp directory and target file path.

#### Scenario: Per-file test invocation
- **WHEN** `reduce()` calls `test(candidate_bytes)` during Phase 2
- **THEN** the adapter writes `candidate_bytes` to the target file's path in the temp directory
- **AND** runs the project-level test script with the temp directory path as the last argument
- **AND** returns the test result

#### Scenario: Test script argument convention
- **WHEN** the test script is invoked in multi-file mode
- **THEN** it receives the temp directory path as its last argument
- **NOTE** this differs from single-file mode which passes a temp file path

### Requirement: Auto-Detection of Directory Input

The system SHALL auto-detect whether the input is a file or directory.

#### Scenario: Directory input
- **WHEN** the input path is a directory
- **THEN** multi-file reduction mode is used
- **AND** `--lang` flag is ignored (language detected per file by extension)
- **AND** `--output` is required (error if omitted)
- **AND** if the output directory already exists, the system reports an error

#### Scenario: File input with inlining support
- **WHEN** the input path is a file
- **AND** the file's language has `inlines.scm` available
- **THEN** a lightweight project context is constructed: the file is the sole root, its containing directory provides dependencies
- **AND** three-phase reduction runs (Phase 0 inlining, Phase 1 dependency exclusion, Phase 2 per-file reduction)

#### Scenario: File input without inlining support
- **WHEN** the input path is a file
- **AND** the file's language does not have `inlines.scm` available
- **THEN** single-file reduction mode is used (existing behavior unchanged)

### Requirement: Reducer Configuration

The system SHALL accept a `ProjectReducerConfig` that wraps the project-level settings (`max_tests`, `max_time`, `jobs`, `strict`, `max_test_errors`, `interrupted`) and applies them across all phases. In Phase 0, `max_tests` and `max_time` bound the inlining pre-pass. In Phase 2, per-file `reduce()` calls receive the remaining budget. The `language`, `transforms`, and `provider` are determined per file by the orchestrator based on the file's extension.

### Requirement: Language Detection Per File

The system SHALL detect the language for each candidate file by its extension using `get_language_by_extension`.

#### Scenario: Recognized extension
- **WHEN** a file has a recognized extension (e.g., `.py`, `.rs`, `.js`)
- **THEN** it is a candidate for reduction using the corresponding language grammar

#### Scenario: Unrecognized extension
- **WHEN** a file has an unrecognized extension
- **THEN** it is not a candidate for reduction (preserved as-is in the temp directory)

### Requirement: Progress Reporting

The system SHALL report progress during all phases using `on_warning` for phase-level messages.

#### Scenario: Phase 0 progress
- **WHEN** a call inlining attempt is made
- **THEN** progress reports which root file, language, and call site are being tried via `on_warning`
- **AND** reports per-language `InlineStats` summary when Phase 0 completes

#### Scenario: Phase 1 progress
- **WHEN** a dependency file exclusion is attempted
- **THEN** progress reports which file is being tried and total dependency count via `on_warning`

#### Scenario: Phase 2 progress
- **WHEN** a root file is being reduced
- **THEN** progress reports which file is being reduced via `on_warning`
- **AND** delegates per-node detail to the existing `ProgressCallback` passed through to `reduce()`
