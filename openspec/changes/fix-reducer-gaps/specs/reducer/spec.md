## ADDED Requirements

### Requirement: Initial Input Validation
The system SHALL verify that the initial input passes the interestingness test before starting reduction. If it does not, the reducer SHALL return immediately with a clear indication.

#### Scenario: Initial input is interesting
- **WHEN** the initial input passes the interestingness test
- **THEN** reduction proceeds normally

#### Scenario: Initial input is not interesting
- **WHEN** the initial input does not pass the interestingness test
- **THEN** the reducer returns immediately with tests_run=1 and the original source unchanged

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
