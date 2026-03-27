## Why

Architecture review identified dead code, unused dependencies, stale documentation, and
error handling gaps in bonsai-fuzz that should be cleaned up before further development.

## What Changes

- F-2: Enforce `FuzzTarget.timeout` — currently stored but never used; both `run_stdin()` and `run_with_file()` block indefinitely
- F-3: Make `TargetResult::error()` sentinel distinguishable — change `FuzzTarget::run` to return `Result<TargetResult, TargetError>`
- F-10: Fix inaccurate `bonsai-fuzz` doc comment (says "fuzzing engine" but crate is a subprocess harness)
- F-11: Remove unused dependencies across `bonsai-core`, `bonsai-reduce`, `bonsai-fuzz`, and workspace `Cargo.toml`
- F-12: Remove dead `supertypes_scm` generated artifact and `supertypes` field from `LanguageEntry`
- F-13: Fix AGENTS.md steering file contradictions (InterestingnessTest bridge, SupertypeProvider description, grammars.toml header)
- F-14: Deduplicate `visit_all` test helper copied across 4 test modules
- F-15: Remove unused `is_interrupted` function from `output.rs`
- F-17: Remove silently-dropped `language` parameter from `compatible_replacements`

## Impact

- bonsai-fuzz: FuzzTarget::run return type changes from TargetResult to Result<TargetResult, TargetError> (breaking)
- bonsai-fuzz: timeout enforcement added
- bonsai-core: LanguageInfo struct loses supertypes_scm field (breaking for any consumer using it — currently none)
- bonsai-core: compatible_replacements loses language parameter (breaking)
- Dependency graph: several unused crates removed
- No runtime behavior changes for bonsai-reduce or bonsai-cli
