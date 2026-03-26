## 1. QueryFileProvider Implementation
- [ ] 1.1 Define the `.scm` file format for supertype annotations (e.g., `(identifier) @supertype.expression`)
- [ ] 1.2 Implement `QueryFileProvider` that parses a `.scm` file using tree-sitter's query API
- [ ] 1.3 Build the supertype→subtype and subtype→supertype mappings from query matches
- [ ] 1.4 Implement `SupertypeProvider` trait for `QueryFileProvider`
- [ ] 1.5 Add tests with a handwritten test `.scm` file

## 2. ChainProvider Wiring
- [ ] 2.1 Wire ChainProvider in the CLI: LanguageApiProvider → QueryFileProvider (if scm exists) → EmptyProvider
- [ ] 2.2 Load the `supertypes_scm` path from `LanguageInfo` at runtime
- [ ] 2.3 Add integration test: grammar with supertypes.scm gets better reduction than EmptyProvider

## 3. Example supertypes.scm
- [ ] 3.1 Create a `supertypes.scm` for one grammar as an example
