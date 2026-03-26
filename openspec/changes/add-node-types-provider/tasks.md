## 1. Build-Time node-types.json Parsing
- [ ] 1.1 In build.rs, locate `node-types.json` in each grammar's src directory
- [ ] 1.2 Parse the JSON: extract types with `subtypes` arrays (these are supertypes)
- [ ] 1.3 Build a mapping: supertype_name → [subtype_names]
- [ ] 1.4 Generate Rust code with static supertype mappings per language (HashMap or match arms)

## 2. NodeTypesProvider
- [ ] 2.1 Implement `NodeTypesProvider` that wraps the build-time-generated mappings
- [ ] 2.2 Resolve type names to kind IDs at construction time using `Language::id_for_node_kind()`
- [ ] 2.3 Implement `SupertypeProvider` trait
- [ ] 2.4 Add test: verify NodeTypesProvider returns the same supertypes as LanguageApiProvider for Python (they should match since both come from the same grammar definition)

## 3. ChainProvider Integration
- [ ] 3.1 Update ChainProvider order: LanguageApiProvider → NodeTypesProvider → QueryFileProvider → EmptyProvider
- [ ] 3.2 For grammars where Language::supertypes() is empty, NodeTypesProvider fills the gap
- [ ] 3.3 Add test: grammar with no runtime supertypes still gets supertype info via NodeTypesProvider

## 4. Verification
- [ ] 4.1 Compare reduction quality with and without NodeTypesProvider on a sample input
- [ ] 4.2 Verify no regressions for grammars that already have runtime supertypes
