#![forbid(unsafe_code)]
//! `patchback-studio export --run <dir> --out <file>`: writes the studio document of a fit run
//! ([`patchback_studio::export`]). A refusal prints one line naming the specification's error
//! and exits 1; the output file is then not left behind.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use patchback_studio::export::write_export;

#[derive(Parser)]
#[command(
    name = "patchback-studio",
    version,
    about = "Exports fit runs for Patchback Studio"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Writes the document of the fit run in `--run` to `--out`, which must not exist.
    Export {
        /// The fit run directory `patchback fit --out` wrote.
        #[arg(long)]
        run: PathBuf,
        /// The JSON file to write.
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Export { run, out } => match write_export(&run, &out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("patchback-studio: export refused: {error}");
                ExitCode::from(1)
            }
        },
    }
}
