## Why

The spec defines three supertype providers: LanguageApiProvider, QueryFileProvider, and ChainProvider. LanguageApiProvider and ChainProvider are implemented, but QueryFileProvider is a TODO placeholder. Many tree-sitter grammars lack built-in supertypes but could have community-contributed `supertypes.scm` files (similar to how `highlights.scm` and `locals.scm` are maintained by nvim-treesitter and Helix).

Without QueryFileProvider, grammars lacking built-in supertypes fall back to Delete/Unwrap only, producing lower-quality reductions.

## What Changes

- Implement `QueryFileProvider` that loads a `.scm` query file and builds supertype mappings
- Wire it into the ChainProvider in the CLI (LanguageApiProvider → QueryFileProvider → EmptyProvider)
- Add `supertypes` field support in the generated `LanguageInfo`

## Impact

- Grammars with `supertypes.scm` files get better reduction quality
- No behavior change for grammars with built-in supertypes (LanguageApiProvider takes precedence)
