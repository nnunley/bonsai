## ADDED Requirements

### Requirement: Initial Input Validation
The system SHALL verify that the initial input passes the interestingness test before starting reduction. The check uses `test()` and must handle all three result variants.

#### Scenario: Initial input is interesting
- **WHEN** the initial input returns `TestResult::Interesting`
- **THEN** reduction proceeds normally

#### Scenario: Initial input is not interesting
- **WHEN** the initial input returns `TestResult::NotInteresting`
- **THEN** the reducer returns immediately with tests_run=1 and the original source unchanged

#### Scenario: Initial input test errors
- **WHEN** the initial input returns `TestResult::Error(msg)`
- **THEN** the reducer returns immediately with a fatal error (the test infrastructure is broken)

### Requirement: Progress Callback During Reduction
The system SHALL support an optional progress callback that is invoked periodically during the reduction loop, not only at completion.

#### Scenario: Progress callback configured
- **WHEN** a progress callback is provided in ReducerConfig
- **THEN** it is called after each accepted reduction and periodically during candidate testing

#### Scenario: No progress callback
- **WHEN** no progress callback is configured
- **THEN** the reducer runs silently (no progress overhead)

### Requirement: Final Output Verification
The system SHALL re-verify the final reduced output against the interestingness test before returning, to guard against hash collision corruption in the test cache.

#### Scenario: Final output passes verification
- **WHEN** the reduction completes and the final output passes the interestingness test
- **THEN** the reduced output is returned

#### Scenario: Final output fails verification (cache corruption)
- **WHEN** the reduction completes but the final output fails the interestingness test
- **THEN** the original source is returned instead

### Requirement: TestResult Enum and Error Tolerance
The system SHALL use a `TestResult` enum instead of a boolean for interestingness tests. The reducer loop SHALL tolerate a configurable number of consecutive test errors before aborting.

#### Scenario: Test returns Interesting
- **WHEN** the interestingness test returns `TestResult::Interesting`
- **THEN** the candidate is accepted, the error counter is reset to zero

#### Scenario: Test returns NotInteresting
- **WHEN** the interestingness test returns `TestResult::NotInteresting`
- **THEN** the candidate is skipped, the error counter is reset to zero

#### Scenario: Test returns Error
- **WHEN** the interestingness test returns `TestResult::Error(msg)`
- **THEN** the consecutive error counter is incremented and a warning is emitted via the progress callback

#### Scenario: Consecutive errors exceed threshold
- **WHEN** consecutive test errors exceed `max_test_errors` (default 3)
- **THEN** the reducer aborts and returns the best result so far

### Requirement: ShellTest Construction Validation
The system SHALL validate `ShellTest` arguments at construction time, returning an error instead of silently failing at runtime.

#### Scenario: ShellTest created with valid args
- **WHEN** `ShellTest::new` is called with a non-empty args list
- **THEN** it returns `Ok(ShellTest)`

#### Scenario: ShellTest created with empty args
- **WHEN** `ShellTest::new` is called with an empty args list
- **THEN** it returns `Err` with a descriptive message

### Requirement: Safe Reparse on Candidate Application
The system SHALL handle reparse failures gracefully instead of panicking.

#### Scenario: Reparse succeeds
- **WHEN** `parse()` returns `Some(tree)` after applying a candidate
- **THEN** the candidate is accepted normally

#### Scenario: Reparse fails
- **WHEN** `parse()` returns `None` after applying a candidate
- **THEN** the candidate is skipped (current_source and tree are not updated) and a warning is emitted via the progress callback

### Requirement: Final Verification with TestResult Semantics
The system SHALL treat a `TestResult::Error` during final verification as a hard failure, returning the original source.

#### Scenario: Final verification returns Error
- **WHEN** the final output verification returns `TestResult::Error(msg)`
- **THEN** the original source is returned (same as cache corruption case)

## MODIFIED Requirements

### Requirement: Replacement Bounds Validation
The system SHALL validate replacement byte ranges before applying them. Invalid ranges (start_byte > end_byte, or end_byte > source length) SHALL be rejected gracefully.

#### Scenario: Valid replacement range
- **WHEN** a replacement has start_byte <= end_byte <= source.len()
- **THEN** the replacement is applied normally

#### Scenario: Invalid replacement range
- **WHEN** a replacement has start_byte > end_byte or end_byte > source.len()
- **THEN** try_replacement returns None without panicking

### Requirement: Content-Based Error Tracking
The system SHALL track pre-existing parse errors by node kind and source text content (not byte positions), so that errors that shift position due to byte deletions are correctly recognized as pre-existing.

#### Scenario: Error shifts position after deletion
- **WHEN** bytes are deleted before a pre-existing error, shifting its position
- **THEN** the error is still recognized as pre-existing (same kind and content)

#### Scenario: Error set updated after each reduction
- **WHEN** a reduction is accepted in lenient mode
- **THEN** the error set is rebuilt from the new tree to stay current
