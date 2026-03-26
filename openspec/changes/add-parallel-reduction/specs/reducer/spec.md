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
