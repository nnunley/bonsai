## Architecture

Bonsai is a Rust workspace with four crates:

```
bonsai/
├── Cargo.toml              # workspace root
├── crates/
│   ├── bonsai-core/        # tree manipulation, node compatibility, transforms
│   │   └── build.rs        # compiles grammars, generates language registry
│   ├── bonsai-reduce/      # Perses reduction algorithm
│   ├── bonsai-fuzz/        # AST splicing fuzzer
│   └── bonsai-cli/         # unified CLI (bonsai reduce / bonsai fuzz)
├── grammars/               # vendored tree-sitter grammars (git submodules)
│   ├── tree-sitter-python/
│   │   └── src/
│   │       └── node-types.json  # supertypes extracted automatically at build time
│   └── ...
└── tests/                  # integration tests with real grammars
```

Note: `build.rs` belongs to `bonsai-core` (Cargo does not support workspace-level build scripts). Other crates access the language registry through `bonsai-core`'s public API.

## Core Library (bonsai-core)

### Node Type Compatibility

The key insight from Perses: when removing a node from the tree, you can only replace it with something the grammar allows in that position. Bonsai uses a layered approach to determine compatibility, with reparse-and-validate as the definitive gate.

**SupertypeProvider trait — pluggable compatibility sources:**

```rust
pub trait SupertypeProvider {
    fn supertypes_for(&self, kind_id: u16) -> &[u16];
    fn subtypes_for(&self, supertype_id: u16) -> &[u16];
}
```

Three built-in providers, tried in order via `ChainProvider`:

1. **`LanguageApiProvider`** — wraps `Language::supertypes()` / `subtypes_for_supertype()`. Best quality: grammar author-defined. Available for well-maintained grammars (Python, JavaScript, Rust, etc.). Many grammars return empty — this is expected.

2. **`NodeTypesProvider`** — parses each grammar's `node-types.json` at build time to extract supertype/subtype relationships. Every tree-sitter grammar ships this file, so this provider works for all grammars automatically. Type names are resolved to kind IDs at runtime via `Language::id_for_node_kind()`. This is the primary fallback when the runtime API returns empty (older grammar ABI versions).

3. **Fallback (no provider)** — when neither provider returns supertype information (should be rare since `node-types.json` is standard), the system still works via Delete and Unwrap transforms only. A warning is logged.

**Lookahead iterator — best-effort optimization hint (NOT an oracle):**

`Language::lookahead_iterator(state)` returns symbols valid at a parse state, but has important limitations:
- The state is the LR state at *entry* to a node, not a direct answer to "can this node be removed?"
- Determining optionality would require checking if the *following* symbol is valid at the node's *start* state — a more nuanced check that may be wrong in edge cases (extras, nested optional structures)
- Therefore: lookahead is used as a **best-effort pre-filter** to skip obviously invalid candidates. It may produce false positives and false negatives. The **definitive validity gate is always reparse + ERROR/MISSING node check.**

**Validity checking is the real safety net:**

For each candidate replacement:
1. *Optional:* Lookahead hint — skip if obviously invalid (may be wrong, that's OK)
2. *Required:* Apply replacement to source bytes, reparse with tree-sitter
3. *Required:* Check for ERROR *and* MISSING nodes — reject if either is present
4. Only then run the interestingness test

This means the system is always correct (no invalid candidates reach the test) even when supertype info is absent or lookahead gives wrong answers. Performance may vary — grammars with good supertype coverage will skip more invalid candidates early.

**Handling inputs with existing errors:**

If the initial input already contains ERROR or MISSING nodes (e.g., the grammar doesn't cover a language feature), the reducer tracks the initial error set and only rejects candidates that introduce *new* errors. Use `--strict` to require fully error-free output regardless.

### Transform Trait

```rust
pub trait Transform {
    /// Propose candidate replacements for a node.
    /// Returns a list of replacement text options.
    fn candidates(&self, node: &Node, source: &[u8], tree: &Tree, provider: &dyn SupertypeProvider) -> Vec<Replacement>;
}
```

Built-in transforms (Phase 1):
- **Delete**: Replace node with empty string (if grammar allows)
- **Unwrap**: Replace node with one of its children of compatible type

Scope-aware transforms (Phase 1b — uses `locals.scm`):
- **Unify identifiers**: Rename all identifiers to canonical short forms (`a`, `b`, `c`, ...) while preserving scope consistency. Uses `locals.scm` queries (`@local.scope`, `@local.definition`, `@local.reference`) to find all definitions and their references, then renames each binding and its references consistently. This is a semantics-preserving transform — the reduced code still has the same binding structure.
- **Dead definition removal**: If a `@local.definition` has no `@local.reference` within its scope, the entire definition statement can be deleted. Stronger than plain Delete because it knows the definition is unused.

Future transforms (Phase 2 — Vulcan):
- **Hoist**: Replace node with a compatible descendant (not just direct child)
- **Quantified node reduction**: Reduce lists (e.g., remove items from argument lists)

### Scope Analysis via locals.scm

Tree-sitter grammars can ship a `locals.scm` query file that defines scope, definition, and reference relationships. This gives bonsai scope-aware analysis without a full type system.

```
; Example locals.scm (JavaScript):
(statement_block) @local.scope
(function_declaration) @local.scope
(pattern/identifier) @local.definition
(variable_declarator name: (identifier) @local.definition)
(identifier) @local.reference
```

Bonsai loads `locals.scm` at startup (path configured in `grammars.toml`) and uses tree-sitter's query API to:
1. **Map scopes**: identify which nodes create new scopes
2. **Map definitions**: identify identifier definitions and their scope
3. **Map references**: identify which references point to which definitions (by name within scope)

This is exposed via a `ScopeAnalysis` struct:

```rust
pub struct ScopeAnalysis {
    /// definition_node_id → (name, scope_node_id)
    definitions: HashMap<usize, (String, usize)>,
    /// reference_node_id → definition_node_id (resolved)
    references: HashMap<usize, usize>,
    /// scope_node_id → list of definition_node_ids
    scopes: HashMap<usize, Vec<usize>>,
}
```

When `locals.scm` is not available for a grammar, scope-aware transforms are simply skipped (the same fallback pattern as SupertypeProvider).

The fuzzer also benefits: when splicing a subtree from one file into another, `ScopeAnalysis` can identify which free variables the subtree expects, and either skip incompatible splices or rename references to match the target scope.

### Tree Reconstruction

After applying a transform, we need valid source text. Strategy:
1. Start with the original source bytes
2. Apply the replacement (byte-range substitution)
3. Re-parse with tree-sitter (incremental parsing is fast)
4. Validate the new tree has no ERROR or MISSING nodes (or no *new* errors if `--strict` is not set)

This is simpler than Perses's approach (which reconstructs from grammar rules) because tree-sitter's incremental parser is fast enough to just re-parse.

**Important:** tree-sitter is an error-recovering parser. ERROR-free does not guarantee the output matches the grammar author's intent — tree-sitter may silently choose a different production. For delta debugging this is acceptable because the interestingness test is the true oracle. The validity check prevents obviously broken output, not subtly misparsed output.

## Reducer (bonsai-reduce)

### Algorithm: Perses-style Priority Queue (Parallel)

```
1. Parse input with tree-sitter → initial tree
2. Build priority queue: collect all named nodes as (byte_range, kind_id, token_count) tuples,
   ordered by token_count descending. Store byte ranges, NOT node handles (nodes are invalidated on reparse).
3. While queue is not empty AND limits not reached (--max-tests, --max-time):
   a. Pop the largest entry (byte_range, kind_id)
   b. Look up the node in the current tree by byte_range (skip if range no longer valid)
   c. For each Transform, generate candidate replacements
   d. Optionally pre-filter via lookahead (best-effort, may be wrong)
   e. For remaining candidates: apply replacement, reparse, check for ERROR/MISSING nodes
   f. Run interestingness tests on valid candidates in parallel (up to --jobs N)
      - With --jobs 1: deterministic (test in order, accept first interesting)
      - With --jobs >1: non-deterministic (accept first to return; document this)
   g. If interesting: accept replacement, reparse to get new tree,
      REBUILD the queue from scratch on the new tree (cheap relative to running tests)
   h. If no candidate worked, skip this entry (mark byte range as tried)
4. Termination: loop ends when queue is empty OR --max-tests/--max-time reached.
   Entries are never re-added for the same byte range within a single pass.
5. Output the final reduced source. On SIGINT, output best-so-far.
```

**Node identity after reparse:** Tree-sitter `Node` handles borrow from a `Tree` and are invalidated on reparse. The queue stores `(byte_range, kind_id)` tuples instead. After accepting a reduction, the entire queue is rebuilt by walking the new tree. This is the same approach Perses uses — queue rebuild is O(n) in tree size, negligible compared to running interestingness tests.

**Token count:** Defined as the number of descendant leaf nodes (both named and anonymous) in the subtree. Byte range size is an acceptable proxy that achieves the same priority ordering.

**Termination:** The loop terminates when (a) the queue drains (all nodes tried and skipped or reduced), (b) `--max-tests N` test invocations reached, or (c) `--max-time <duration>` wall-clock elapsed. At least one of these bounds should be set for large inputs.

**Determinism:** With `--jobs 1`, reduction is fully deterministic. With `--jobs >1`, the order of accepted candidates depends on OS scheduling. This is documented and acceptable — any interesting result is progress.

Key differences from level-by-level HDD:
- Processes largest nodes first (maximum reduction per test call)
- Lookahead as best-effort hint, reparse as definitive validity gate
- Parallel interestingness testing (candidates at the same node are independent)
- Priority queue rebuilt after each accepted reduction (handles node invalidation)
- Configurable termination bounds

### Interestingness Test Interface

```rust
pub trait InterestingnessTest: Send + Sync {
    fn is_interesting(&self, input: &[u8]) -> bool;
}

// CLI: wraps a shell command
pub struct ShellTest {
    command: Vec<String>,  // e.g., ["./check.sh"] — uses Command::new with args, no shell interpolation
    timeout: Duration,
}
```

The shell test writes the candidate to a temp file, invokes the command via `std::process::Command` with the file path as an argument (no shell interpolation to avoid injection issues), and checks exit code (0 = interesting).

### Caching

Hash each candidate's source bytes (xxhash) and cache the test result. Literature shows 24-62% of test calls are duplicates.

### Output

Reduced output goes to stdout by default. Use `--output <path>` to write to a file instead.

On SIGINT, the reducer outputs the best-so-far reduced result before exiting (to stdout or `--output`). Long reductions should not lose progress to Ctrl-C.

### Progress Reporting

The reducer reports progress to stderr:
- Current size vs. original size (bytes and percentage)
- Iteration count and candidates tested
- Cache hit rate
- Configurable via `--quiet` (no progress) and `--verbose` (per-candidate detail)

## Fuzzer (bonsai-fuzz)

### Algorithm: Type-Compatible AST Splicing

```
1. Parse all corpus files with tree-sitter
2. Build a node pool: map from (parent_kind, field_name, grammar_symbol) → list of (source_text)
   This indexes by POSITION (what parent/field the node appeared in), not just node type.
   Fragments from the same positional context are more likely to produce valid splices.
3. To generate a new input:
   a. Pick a random corpus file as the base
   b. Pick a random named node in the base tree
   c. Look up compatible fragments from the pool:
      - First try: same (parent_kind, field_name, grammar_symbol) — positional match
      - Fallback: same grammar_symbol via SupertypeProvider — type match only
   d. Replace the node with a random compatible fragment
   e. Re-parse to verify no ERROR/MISSING nodes
   f. Repeat b-e for N mutations (configurable via --mutations, default uniform random in [1,3])
4. Run the target program with the generated input
5. If interesting (crash, specific stderr, etc.): auto-reduce with bonsai-reduce (timeout: min(60s, 10x target timeout)), then save
```

### Corpus Management

- **Node pool construction**: On startup, parse all files in the corpus directory and index subtrees by grammar symbol. The pool is rebuilt if files change.
- **Deduplication of findings**: First by content hash (xxhash), then by error message (normalized stderr). Duplicate findings are counted but not saved again.
- **Grammar-diversity-guided corpus evolution** (opt-in via `--diversity-guided`): When a generated input produces a novel parse tree structure — measured by (parent_kind, child_kind) bigrams, not just the set of node kinds — it is added back to the corpus to diversify future generation. Note: this is structural diversity, not coverage-guided in the AFL/libFuzzer sense (which requires instrumentation). The bigram metric avoids premature saturation that a flat node-kind set would produce. Default mode uses a static corpus.

### Session Persistence

Fuzzer state is resumable by default:
- **State directory**: `--state-dir <path>` (default: `.bonsai-fuzz/` in the working directory)
- **Persisted state**: corpus index, node pool cache, findings with dedup hashes, execution stats, full PRNG state (using `rand_chacha::ChaCha8Rng` which implements `Serialize`/`Deserialize` — not just the seed)
- **Resume**: If `--state-dir` exists with prior state and corpus files have not changed, the fuzzer resumes with identical generation sequence. If corpus files changed, the PRNG state is kept but the node pool is rebuilt (generation diverges from the original sequence). Use `--fresh` to start over.
- **State format versioning**: state files include a version number. Incompatible versions trigger a warning and fresh start rather than deserialization errors.
- **Integrity**: state files include a checksum to detect corruption. Corrupted state triggers a warning and fresh start.

### Target Interface

```rust
pub struct FuzzTarget {
    command: Vec<String>,          // program to test — no shell interpolation
    input_mode: InputMode,         // Stdin or TempFile or ArgReplace("@@")
    interesting: InterestCriteria, // ExitCode, Stderr(regex), Timeout, Signal
}
```

`InterestCriteria` bridges to `InterestingnessTest` via `impl InterestingnessTest for FuzzTarget` — the fuzzer target IS an interestingness test. This avoids a separate type bridge. When auto-reducing a finding, the same `FuzzTarget` instance is passed directly to the reducer.

### Progress Reporting

The fuzzer reports to stderr:
- Executions per second, total executions
- Findings count (unique crashes/interesting results)
- Corpus size (and growth if coverage-guided)
- Configurable via `--quiet` and `--verbose`

## Grammar Plugin System

Following difftastic's pattern:

### Directory Structure
```
grammars/
├── tree-sitter-python/      # git submodule
├── tree-sitter-javascript/  # git submodule
├── tree-sitter-rust/        # git submodule
└── ...
```

### Registration

A `grammars.toml` file maps languages to their grammars:

```toml
[[language]]
name = "python"
grammar = "grammars/tree-sitter-python"
extensions = ["py", "pyi"]
src = "src"  # relative path to parser.c within the grammar

[[language]]
name = "rust"
grammar = "grammars/tree-sitter-rust"
extensions = ["rs"]
src = "src"
```

### Build System

`build.rs` (in `bonsai-core`) reads `grammars.toml`, compiles each grammar's C/C++ source via the `cc` crate, and generates a Rust module with:
- A function to get a `tree_sitter::Language` by name or file extension
- A list of supported languages for CLI help

The build script handles **external scanners** (`scanner.c` or `scanner.cc`) which many grammars require for correct parsing (Python indentation, JavaScript template literals, etc.). It detects scanner files in the grammar's `src/` directory and compiles them alongside `parser.c`.

This is simpler than difftastic's approach (which uses per-grammar feature flags) — we always compile all registered grammars.

## CLI Design

```
bonsai reduce --test "./check.sh" --lang python input.py
bonsai reduce --test "./check.sh" input.py                  # auto-detect lang from extension
bonsai reduce --test "./check.sh" --output min.py input.py  # write to file
bonsai reduce --test "./check.sh" --jobs 4 input.py         # parallel test execution
bonsai reduce --test "./check.sh" --max-tests 10000 input.py  # limit test invocations
bonsai reduce --test "./check.sh" --max-time 30m input.py   # wall-clock time limit
bonsai reduce --test "./check.sh" --test-timeout 10s input.py  # per-test timeout

bonsai fuzz --corpus ./seeds/ --lang python -- ./my-compiler @@
bonsai fuzz --corpus ./seeds/ --interesting-stderr "panic" -- ./my-compiler @@
bonsai fuzz --corpus ./seeds/ --diversity-guided -- ./my-compiler @@
bonsai fuzz --corpus ./seeds/ --state-dir ./fuzz-state/ -- ./my-compiler @@
bonsai fuzz --fresh --corpus ./seeds/ -- ./my-compiler @@   # discard prior state
bonsai fuzz --corpus ./seeds/ --max-execs 50000 -- ./my-compiler @@
bonsai fuzz --corpus ./seeds/ --max-time 1h -- ./my-compiler @@
bonsai fuzz --corpus ./seeds/ --test-timeout 5s -- ./my-compiler @@

bonsai languages                                            # list supported languages
```

All subcommands support `--quiet` and `--verbose` for progress control.

**Flag precedence:** `--lang` always takes precedence over auto-detection from file extension. If neither is provided and the extension is unrecognized, error with a clear message listing supported languages.

**Input mode for fuzzer:** If `@@` appears in the target command, use ArgReplace. Otherwise, default to Stdin. Use `--input-mode {stdin,file}` for explicit override.

## Technical Decisions

1. **Rust** — Performance for thousands of test iterations; first-class tree-sitter bindings; good CLI ecosystem (clap).
2. **Perses over HDD** — 2x faster, 1.13x smaller results, all intermediates valid.
3. **Re-parse for validation** — Simpler than grammar-based reconstruction; tree-sitter incremental parsing makes this fast.
4. **xxhash for caching** — Fast non-cryptographic hash; sufficient for dedup.
5. **grammars.toml over feature flags** — Simpler to add new languages; one config file vs. Cargo.toml changes.
6. **Workspace crates** — Clean separation of concerns; users can depend on just `bonsai-reduce` or `bonsai-fuzz` as a library.
