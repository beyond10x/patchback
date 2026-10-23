//! `patchback`, the application entry point (DESIGN.md § 4.1, § 11).
//!
//! The binary recognises the Milestone A subcommands, `patch validate`, `render`, `analyze`,
//! `compare` and `fit`. `analyze --input <wav> --out <dir>` is implemented ([`analyze`]); the
//! others are not implemented yet and take no argument yet. The command line is parsed with clap
//! derive, the one reviewed command-line crate (DESIGN.md § 3).
//!
//! Exit statuses:
//!
//! | Status | Meaning |
//! |---|---|
//! | 0 | `analyze` wrote its analysis file and printed its path to stdout; or `--help` or `-h` (on the root or on any command) or `help [command]` printed clap's help to stdout; or `--version` or `-V` printed `patchback <version>` to stdout |
//! | 1 | `analyze` refused (unreadable or refused WAV, a refused analysis, an existing analysis file, an output it cannot write) and wrote nothing, with one line on stderr naming the reason: `patchback: analyze refused: <domain error>: <message>` for a domain refusal |
//! | 2 | usage error, reported by clap on stderr: no arguments or `patch` alone (the help, to stderr), an unknown command or flag, a missing `--input` or `--out`, or an argument a command does not take |
//! | 3 | a recognised command that is not implemented yet (`patch validate`, `render`, `compare`, `fit`), with one line on stderr |
//!
//! Where clap's conventions differ from the std-only parser of wave a1: help is the help of the
//! command it follows, not one shared usage text; an unknown command is reported as clap's
//! `error: unrecognized subcommand '<name>'`; and an argument a stub does not take, or a help
//! flag after such an argument, is a usage error (2) rather than the stub's status 3.

mod analyze;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// The command is recognised but not implemented yet.
const NOT_IMPLEMENTED: u8 = 3;

/// The command ran and refused; it wrote nothing.
const REFUSED: u8 = 1;

/// Inverse synthesis for electronic music: recover an editable DSP patch from a recording.
#[derive(Parser)]
#[command(
    name = "patchback",
    version,
    subcommand_required = true,
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Work with .patch files
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Patch {
        #[command(subcommand)]
        command: PatchCommand,
    },
    /// Render a patch with a score into a WAV
    Render,
    /// Measure a reference WAV
    Analyze {
        /// The WAV to analyse: mono or stereo, PCM16 or float32, 48 kHz
        #[arg(long)]
        input: PathBuf,
        /// The directory to write analysis.txt into; created when absent
        #[arg(long)]
        out: PathBuf,
    },
    /// Compare a candidate WAV with a reference
    Compare,
    /// Recover a patch that reproduces a reference WAV
    Fit,
}

#[derive(Subcommand)]
enum PatchCommand {
    /// Check a .patch file without rendering it
    Validate,
}

impl Command {
    /// The command as typed, for its one stderr line.
    fn name(&self) -> &'static str {
        match self {
            Command::Patch {
                command: PatchCommand::Validate,
            } => "patch validate",
            Command::Render => "render",
            Command::Analyze { .. } => "analyze",
            Command::Compare => "compare",
            Command::Fit => "fit",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match &cli.command {
        Command::Analyze { input, out } => match analyze::run(input, out) {
            Ok(written) => {
                println!("{}", written.display());
                ExitCode::SUCCESS
            }
            Err(refusal) => {
                eprintln!("{}", refusal.line());
                ExitCode::from(REFUSED)
            }
        },
        command => {
            eprintln!("patchback: {} is not implemented yet", command.name());
            ExitCode::from(NOT_IMPLEMENTED)
        }
    }
}
