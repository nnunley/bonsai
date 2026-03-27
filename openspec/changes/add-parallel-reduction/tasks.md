## 1. Parallel Candidate Testing
- [ ] 1.1 Refactor the inner candidate loop to collect valid candidates first, then test in parallel
- [ ] 1.2 Use rayon's `par_iter().find_first()` or a thread pool to test candidates concurrently
- [ ] 1.3 Accept the first interesting result; cancel/ignore remaining
- [ ] 1.4 With --jobs 1: preserve current sequential behavior exactly
- [ ] 1.5 Add test: mock test with artificial delay, verify --jobs 2 is faster than --jobs 1
- [ ] 1.6 Add test: --jobs 1 produces deterministic output (same input → same result)
- [ ] 1.7 Document that --jobs > 1 produces non-deterministic results

## 2. Thread Pool Lifecycle
- [ ] 2.1 Create a scoped `rayon::ThreadPool` with `jobs` threads
- [ ] 2.2 Use `pool.install(|| candidates.par_iter().find_first(|c| ...))` for parallel testing
- [ ] 2.3 Remove the "not yet implemented" doc comment from `ReducerConfig.jobs`

## 3. Cache Concurrency
- [ ] 3.1 Wrap `TestCache` in `Mutex<TestCache>` when `jobs > 1`
- [ ] 3.2 Hold the lock only during `cache.get()` and `cache.put()` — not during shell test execution
- [ ] 3.3 For `jobs == 1`, use the cache directly without mutex wrapping
- [ ] 3.4 Add test: concurrent cache access with `jobs > 1` does not corrupt or lose entries
- [ ] 3.5 Add test: `jobs == 1` path has no mutex overhead (cache used directly)

## 4. Error Tolerance Under Parallelism (depends on fix-reducer-gaps)
- [ ] 4.1 Use `AtomicUsize` for the `max_test_errors` consecutive error counter when `jobs > 1`
- [ ] 4.2 On `Error`: atomically increment the counter
- [ ] 4.3 On `Interesting`/`NotInteresting`: atomically reset the counter to 0
- [ ] 4.4 Before each test, check if counter exceeds threshold; if so, skip remaining candidates
- [ ] 4.5 Add test: error counter correctly trips under parallel execution
- [ ] 4.6 Add test: a successful test resets the counter even when other threads see errors concurrently
