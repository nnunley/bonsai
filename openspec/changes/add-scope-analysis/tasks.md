## 1. ScopeAnalysis
- [ ] 1.1 Define `ScopeAnalysis` struct: maps definition_node_id → (name, scope_node_id), reference_node_id → definition_node_id, scope_node_id → definition list
- [ ] 1.2 Implement `ScopeAnalysis::from_tree(tree, source, language, locals_query)` that runs the `locals.scm` query and builds the mappings
- [ ] 1.3 Handle `@local.scope`, `@local.definition`, `@local.reference` captures
- [ ] 1.4 Resolve references to definitions by name within scope (walk scope chain)
- [ ] 1.5 Add tests with JavaScript (which ships locals.scm): verify definitions and references are correctly mapped
- [ ] 1.6 Add test: grammar without locals.scm returns empty analysis

## 2. Unify Identifiers Transform
- [ ] 2.1 Implement `UnifyIdentifiersTransform` that takes ScopeAnalysis
- [ ] 2.2 For each definition, generate a canonical name (a, b, c, ..., aa, ab, ...)
- [ ] 2.3 Produce a multi-site Replacement that renames the definition and all its references
- [ ] 2.4 Note: this requires extending Replacement to support multiple edits, or applying them as a batch
- [ ] 2.5 Add test: Python code with long variable names → canonical short names, verifying the result parses and has the same structure

## 3. Dead Definition Removal Transform
- [ ] 3.1 Implement `DeadDefinitionTransform` that takes ScopeAnalysis
- [ ] 3.2 For each definition with zero references in scope, propose deletion of the containing statement
- [ ] 3.3 Add test: Python code with unused variables → unused definitions are removed
- [ ] 3.4 Add test: definition with references is NOT removed

## 4. Build System Integration
- [ ] 4.1 Parse `locals` field in build.rs `LanguageEntry`
- [ ] 4.2 Include `locals_scm` (embedded content) in generated `LanguageInfo`
- [ ] 4.3 Load locals.scm at runtime in the CLI when available
- [ ] 4.4 Add UnifyIdentifiers and DeadDefinition transforms to the reducer's transform list when locals.scm is available

## 5. Fuzzer Scope-Aware Splicing
- [ ] 5.1 Use ScopeAnalysis to identify free variables in a spliced subtree
- [ ] 5.2 Skip splices where free variables are undefined in the target scope
- [ ] 5.3 Optionally rename free variables to match target scope definitions
- [ ] 5.4 Add test: scope-aware splicing has higher acceptance rate than blind splicing
