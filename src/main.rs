use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "plutoc", version, about = "The Pluto compiler")]
struct Cli {
    /// Path to stdlib root directory
    #[arg(long, global = true)]
    stdlib: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
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
    /// Start the LSP server (communicates over stdin/stdout)
    Lsp,
    /// Watch files and automatically recompile/rerun on changes
    Watch {
        #[command(subcommand)]
        command: WatchCommands,
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
}

fn main() {
    let cli = Cli::parse();

    let stdlib = cli.stdlib.as_deref();

    match cli.command {
        Commands::Compile { file, output } => {
            // Check if this is a system file (contains a `system` declaration)
            match plutoc::detect_system_file(&file) {
                Ok(Some(_program)) => {
                    // System file: compile each member app to its own binary
                    match plutoc::compile_system_file_with_stdlib(&file, &output, stdlib) {
                        Ok(members) => {
                            for (name, path) in &members {
                                eprintln!("  compiled {} → {}", name, path.display());
                            }
                            eprintln!("system: {} member(s) compiled", members.len());
                        }
                        Err(err) => {
                            let filename = file.to_string_lossy().to_string();
                            eprintln!("error [{}]: {err}", filename);
                            std::process::exit(1);
                        }
                    }
                }
                Ok(None) => {
                    // Regular file: compile to a single binary
                    if let Err(err) = plutoc::compile_file_with_stdlib(&file, &output, stdlib) {
                        let filename = file.to_string_lossy().to_string();
                        eprintln!("error [{}]: {err}", filename);
                        std::process::exit(1);
                    }
                }
                Err(err) => {
                    let filename = file.to_string_lossy().to_string();
                    eprintln!("error [{}]: {err}", filename);
                    std::process::exit(1);
                }
            }
        }
        Commands::Run { file } => {
            // Reject system files — they produce multiple binaries
            match plutoc::detect_system_file(&file) {
                Ok(Some(_)) => {
                    eprintln!("error: cannot run a system file directly; use `plutoc compile` to produce individual binaries");
                    std::process::exit(1);
                }
                Ok(None) => {}
                Err(err) => {
                    let filename = file.to_string_lossy().to_string();
                    eprintln!("error [{}]: {err}", filename);
                    std::process::exit(1);
                }
            }

            let tmp = std::env::temp_dir().join("pluto_run");
            if let Err(err) = plutoc::compile_file_with_stdlib(&file, &tmp, stdlib) {
                let filename = file.to_string_lossy().to_string();
                eprintln!("error [{}]: {err}", filename);
                std::process::exit(1);
            }

            let status = std::process::Command::new(&tmp)
                .status()
                .unwrap_or_else(|e| {
                    eprintln!("error: could not run compiled binary: {e}");
                    std::process::exit(1);
                });

            let _ = std::fs::remove_file(&tmp);

            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::Lsp => {
            if let Err(err) = plutoc::lsp::run_lsp_server() {
                eprintln!("LSP server error: {err}");
                std::process::exit(1);
            }
        }
        Commands::Watch { command } => match command {
            WatchCommands::Run { file, no_clear } => {
                if let Err(err) = plutoc::watch::watch_run(&file, stdlib, no_clear) {
                    eprintln!("Watch error: {err}");
                    std::process::exit(1);
                }
            }
        },
        Commands::Test { file, seed, iterations, no_cache } => {
            let tmp = std::env::temp_dir().join("pluto_test");
            let use_cache = !no_cache;
            if let Err(err) = plutoc::compile_file_for_tests(&file, &tmp, stdlib, use_cache) {
                let filename = file.to_string_lossy().to_string();
                eprintln!("error [{}]: {err}", filename);
                std::process::exit(1);
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
                    let filename = file.to_string_lossy().to_string();
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
                        "synced {} → {} ({} added, {} removed, {} unchanged)",
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
    }
}
