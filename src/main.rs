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
    /// Start the LSP server (communicates over stdin/stdout)
    Lsp,
}

fn main() {
    let cli = Cli::parse();

    let stdlib = cli.stdlib.as_deref();

    match cli.command {
        Commands::Compile { file, output } => {
            if let Err(err) = plutoc::compile_file_with_stdlib(&file, &output, stdlib) {
                let filename = file.to_string_lossy().to_string();
                // For file-based compilation, we don't have a single source string for rendering.
                // Fall back to basic error display.
                eprintln!("error [{}]: {err}", filename);
                std::process::exit(1);
            }
        }
        Commands::Run { file } => {
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
        Commands::Test { file } => {
            let tmp = std::env::temp_dir().join("pluto_test");
            if let Err(err) = plutoc::compile_file_for_tests(&file, &tmp, stdlib) {
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
        Commands::EmitAst { file, output } => {
            let output = output.unwrap_or_else(|| file.with_extension("pluto"));

            match plutoc::analyze_file(&file, stdlib) {
                Ok((program, source)) => {
                    match plutoc::binary::serialize_program(&program, &source) {
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

            let (program, _source) = match plutoc::binary::deserialize_program(&data) {
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
        Commands::Update { dir } => {
            if let Err(err) = plutoc::update_git_deps(&dir) {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
    }
}
