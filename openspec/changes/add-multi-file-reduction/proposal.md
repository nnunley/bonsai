## Why

Bonsai currently reduces a single source file. Many bugs require multiple files to reproduce — a library plus a driver program, a module plus its tests, or a project with build configuration. Users must manually identify the relevant file before running bonsai, which defeats the purpose of automated reduction.

## What Changes

- Add `ProjectFileSet` to hold the in-memory file set and manage a persistent temp directory
- Add `reduce_project()` orchestrator with two-phase reduction:
  - Phase 1: delete entire files (largest first)
  - Phase 2: reduce each surviving file individually using existing `reduce()`
- Add `ProjectTest` adapter that bridges per-file `InterestingnessTest` to project-level test scripts
- Auto-detect directory input in CLI (`bonsai reduce --test "./check.sh" my-project/`)
- Detect language per file by extension using `get_language_by_extension`

## Dependencies

- Existing `reduce()` function (no changes needed)
- Existing `InterestingnessTest` trait
- `tempfile` crate (already a dependency)

## Impact

- New capability: reduce entire projects, not just single files
- No changes to single-file reduction behavior
- CLI gains directory-as-input mode (auto-detected)
- Test script interface: receives temp directory path as $1 (same convention as single-file)
