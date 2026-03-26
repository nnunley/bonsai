## ADDED Requirements

### Requirement: NodeTypesProvider from node-types.json
The system SHALL parse each grammar's `node-types.json` at build time to extract supertype/subtype relationships, and expose them via a `NodeTypesProvider` that implements `SupertypeProvider`.

#### Scenario: Grammar with supertypes in node-types.json
- **WHEN** a grammar's `node-types.json` contains types with `subtypes` arrays (e.g., `_expression` with subtypes `identifier`, `binary_expression`, etc.)
- **THEN** NodeTypesProvider maps each supertype to its subtypes and vice versa

#### Scenario: All grammars get supertype information
- **WHEN** any registered grammar is used for reduction
- **THEN** NodeTypesProvider provides supertype information (since every grammar has `node-types.json`)

#### Scenario: Runtime name-to-ID resolution
- **WHEN** NodeTypesProvider is constructed with a tree-sitter Language
- **THEN** type names from `node-types.json` are resolved to kind IDs using `Language::id_for_node_kind()`

### Requirement: ChainProvider Priority Order
The system SHALL compose supertype providers in this order: LanguageApiProvider (runtime API) → NodeTypesProvider (build-time JSON) → QueryFileProvider (.scm files) → EmptyProvider.

#### Scenario: Runtime API supersedes node-types.json
- **WHEN** LanguageApiProvider returns non-empty results for a type
- **THEN** those results take precedence (NodeTypesProvider results are merged but API results are included first)

#### Scenario: node-types.json fills gaps
- **WHEN** LanguageApiProvider returns empty (older grammar ABI)
- **THEN** NodeTypesProvider provides the mappings from `node-types.json`

#### Scenario: .scm files extend further
- **WHEN** a QueryFileProvider `.scm` file defines additional compatibility groups not in `node-types.json`
- **THEN** those are merged into the result
