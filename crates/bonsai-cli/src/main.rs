use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
    Reduce {
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

        /// Maximum consecutive test errors before aborting
        #[arg(long, default_value = "3")]
        max_test_errors: usize,

        /// Reject any ERROR/MISSING nodes (even pre-existing)
        #[arg(long)]
        strict: bool,

        /// Suppress progress output
        #[arg(short, long)]
        quiet: bool,

        /// Show per-candidate detail
        #[arg(short, long)]
        verbose: bool,

        /// Input file to reduce
        input: PathBuf,
    },

    /// List supported languages and their file extensions
    Languages,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Reduce {
            test,
            lang,
            output,
            jobs,
            max_tests,
            max_time,
            test_timeout,
            max_test_errors,
            strict,
            quiet,
            verbose,
            input,
        } => {
            cmd_reduce(
                test,
                lang,
                output,
                jobs,
                max_tests,
                max_time,
                test_timeout,
                max_test_errors,
                strict,
                quiet,
                verbose,
                input,
            );
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
    let (num_str, unit) = if s.ends_with("ms") {
        (&s[..s.len() - 2], "ms")
    } else if s.ends_with('s') {
        (&s[..s.len() - 1], "s")
    } else if s.ends_with('m') {
        (&s[..s.len() - 1], "m")
    } else if s.ends_with('h') {
        (&s[..s.len() - 1], "h")
    } else {
        // Default to seconds
        (s, "s")
    };

    let num: f64 = num_str
        .parse()
        .map_err(|_| format!("invalid duration: {}", s))?;
    match unit {
        "ms" => Ok(Duration::from_millis(num as u64)),
        "s" => Ok(Duration::from_secs_f64(num)),
        "m" => Ok(Duration::from_secs_f64(num * 60.0)),
        "h" => Ok(Duration::from_secs_f64(num * 3600.0)),
        _ => Err(format!("invalid duration unit: {}", s)),
    }
}

fn cmd_reduce(
    test_cmd: String,
    lang: Option<String>,
    output: Option<String>,
    jobs: usize,
    max_tests: usize,
    max_time: Option<String>,
    test_timeout: String,
    max_test_errors: usize,
    strict: bool,
    quiet: bool,
    verbose: bool,
    input: PathBuf,
) {
    // Read input file
    let source = match std::fs::read(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bonsai: error reading {}: {}", input.display(), e);
            process::exit(1);
        }
    };

    // Determine language
    let language = if let Some(name) = &lang {
        match bonsai_core::languages::get_language(name) {
            Some(l) => l,
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
            Some((_name, l)) => l,
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

    // Set up provider
    let provider = bonsai_core::supertype::LanguageApiProvider::new(&language);

    // Set up config
    let config = bonsai_reduce::reducer::ReducerConfig {
        language: language.clone(),
        transforms: vec![
            Box::new(bonsai_core::transforms::delete::DeleteTransform),
            Box::new(bonsai_core::transforms::unwrap::UnwrapTransform),
        ],
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
