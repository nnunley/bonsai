# Bonsai

Syntax-guided test case reducer and grammar-based fuzzer using [tree-sitter](https://tree-sitter.github.io/) grammars.

Bonsai implements the [Perses algorithm](https://doi.org/10.1109/ICSE.2018.00046) (Sun et al., ICSE 2018) adapted for tree-sitter, producing smaller reduced outputs than traditional delta debugging while guaranteeing syntactic validity at every step.

## Quick Start

```bash
# Reduce a Python file, keeping only what's needed to trigger "x = 1"
bonsai reduce --test "grep -q 'x = 1'" input.py

# Reduce with a custom test script
bonsai reduce --test "./check.sh" input.js

# Write output to a file instead of stdout
bonsai reduce --test "./check.sh" --output reduced.py input.py

# Limit reduction time
bonsai reduce --test "./check.sh" --max-time 5m --max-tests 1000 input.py

# List supported languages
bonsai languages
```

## How It Works

### Reducer

Bonsai parses your input file into a concrete syntax tree using tree-sitter, then systematically tries to remove or simplify subtrees while preserving an "interesting" property (defined by your test script).

The algorithm processes nodes largest-first via a priority queue. For each node, it generates candidate replacements:

- **Delete** — remove the node entirely
- **Unwrap** — replace a node with one of its type-compatible children
- **Unify identifiers** — rename bindings to canonical short forms (requires `locals.scm`)
- **Dead definition removal** — delete definitions with no references (requires `locals.scm`)

Every candidate is validated by reparsing with tree-sitter — only syntactically valid reductions are tested for interestingness. This avoids wasting test invocations on broken code.

**Key properties:**
- All intermediate results are syntactically valid
- Largest subtrees are tried first (maximum reduction per test)
- Test results are cached (24-62% of calls are typically duplicates)
- Handles inputs with pre-existing parse errors (tracks initial errors, only rejects new ones)

### Interestingness Test

The test is any shell command that exits 0 when the input is "interesting" (still triggers the bug). Bonsai writes each candidate to a temp file and passes the path as an argument:

```bash
# Your test script receives the candidate file path as $1
#!/bin/bash
my-compiler "$1" 2>&1 | grep -q "internal error"
```

### Grammar Support

Bonsai uses tree-sitter grammars vendored as git submodules. Currently supported:

| Language   | Extensions        |
|------------|-------------------|
| Python     | .py, .pyi         |
| JavaScript | .js, .mjs, .cjs   |
| Rust       | .rs               |

Adding a new language requires adding a tree-sitter grammar submodule and an entry in `grammars.toml`.

## Installation

```bash
# From source
git clone --recurse-submodules https://github.com/nnunley/bonsai.git
cd bonsai
cargo install --path crates/bonsai-cli
```

## CLI Reference

### `bonsai reduce`

```
bonsai reduce [OPTIONS] --test <TEST> <INPUT>

Arguments:
  <INPUT>  Input file to reduce

Options:
  -t, --test <TEST>                  Shell command (exit 0 = interesting)
  -l, --lang <LANG>                  Language (auto-detected from extension)
  -o, --output <OUTPUT>              Write to file instead of stdout
  -j, --jobs <JOBS>                  Parallel test workers [default: 1]
      --max-tests <N>                Stop after N test invocations [default: unlimited]
      --max-time <DURATION>          Stop after duration (e.g., "30m", "1h")
      --test-timeout <DURATION>      Per-test timeout [default: 30s]
      --strict                       Reject any parse errors (even pre-existing)
  -q, --quiet                        Suppress progress output
  -v, --verbose                      Show per-candidate detail
```

### `bonsai languages`

Lists all supported languages and their file extensions.

## Architecture

Bonsai is a Rust workspace with four crates:

- **bonsai-core** — tree-sitter parsing, node type compatibility ([SupertypeProvider](openspec/changes/add-bonsai-core/design.md#supertypeprovider-trait)), transforms, validity checking
- **bonsai-reduce** — Perses-style priority queue reducer with caching and parallel testing
- **bonsai-fuzz** — grammar-guided test case generation *(in progress)*
- **bonsai-cli** — unified CLI

### Node Type Compatibility

The reducer needs to know what can legally replace a given node. Bonsai uses a layered approach:

1. **tree-sitter's supertype/subtype system** — grammar-author-defined type hierarchies
2. **`supertypes.scm` query files** — community-contributed compatibility annotations
3. **Fallback** — Delete and Unwrap transforms work without any type information

Reparse-and-check-for-errors is always the definitive validity gate, regardless of what the compatibility layer says.

### Scope Analysis

When a grammar ships a `locals.scm` file (tree-sitter's scope/definition/reference queries), bonsai can perform scope-aware transforms like identifier unification and dead definition removal.

## Research Basis

| Paper | What it contributes |
|-------|-------------------|
| [Perses](https://doi.org/10.1109/ICSE.2018.00046) (Sun et al., ICSE 2018) | Priority-queue reduction, syntactic validity guarantee |
| [Vulcan](https://doi.org/10.1145/3586049) (Xu et al., OOPSLA 2023) | Transforms to escape 1-minimality (future work) |
| [HDD](https://doi.org/10.1145/1134285.1134307) (Misherghi & Su, ICSE 2006) | Original hierarchical delta debugging |
| [T-Rec](https://doi.org/10.1145/3690631) (Xu et al., TOSEM 2024) | Token-level reduction (future work) |

## Development

```bash
make all        # lint + format + test
make test       # cargo test
make lint       # cargo clippy with autofix
make format     # cargo fmt
make cover      # cargo tarpaulin

# Task tracking
git issue ready             # what to work on next
git issue topo              # full execution order
git issue deps --dot        # graphviz dependency graph
```

See [AGENTS.md](AGENTS.md) for development workflow and [openspec/](openspec/changes/add-bonsai-core/) for the full spec.

## License

MIT
