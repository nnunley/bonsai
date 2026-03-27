## Why

The reducer spec calls for `--jobs N` parallel test execution, which is accepted but silently ignored. Interestingness tests are often the bottleneck (running compilers, interpreters, etc.), and candidates at the same node are independent — natural parallelism.

## Dependencies

- **fix-reducer-gaps**: This change depends on the `TestResult` enum and `max_test_errors` consecutive error counter introduced in fix-reducer-gaps.

## What Changes

- Implement parallel candidate testing using rayon (already a dependency)
- With --jobs 1: sequential, deterministic (current behavior)
- With --jobs N > 1: test up to N candidates concurrently, accept first interesting result
- Document non-determinism with --jobs > 1
- Create a scoped `rayon::ThreadPool` with `jobs` threads; use `pool.install(|| candidates.par_iter().find_first(|c| ...))` for parallel testing
- Remove the "not yet implemented" doc comment from `ReducerConfig.jobs`

### Cache concurrency

- `TestCache` needs to be thread-safe for `jobs > 1`
- Wrap in `Mutex<TestCache>` — lock held only during `cache.get()` and `cache.put()` (hash lookup/insert)
- Shell test execution runs outside the lock — minimal contention
- For `jobs == 1`, no mutex overhead (use the cache directly, no wrapping)

### Error tolerance under parallelism

- The `max_test_errors` consecutive error counter (from fix-reducer-gaps) needs concurrency semantics
- Use `AtomicUsize` for the counter when `jobs > 1`
- On `Error`: atomically increment
- On `Interesting`/`NotInteresting`: atomically reset to 0
- Before each test, check if counter exceeds threshold; if so, skip remaining candidates
- "Consecutive" under parallelism means "no successful test has reset it" — not strict sequencing

## Impact

- Performance improvement for CPU-bound interestingness tests
- No behavior change for --jobs 1 (default)
