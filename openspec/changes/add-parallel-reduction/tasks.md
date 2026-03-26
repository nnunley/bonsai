## 1. Parallel Candidate Testing
- [ ] 1.1 Refactor the inner candidate loop to collect valid candidates first, then test in parallel
- [ ] 1.2 Use rayon's `par_iter().find_first()` or a thread pool to test candidates concurrently
- [ ] 1.3 Accept the first interesting result; cancel/ignore remaining
- [ ] 1.4 With --jobs 1: preserve current sequential behavior exactly
- [ ] 1.5 Add test: mock test with artificial delay, verify --jobs 2 is faster than --jobs 1
- [ ] 1.6 Add test: --jobs 1 produces deterministic output (same input → same result)
- [ ] 1.7 Document that --jobs > 1 produces non-deterministic results
