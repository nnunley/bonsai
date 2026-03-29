## Why

Bonsai currently reduces a single source file. Many bugs require multiple files to reproduce — a library plus a driver program, a module plus its tests, or a project with build configuration. Users must manually identify the relevant file before running bonsai, which defeats the purpose of automated reduction.

## What Changes

- Add `ProjectFileSet` to hold the in-memory file set and manage a persistent temp directory
- Add root/dependency boundary model: user declares which files are under reduction (roots) vs. source material (dependencies)
- Add `bonsai.toml` config file for declaring roots, dependency paths, cargo dep inclusion, and test command
- Add `reduce_project()` orchestrator with three-phase reduction:
  - Phase 0: call inlining pre-pass — pull dependency code into roots (requires `inlines.scm` per language, see `docs/superpowers/specs/2026-03-28-call-inlining-design.md`)
  - Phase 1: exclude dependency files from temp dir (largest first)
  - Phase 2: reduce each surviving root file individually using existing `reduce()`
- Add `ProjectTest` adapter that bridges per-file `InterestingnessTest` to project-level test scripts
- Auto-detect directory input in CLI (`bonsai reduce --test "./check.sh" my-project/`)
- Detect language per file by extension using `get_language_by_extension`

## Dependencies

- Existing `reduce()` function (no API changes; orchestration handles budget handoff)
- Existing `InterestingnessTest` trait
- `tempfile` crate (already a dependency)
- `toml` crate (new, for config parsing)

## Impact

- New capability: reduce entire projects, not just single files
- No changes to single-file reduction behavior
- CLI gains directory-as-input mode (auto-detected), `--roots`, `--deps`, `--exclude-deps`, `--config` flags
- Single-file input with `inlines.scm` available automatically constructs lightweight project context
- Test script interface: receives temp directory path as last argument (same convention as single-file)
- New `bonsai.toml` config file format
