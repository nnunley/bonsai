## ADDED Requirements

### Requirement: Tree-Sitter Parse Tree Manipulation
The system SHALL parse source files into tree-sitter concrete syntax trees and support incremental reparsing after byte-range modifications.

#### Scenario: Parse source file
- **WHEN** a source file and language are provided
- **THEN** the system produces a tree-sitter parse tree

#### Scenario: Incremental reparse after edit
- **WHEN** a byte range in the source is replaced
- **THEN** the system re-parses incrementally and returns an updated tree

### Requirement: SupertypeProvider Trait
The system SHALL provide a pluggable trait for determining node type compatibility, with three built-in providers: LanguageApiProvider (tree-sitter's runtime API), QueryFileProvider (supertypes.scm files), and a ChainProvider that composes them in order.

#### Scenario: Language API supertypes available
- **WHEN** a grammar defines supertypes via the tree-sitter Language API
- **THEN** LanguageApiProvider returns subtype lists for each supertype

#### Scenario: Query file supertypes
- **WHEN** a grammar has no built-in supertypes but ships a supertypes.scm file
- **THEN** QueryFileProvider loads the file and provides compatibility mappings

#### Scenario: No supertypes available
- **WHEN** neither the Language API nor a query file provides supertypes
- **THEN** the system falls back to Delete and Unwrap transforms only, and logs a warning

#### Scenario: Chain provider composition
- **WHEN** multiple providers are available
- **THEN** ChainProvider tries them in order and merges results

### Requirement: Node Type Compatibility
The system SHALL determine which nodes can legally replace a given node using the SupertypeProvider and reparse validation as the definitive gate.

#### Scenario: Supertype-compatible replacement
- **WHEN** a node occupies a position expecting supertype S (per SupertypeProvider)
- **THEN** any subtree whose type is a subtype of S is a candidate replacement

#### Scenario: Optional node deletion
- **WHEN** a node deletion is attempted
- **THEN** the candidate is validated by reparsing and checking for ERROR/MISSING nodes

#### Scenario: Concrete position replacement
- **WHEN** a node occupies a position expecting a specific concrete type
- **THEN** only subtrees of that exact type are candidate replacements

### Requirement: Transform System
The system SHALL provide a trait-based transform system that proposes candidate replacements for tree nodes.

#### Scenario: Delete transform
- **WHEN** a node can be removed without violating grammar constraints
- **THEN** the Delete transform proposes an empty-string replacement

#### Scenario: Unwrap transform
- **WHEN** a node has a child with a compatible grammar symbol
- **THEN** the Unwrap transform proposes replacing the node with that child

### Requirement: Scope Analysis via locals.scm
The system SHALL support loading tree-sitter locals.scm query files to analyze scope, definitions, and references. When available, this enables scope-aware transforms and smarter fuzzer splicing.

#### Scenario: Load locals.scm
- **WHEN** a grammar has a locals field in grammars.toml pointing to a valid locals.scm file
- **THEN** the system builds a ScopeAnalysis mapping definitions to their references within scope boundaries

#### Scenario: No locals.scm available
- **WHEN** a grammar has no locals field in grammars.toml
- **THEN** scope-aware transforms are skipped and the system falls back to syntactic-only transforms

#### Scenario: Resolve identifier references
- **WHEN** a locals.scm is loaded and an identifier node is encountered
- **THEN** the system can determine whether it is a definition or a reference, and which definition a reference points to

### Requirement: Scope-Aware Transforms
The system SHALL provide transforms that use ScopeAnalysis to perform semantics-preserving reductions when locals.scm is available.

#### Scenario: Unify identifiers
- **WHEN** locals.scm is available and multiple identifiers exist
- **THEN** the Unify Identifiers transform renames each binding and all its references to a canonical short form (a, b, c, ...) preserving scope consistency

#### Scenario: Dead definition removal
- **WHEN** locals.scm is available and a definition has no references within its scope
- **THEN** the Dead Definition Removal transform proposes deleting the entire definition statement

#### Scenario: Scope-aware transforms skipped without locals.scm
- **WHEN** no locals.scm is available for the grammar
- **THEN** scope-aware transforms produce no candidates (graceful fallback)

### Requirement: Syntactic Validity Checking
The system SHALL verify all candidate replacements by reparsing and checking for ERROR and MISSING nodes. Lookahead iterator MAY be used as a best-effort pre-filter but is NOT the definitive gate.

#### Scenario: Valid replacement accepted
- **WHEN** a replacement is applied and the reparsed tree contains no new ERROR or MISSING nodes
- **THEN** the replacement is considered syntactically valid

#### Scenario: Invalid replacement rejected
- **WHEN** a replacement is applied and the reparsed tree contains new ERROR or MISSING nodes
- **THEN** the replacement is rejected

#### Scenario: Input with existing errors
- **WHEN** the initial input already contains ERROR or MISSING nodes
- **THEN** the system tracks the initial error set and only rejects candidates that introduce new errors

#### Scenario: Strict mode
- **WHEN** --strict is specified
- **THEN** the system requires fully error-free output regardless of initial input state
