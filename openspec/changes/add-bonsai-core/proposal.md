## Why

Grammar-aware test case reduction and fuzzing are essential tools for language development. The current landscape has two problems:

1. **Perses/Vulcan** (state-of-the-art reducers) are tied to ANTLR grammars, limiting language coverage. Tree-sitter has far broader grammar availability (300+ languages) and is actively maintained by the developer tools community.
2. **treereduce** exists for tree-sitter but uses a simpler algorithm than Perses, producing larger results and lacking syntactic validity guarantees.
3. **Reduction and fuzzing share a core** — grammar-aware tree manipulation — but existing tools treat them as separate concerns.

Bonsai unifies syntax-guided reduction and grammar-based fuzzing in a single Rust tool using tree-sitter grammars.

## What Changes

This is a greenfield project. We're building:

- **Core library** (`bonsai-core`): Tree-sitter parse tree manipulation, node type compatibility checking, and the transform trait system that both reducer and fuzzer build on.
- **Reducer** (`bonsai-reduce`): Perses-style priority-queue reduction with syntactic validity guarantees. Architected to support Vulcan-style transforms later.
- **Fuzzer** (`bonsai-fuzz`): Grammar-guided test case generation via AST splicing from a corpus.
- **Grammar plugin system**: Difftastic-style git submodule approach with a registration system for adding tree-sitter grammars.
- **CLI** (`bonsai`): Unified CLI with `bonsai reduce` and `bonsai fuzz` subcommands. Shell-command interestingness test interface.
- **Library API**: Embeddable Rust API for both reduction and fuzzing.

## Impact

- New project, no breaking changes.
- Depends on: `tree-sitter` Rust crate, vendored grammar submodules.
- Target users: language developers, compiler engineers, fuzzing practitioners.

## Research Basis

The reducer implements the Perses algorithm (Sun et al., ICSE 2018) adapted for tree-sitter. The architecture supports incremental addition of:
- Vulcan transforms (OOPSLA 2023) for escaping 1-minimality
- T-Rec token-level reduction (TOSEM 2024)
- Hoisting (Vince et al., 2021)
- Test outcome caching (Hodovan et al., 2017)

The fuzzer draws from tree-crasher's AST splicing approach (Barrett, 2022) with type-compatible node replacement.
