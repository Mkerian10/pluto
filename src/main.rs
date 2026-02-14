use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "plutoc", version, about = "The Pluto compiler")]
struct Cli {
    /// Path to stdlib root directory
    #[arg(long, global = true)]
    stdlib: Option<PathBuf>,

    /// Garbage collector backend: "marksweep" (default) or "noop"
    #[arg(long, global = true, default_value = "marksweep")]
    gc: String,

    #[command(subcommand)]
    command: Commands,
}

fn parse_gc_backend(s: &str) -> Result<plutoc::GcBackend, String> {
    match s {
        "marksweep" => Ok(plutoc::GcBackend::MarkSweep),
        "noop" => Ok(plutoc::GcBackend::Noop),
        other => Err(format!("unknown GC backend '{}'; expected 'marksweep' or 'noop'", other)),
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a .pluto source file to a native binary
    Compile {
        /// Source file path
        file: PathBuf,
        /// Output binary path
        #[arg(short, long, default_value = "a.out")]
        output: PathBuf,
    },
    /// Compile and run a .pluto source file
    Run {
        /// Source file path
        file: PathBuf,
        /// Enable code coverage instrumentation
        #[arg(long)]
        coverage: bool,
    },
    /// Run tests in a .pluto source file
    Test {
        /// Source file path
        file: PathBuf,
        /// Override seed for random test strategies (for reproducibility)
        #[arg(long)]
        seed: Option<u64>,
        /// Override number of iterations for random test strategies
        #[arg(long)]
        iterations: Option<u64>,
        /// Disable test caching, run all tests
        #[arg(long)]
        no_cache: bool,
        /// Enable code coverage instrumentation
        #[arg(long)]
        coverage: bool,
    },
    /// Analyze a .pt source file and emit a .pluto binary AST
    EmitAst {
        /// Source file path (.pt)
        file: PathBuf,
        /// Output binary path (.pluto)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Read a .pluto binary AST and emit human-readable .pt source
    GeneratePt {
        /// Binary AST file path (.pluto)
        file: PathBuf,
        /// Output text path (.pt). If omitted, prints to stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Fetch latest versions of all git dependencies
    Update {
        /// Directory to search for pluto.toml (defaults to current dir)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    /// Sync edits from a .pt text file back to a .pluto binary, preserving UUIDs
    Sync {
        /// .pt text file to sync from
        file: PathBuf,
        /// .pluto binary file to sync to (defaults to same name with .pluto extension)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Watch files and automatically recompile/rerun on changes
    Watch {
        #[command(subcommand)]
        command: WatchCommands,
    },
    /// Generate coverage reports from .pluto-coverage/ data
    Coverage {
        #[command(subcommand)]
        command: CoverageCommands,
    },
}

#[derive(Subcommand)]
enum CoverageCommands {
    /// Generate a coverage report
    Report {
        /// Output format: terminal (default), lcov, json, html
        #[arg(long, default_value = "terminal")]
        format: String,
        /// Coverage data directory (defaults to .pluto-coverage/)
        #[arg(long, default_value = ".pluto-coverage")]
        dir: PathBuf,
        /// Output file (defaults to stdout for lcov/json, stderr for terminal)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum WatchCommands {
    /// Watch and automatically re-run a Pluto program
    Run {
        /// The Pluto file to watch and run
        file: PathBuf,

        /// Don't clear terminal between runs
        #[arg(long)]
        no_clear: bool,
    },
    /// Watch and automatically re-run tests
    Test {
        /// The Pluto file to watch and test
        file: PathBuf,

        /// Don't clear terminal between runs
        #[arg(long)]
        no_clear: bool,

        /// Disable test caching, run all tests
        #[arg(long)]
        no_cache: bool,
    },
}

/// Get the appropriate filename to display in error messages.
/// For sibling file errors, returns the sibling file path instead of the entry file.
fn error_filename(err: &plutoc::diagnostics::CompileError) -> Option<String> {
    match err {
        plutoc::diagnostics::CompileError::SiblingFile { path, .. } => {
            Some(path.display().to_string())
        }
        _ => None,
    }
}

fn main() {
    let cli = Cli::parse();

    let stdlib = cli.stdlib.as_deref();
    let gc = match parse_gc_backend(&cli.gc) {
        Ok(gc) => gc,
        Err(msg) => {
            eprintln!("error: {msg}");
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Compile { file, output } => {
            // Check if this is a system file (contains a `system` declaration)
            match plutoc::detect_system_file(&file) {
                Ok(Some(_program)) => {
                    // System file: compile each member app to its own binary
                    match plutoc::compile_system_file_with_stdlib(&file, &output, stdlib) {
                        Ok(members) => {
                            for (name, path) in &members {
                                eprintln!("  compiled {} \u{2192} {}", name, path.display());
                            }
                            eprintln!("system: {} member(s) compiled", members.len());
                        }
                        Err(err) => {
                            let filename = error_filename(&err)
                                .unwrap_or_else(|| file.to_string_lossy().to_string());
                            eprintln!("error [{}]: {err}", filename);
                            std::process::exit(1);
                        }
                    }
                }
                Ok(None) => {
                    // Regular file: compile to a single binary
                    if let Err(err) = plutoc::compile_file_with_options(&file, &output, stdlib, gc) {
                        let filename = error_filename(&err)
                            .unwrap_or_else(|| file.to_string_lossy().to_string());
                        eprintln!("error [{}]: {err}", filename);
                        std::process::exit(1);
                    }
                }
                Err(err) => {
                    let filename = error_filename(&err)
                        .unwrap_or_else(|| file.to_string_lossy().to_string());
                    eprintln!("error [{}]: {err}", filename);
                    std::process::exit(1);
                }
            }
        }
        Commands::Run { file, coverage } => {
            // Reject system files — they produce multiple binaries
            match plutoc::detect_system_file(&file) {
                Ok(Some(_)) => {
                    eprintln!("error: cannot run a system file directly; use `plutoc compile` to produce individual binaries");
                    std::process::exit(1);
                }
                Ok(None) => {}
                Err(err) => {
                    let filename = error_filename(&err)
                        .unwrap_or_else(|| file.to_string_lossy().to_string());
                    eprintln!("error [{}]: {err}", filename);
                    std::process::exit(1);
                }
            }

            let tmp = std::env::temp_dir().join("pluto_run");

            let coverage_map = if coverage {
                match plutoc::compile_file_with_coverage(&file, &tmp, stdlib) {
                    Ok(map) => Some(map),
                    Err(err) => {
                        let filename = error_filename(&err)
                            .unwrap_or_else(|| file.to_string_lossy().to_string());
                        eprintln!("error [{}]: {err}", filename);
                        std::process::exit(1);
                    }
                }
            } else {
                if let Err(err) = plutoc::compile_file_with_options(&file, &tmp, stdlib, gc) {
                    let filename = error_filename(&err)
                        .unwrap_or_else(|| file.to_string_lossy().to_string());
                    eprintln!("error [{}]: {err}", filename);
                    std::process::exit(1);
                }
                None
            };

            // Write coverage map before running
            if let Some(ref map) = coverage_map {
                let cov_dir = std::path::Path::new(".pluto-coverage");
                let _ = std::fs::create_dir_all(cov_dir);
                if let Err(e) = map.write_json(&cov_dir.join("coverage-map.json")) {
                    eprintln!("warning: failed to write coverage map: {e}");
                }
            }

            let status = std::process::Command::new(&tmp)
                .status()
                .unwrap_or_else(|e| {
                    eprintln!("error: could not run compiled binary: {e}");
                    std::process::exit(1);
                });

            let _ = std::fs::remove_file(&tmp);

            // Print coverage summary after run
            if coverage_map.is_some() {
                let cov_dir = std::path::Path::new(".pluto-coverage");
                let data_path = cov_dir.join("coverage-data.bin");
                match plutoc::coverage::CoverageData::read_binary(&data_path) {
                    Ok(data) => {
                        let map = coverage_map.as_ref().unwrap();
                        let stats = plutoc::coverage::generate_terminal_report(map, &data);
                        plutoc::coverage::print_terminal_summary(&stats);
                    }
                    Err(e) => {
                        eprintln!("warning: failed to read coverage data: {e}");
                    }
                }
            }

            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::Watch { command } => match command {
            WatchCommands::Run { file, no_clear } => {
                if let Err(err) = plutoc::watch::watch_run(&file, stdlib, no_clear) {
                    eprintln!("Watch error: {err}");
                    std::process::exit(1);
                }
            }
            WatchCommands::Test { file, no_clear, no_cache } => {
                let use_cache = !no_cache;
                if let Err(err) = plutoc::watch::watch_test(&file, stdlib, no_clear, use_cache) {
                    eprintln!("Watch test error: {err}");
                    std::process::exit(1);
                }
            }
        },
        Commands::Test { file, seed, iterations, no_cache, coverage } => {
            let tmp = std::env::temp_dir().join("pluto_test");
            let use_cache = !no_cache;
            let coverage_map = match plutoc::compile_file_for_tests_with_coverage(&file, &tmp, stdlib, use_cache, coverage) {
                Ok(map) => map,
                Err(err) => {
                    let filename = file.to_string_lossy().to_string();
                    eprintln!("error [{}]: {err}", filename);
                    std::process::exit(1);
                }
            };

            // Write coverage map before running tests
            if let Some(ref map) = coverage_map {
                let cov_dir = std::path::Path::new(".pluto-coverage");
                let _ = std::fs::create_dir_all(cov_dir);
                if let Err(e) = map.write_json(&cov_dir.join("coverage-map.json")) {
                    eprintln!("warning: failed to write coverage map: {e}");
                }
            }

            let mut cmd = std::process::Command::new(&tmp);
            if let Some(s) = seed {
                cmd.env("PLUTO_TEST_SEED", s.to_string());
            }
            if let Some(i) = iterations {
                cmd.env("PLUTO_TEST_ITERATIONS", i.to_string());
            }
            let status = cmd
                .status()
                .unwrap_or_else(|e| {
                    eprintln!("error: could not run compiled binary: {e}");
                    std::process::exit(1);
                });

            let _ = std::fs::remove_file(&tmp);

            // Print coverage summary after tests
            if coverage_map.is_some() {
                let cov_dir = std::path::Path::new(".pluto-coverage");
                let data_path = cov_dir.join("coverage-data.bin");
                match plutoc::coverage::CoverageData::read_binary(&data_path) {
                    Ok(data) => {
                        let map = coverage_map.as_ref().unwrap();
                        let stats = plutoc::coverage::generate_terminal_report(map, &data);
                        plutoc::coverage::print_terminal_summary(&stats);
                    }
                    Err(e) => {
                        eprintln!("warning: failed to read coverage data: {e}");
                    }
                }
            }

            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::EmitAst { file, output } => {
            let output = output.unwrap_or_else(|| file.with_extension("pluto"));

            match plutoc::analyze_file(&file, stdlib) {
                Ok((program, source, derived)) => {
                    match plutoc::binary::serialize_program(&program, &source, &derived) {
                        Ok(bytes) => {
                            if let Err(e) = std::fs::write(&output, &bytes) {
                                eprintln!("error: failed to write {}: {e}", output.display());
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("error: serialization failed: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                Err(err) => {
                    let filename = error_filename(&err)
                        .unwrap_or_else(|| file.to_string_lossy().to_string());
                    eprintln!("error [{}]: {err}", filename);
                    std::process::exit(1);
                }
            }
        }
        Commands::GeneratePt { file, output } => {
            let data = match std::fs::read(&file) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("error: failed to read {}: {e}", file.display());
                    std::process::exit(1);
                }
            };

            if !plutoc::binary::is_binary_format(&data) {
                eprintln!("error: {} is not a valid .pluto binary file", file.display());
                std::process::exit(1);
            }

            let (program, _source, _derived) = match plutoc::binary::deserialize_program(&data) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: failed to deserialize {}: {e}", file.display());
                    std::process::exit(1);
                }
            };

            let text = plutoc::pretty::pretty_print(&program);

            match output {
                Some(path) => {
                    if let Err(e) = std::fs::write(&path, &text) {
                        eprintln!("error: failed to write {}: {e}", path.display());
                        std::process::exit(1);
                    }
                }
                None => {
                    print!("{}", text);
                }
            }
        }
        Commands::Sync { file, output } => {
            let pluto_path = output.unwrap_or_else(|| file.with_extension("pluto"));

            match plutoc::sync::sync_pt_to_pluto(&file, &pluto_path) {
                Ok(result) => {
                    if !result.added.is_empty() {
                        for name in &result.added {
                            eprintln!("  + {name}");
                        }
                    }
                    if !result.removed.is_empty() {
                        for name in &result.removed {
                            eprintln!("  - {name}");
                        }
                    }
                    if !result.modified.is_empty() {
                        for name in &result.modified {
                            eprintln!("  ~ {name}");
                        }
                    }
                    eprintln!(
                        "synced {} \u{2192} {} ({} added, {} removed, {} unchanged)",
                        file.display(),
                        pluto_path.display(),
                        result.added.len(),
                        result.removed.len(),
                        result.unchanged,
                    );
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Update { dir } => {
            if let Err(err) = plutoc::update_git_deps(&dir) {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
        Commands::Coverage { command } => match command {
            CoverageCommands::Report { format, dir, output } => {
                let map_path = dir.join("coverage-map.json");
                let data_path = dir.join("coverage-data.bin");

                if !map_path.exists() {
                    eprintln!("error: coverage map not found at {}", map_path.display());
                    eprintln!("hint: run with --coverage flag first, e.g. `plutoc test file.pluto --coverage`");
                    std::process::exit(1);
                }
                if !data_path.exists() {
                    eprintln!("error: coverage data not found at {}", data_path.display());
                    eprintln!("hint: run with --coverage flag first, e.g. `plutoc test file.pluto --coverage`");
                    std::process::exit(1);
                }

                let map = match plutoc::coverage::CoverageMap::read_json(&map_path) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("error: failed to read coverage map: {e}");
                        std::process::exit(1);
                    }
                };
                let data = match plutoc::coverage::CoverageData::read_binary(&data_path) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("error: failed to read coverage data: {e}");
                        std::process::exit(1);
                    }
                };

                match format.as_str() {
                    "terminal" => {
                        let stats = plutoc::coverage::generate_terminal_report(&map, &data);
                        plutoc::coverage::print_terminal_summary(&stats);
                    }
                    "lcov" => {
                        let lcov = plutoc::coverage::generate_lcov(&map, &data);
                        match output {
                            Some(path) => {
                                if let Err(e) = std::fs::write(&path, &lcov) {
                                    eprintln!("error: failed to write {}: {e}", path.display());
                                    std::process::exit(1);
                                }
                                eprintln!("LCOV report written to {}", path.display());
                            }
                            None => print!("{}", lcov),
                        }
                    }
                    "json" => {
                        let report = plutoc::coverage::generate_json_report(&map, &data);
                        let json = serde_json::to_string_pretty(&report).unwrap();
                        match output {
                            Some(path) => {
                                if let Err(e) = std::fs::write(&path, &json) {
                                    eprintln!("error: failed to write {}: {e}", path.display());
                                    std::process::exit(1);
                                }
                                eprintln!("JSON report written to {}", path.display());
                            }
                            None => println!("{}", json),
                        }
                    }
                    "html" => {
                        // Determine source directory from file paths in the coverage map
                        let source_dir = map.files.first()
                            .and_then(|f| {
                                let p = std::path::Path::new(&f.path);
                                p.parent().map(|d| {
                                    if d.as_os_str().is_empty() {
                                        // File is at root level — use the coverage dir's parent
                                        dir.parent().unwrap_or(std::path::Path::new(".")).to_path_buf()
                                    } else {
                                        dir.parent().unwrap_or(std::path::Path::new(".")).join(d)
                                    }
                                })
                            })
                            .unwrap_or_else(|| dir.parent().unwrap_or(std::path::Path::new(".")).to_path_buf());

                        let html = plutoc::coverage::generate_html_report(&map, &data, &source_dir);
                        let out_path = output.unwrap_or_else(|| dir.join("report.html"));
                        if let Err(e) = std::fs::write(&out_path, &html) {
                            eprintln!("error: failed to write {}: {e}", out_path.display());
                            std::process::exit(1);
                        }
                        eprintln!("HTML report written to {}", out_path.display());
                    }
                    other => {
                        eprintln!("error: unknown format '{}'; expected 'terminal', 'lcov', 'json', or 'html'", other);
                        std::process::exit(1);
                    }
                }
            }
        },
    }
}
