## ADDED Requirements

### Requirement: Perses-Style Priority Queue Reduction
The system SHALL reduce input files by processing tree nodes in descending order of token count (leaf node count), applying transforms and validating with an interestingness test. The priority queue SHALL store (byte_range, kind_id, token_count) tuples and be rebuilt from the new tree after each accepted reduction.

#### Scenario: Successful reduction
- **WHEN** an input file and interestingness test are provided
- **THEN** the system outputs a smaller file that still passes the interestingness test

#### Scenario: All candidates rejected
- **WHEN** no candidate replacement for a node passes the interestingness test
- **THEN** the node is skipped and the next largest node is tried

#### Scenario: Validity checking
- **WHEN** a candidate replacement is generated
- **THEN** it is validated by reparsing and checking for ERROR/MISSING nodes before testing interestingness

#### Scenario: Queue rebuild after accepted reduction
- **WHEN** a reduction is accepted and the tree is reparsed
- **THEN** the priority queue is rebuilt from the new tree's named nodes

#### Scenario: Parallel test execution
- **WHEN** multiple valid candidates exist for a node and --jobs N > 1 is specified
- **THEN** up to N interestingness tests run concurrently, accepting the first interesting result

#### Scenario: Deterministic mode
- **WHEN** --jobs 1 is specified (or default)
- **THEN** candidates are tested in order and reduction is fully deterministic

### Requirement: Termination Bounds
The system SHALL support configurable termination bounds to prevent unbounded execution.

#### Scenario: Max test invocations
- **WHEN** --max-tests N is specified
- **THEN** the reducer stops after N interestingness test invocations and outputs best-so-far

#### Scenario: Max wall-clock time
- **WHEN** --max-time <duration> is specified
- **THEN** the reducer stops after the duration elapses and outputs best-so-far

#### Scenario: Natural termination
- **WHEN** no bounds are specified and all queue entries are exhausted
- **THEN** the reducer terminates and outputs the result

### Requirement: Graceful Shutdown
The system SHALL handle SIGINT by outputting the best-so-far reduced result before exiting.

#### Scenario: SIGINT during reduction
- **WHEN** SIGINT is received during reduction
- **THEN** the current best reduced source is written to stdout or --output before exiting

### Requirement: Interestingness Test Interface
The system SHALL support both a shell-command interface (exit code 0 = interesting) and a programmatic Rust trait for determining interestingness. Shell commands SHALL be invoked via std::process::Command with an args array, not shell interpolation.

#### Scenario: Shell command test
- **WHEN** a shell command is provided as the test
- **THEN** the system writes the candidate to a temp file, invokes the command via Command::new with the file path as an argument, and treats exit code 0 as interesting

#### Scenario: Programmatic test
- **WHEN** a Rust implementation of InterestingnessTest is provided
- **THEN** the reducer calls it directly without shell overhead

#### Scenario: Test timeout
- **WHEN** --test-timeout <duration> is specified and a test exceeds it
- **THEN** the candidate is treated as not interesting

### Requirement: Test Result Caching
The system SHALL cache interestingness test results keyed by a hash of the candidate source bytes to avoid redundant test executions.

#### Scenario: Cache hit
- **WHEN** a candidate's hash matches a previously tested candidate
- **THEN** the cached result is returned without re-running the test

#### Scenario: Cache miss
- **WHEN** a candidate's hash has not been seen before
- **THEN** the test is executed and the result is cached

### Requirement: Reduction Output
The system SHALL write reduced output to stdout by default, with an --output flag to write to a file.

#### Scenario: Default stdout output
- **WHEN** no --output flag is provided
- **THEN** the reduced source is written to stdout

#### Scenario: File output
- **WHEN** --output <path> is provided
- **THEN** the reduced source is written to the specified file

### Requirement: Progress Reporting
The system SHALL report reduction progress to stderr, including current size, reduction percentage, iteration count, and cache hit rate.

#### Scenario: Default progress
- **WHEN** the reducer runs without --quiet
- **THEN** progress updates are printed to stderr

#### Scenario: Quiet mode
- **WHEN** --quiet is specified
- **THEN** no progress output is produced

#### Scenario: Verbose mode
- **WHEN** --verbose is specified
- **THEN** per-candidate detail is printed to stderr
