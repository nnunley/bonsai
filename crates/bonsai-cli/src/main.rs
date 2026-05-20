use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Encapsulates interrupt flag creation and signal handler registration.
struct InterruptFlag {
    flag: Arc<AtomicBool>,
}

impl InterruptFlag {
    fn new() -> Result<Self, ctrlc::Error> {
        let flag = Arc::new(AtomicBool::new(false));
        let clone = flag.clone();
        ctrlc::set_handler(move || {
            clone.store(true, Ordering::Relaxed);
        })?;
        Ok(Self { flag })
    }

    fn as_atomic(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }
}

#[derive(Parser)]
#[command(
    name = "bonsai",
    about = "Tree-sitter based test case reducer and fuzzer"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Reduce a test case while preserving an interesting property
    Reduce(ReduceArgs),

    /// List supported languages and their file extensions
    Languages,
}

#[derive(Args)]
struct ReduceArgs {
    /// Shell command for the interestingness test (exit 0 = interesting)
    #[arg(short, long)]
    test: String,

    /// Language name (auto-detected from extension if not specified)
    #[arg(short, long)]
    lang: Option<String>,

    /// Write output to file instead of stdout
    #[arg(short, long)]
    output: Option<String>,

    /// Number of parallel test workers (1 = sequential/deterministic)
    #[arg(short, long, default_value = "1")]
    jobs: usize,

    /// Maximum number of interestingness test invocations (0 = unlimited)
    #[arg(long, default_value = "0")]
    max_tests: usize,

    /// Maximum wall-clock time (e.g., "30m", "1h"). 0 = unlimited
    #[arg(long)]
    max_time: Option<String>,

    /// Per-test timeout (e.g., "10s", "1m")
    #[arg(long, default_value = "30s")]
    test_timeout: String,

    /// Reject any ERROR/MISSING nodes (even pre-existing)
    #[arg(long)]
    strict: bool,

    /// Abort after N consecutive test errors (0 = unlimited)
    #[arg(long, default_value = "3")]
    max_test_errors: usize,

    /// Suppress progress output
    #[arg(short, long)]
    quiet: bool,

    /// Show per-candidate detail
    #[arg(short, long)]
    verbose: bool,

    /// Input file to reduce
    input: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Reduce(args) => {
            cmd_reduce(args);
        }
        Commands::Languages => {
            cmd_languages();
        }
    }
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s == "0" {
        return Ok(Duration::ZERO);
    }
    let (num_str, unit) = if let Some(n) = s.strip_suffix("ms") {
        (n, "ms")
    } else if let Some(n) = s.strip_suffix('s') {
        (n, "s")
    } else if let Some(n) = s.strip_suffix('m') {
        (n, "m")
    } else if let Some(n) = s.strip_suffix('h') {
        (n, "h")
    } else {
        // Default to seconds
        (s, "s")
    };

    let num: f64 = num_str
        .parse()
        .map_err(|_| format!("invalid duration: {}", s))?;
    if num < 0.0 || !num.is_finite() {
        return Err(format!("duration must be non-negative: {}", s));
    }
    match unit {
        "ms" => Ok(Duration::from_secs_f64(num / 1000.0)),
        "s" => Ok(Duration::from_secs_f64(num)),
        "m" => Ok(Duration::from_secs_f64(num * 60.0)),
        "h" => Ok(Duration::from_secs_f64(num * 3600.0)),
        _ => Err(format!("invalid duration unit: {}", s)),
    }
}

fn cmd_reduce(args: ReduceArgs) {
    let ReduceArgs {
        test: test_cmd,
        lang,
        output,
        jobs,
        max_tests,
        max_time,
        test_timeout,
        strict,
        max_test_errors,
        quiet,
        verbose,
        input,
    } = args;
    // Read input file
    let source = match std::fs::read(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bonsai: error reading {}: {}", input.display(), e);
            process::exit(1);
        }
    };

    // Determine language
    let (lang_name, language) = if let Some(name) = &lang {
        match bonsai_core::languages::get_language(name) {
            Some(l) => (name.clone(), l),
            None => {
                eprintln!("bonsai: unknown language '{}'. Supported languages:", name);
                print_languages();
                process::exit(1);
            }
        }
    } else {
        // Auto-detect from extension
        let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("");
        match bonsai_core::languages::get_language_by_extension(ext) {
            Some((name, l)) => (name.to_string(), l),
            None => {
                eprintln!(
                    "bonsai: cannot detect language from extension '.{}'. Use --lang or supported extensions:",
                    ext
                );
                print_languages();
                process::exit(1);
            }
        }
    };

    // Parse test timeout
    let timeout = match parse_duration(&test_timeout) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("bonsai: invalid --test-timeout: {}", e);
            process::exit(1);
        }
    };

    // Parse max time
    let max_time_dur = match max_time.as_deref() {
        Some(s) => match parse_duration(s) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("bonsai: invalid --max-time: {}", e);
                process::exit(1);
            }
        },
        None => Duration::ZERO,
    };

    // Parse test command
    let test_args: Vec<String> = shell_words::split(&test_cmd).unwrap_or_else(|e| {
        eprintln!("bonsai: invalid --test command: {}", e);
        process::exit(1);
    });

    // Set up interestingness test
    let shell_test = bonsai_reduce::ShellTest::new(test_args, timeout).unwrap_or_else(|e| {
        eprintln!("bonsai: {}", e);
        process::exit(1);
    });

    // Set up interrupt handler
    let interrupt = InterruptFlag::new().unwrap_or_else(|e| {
        eprintln!(
            "bonsai: warning: failed to register interrupt handler: {}",
            e
        );
        // Fall back to an unregistered flag — Ctrl-C won't gracefully stop reduction
        InterruptFlag {
            flag: Arc::new(AtomicBool::new(false)),
        }
    });

    // Discover per-project bonsai.toml. The config lives alongside
    // the project tree and extends bonsai's built-in scope-analysis
    // patterns + supertype declarations with project-specific
    // knowledge (e.g., custom defining-form macros).
    //
    // Walking starts from the input file's directory and walks upward
    // until a bonsai.toml is found or the filesystem root is reached.
    let project_config = {
        let start = input
            .canonicalize()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        match bonsai_core::project::discover_and_load(&start) {
            Ok(Some((path, config))) => {
                eprintln!("bonsai: using project config {}", path.display());
                Some(config)
            }
            Ok(None) => None,
            Err(e) => {
                eprintln!("bonsai: warning: {}", e);
                None
            }
        }
    };

    // Extract the language-specific section of the project config (if any).
    let project_lang_config = project_config
        .as_ref()
        .and_then(|c| c.languages.get(&lang_name));

    // Set up provider chain (resolved in this order, results merged):
    //   1. LanguageApiProvider     — runtime tree-sitter Language::supertypes()
    //   2. NodeTypesProvider       — parsed from node-types.json at build
    //   3. ConfigSupertypeProvider — hand-declared in grammars.toml
    //   4. ProjectSupertypeProvider — declared in bonsai.toml (per-project)
    //
    // Providers 3 & 4 are escape hatches for grammars (like
    // tree-sitter-clojure) where the first two return nothing, and for
    // projects that know context the grammar can't see.
    let api_provider = bonsai_core::supertype::LanguageApiProvider::new(&language);
    let ntp_provider = bonsai_core::supertype::NodeTypesProvider::new(&language, &lang_name);
    let cfg_provider = bonsai_core::supertype::ConfigSupertypeProvider::new(&language, &lang_name);
    let project_supertype_mappings: Vec<(String, Vec<String>)> = project_lang_config
        .map(|c| {
            c.supertypes
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    let project_provider = bonsai_core::supertype::ProjectSupertypeProvider::new(
        &language,
        &project_supertype_mappings,
    );
    let has_supertypes = api_provider.has_supertypes()
        || ntp_provider.has_supertypes()
        || cfg_provider.has_supertypes()
        || project_provider.has_supertypes();
    let provider = bonsai_core::supertype::ChainProvider::new(vec![
        Box::new(api_provider),
        Box::new(ntp_provider),
        Box::new(cfg_provider),
        Box::new(project_provider),
    ]);
    if !has_supertypes {
        eprintln!(
            "bonsai: note: grammar has no supertypes — only Delete transform will be effective. \
             Unwrap requires type compatibility information."
        );
    }

    // Build transforms — include DeadDefinitionTransform if locals.scm is available
    let mut transforms: Vec<Box<dyn bonsai_core::transform::Transform>> = vec![
        Box::new(bonsai_core::transforms::delete::DeleteTransform),
        Box::new(bonsai_core::transforms::unwrap::UnwrapTransform),
    ];

    // Check if this language has locals.scm for scope-aware transforms.
    // If the project config declares extra defining-form rules, merge
    // them into the locals.scm before scope analysis runs. This is
    // what lets a project teach bonsai about its own macros (e.g.
    // `defcreature`, `defcomponent`) without forking the queries.
    let lang_info = bonsai_core::languages::list_languages()
        .iter()
        .find(|l| l.name == lang_name);
    if let Some(info) = lang_info
        && let Some(base_locals) = info.locals_scm
    {
        // Compose locals.scm: base + project extensions (if any).
        let project_rules: &[bonsai_core::project::DefiningFormRule] = project_lang_config
            .map(|c| c.defining_forms.as_slice())
            .unwrap_or(&[]);
        let merged_locals_owned =
            bonsai_core::project::merge_locals_scm(base_locals, project_rules);
        // We need a &str with a lifetime tied to either `info` (the
        // embedded base) or the new owned String. Bind once for use
        // below.
        let locals_content: &str = match &merged_locals_owned {
            Some(s) => s.as_str(),
            None => base_locals,
        };

        if !project_rules.is_empty() {
            eprintln!(
                "bonsai: bonsai.toml adds {} defining-form rule(s) to locals.scm",
                project_rules.len()
            );
        }

        if let Some(tree) = bonsai_core::parse::parse(&source, &language)
            && let Some(analysis) = bonsai_core::scope::ScopeAnalysis::from_tree(
                &tree,
                &source,
                &language,
                locals_content,
            )
        {
            let dead_defs = analysis.unreferenced_definitions();
            if !dead_defs.is_empty() {
                eprintln!(
                    "bonsai: found {} unreferenced definitions via scope analysis",
                    dead_defs.len()
                );
            }
            transforms.push(Box::new(
                bonsai_core::transforms::dead_definition::DeadDefinitionTransform::from_analysis(
                    &analysis,
                    &tree,
                    locals_content,
                ),
            ));
        }
    }

    // Set up config
    let config = bonsai_reduce::reducer::ReducerConfig {
        language: language.clone(),
        transforms,
        provider: Box::new(provider),
        max_tests,
        max_time: max_time_dur,
        jobs,
        strict,
        max_test_errors,
        interrupted: interrupt.as_atomic(),
    };

    // Set up progress reporter
    let verbosity = if quiet {
        bonsai_reduce::progress::Verbosity::Quiet
    } else if verbose {
        bonsai_reduce::progress::Verbosity::Verbose
    } else {
        bonsai_reduce::progress::Verbosity::Normal
    };
    let reporter = bonsai_reduce::progress::ProgressReporter::new(verbosity, source.len());

    // Run reduction
    let result = bonsai_reduce::reducer::reduce(&source, &shell_test, config, Some(&reporter));

    // Report final summary
    if verbosity != bonsai_reduce::progress::Verbosity::Quiet {
        let percentage = if !source.is_empty() {
            100.0 * (1.0 - result.source.len() as f64 / source.len() as f64)
        } else {
            0.0
        };
        eprintln!(
            "bonsai: done. {} -> {} bytes ({:.1}% reduced) in {:.1}s | tests: {} | reductions: {} | cache: {:.1}%",
            source.len(),
            result.source.len(),
            percentage,
            result.elapsed.as_secs_f64(),
            result.tests_run,
            result.reductions,
            result.cache_hit_rate * 100.0,
        );
    }

    // Write output
    let target = match output {
        Some(path) => bonsai_reduce::OutputTarget::File(path),
        None => bonsai_reduce::OutputTarget::Stdout,
    };

    if let Err(e) = bonsai_reduce::write_output(&result.source, &target) {
        eprintln!("bonsai: error writing output: {}", e);
        process::exit(1);
    }
}

fn cmd_languages() {
    println!("Supported languages:");
    println!();
    let langs = bonsai_core::languages::list_languages();
    let mut sorted: Vec<&bonsai_core::languages::LanguageInfo> = langs.iter().collect();
    sorted.sort_by_key(|l| l.name);
    for lang in &sorted {
        println!("  {:<15} {}", lang.name, lang.extensions.join(", "));
    }
}

fn print_languages() {
    for lang in bonsai_core::languages::list_languages() {
        eprintln!("  {:<15} {}", lang.name, lang.extensions.join(", "));
    }
}
