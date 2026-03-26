## ADDED Requirements

### Requirement: Scope Analysis
The system SHALL support loading tree-sitter locals.scm query files to build a ScopeAnalysis mapping scopes, definitions, and references.

#### Scenario: Build scope analysis from locals.scm
- **WHEN** a grammar has a locals.scm file and source is parsed
- **THEN** ScopeAnalysis maps each definition to its scope and resolves references to definitions by name within scope

#### Scenario: No locals.scm available
- **WHEN** a grammar has no locals.scm
- **THEN** ScopeAnalysis returns an empty mapping and scope-aware transforms are skipped

#### Scenario: Definition resolution
- **WHEN** an identifier reference is encountered
- **THEN** the system walks the scope chain to find the nearest definition with that name

### Requirement: Unify Identifiers Transform
The system SHALL provide a transform that renames all identifier bindings and their references to canonical short forms, preserving scope consistency.

#### Scenario: Rename bindings
- **WHEN** locals.scm is available and ScopeAnalysis has resolved definitions
- **THEN** each definition and its references are renamed to a, b, c, ..., preserving scope boundaries

#### Scenario: Multi-site replacement
- **WHEN** a definition has N references
- **THEN** the transform produces a batch replacement covering the definition site and all N reference sites

#### Scenario: No locals.scm
- **WHEN** locals.scm is not available
- **THEN** the transform produces no candidates

### Requirement: Dead Definition Removal Transform
The system SHALL provide a transform that deletes definitions with no references within their scope.

#### Scenario: Unused definition
- **WHEN** a definition has zero references in its scope
- **THEN** the transform proposes deleting the entire containing statement

#### Scenario: Used definition
- **WHEN** a definition has one or more references
- **THEN** the transform produces no candidates for that definition

#### Scenario: No locals.scm
- **WHEN** locals.scm is not available
- **THEN** the transform produces no candidates
