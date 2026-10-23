//! `patchback`, the application entry point (DESIGN.md § 4.1, § 11).
//!
//! The scaffold recognises the Milestone A subcommands, `patch validate`, `render`, `analyze`,
//! `compare` and `fit`; none is implemented yet, and none takes an argument yet. The command
//! line is parsed with clap derive, the one reviewed command-line crate (DESIGN.md § 3).
//!
//! Exit statuses:
//!
//! | Status | Meaning |
//! |---|---|
//! | 0 | `--help` or `-h` (on the root or on any command) or `help [command]` printed clap's help to stdout; `--version` or `-V` printed `patchback <version>` to stdout |
//! | 2 | usage error, reported by clap on stderr: no arguments or `patch` alone (the help, to stderr), an unknown command or flag, or an argument a command does not take |
//! | 3 | a recognised command that is not implemented yet, with one line on stderr |
//!
//! Status 1 is left for a recognised command that runs and fails, once commands exist.
//!
//! Where clap's conventions differ from the std-only parser of wave a1: help is the help of the
//! command it follows, not one shared usage text; an unknown command is reported as clap's
//! `error: unrecognized subcommand '<name>'`; and an argument a stub does not take, or a help
//! flag after such an argument, is a usage error (2) rather than the stub's status 3.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// The command is recognised but not implemented yet.
const NOT_IMPLEMENTED: u8 = 3;

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
    Analyze,
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
            Command::Analyze => "analyze",
            Command::Compare => "compare",
            Command::Fit => "fit",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    eprintln!("patchback: {} is not implemented yet", cli.command.name());
    ExitCode::from(NOT_IMPLEMENTED)
}
