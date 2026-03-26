## Why

Code review identified several gaps between the reducer spec and implementation:
1. Reducer does not verify the initial input passes the interestingness test before starting
2. Progress reporting exists as a module but is not called during the reduction loop (only final summary)
3. No warning logged when a grammar has no supertypes
4. `locals` field in grammars.toml is not parsed by build.rs
5. Replacement bounds validation was missing (now fixed, spec should document it)
6. Error tracking in lenient mode used byte positions (now fixed to content+kind, spec should reflect)

## What Changes

- Add initial input validation to reducer spec
- Add progress callback mechanism to reducer
- Add no-supertype warning requirement
- Add `locals` field parsing to build.rs grammar entry
- Update validity spec to document bounds checking and content-based error tracking

## Impact

- Reducer behavior change: fails fast if initial input is not interesting
- Progress: users see periodic updates during long reductions, not just final summary
- Build: `locals` field is parsed (no runtime behavior change yet — ScopeAnalysis is a separate change)
