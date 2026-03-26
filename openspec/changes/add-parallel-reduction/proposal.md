## Why

The reducer spec calls for `--jobs N` parallel test execution, which is accepted but silently ignored. Interestingness tests are often the bottleneck (running compilers, interpreters, etc.), and candidates at the same node are independent — natural parallelism.

## What Changes

- Implement parallel candidate testing using rayon (already a dependency)
- With --jobs 1: sequential, deterministic (current behavior)
- With --jobs N > 1: test up to N candidates concurrently, accept first interesting result
- Document non-determinism with --jobs > 1

## Impact

- Performance improvement for CPU-bound interestingness tests
- No behavior change for --jobs 1 (default)
