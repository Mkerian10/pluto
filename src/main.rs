use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "plutoc", about = "The Pluto compiler")]
struct Cli {
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
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile { file, output } => {
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: could not read '{}': {e}", file.display());
                    std::process::exit(1);
                }
            };

            let filename = file.to_string_lossy().to_string();
            if let Err(err) = plutoc::compile(&source, &output) {
                plutoc::diagnostics::render_error(&source, &filename, &err);
                std::process::exit(1);
            }
        }
        Commands::Run { file } => {
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: could not read '{}': {e}", file.display());
                    std::process::exit(1);
                }
            };

            let filename = file.to_string_lossy().to_string();

            let tmp = std::env::temp_dir().join("pluto_run");
            if let Err(err) = plutoc::compile(&source, &tmp) {
                plutoc::diagnostics::render_error(&source, &filename, &err);
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
