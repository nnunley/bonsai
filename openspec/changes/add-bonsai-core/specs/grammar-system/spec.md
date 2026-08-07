## ADDED Requirements

### Requirement: Released Grammar Dependencies
The system SHALL consume published tree-sitter grammar crates through Cargo and keep Bonsai-specific metadata separate from upstream parser sources.

#### Scenario: Add a new grammar
- **WHEN** a released tree-sitter grammar crate is added as a dependency and registered in bonsai-core/grammars.toml
- **THEN** Cargo obtains and compiles it without Git submodule initialization

### Requirement: Grammar Registry
The system SHALL maintain a grammars.toml file mapping language names to grammar paths, file extensions, source directories, and optional locals query files for scope analysis. Supertypes are extracted automatically from node-types.json at build time.

#### Scenario: Lookup by language name
- **WHEN** a language name is provided (e.g., "python")
- **THEN** the system returns the corresponding tree-sitter Language

#### Scenario: Lookup by file extension
- **WHEN** a file extension is provided (e.g., ".py")
- **THEN** the system returns the corresponding tree-sitter Language

#### Scenario: Unknown language
- **WHEN** an unrecognized language name or file extension is provided
- **THEN** the system returns an error listing supported languages

#### Scenario: Automatic supertype extraction
- **WHEN** a grammar's src/ directory contains node-types.json
- **THEN** the build system extracts supertype/subtype relationships and generates a NodeTypesProvider

#### Scenario: Locals query file
- **WHEN** a grammar entry in grammars.toml specifies a locals field
- **THEN** the ScopeAnalysis module loads it for scope-aware transforms (identifier unification, dead definition removal)

#### Scenario: Missing locals file
- **WHEN** a grammar entry does not specify a locals field
- **THEN** scope-aware transforms are skipped for that language

### Requirement: Build-Time Grammar Compilation
The system SHALL link released tree-sitter grammar crates and generate a Rust module for language lookup and Bonsai-owned metadata via build.rs in bonsai-core.

#### Scenario: Successful compilation with parser only
- **WHEN** a registered grammar crate exports `LANGUAGE` and `NODE_TYPES`
- **THEN** it is compiled by Cargo and available at runtime

#### Scenario: Successful compilation with external scanner
- **WHEN** a grammar crate requires an external scanner
- **THEN** the grammar crate owns its compilation and linking

#### Scenario: List supported languages
- **WHEN** `bonsai languages` is run
- **THEN** all registered languages and their file extensions are listed
