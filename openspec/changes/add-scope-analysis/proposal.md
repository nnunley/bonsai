## Why

Tree-sitter grammars can ship `locals.scm` query files that define scope, definition, and reference relationships. This enables semantics-preserving transforms that pure syntactic reduction cannot achieve:

- **Identifier unification**: rename all bindings to short canonical forms (`a`, `b`, `c`), reducing token count
- **Dead definition removal**: delete definitions with no references — stronger than blind deletion because it knows the definition is unused
- **Scope-aware fuzzer splicing**: identify free variables in spliced subtrees and skip or rename to match

These transforms significantly improve reduction quality (Vulcan's identifier unification alone accounts for measurable improvement in the literature).

## What Changes

- Implement `ScopeAnalysis` that loads `locals.scm` and maps scopes, definitions, and references
- Implement `UnifyIdentifiersTransform` using ScopeAnalysis
- Implement `DeadDefinitionTransform` using ScopeAnalysis
- Parse the `locals` field in build.rs and expose in `LanguageInfo`

## Impact

- Reduction quality improves for grammars with `locals.scm` (JavaScript ships one, others available from nvim-treesitter)
- No behavior change for grammars without `locals.scm`
- Depends on: fix-reducer-gaps (for `locals` field parsing in build.rs)
