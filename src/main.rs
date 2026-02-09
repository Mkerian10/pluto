use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "plutoc", about = "The Pluto compiler")]
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
    }
}
