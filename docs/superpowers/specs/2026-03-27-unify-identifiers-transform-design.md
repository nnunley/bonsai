# UnifyIdentifiersTransform Design

## Overview

A scope-aware transform that renames each binding (definition + all its references) to a canonical short name (`a`, `b`, `c`, ...), reducing the number of unique identifiers in a test case. This is a standard test case reduction technique that shrinks output without changing program structure.

## Architecture

`UnifyIdentifiersTransform` follows the same pattern as `DeadDefinitionTransform`:

- Takes a `ScopeAnalysis` at construction time
- Precomputes a rename map from the analysis data
- At `candidates()` time, matches visited nodes against precomputed spans and emits replacements

### Data model

At construction, for each binding that needs renaming, the transform stores:

- `canonical_name: String` — the short name to rename to (e.g. `"a"`)
- `occurrences: Vec<(usize, usize)>` — sorted byte ranges of the definition and all its resolved references, pulled directly from `ScopeAnalysis.definitions` and `ScopeAnalysis.references`
- `span: (usize, usize)` — the byte range from first occurrence start to last occurrence end

Bindings whose name already equals their canonical name are excluded from the map.

### Multi-point replacement strategy

A rename must change multiple byte ranges (1 definition + N references). The current `Replacement` type is a single contiguous byte range. The solution: for each binding, construct a replacement that spans from the earliest occurrence to the latest occurrence end. The new bytes are built by walking through the span, copying verbatim bytes between occurrences and substituting the canonical name at each occurrence position.

Example: `let foo = 1; return foo;` renaming `foo` → `a`:
- Span: byte 4 to byte 25
- New bytes: `a = 1; return a`
- Single `Replacement { start_byte: 4, end_byte: 25, new_bytes: "a = 1; return a" }`

The reducer validates all replacements by reparsing, so any structurally invalid result is rejected.

### Canonical name assignment

Names are assigned in global file order (sorted by definition `start_byte`): `a`, `b`, ..., `z`, `aa`, `ab`, ... using base-26 encoding.

### Candidate generation

`candidates()` is called for each node visited by the reducer. When a node's byte range matches a binding's span (first occurrence start to last occurrence end), the transform builds and returns the replacement. One candidate per binding.

## Construction

- `from_analysis(analysis: &ScopeAnalysis) -> Self` — builds the rename map from scope analysis data
- `empty() -> Self` — no-op fallback when `locals.scm` is unavailable (returns no candidates)

## CLI wiring

Same pattern as `DeadDefinitionTransform`: conditionally added when `locals_scm` is available. Reuses the same `ScopeAnalysis` instance already built for dead definition detection.

## Testing

1. **Basic rename**: `let foo = 1; return foo;` → proposes renaming `foo` to `a`
2. **Multiple bindings**: `let foo = 1; let bar = 2;` → `foo` → `a`, `bar` → `b`
3. **Skip already canonical**: `let a = 1;` → no candidate emitted
4. **Empty analysis**: produces no candidates
5. **References included**: verify the replacement bytes contain the canonical name at both definition and reference positions
