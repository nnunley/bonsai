## Dependency Graph

```
1.1 (workspace)
├── 1.2 (submodules)
│   └── 1.3 (grammars.toml)
│       └── 1.4 (build.rs)
│           ├── 2.1 (parse/reparse)
│           │   ├── 2.2 (SupertypeProvider) ──┐
│           │   │   └── 2.3 (compatibility)   │
│           │   │       └── 2.4 (Transform)   │
│           │   │           ├── 2.5 (Delete)   │
│           │   │           └── 2.6 (Unwrap)   │
│           │   └── 2.7 (validity) ───────────┘
│           │       └── 2.8 (core tests)
│           │
│           ├── 3.3 (InterestingnessTest) ─── can start after 1.4
│           │   └── 3.4 (caching)
│           │
│           └── 4.3 (FuzzTarget) ──────────── can start after 1.4
│               └── 4.4 (interest criteria)
│
│   After 2.8 (core complete):
│   ├── 3.1 (priority queue)
│   │   └── 3.2 (reduction loop) ← also needs 3.3, 3.4
│   │       ├── 3.5 (output/SIGINT)
│   │       ├── 3.6 (progress)
│   │       └── 3.7 (reducer integration tests)
│   │
│   └── 4.1 (corpus/node pool) ← also needs 2.2, 2.3
│       └── 4.2 (splicing)
│           └── 4.5 (dedup)
│               ├── 4.6 (diversity-guided)
│               ├── 4.7 (persistence)
│               └── 4.8 (auto-reduction) ← also needs 3.2
│                   └── 4.9 (progress)
│                       └── 4.10 (fuzzer integration tests)
│
│   After 3.7 AND 4.10:
│   └── 5.1 (clap setup)
│       ├── 5.2 (reduce cmd)
│       ├── 5.3 (fuzz cmd)
│       ├── 5.4 (languages cmd)
│       └── 5.5 (e2e tests) ← needs 5.2, 5.3, 5.4
```

### Parallelizable work streams

After 1.4 completes, three independent streams can run concurrently:
1. **Core** (2.1→2.8): tree manipulation, compatibility, transforms, validity
2. **Shell execution** (3.3→3.4): InterestingnessTest, ShellTest, caching — no core dependency
3. **Target execution** (4.3→4.4): FuzzTarget, interest criteria — no core dependency

After core completes (2.8), two more streams:
4. **Reducer** (3.1→3.7): needs core + shell execution
5. **Fuzzer** (4.1→4.10): needs core + target execution + reducer (for auto-reduction)

CLI (5.x) is last — needs both reducer and fuzzer.

---

## 1. Project Scaffolding

### Task 1.1: Initialize Cargo workspace

**Depends on:** nothing
**Requirement:** All — project foundation

#### RED
- Write a test that: runs `cargo check` on the workspace and expects it to succeed with four crates visible
- Expected failure: no Cargo.toml or crates exist yet
- If it passes unexpectedly: workspace was already initialized

#### GREEN
- Create root `Cargo.toml` with workspace members: bonsai-core, bonsai-reduce, bonsai-fuzz, bonsai-cli
- Create each crate under `crates/` with minimal `Cargo.toml` and `src/lib.rs` (or `src/main.rs` for CLI)
- Add tree-sitter dependency to bonsai-core
- Set up workspace dependency inheritance (shared deps in workspace Cargo.toml)

#### REFACTOR
- Ensure consistent edition, license, and repository fields across all crate manifests

---

### Task 1.2: Set up grammar submodules

**Requirement:** Vendored Grammar Submodules

#### RED
- Write a test that: checks `grammars/tree-sitter-python/src/parser.c` exists
- Expected failure: no grammars directory or submodules
- If it passes unexpectedly: submodules were already added

#### GREEN
- Create `grammars/` directory
- Add git submodules: tree-sitter-python, tree-sitter-javascript, tree-sitter-rust

#### REFACTOR
- Verify `.gitmodules` is clean and submodule paths are consistent

---

### Task 1.3: Create grammars.toml

**Requirement:** Grammar Registry

#### RED
- Write a test that: parses `grammars.toml` and finds entries for python, javascript, rust with correct extensions and optional supertypes field
- Expected failure: file doesn't exist
- If it passes unexpectedly: registry was already created

#### GREEN
- Create `grammars.toml` with `[[language]]` entries for each grammar
- Include name, grammar path, extensions, src directory, and optional supertypes field

#### REFACTOR
- Ensure TOML structure is consistent and documented with comments

---

### Task 1.4: Implement build.rs in bonsai-core

**Requirement:** Build-Time Grammar Compilation

#### RED
- Write a test that: calls `bonsai_core::languages::get_language("python")` and gets a valid `tree_sitter::Language`
- Write a test that: a grammar with an external scanner (Python has scanner.c) parses indentation-sensitive code correctly
- Expected failure: no build.rs, no generated module
- If it passes unexpectedly: build system was already in place

#### GREEN
- Implement `build.rs` in `crates/bonsai-core/` that reads `grammars.toml`
- Compile each grammar's C source via `cc` crate, detecting and compiling `scanner.c`/`scanner.cc` alongside `parser.c`
- Generate a Rust module with `get_language(name)`, `get_language_by_extension(ext)`, and `list_languages()`
- Set `cargo:rerun-if-changed` for grammar source files

#### REFACTOR
- Extract TOML parsing into a helper struct

---

## 2. Core Library (bonsai-core)

### Task 2.1: Parse/reparse utilities

**Requirement:** Tree-Sitter Parse Tree Manipulation

#### RED
- Write a test that: parses `def foo(): pass` as Python and asserts root node kind is `module`
- Write a test that: modifies a byte range and reparses incrementally, asserting the tree updates correctly
- Expected failure: no parse utilities exist
- If it passes unexpectedly: utilities already implemented

#### GREEN
- Implement `parse(source: &[u8], language: Language) -> Tree`
- Implement `reparse(tree: &mut Tree, source: &[u8], edit: &InputEdit) -> Tree` using tree-sitter's incremental parsing

#### REFACTOR
- Ensure the API takes `&[u8]` consistently
- Check if Parser can be reused across calls (avoid repeated allocation)

---

### Task 2.2: SupertypeProvider trait and implementations

**Requirement:** SupertypeProvider Trait

#### RED
- Write a test that: LanguageApiProvider for Python returns subtypes for `_expression` supertype
- Write a test that: LanguageApiProvider for a grammar with no supertypes returns empty lists
- Write a test that: QueryFileProvider loads a test `.scm` file and returns the defined mappings
- Write a test that: ChainProvider tries LanguageApiProvider first, falls back to QueryFileProvider
- Expected failure: no SupertypeProvider exists
- If it passes unexpectedly: already implemented

#### GREEN
- Define `SupertypeProvider` trait with `supertypes_for(kind_id)` and `subtypes_for(supertype_id)`
- Implement `LanguageApiProvider` wrapping `Language::supertypes()` / `subtypes_for_supertype()`
- Implement `QueryFileProvider` that parses a `.scm` file into compatibility mappings
- Implement `ChainProvider` that tries providers in order and merges results
- Cache supertype→subtype mappings per language (they don't change)

#### REFACTOR
- Ensure the ChainProvider merge is correct (union of subtypes, not replacement)
- Add logging when falling back to lower-tier providers

---

### Task 2.3: Node type compatibility checking

**Requirement:** Node Type Compatibility

#### RED
- Write a test that: for a Python `if_statement` containing a `binary_expression`, queries compatible replacements for the expression position and finds other expression subtypes via SupertypeProvider
- Write a test that: for a concrete-only position, confirms only exact type matches are returned
- Expected failure: no compatibility module exists
- If it passes unexpectedly: compatibility logic already implemented

#### GREEN
- Implement `compatible_replacements(node: &Node, provider: &dyn SupertypeProvider) -> Vec<u16>` using provider's subtype info
- Handle named vs anonymous nodes via `node_kind_is_named()`

#### REFACTOR
- Ensure the API is ergonomic for both reducer and fuzzer consumers

---

### Task 2.4: Transform trait and Replacement type

**Requirement:** Transform System

#### RED
- Write a test that: defines a mock Transform, calls `candidates()` on a node, and gets back `Vec<Replacement>`
- Expected failure: no Transform trait exists
- If it passes unexpectedly: trait already defined

#### GREEN
- Define `Replacement` struct (start_byte, end_byte, new_bytes)
- Define `Transform` trait with `fn candidates(&self, node: &Node, source: &[u8], tree: &Tree, provider: &dyn SupertypeProvider) -> Vec<Replacement>`

#### REFACTOR
- Ensure Replacement includes enough context for the caller to apply it

---

### Task 2.5: Delete transform

**Requirement:** Transform System — Delete

#### RED
- Write a test that: applies Delete to a removable node (e.g., else clause) and gets an empty-string replacement
- Write a test that: applies Delete to a required node — candidate is generated but rejected by reparse validation
- Expected failure: no Delete transform exists
- If it passes unexpectedly: Delete already implemented

#### GREEN
- Implement `DeleteTransform` that proposes empty-string replacement for named nodes
- Validity is determined by the caller via reparse, not by the transform itself

#### REFACTOR
- Ensure Delete handles edge cases: whitespace/delimiter cleanup around removed nodes

---

### Task 2.6: Unwrap transform

**Requirement:** Transform System — Unwrap

#### RED
- Write a test that: applies Unwrap to a `parenthesized_expression` containing an `identifier`, and gets back the identifier text as a replacement
- Write a test that: applies Unwrap to a node with no compatible children and gets no candidates
- Expected failure: no Unwrap transform exists
- If it passes unexpectedly: Unwrap already implemented

#### GREEN
- Implement `UnwrapTransform` that iterates children, checks type compatibility via SupertypeProvider, and proposes the child's source text as a replacement

#### REFACTOR
- Consider whether to propose all compatible children or just the largest

---

### Task 2.7: Tree reconstruction and validity checking

**Requirement:** Syntactic Validity Checking

#### RED
- Write a test that: applies a Replacement to source bytes and gets back new source with the replacement applied
- Write a test that: a valid replacement produces a tree with no ERROR/MISSING nodes
- Write a test that: an invalid replacement produces ERROR or MISSING nodes and is rejected
- Write a test that: an input with existing ERROR nodes — the system tracks initial errors and only rejects NEW errors
- Write a test that: --strict mode rejects any ERROR nodes regardless of initial state
- Write a test that: MISSING nodes are treated as errors
- Expected failure: no reconstruction or validation functions exist
- If it passes unexpectedly: already implemented

#### GREEN
- Implement `apply_replacement(source: &[u8], replacement: &Replacement) -> Vec<u8>`
- Implement `validate_tree(tree: &Tree, initial_errors: Option<&ErrorSet>) -> bool` that walks the tree checking for ERROR and MISSING nodes
- Implement `collect_errors(tree: &Tree) -> ErrorSet` for tracking initial input state
- Compose into `is_valid_replacement()` that applies, reparses, and validates

#### REFACTOR
- Ensure ERROR/MISSING walk is efficient (early return on first new error)

---

### Task 2.8: Unit tests for core

**Requirement:** All core requirements

#### RED
- Review all existing tests from tasks 2.1–2.7 for coverage gaps
- Write tests for: empty source, single-token source, deeply nested trees
- Write a test that: exercises a grammar with zero supertypes end-to-end (system still makes progress via Delete/Unwrap)
- Write a test that: exercises multiple grammars (Python, JavaScript, Rust)

#### GREEN
- Fill coverage gaps with targeted tests

#### REFACTOR
- Extract common test fixtures into test helpers

---

## 3. Reducer (bonsai-reduce)

### Task 3.1: Priority queue with byte-range storage

**Requirement:** Perses-Style Priority Queue Reduction

#### RED
- Write a test that: builds a priority queue from a parsed tree and pops entries in descending order of token count (leaf node count)
- Write a test that: entries are (byte_range, kind_id, token_count) tuples, NOT Node handles
- Write a test that: after rebuilding the queue on a modified tree, old byte ranges that no longer exist are absent
- Expected failure: no priority queue implementation
- If it passes unexpectedly: already implemented

#### GREEN
- Implement `ReductionQueue` that collects all named nodes as (byte_range, kind_id, token_count) tuples
- Sort by token_count descending using `BinaryHeap`
- Implement `rebuild(tree: &Tree)` that walks the new tree and creates a fresh queue

#### REFACTOR
- Verify that token_count (leaf node count) produces the same ordering as byte range size in practice

---

### Task 3.2: Perses-style reduction loop with parallel testing

**Requirement:** Perses-Style Priority Queue Reduction, Termination Bounds

#### RED
- Write a test that: reduces `if True:\n  x = 1\n  y = 2\nelse:\n  z = 3` with a test that checks for `x = 1`, and produces a minimal output
- Write a test that: with --jobs 2, runs candidate tests concurrently (mock test with artificial delay to verify parallelism)
- Write a test that: with --jobs 1, reduction is deterministic (same input → same output)
- Write a test that: an already-minimal input (single token) terminates in bounded time
- Write a test that: --max-tests stops after N test invocations
- Write a test that: --max-time stops after duration elapses
- Expected failure: no reduction loop exists
- If it passes unexpectedly: loop already implemented

#### GREEN
- Implement the main reduction loop:
  - Pop entry → look up node by byte_range in current tree → generate candidates → validate → test → accept or skip
  - On acceptance: reparse, rebuild queue from new tree
  - Entries never re-added for the same byte range within a pass
- Use a thread pool (rayon or tokio) sized by `--jobs` for parallel testing
- Implement --max-tests and --max-time bounds

#### REFACTOR
- Extract the single-node reduction step into its own function for testability

---

### Task 3.3: InterestingnessTest trait and ShellTest

**Requirement:** Interestingness Test Interface

#### RED
- Write a test that: creates a ShellTest with `["grep", "hello"]`, passes it input containing "hello", and gets `true`
- Write a test that: ShellTest times out after --test-timeout and returns `false`
- Write a test that: ShellTest uses Command::new (not shell interpolation) — test with filename containing spaces and special characters
- Write a test that: temp files are created with restrictive permissions (0600)
- Expected failure: no InterestingnessTest trait or ShellTest
- If it passes unexpectedly: already implemented

#### GREEN
- Define `InterestingnessTest` trait with `Send + Sync` bounds
- Implement `ShellTest`: write input to temp file (via `tempfile::NamedTempFile`), invoke via `Command::new(&args[0]).args(&args[1..]).arg(temp_path)`, check exit code 0
- Implement configurable timeout that kills process and descendants (process group kill)

#### REFACTOR
- Consider reusing temp file path across calls to reduce filesystem overhead

---

### Task 3.4: Test result caching

**Requirement:** Test Result Caching

#### RED
- Write a test that: caches a result, queries the same bytes, gets a cache hit without re-running the test
- Write a test that: queries different bytes and gets a cache miss
- Expected failure: no cache implementation
- If it passes unexpectedly: caching already implemented

#### GREEN
- Implement `TestCache` using `HashMap<u64, bool>` keyed by xxhash-64 of source bytes
- Integrate into the reduction loop (check cache before running test)

#### REFACTOR
- Document that 64-bit hash collisions are tolerable for the test cache (wrong cache = one extra/skipped test, not catastrophic)
- Consider LRU eviction for very long reduction runs

---

### Task 3.5: Output handling and graceful shutdown

**Requirement:** Reduction Output, Graceful Shutdown

#### RED
- Write a test that: reduces with no --output and captures stdout, verifying it contains the reduced source
- Write a test that: reduces with --output and verifies the file is written
- Write a test that: on SIGINT, the best-so-far result is written before exit
- Expected failure: no output handling
- If it passes unexpectedly: output handling already implemented

#### GREEN
- Default: write reduced bytes to stdout
- With `--output <path>`: write to file
- Install SIGINT handler that writes best-so-far and exits cleanly

#### REFACTOR
- Ensure stdout output doesn't mix with progress (progress goes to stderr)

---

### Task 3.6: Progress reporting

**Requirement:** Progress Reporting (reducer)

#### RED
- Write a test that: runs reducer in default mode and captures stderr, verifying it contains size/percentage info
- Write a test that: runs with --quiet and verifies stderr is empty
- Expected failure: no progress reporting
- If it passes unexpectedly: progress already implemented

#### GREEN
- Implement progress reporter that writes to stderr: current size, percentage reduction, iteration count, cache hit rate
- Respect --quiet (suppress) and --verbose (per-candidate detail)
- Rate-limit updates to ~1/sec

#### REFACTOR
- Use a consistent progress format

---

### Task 3.7: Reducer integration tests

**Requirement:** All reducer requirements

#### RED
- Write integration tests with real grammars:
  - Python: reduce a script with a known interesting property
  - JavaScript: reduce a file that triggers a specific parse pattern
- Write a test that: verifies the reduced output has no new ERROR/MISSING nodes
- Write a test that: verifies caching reduces the number of test invocations
- Write a test that: exercises a grammar with no supertypes — reducer still makes progress via Delete/Unwrap

#### GREEN
- Create test fixtures with known-reducible inputs and shell-script interestingness tests
- Run end-to-end reduction and assert on output size and validity

#### REFACTOR
- Extract test fixture creation into helpers
- Ensure tests are deterministic (--jobs 1)

---

## 4. Fuzzer (bonsai-fuzz)

### Task 4.1: Corpus parsing and positional node pool

**Requirement:** Corpus-Based AST Splicing

#### RED
- Write a test that: parses a corpus of 3 Python files and builds a node pool indexed by (parent_kind, field_name, grammar_symbol)
- Write a test that: the pool contains positional entries, not just flat symbol entries
- Expected failure: no corpus parsing
- If it passes unexpectedly: corpus parsing already implemented

#### GREEN
- Implement `CorpusIndex` that parses all files in a directory with tree-sitter
- Build `NodePool`: `HashMap<(u16, Option<u16>, u16), Vec<NodeFragment>>` mapping (parent_kind, field_id, grammar_symbol) → list of source text fragments
- Also maintain a flat `HashMap<u16, Vec<NodeFragment>>` for fallback by symbol only

#### REFACTOR
- Consider memory efficiency for large corpora (store byte ranges into source rather than copying text)

---

### Task 4.2: Type-compatible AST splicing with positional preference

**Requirement:** Corpus-Based AST Splicing

#### RED
- Write a test that: splices a node using a positional match and the result parses without ERROR/MISSING nodes
- Write a test that: when no positional match exists, falls back to type-compatible match via SupertypeProvider
- Write a test that: with --mutations 3, exactly 3 splice operations are applied
- Write a test that: default mutation count is uniform random in [1,3]
- Expected failure: no splicing implementation
- If it passes unexpectedly: splicing already implemented

#### GREEN
- Implement `splice()` that picks random node, looks up positional matches first, falls back to type matches
- Re-parse and check for ERROR/MISSING nodes
- Support configurable mutation count

#### REFACTOR
- Handle the case where no compatible fragment exists (skip and pick another node)
- Measure rejection rate and log it in verbose mode

---

### Task 4.3: FuzzTarget with input modes and InterestingnessTest impl

**Requirement:** Fuzz Target Interface

#### RED
- Write a test that: sends input via stdin to `cat` and captures stdout matching the input
- Write a test that: writes input to a temp file and passes the path as an argument
- Write a test that: replaces `@@` in the command with the temp file path
- Write a test that: default mode (no @@) uses stdin
- Write a test that: FuzzTarget implements InterestingnessTest trait (can be passed directly to the reducer)
- Expected failure: no FuzzTarget implementation
- If it passes unexpectedly: FuzzTarget already implemented

#### GREEN
- Implement `FuzzTarget` with `InputMode::Stdin`, `InputMode::TempFile`, `InputMode::ArgReplace(String)`
- Auto-detect: if `@@` in command → ArgReplace; else → Stdin. Allow `--input-mode` override.
- Implement `InterestingnessTest for FuzzTarget` so it bridges directly to the reducer

#### REFACTOR
- Share temp file management logic with ShellTest (extract into common utility)

---

### Task 4.4: Interest criteria with signal support

**Requirement:** Interest Criteria

#### RED
- Write a test that: detects a non-zero exit code as interesting
- Write a test that: detects a specific exit code (--interesting-exit 139) as interesting
- Write a test that: detects a signal kill (SIGSEGV) on Unix as interesting
- Write a test that: matches stderr against a regex and marks as interesting
- Write a test that: detects a timeout and kills process group
- Write a test that: criteria can be combined (crash AND stderr match)
- Expected failure: no interest criteria
- If it passes unexpectedly: criteria already implemented

#### GREEN
- Implement `InterestCriteria` enum/struct with ExitCode(Option<i32>), Signal, Stderr(Regex), Timeout(Duration)
- Use `ExitStatus::signal()` on Unix for signal detection
- Kill process group (not just direct child) on timeout

#### REFACTOR
- Ensure platform-specific signal handling is behind cfg(unix)

---

### Task 4.5: Finding deduplication with xxhash-128

**Requirement:** Finding Deduplication

#### RED
- Write a test that: saves a finding, then encounters identical content and it's counted but not saved again
- Write a test that: saves a finding, then encounters different content with the same normalized stderr, and it's counted but not saved
- Write a test that: different content with different stderr is saved as a new finding
- Write a test that: uses xxhash-128 (not 64) for content hashing
- Expected failure: no dedup implementation
- If it passes unexpectedly: dedup already implemented

#### GREEN
- Implement `FindingStore` with content hash (xxhash-128) index and normalized stderr index
- Default normalization: strip hex addresses (`0x[0-9a-f]+`), numeric sequences of 4+ digits, absolute paths
- Make normalization extensible via `--normalize-stderr` regex substitutions

#### REFACTOR
- Document default normalization rules clearly

---

### Task 4.6: Grammar-diversity-guided corpus evolution

**Requirement:** Grammar-Diversity-Guided Corpus Evolution

#### RED
- Write a test that: with --diversity-guided, a generated input producing new (parent_kind, child_kind) bigrams is added to the corpus
- Write a test that: a generated input with only known bigrams is NOT added
- Write a test that: without --diversity-guided, no inputs are added regardless
- Expected failure: no diversity tracking
- If it passes unexpectedly: diversity tracking already implemented

#### GREEN
- Track a set of seen (parent_kind, child_kind) bigrams (hash of the bigram set per tree)
- When a generated input produces novel bigrams, add it to the corpus and incrementally update the node pool

#### REFACTOR
- Ensure pool update is incremental (add new file, don't reparse everything)

---

### Task 4.7: Session persistence with full PRNG state

**Requirement:** Session Persistence

#### RED
- Write a test that: runs fuzzer for N iterations, stops, restarts, and it resumes (same PRNG sequence if corpus unchanged)
- Write a test that: --fresh discards prior state
- Write a test that: default state directory is `.bonsai-fuzz/`
- Write a test that: state file includes version number and checksum
- Write a test that: corrupted state file triggers warning and fresh start (not crash)
- Write a test that: incompatible version triggers warning and fresh start
- Expected failure: no persistence implementation
- If it passes unexpectedly: persistence already implemented

#### GREEN
- Serialize fuzzer state: corpus index, node pool cache, findings hashes, execution stats, full `ChaCha8Rng` state (not just seed)
- Use serde + bincode with a version header and checksum
- On startup, validate version and checksum; warn and start fresh on failure

#### REFACTOR
- Handle corpus-changed-between-sessions: rebuild pool, keep PRNG state, document that sequence diverges

---

### Task 4.8: Auto-reduction of findings with timeout

**Requirement:** Auto-Reduction of Findings

#### RED
- Write a test that: when a 100-byte interesting input is found, auto-reduction produces a smaller output that still triggers the same interest criteria
- Write a test that: auto-reduction respects its timeout (min(60s, 10x target timeout))
- Write a test that: FuzzTarget is passed directly to the reducer as InterestingnessTest (no type bridge)
- Expected failure: no auto-reduction wiring
- If it passes unexpectedly: auto-reduction already wired up

#### GREEN
- When a finding is detected, invoke the reducer with the same FuzzTarget and a bounded timeout
- Save the reduced version (or best-so-far if timeout hit)
- Log reduction ratio in stats

#### REFACTOR
- Consider running auto-reduction in a background thread to not block fuzzing

---

### Task 4.9: Fuzzer progress reporting

**Requirement:** Progress Reporting (fuzzer)

#### RED
- Write a test that: runs fuzzer and captures stderr, verifying it contains execs/sec and findings count
- Write a test that: --quiet suppresses progress
- Expected failure: no progress reporting
- If it passes unexpectedly: progress already implemented

#### GREEN
- Report to stderr: executions/sec, total executions, unique findings, corpus size, splice rejection rate
- Respect --quiet and --verbose
- Rate-limit updates to ~1/sec

#### REFACTOR
- Share progress reporting infrastructure with reducer if patterns are similar

---

### Task 4.10: Fuzzer integration tests

**Requirement:** All fuzzer requirements

#### RED
- Write integration tests:
  - Fuzz a deliberately buggy Python script processor with a corpus of valid Python files
  - Verify findings are saved, deduplicated, and auto-reduced
  - Verify session persistence (stop and resume)
  - Verify diversity-guided mode adds novel inputs to corpus
  - Verify execution bounds (--max-execs)

#### GREEN
- Create test fixtures: corpus directory, buggy target script
- Run end-to-end fuzzing for a bounded number of iterations

#### REFACTOR
- Ensure tests are deterministic (fixed PRNG seed, --jobs 1)
- Extract fixture setup into helpers shared with reducer tests

---

## 5. CLI (bonsai-cli)

### Task 5.1: Clap setup

**Requirement:** All CLI requirements

#### RED
- Write a test that: `bonsai --help` succeeds and lists reduce, fuzz, languages subcommands
- Write a test that: `bonsai reduce --help` shows --test, --lang, --output, --jobs, --max-tests, --max-time, --test-timeout, --strict, --quiet, --verbose
- Write a test that: `bonsai fuzz --help` shows --corpus, --lang, --mutations, --diversity-guided, --state-dir, --fresh, --max-execs, --max-time, --test-timeout, --input-mode, --interesting-exit, --interesting-stderr, --quiet, --verbose
- Expected failure: no CLI binary
- If it passes unexpectedly: CLI already configured

#### GREEN
- Set up clap derive with three subcommands: Reduce, Fuzz, Languages
- Define all flags and arguments per the spec

#### REFACTOR
- Ensure help text is clear and includes examples
- Group related flags (output, verbosity, limits)

---

### Task 5.2: `bonsai reduce` command

**Requirement:** All reducer requirements

#### RED
- Write an e2e test that: `bonsai reduce --test "grep hello" input.py` produces a smaller valid Python file on stdout
- Write an e2e test that: language is auto-detected from .py extension
- Write an e2e test that: `--lang` overrides auto-detection (even if they conflict)
- Write an e2e test that: missing extension and no --lang produces a clear error listing supported languages
- Write an e2e test that: `--output out.py` writes to file instead of stdout
- Expected failure: no reduce command wiring
- If it passes unexpectedly: reduce command already implemented

#### GREEN
- Wire clap args to `bonsai_reduce` library: parse input, create ShellTest, configure parallelism and bounds, run reduction, write output

#### REFACTOR
- Ensure error messages are user-friendly (file not found, unknown language, test command failed)

---

### Task 5.3: `bonsai fuzz` command

**Requirement:** All fuzzer requirements

#### RED
- Write an e2e test that: `bonsai fuzz --corpus ./seeds/ -- ./target @@` runs for bounded iterations and produces output
- Write an e2e test that: `--state-dir` creates state files and `--fresh` clears them
- Write an e2e test that: `--max-execs 10` stops after 10 executions
- Expected failure: no fuzz command wiring
- If it passes unexpectedly: fuzz command already implemented

#### GREEN
- Wire clap args to `bonsai_fuzz` library: parse corpus, create FuzzTarget, configure options, run fuzzing loop
- Install SIGINT handler for graceful shutdown (save state before exit)

#### REFACTOR
- Ensure graceful shutdown kills child processes cleanly

---

### Task 5.4: `bonsai languages` command

**Requirement:** Grammar Registry — List supported languages

#### RED
- Write an e2e test that: `bonsai languages` lists python, javascript, rust with their extensions
- Expected failure: no languages command
- If it passes unexpectedly: command already implemented

#### GREEN
- Call `list_languages()` from the generated registry module and format output

#### REFACTOR
- Sort alphabetically, align columns

---

### Task 5.5: End-to-end CLI tests

**Requirement:** All requirements

#### RED
- Write comprehensive e2e tests covering:
  - Reduce with various languages and test commands
  - Fuzz with various corpus sizes and interest criteria
  - Error cases: missing input file, invalid language, test command not found
  - Flag combinations: --quiet + --output, --jobs + --verbose, --max-tests + --max-time
  - Signal handling: SIGINT during reduce and fuzz

#### GREEN
- Use assert_cmd crate for CLI testing
- Create minimal test fixtures

#### REFACTOR
- Deduplicate test setup
- Ensure all tests clean up temp files and state directories
