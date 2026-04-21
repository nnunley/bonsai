# Bonsai Usage Guide

This guide walks through using Bonsai to reduce a buggy source file to the smallest version that still triggers the bug.

## What is Test Case Reduction?

When you find a bug in a compiler, interpreter, or other language tool, the file that triggers it is often large and full of code unrelated to the bug. **Test case reduction** systematically removes code until only the essential trigger remains.

Bonsai does this by:
1. Parsing your file into a syntax tree (using tree-sitter)
2. Trying to delete or simplify subtrees
3. Checking each candidate against your "interestingness test"
4. Keeping only changes where the test still passes (the bug still triggers)

The result is a minimal file that reproduces the bug — ideal for filing issues or debugging.

## Writing an Interestingness Test

The interestingness test is a shell command that exits `0` when the input "still has the bug" and non-zero otherwise. Bonsai writes each candidate to a temporary file and passes the path as the last argument.

### Basic pattern

```bash
#!/bin/bash
# check.sh — exits 0 if the bug reproduces
my-compiler "$1" 2>&1 | grep -q "internal error"
```

Make it executable: `chmod +x check.sh`

### Tips for writing good tests

- **Be specific.** Match the exact error message, not just "error". Otherwise the reducer may find a shorter file that triggers a *different* error.
- **Check for the right failure mode.** If the bug is a crash (segfault), check the exit code:
  ```bash
  #!/bin/bash
  my-compiler "$1" 2>/dev/null
  test $? -eq 139  # SIGSEGV on Linux
  ```
- **Keep it fast.** The test runs hundreds or thousands of times. If your tool is slow, set a timeout (`--test-timeout 5s`) so hung processes don't stall reduction.
- **Test the test first.** Run it manually on your original file to confirm it exits 0:
  ```bash
  ./check.sh original.py && echo "INTERESTING" || echo "NOT INTERESTING"
  ```

## Step-by-Step Walkthrough

### Example: Reducing a Python file that crashes a linter

Suppose `big_module.py` (500 lines) triggers an internal error in `my-linter`:

```
$ my-linter big_module.py
my-linter: internal error: unexpected NoneType in resolve_scope
```

**Step 1: Write the interestingness test**

```bash
#!/bin/bash
# check_linter.sh
my-linter "$1" 2>&1 | grep -q "unexpected NoneType in resolve_scope"
```

**Step 2: Verify the test works**

```bash
$ ./check_linter.sh big_module.py && echo "PASS" || echo "FAIL"
PASS
```

**Step 3: Run Bonsai**

```bash
$ bonsai reduce --test "./check_linter.sh" big_module.py > reduced.py
bonsai: 12458 -> 847 bytes (93.2% reduced) | tests: 342 | reductions: 28 | cache: 38.4%
bonsai: done in 45s — 12458 -> 127 bytes (99.0% reduced)
```

**Step 4: Inspect the result**

```bash
$ cat reduced.py
class A:
    def f(self):
        x = None
        x.y
```

**Step 5: Verify**

```bash
$ my-linter reduced.py
my-linter: internal error: unexpected NoneType in resolve_scope
```

The 500-line file is now 4 lines that pinpoint the exact trigger.

### Example: Reducing a JavaScript file that crashes Node.js

```bash
# Interestingness test: Node.js segfaults (exit code 139 on Linux, 134 on macOS abort)
#!/bin/bash
node "$1" 2>/dev/null; test $? -gt 128

# Reduce with parallel testing for speed
bonsai reduce --test "./check_crash.sh" --jobs 4 big_app.js -o reduced.js
```

### Example: Using inline test commands

For simple cases, skip the script file:

```bash
# Keep anything that still contains "import foo"
bonsai reduce --test "grep -q 'import foo'" input.py

# Keep anything where python raises SyntaxError (testing a parser)
bonsai reduce --test "python3 -c 'import ast; ast.parse(open(\"\$1\").read())' 2>&1 | grep -q SyntaxError" input.py
```

## Command Reference

```
bonsai reduce [OPTIONS] --test <TEST> <INPUT>
```

| Option | Description | Default |
|--------|-------------|---------|
| `-t, --test <CMD>` | Shell command (exit 0 = interesting) | *required* |
| `-l, --lang <LANG>` | Language name | auto-detected from extension |
| `-o, --output <FILE>` | Write result to file | stdout |
| `-j, --jobs <N>` | Parallel test workers | 1 |
| `--max-tests <N>` | Stop after N tests | unlimited |
| `--max-time <DUR>` | Wall-clock limit (e.g., `5m`, `1h`) | unlimited |
| `--test-timeout <DUR>` | Per-test timeout | 30s |
| `--strict` | Reject any parse errors (even pre-existing) | off |
| `-q, --quiet` | Suppress progress output | off |
| `-v, --verbose` | Show per-candidate detail | off |

### Parallel reduction

Use `-j` to run interestingness tests in parallel:

```bash
bonsai reduce --test "./check.sh" -j 8 input.py -o reduced.py
```

With `-j 1` (default), reduction is deterministic. With `-j > 1`, candidates are tested concurrently and the first interesting result wins, so results may vary between runs.

### Time and test limits

```bash
# Stop after 5 minutes regardless of progress
bonsai reduce --test "./check.sh" --max-time 5m input.py

# Stop after 500 test invocations
bonsai reduce --test "./check.sh" --max-tests 500 input.py

# Kill individual tests that take too long
bonsai reduce --test "./slow_check.sh" --test-timeout 10s input.py
```

### Strict mode

By default, Bonsai handles files with pre-existing parse errors gracefully — it tracks the initial errors and only rejects candidates that introduce *new* errors. This is useful for reducing files that are already syntactically broken.

With `--strict`, any parse error in a candidate causes rejection:

```bash
bonsai reduce --test "./check.sh" --strict input.py
```

### Listing supported languages

```bash
$ bonsai languages
python    .py .pyi
javascript .js .mjs .cjs
rust      .rs
```

## Programmatic Usage (Library API)

You can use `bonsai-core` and `bonsai-reduce` as Rust libraries for custom reduction pipelines.

### Basic reduction

```rust
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use bonsai_core::supertype::LanguageApiProvider;
use bonsai_core::transforms::delete::DeleteTransform;
use bonsai_core::transforms::unwrap::UnwrapTransform;
use bonsai_reduce::reducer::{reduce, ReducerConfig};
use bonsai_reduce::{InterestingnessTest, TestResult};

// Define your interestingness test
struct ContainsPattern(Vec<u8>);

impl InterestingnessTest for ContainsPattern {
    fn test(&self, input: &[u8]) -> TestResult {
        if input.windows(self.0.len()).any(|w| w == self.0.as_slice()) {
            TestResult::Interesting
        } else {
            TestResult::NotInteresting
        }
    }
}

fn main() {
    let lang = bonsai_core::languages::get_language("python").unwrap();
    let source = std::fs::read("input.py").unwrap();

    let config = ReducerConfig {
        language: lang.clone(),
        transforms: vec![Box::new(DeleteTransform), Box::new(UnwrapTransform)],
        provider: Box::new(LanguageApiProvider::new(&lang)),
        max_tests: 0,       // unlimited
        max_time: Duration::from_secs(300), // 5 minute limit
        jobs: 4,
        strict: true,
        max_test_errors: 0, // unlimited
        interrupted: Arc::new(AtomicBool::new(false)),
    };

    let test = ContainsPattern(b"trigger_bug".to_vec());
    let result = reduce(&source, &test, config, None);

    println!("Reduced {} -> {} bytes ({:.1}% reduction)",
        source.len(), result.source.len(),
        100.0 * (1.0 - result.source.len() as f64 / source.len() as f64));
    println!("Tests run: {}, Reductions: {}", result.tests_run, result.reductions);

    std::fs::write("reduced.py", &result.source).unwrap();
}
```

### Using scope-aware transforms

When a grammar has `locals.scm` (e.g., JavaScript), you can enable dead definition removal:

```rust
use bonsai_core::scope::ScopeAnalysis;
use bonsai_core::transforms::dead_definition::DeadDefinitionTransform;

let lang = bonsai_core::languages::get_language("javascript").unwrap();
let info = bonsai_core::languages::list_languages()
    .into_iter()
    .find(|l| l.name == "javascript")
    .unwrap();

if let Some(locals_scm) = info.locals_scm {
    let source = b"function foo() { let unused = 1; let used = 2; return used; }";
    let tree = bonsai_core::parse::parse(source, &lang).unwrap();

    if let Some(analysis) = ScopeAnalysis::from_tree(&tree, source, &lang, locals_scm) {
        let transform = DeadDefinitionTransform::from_analysis(&analysis, &tree, locals_scm);
        // Add this transform to your ReducerConfig.transforms vec
    }
}
```

### Custom transforms

Implement the `Transform` trait to add domain-specific reduction strategies:

```rust
use bonsai_core::transform::Transform;
use bonsai_core::validity::Replacement;
use bonsai_core::supertype::SupertypeProvider;
use tree_sitter::{Node, Tree};

struct SimplifyStrings;

impl Transform for SimplifyStrings {
    fn candidates(
        &self, node: &Node, source: &[u8], _tree: &Tree,
        _provider: &dyn SupertypeProvider,
    ) -> Vec<Replacement> {
        // Replace string literals with empty strings
        if node.kind() == "string" {
            vec![Replacement {
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                new_bytes: b"\"\"".to_vec(),
            }]
        } else {
            vec![]
        }
    }
    fn name(&self) -> &str { "simplify_strings" }
}
```

## How the Algorithm Works

Bonsai implements the [Perses algorithm](https://doi.org/10.1109/ICSE.2018.00046):

1. **Parse** the input into a concrete syntax tree
2. **Build a priority queue** of all named nodes, ordered by size (token count)
3. **Pop the largest node** and generate candidate replacements:
   - **Delete** — remove the node entirely
   - **Unwrap** — replace with a type-compatible child
   - **Dead definition removal** — delete unreferenced definitions (scope-aware)
4. **Validate** each candidate by reparsing — reject if it introduces parse errors
5. **Test** valid candidates for interestingness (does the bug still trigger?)
6. **Accept** the first interesting candidate, rebuild the queue, repeat
7. **Stop** when the queue is exhausted or limits are reached

Key properties:
- Every intermediate result is syntactically valid (guaranteed by tree-sitter reparsing)
- Largest nodes are tried first, maximizing reduction per test invocation
- Test results are cached (typically 24-62% cache hit rate)
- The final output is re-verified to catch any hash collision in the cache

## Troubleshooting

### "initial input is not interesting"

Your test command doesn't return exit code 0 on the original file. Debug by running it manually:
```bash
./check.sh input.py; echo "exit code: $?"
```

### Reduction is slow

- Use `-j N` for parallel testing (try `-j` equal to your CPU count)
- Set `--test-timeout` to kill hung test processes
- Set `--max-time` to cap total wall-clock time
- Make your test script as fast as possible (avoid unnecessary I/O)

### Result is not minimal enough

- Ensure your test is specific (match exact error text, not generic patterns)
- Try running Bonsai again on the already-reduced output — multi-pass reduction can find further reductions
- Consider adding custom transforms for domain-specific simplifications

### Parse errors in the original file

By default, Bonsai handles files with pre-existing parse errors using lenient mode — it tracks the initial error set and only rejects candidates that introduce *new* errors. Use `--strict` to reject all errors.

### Language not detected

Specify the language explicitly with `--lang`:
```bash
bonsai reduce --test "./check.sh" --lang python input.txt
```
