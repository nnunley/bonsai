# Bonsai - Agent Instructions

## Project Overview

Bonsai is a tree-sitter-based syntax-guided test case reducer and grammar-based fuzzer, written in Rust. It implements the Perses algorithm (Sun et al., ICSE 2018) adapted for tree-sitter grammars, with a companion AST-splicing fuzzer.

## Specs and Design

All requirements, design decisions, and TDD task definitions live in the OpenSpec change:

```
openspec/changes/add-bonsai-core/
├── proposal.md          # Why this project exists, research basis
├── design.md            # Architecture, algorithms, technical decisions
├── tasks.md             # TDD tasks with RED/GREEN/REFACTOR steps
└── specs/
    ├── core/spec.md     # Parse tree, SupertypeProvider, transforms, validity
    ├── reducer/spec.md  # Priority queue, interestingness test, caching, output
    ├── fuzzer/spec.md   # AST splicing, target interface, dedup, persistence
    └── grammar-system/spec.md  # Submodules, registry, build system
```

**Read these before implementing anything.** The design.md has critical notes about tree-sitter API limitations (lookahead is a hint not an oracle, supertypes are grammar-dependent, node handles invalidate on reparse).

## Task Tracking

Tasks are tracked in `git-issue`. Key commands:

```bash
git issue topo              # Topological execution order
git issue ready             # What's ready to work on (deps resolved)
git issue show <id>         # Full issue details
git issue deps --dot        # Graphviz dependency graph
git issue start <id>        # Mark issue as in_progress
git issue close <id>        # Mark issue as done
git issue commit --fixes <id>  # Link commit to issue via git trailer
```

### Workflow

1. Run `git issue topo` or `git issue ready` to find the next task
2. Read the corresponding section in `openspec/changes/add-bonsai-core/tasks.md` for TDD steps
3. `git issue start <id>` before beginning work
4. Follow RED → GREEN → REFACTOR strictly
5. Commit with `git issue commit --fixes <id>` to link the commit
6. `git issue close <id>` when done

### Issue structure

Issues use two relationship types:
- **`depends_on` / `blocks`** — execution ordering. A task can't start until its dependencies are closed.
- **`parent_of`** — umbrella grouping. A parent epic auto-completes when all its children close. Parents are NOT work items — they're progress trackers.

Five umbrella epics group the concrete tasks:
- **Scaffolding** — workspace, submodules, registry, build system
- **Core Library** — parsing, SupertypeProvider, compatibility, transforms, validity
- **Reducer** — priority queue, reduction loop, interestingness test, caching, output
- **Fuzzer** — corpus, splicing, dedup, persistence, auto-reduction
- **CLI** — clap setup, reduce/fuzz/languages commands, e2e tests

Epic dependency chain: Scaffolding → Core → Reducer + Fuzzer → CLI

### Parallel work streams

After scaffolding completes, three independent streams can run concurrently:
1. **Core** (2.x): tree manipulation, SupertypeProvider, transforms, validity
2. **Shell execution** (3.3, 3.4): InterestingnessTest, ShellTest, caching — no core dependency
3. **Target execution** (4.3, 4.4): FuzzTarget, interest criteria — no core dependency

After core completes:
- **Reducer** (3.1, 3.2, 3.5-3.7): needs core + shell execution
- **Fuzzer** (4.1, 4.2, 4.5-4.10): needs core + target execution + reducer (for auto-reduction)

**CLI** (5.x) is last — needs both reducer and fuzzer.

## Build & Test

```bash
make all        # lint + format + test
make test       # cargo test
make lint       # cargo clippy with autofix
make format     # cargo fmt
make cover      # cargo tarpaulin
```

## Architecture

Rust workspace with four crates:
- `crates/bonsai-core/` — tree-sitter parsing, SupertypeProvider, transforms, validity checking. Has `build.rs` for grammar compilation.
- `crates/bonsai-reduce/` — Perses-style priority queue reducer with parallel testing
- `crates/bonsai-fuzz/` — corpus-based AST splicing fuzzer with positional node pool
- `crates/bonsai-cli/` — unified CLI (`bonsai reduce`, `bonsai fuzz`, `bonsai languages`)

Grammars are vendored as git submodules under `grammars/` and registered in `grammars.toml`.

## Key Design Decisions

1. **Reparse is the definitive validity gate.** Lookahead iterator is a best-effort hint only. Always check for ERROR and MISSING nodes after reparsing.
2. **SupertypeProvider is pluggable.** Three tiers: Language API → supertypes.scm query files → Delete/Unwrap fallback. Many grammars have no supertypes — this is expected.
3. **Node handles invalidate on reparse.** The priority queue stores (byte_range, kind_id, token_count) tuples, never Node handles. Queue is rebuilt from scratch after each accepted reduction.
4. **No shell interpolation.** All external commands use `Command::new` with args arrays.
5. **FuzzTarget implements InterestingnessTest.** No separate type bridge needed for auto-reduction.
