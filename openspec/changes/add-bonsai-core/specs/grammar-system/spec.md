## ADDED Requirements

### Requirement: Vendored Grammar Submodules
The system SHALL support tree-sitter grammars as git submodules in a grammars/ directory, following difftastic's vendoring pattern.

#### Scenario: Add a new grammar
- **WHEN** a tree-sitter grammar is added as a git submodule under grammars/
- **THEN** it can be registered in grammars.toml and compiled at build time

### Requirement: Grammar Registry
The system SHALL maintain a grammars.toml file mapping language names to grammar paths, file extensions, source directories, optional supertype query files, and optional locals query files for scope analysis.

#### Scenario: Lookup by language name
- **WHEN** a language name is provided (e.g., "python")
- **THEN** the system returns the corresponding tree-sitter Language

#### Scenario: Lookup by file extension
- **WHEN** a file extension is provided (e.g., ".py")
- **THEN** the system returns the corresponding tree-sitter Language

#### Scenario: Unknown language
- **WHEN** an unrecognized language name or file extension is provided
- **THEN** the system returns an error listing supported languages

#### Scenario: Supertype query file
- **WHEN** a grammar entry in grammars.toml specifies a supertypes field
- **THEN** the QueryFileProvider loads it for compatibility checking

#### Scenario: Locals query file
- **WHEN** a grammar entry in grammars.toml specifies a locals field
- **THEN** the ScopeAnalysis module loads it for scope-aware transforms (identifier unification, dead definition removal)

#### Scenario: Missing locals file
- **WHEN** a grammar entry does not specify a locals field
- **THEN** scope-aware transforms are skipped for that language

### Requirement: Build-Time Grammar Compilation
The system SHALL compile tree-sitter grammar C/C++ sources at build time via build.rs in bonsai-core, including external scanners, and generate a Rust module for language lookup.

#### Scenario: Successful compilation with parser only
- **WHEN** a grammar has only parser.c
- **THEN** it is compiled and available at runtime

#### Scenario: Successful compilation with external scanner
- **WHEN** a grammar has scanner.c or scanner.cc alongside parser.c
- **THEN** both are compiled and linked correctly

#### Scenario: List supported languages
- **WHEN** `bonsai languages` is run
- **THEN** all registered languages and their file extensions are listed
