## ADDED Requirements

### Requirement: Corpus-Based AST Splicing
The system SHALL generate new test inputs by parsing corpus files and splicing type-compatible subtrees between them, using positional context (parent_kind, field_name) for higher splice acceptance rates.

#### Scenario: Generate mutated input
- **WHEN** a corpus of source files is provided
- **THEN** the system produces new inputs by replacing random nodes with compatible fragments, preferring positional matches (same parent_kind and field_name)

#### Scenario: Positional fallback
- **WHEN** no positional match exists in the node pool
- **THEN** the system falls back to type-compatible fragments via SupertypeProvider

#### Scenario: Configurable mutation count
- **WHEN** --mutations N is specified
- **THEN** each generated input has N splice operations applied

#### Scenario: Default mutation count
- **WHEN** --mutations is not specified
- **THEN** each generated input has uniform random [1,3] splice operations applied

#### Scenario: Validity after splicing
- **WHEN** a splice is performed
- **THEN** the result is reparsed and only kept if it contains no ERROR or MISSING nodes

### Requirement: Fuzz Target Interface
The system SHALL support passing generated inputs to a target program via stdin, temp file, or argument replacement (@@). Commands SHALL be invoked via std::process::Command, not shell interpolation. FuzzTarget SHALL implement InterestingnessTest directly (no separate type bridge needed).

#### Scenario: Stdin input mode (default when @@ absent)
- **WHEN** the target command does not contain "@@" and --input-mode is not specified
- **THEN** the generated input is piped to the target's stdin

#### Scenario: File input mode
- **WHEN** --input-mode file is specified
- **THEN** the generated input is written to a temp file and the path is passed as an argument

#### Scenario: Argument replacement mode
- **WHEN** the target command contains "@@"
- **THEN** "@@" is replaced with the path to the generated input file

### Requirement: Interest Criteria
The system SHALL support multiple criteria for identifying interesting test results: exit code, signal, stderr regex matching, and timeout detection. Criteria can be combined.

#### Scenario: Crash detection (any non-zero exit)
- **WHEN** the target exits with a non-zero exit code
- **THEN** the result is marked as interesting

#### Scenario: Specific exit code
- **WHEN** --interesting-exit <N> is specified and the target exits with code N
- **THEN** the result is marked as interesting

#### Scenario: Signal detection (Unix)
- **WHEN** the target is killed by a signal (e.g., SIGSEGV)
- **THEN** the result is marked as interesting (uses ExitStatus::signal() on Unix)

#### Scenario: Stderr pattern matching
- **WHEN** the target's stderr matches a user-provided regex
- **THEN** the result is marked as interesting

#### Scenario: Timeout detection
- **WHEN** the target exceeds --test-timeout
- **THEN** the target process and all descendants are killed, and the result is marked as interesting (if configured as such)

### Requirement: Finding Deduplication
The system SHALL deduplicate findings first by content hash (xxhash-128 for correctness), then by normalized error message.

#### Scenario: Content-identical finding
- **WHEN** a new finding has the same content hash as a previously saved finding
- **THEN** it is counted but not saved again

#### Scenario: Same-error finding with different content
- **WHEN** a new finding has different content but the same normalized stderr
- **THEN** it is counted but not saved again

### Requirement: Grammar-Diversity-Guided Corpus Evolution
The system SHALL optionally (--diversity-guided) add generated inputs that produce novel parse tree structures back to the corpus, measured by (parent_kind, child_kind) bigrams. This is structural diversity, not instrumentation-based coverage.

#### Scenario: Novel tree structure discovered
- **WHEN** --diversity-guided is enabled and a generated input produces new (parent_kind, child_kind) bigrams not seen before
- **THEN** the input is added to the corpus for future generation

#### Scenario: Default static corpus
- **WHEN** --diversity-guided is not specified
- **THEN** the corpus remains unchanged during the fuzzing session

### Requirement: Auto-Reduction of Findings
The system SHALL automatically reduce interesting findings using bonsai-reduce before saving them, with a bounded timeout.

#### Scenario: Reduce before save
- **WHEN** an interesting input is found
- **THEN** the system runs the reducer using the same FuzzTarget as the interestingness test, with a timeout of min(60s, 10x target timeout), then saves the reduced version

#### Scenario: Reduction timeout
- **WHEN** auto-reduction exceeds its timeout
- **THEN** the best-so-far reduced version is saved

### Requirement: Session Persistence
The system SHALL persist fuzzer state to a state directory with full PRNG state, resuming by default if prior state exists.

#### Scenario: State directory default
- **WHEN** --state-dir is not specified
- **THEN** state is persisted to .bonsai-fuzz/ in the working directory

#### Scenario: Resume from prior state
- **WHEN** the state directory contains prior state with a valid version and checksum
- **THEN** the fuzzer resumes with full PRNG state (not just seed), corpus index, findings, and stats

#### Scenario: Corpus changed between sessions
- **WHEN** resuming and corpus files have changed since last session
- **THEN** the node pool is rebuilt from the new corpus (PRNG state is kept but generation sequence will diverge)

#### Scenario: Fresh start
- **WHEN** --fresh is specified
- **THEN** prior state is discarded and fuzzing starts from scratch

#### Scenario: Corrupted or incompatible state
- **WHEN** the state file fails version or checksum validation
- **THEN** a warning is logged and fuzzing starts fresh

### Requirement: Execution Bounds
The system SHALL support configurable execution bounds for scripted and CI usage.

#### Scenario: Max executions
- **WHEN** --max-execs N is specified
- **THEN** the fuzzer stops after N target executions

#### Scenario: Max wall-clock time
- **WHEN** --max-time <duration> is specified
- **THEN** the fuzzer stops after the duration elapses

### Requirement: Progress Reporting
The system SHALL report fuzzer progress to stderr, including executions per second, total executions, findings count, and corpus size.

#### Scenario: Default progress
- **WHEN** the fuzzer runs without --quiet
- **THEN** progress updates are printed to stderr

#### Scenario: Quiet mode
- **WHEN** --quiet is specified
- **THEN** no progress output is produced
