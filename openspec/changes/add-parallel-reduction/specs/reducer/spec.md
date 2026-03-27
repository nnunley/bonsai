## MODIFIED Requirements

### Requirement: Parallel Test Execution
The system SHALL support parallel interestingness test execution when --jobs N > 1 is specified.

#### Scenario: Parallel testing
- **WHEN** --jobs N > 1 is specified and multiple valid candidates exist for a node
- **THEN** up to N candidates are tested concurrently, and the first interesting result is accepted

#### Scenario: Sequential testing (default)
- **WHEN** --jobs 1 is specified or defaulted
- **THEN** candidates are tested sequentially in deterministic order

#### Scenario: Non-determinism with parallel testing
- **WHEN** --jobs > 1 and multiple candidates are interesting
- **THEN** which candidate is accepted depends on OS scheduling (documented, acceptable)

### Requirement: Thread-Safe Cache Access
The system SHALL ensure `TestCache` is safe for concurrent access under parallel testing.

#### Scenario: Cache with jobs > 1
- **WHEN** --jobs N > 1 is specified
- **THEN** the cache is wrapped in a `Mutex<TestCache>`, with the lock held only during `cache.get()` and `cache.put()` (not during test execution)

#### Scenario: Cache with jobs == 1
- **WHEN** --jobs 1 is specified or defaulted
- **THEN** the cache is used directly without mutex overhead

### Requirement: Atomic Error Counter Under Parallelism
The system SHALL use atomic operations for the consecutive error counter when running parallel tests (depends on fix-reducer-gaps for `TestResult` and `max_test_errors`).

#### Scenario: Error counting with jobs > 1
- **WHEN** --jobs N > 1 is specified and `max_test_errors` is configured
- **THEN** the consecutive error counter uses `AtomicUsize`
- **AND** on `Error`, the counter is atomically incremented
- **AND** on `Interesting` or `NotInteresting`, the counter is atomically reset to 0
- **AND** before each test, if the counter exceeds the threshold, remaining candidates are skipped

#### Scenario: Consecutive semantics under parallelism
- **WHEN** multiple tests run concurrently
- **THEN** "consecutive" means "no successful test has reset the counter" — not strict sequential ordering

### Requirement: Thread Pool Lifecycle
The system SHALL create a scoped thread pool sized to the `--jobs` parameter.

#### Scenario: Thread pool creation
- **WHEN** --jobs N > 1 is specified
- **THEN** a scoped `rayon::ThreadPool` with N threads is created and used via `pool.install(|| ...)`

#### Scenario: Thread pool cleanup
- **WHEN** a reduction pass completes
- **THEN** the thread pool is dropped and all threads are joined
