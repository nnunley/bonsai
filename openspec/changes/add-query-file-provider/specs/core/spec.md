## ADDED Requirements

### Requirement: QueryFileProvider
The system SHALL support loading supertype/subtype mappings from tree-sitter `.scm` query files, following the ecosystem pattern of `highlights.scm` and `locals.scm`.

#### Scenario: Load supertypes.scm
- **WHEN** a grammar entry in grammars.toml specifies a `supertypes` field pointing to a valid `.scm` file
- **THEN** QueryFileProvider loads the file and builds supertype/subtype mappings

#### Scenario: Invalid or missing .scm file
- **WHEN** the `supertypes` field points to a non-existent or unparseable file
- **THEN** QueryFileProvider logs a warning and provides no mappings (fallback to EmptyProvider behavior)

#### Scenario: Query format
- **WHEN** a `supertypes.scm` file annotates nodes with `@supertype.<group>` captures
- **THEN** all node types captured under the same group name are treated as subtypes of that group

### Requirement: ChainProvider Integration
The system SHALL use ChainProvider in the CLI to compose providers: LanguageApiProvider first, QueryFileProvider second (if `.scm` exists), EmptyProvider as fallback.

#### Scenario: Grammar with built-in supertypes and .scm file
- **WHEN** both LanguageApiProvider and QueryFileProvider return results
- **THEN** ChainProvider merges them (union of subtypes)

#### Scenario: Grammar with only .scm supertypes
- **WHEN** LanguageApiProvider returns empty but QueryFileProvider has mappings
- **THEN** the .scm mappings are used for compatibility checking
