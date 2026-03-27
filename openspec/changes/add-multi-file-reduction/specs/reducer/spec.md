## ADDED Requirements

### Requirement: Project File Set

The system SHALL maintain an in-memory file set (`HashMap<PathBuf, Vec<u8>>`) as the source of truth for project contents, backed by a persistent temp directory (`TempDir` handle owned by `ProjectFileSet`, kept alive for the duration of the reduction) for test execution.

#### Scenario: Construction from directory
- **WHEN** a directory path is provided as input
- **THEN** all files are recursively loaded into the file set and copied to a persistent temp directory
- **AND** hidden directories (`.git/`, `.hg/`) and common build artifacts (`target/`, `node_modules/`, `__pycache__/`) are excluded
- **AND** symlinks are not followed (skipped with a warning)

#### Scenario: File deletion
- **WHEN** a file is deleted from the file set
- **THEN** it is also removed from the temp directory

#### Scenario: File modification
- **WHEN** a file's contents are updated in the file set
- **THEN** the updated contents are written to the temp directory at the same relative path

#### Scenario: File restoration
- **WHEN** a file deletion is reverted (test failed)
- **THEN** the original contents are restored in both the file set and temp directory

#### Scenario: Empty directory
- **WHEN** the input directory is empty or contains no files
- **THEN** the system reports an error and exits

#### Scenario: No candidate files
- **WHEN** the directory contains only files with unrecognized extensions
- **THEN** the system warns that no candidate files were found and returns the project as-is

### Requirement: Two-Phase Reduction

The system SHALL reduce a project in two phases: coarse-grained file deletion followed by fine-grained per-file reduction. Both phases iterate files sequentially.

#### Scenario: Initial project validation
- **WHEN** multi-file reduction begins
- **THEN** the interestingness test is run on the unmodified project first
- **AND** if it fails, the system reports an error and returns immediately

#### Scenario: Phase 1 — File deletion (largest first)
- **WHEN** initial validation passes
- **THEN** candidate files (those with recognized language extensions) are sorted by size in descending order
- **AND** each candidate is tried for deletion: remove from temp dir, run test, keep deletion if still interesting, restore if not

#### Scenario: All candidates deleted in Phase 1
- **WHEN** Phase 1 deletes every candidate file
- **THEN** Phase 2 is a no-op
- **AND** the system returns successfully with only non-candidate files remaining

#### Scenario: Phase 2 — Per-file reduction
- **WHEN** Phase 1 completes with surviving candidate files
- **THEN** each surviving candidate file is reduced individually using the existing `reduce()` function
- **AND** the interestingness test runs against the full project temp directory after each candidate replacement

#### Scenario: Phase 2 state synchronization
- **WHEN** `reduce()` returns a `ReducerResult` for a file
- **THEN** the orchestrator updates the `ProjectFileSet` HashMap with `result.source` to keep the in-memory state in sync with the temp directory

#### Scenario: Non-candidate files preserved
- **WHEN** files have unrecognized extensions (configs, docs, binaries)
- **THEN** they are copied to the temp directory but never modified or deleted

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
- **NOTE** this differs from single-file mode which passes a temp file path; test scripts must be written for the appropriate mode

### Requirement: Auto-Detection of Directory Input

The system SHALL auto-detect whether the input is a file or directory.

#### Scenario: Directory input
- **WHEN** the input path is a directory
- **THEN** multi-file reduction mode is used
- **AND** `--lang` flag is ignored (language detected per file by extension)
- **AND** `--output` is required (writes the reduced project to the output directory; error if omitted)
- **AND** if the output directory already exists, the system reports an error (no silent overwrite)

#### Scenario: File input
- **WHEN** the input path is a file
- **THEN** single-file reduction mode is used (existing behavior unchanged)

### Requirement: Reducer Configuration for Phase 2

The system SHALL accept a `ProjectReducerConfig` that wraps the per-file reduction settings (`max_tests`, `max_time`, `jobs`, `strict`, `max_test_errors`, `interrupted`) and applies them to each per-file `reduce()` call. The `language`, `transforms`, and `provider` are determined per file by the orchestrator based on the file's extension.

### Requirement: Language Detection Per File

The system SHALL detect the language for each candidate file by its extension using `get_language_by_extension`.

#### Scenario: Recognized extension
- **WHEN** a file has a recognized extension (e.g., `.py`, `.rs`, `.js`)
- **THEN** it is a candidate for reduction using the corresponding language grammar

#### Scenario: Unrecognized extension
- **WHEN** a file has an unrecognized extension
- **THEN** it is not a candidate for reduction (preserved as-is in the temp directory)

### Requirement: Progress Reporting

The system SHALL report progress during both phases using `on_warning` for phase-level messages.

#### Scenario: Phase 1 progress
- **WHEN** a file deletion is attempted
- **THEN** progress reports which file is being tried and total candidate count via `on_warning`

#### Scenario: Phase 2 progress
- **WHEN** a file is being reduced
- **THEN** progress reports which file is being reduced via `on_warning`
- **AND** delegates per-node detail to the existing `ProgressCallback` passed through to `reduce()`
