## Why

Every tree-sitter grammar ships a `node-types.json` file in its `src/` directory. This file contains the complete type hierarchy: types starting with `_` (like `_expression`, `_statement`) that have a `subtypes` array are supertype definitions. This is the same data that `Language::supertypes()` exposes at runtime — but many older grammars don't expose it via the runtime API.

Currently, grammars without runtime supertypes fall back to Delete/Unwrap only. But `node-types.json` is always available. By parsing it at build time, we can generate supertype mappings for every grammar automatically — no manual `.scm` files needed.

## What Changes

- Parse `node-types.json` at build time in `build.rs`
- Generate a static supertype mapping for each grammar
- Implement `NodeTypesProvider` that uses the build-time-generated mappings
- Insert it into the ChainProvider: LanguageApiProvider → NodeTypesProvider → QueryFileProvider → EmptyProvider

## Impact

- Every grammar automatically gets supertype information
- Significant reduction quality improvement for grammars where `Language::supertypes()` returns empty
- No manual `supertypes.scm` files needed (though they can still override/extend)
- Zero runtime cost (mappings are compiled in)
