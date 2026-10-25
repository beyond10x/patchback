//! `patchback`, the application entry point (DESIGN.md § 4.1, § 11).
//!
//! The binary recognises the Milestone A subcommands, `patch validate`, `render`, `analyze`,
//! `compare` and `fit`, and the Milestone B `generate-dataset`, all implemented: `patch validate
//! <file>` ([`patch_validate`]), `render --patch <file> --notes <file> --out <wav> [--block-size
//! <frames>]` ([`render`]), `analyze --input <wav> --out <dir>` ([`analyze`]), `compare
//! --reference <wav> --candidate <wav>` ([`compare`]), `fit --input <wav> --notes <file> --out
//! <dir> [--max-renders <n>] [--max-wall-time-ms <ms>] [--max-restarts <n>]
//! [--improvement-threshold <loss>] [--seed <n>] [--block-size <frames>]` ([`fit`]) and
//! `generate-dataset --seed <n> --samples <n> --out <dir>` ([`generate_dataset`]). The command
//! line is parsed with clap derive, the one reviewed command-line crate (DESIGN.md § 3).
//!
//! Exit statuses:
//!
//! | Status | Meaning |
//! |---|---|
//! | 0 | `patch validate` found the patch valid and printed one line naming it, its node and edge counts, to stdout; or `render` wrote its WAV and printed the output path, sample rate, block size, frame count, `limited true` or `limited false` and the limited sample count as `key value` lines to stdout; or `analyze` wrote its analysis file and printed its path to stdout; or `compare` printed the loss, its four components, their weights and the alignment lag as `key value` lines to stdout; or `fit` wrote its six files (`patch.patch`, `notes.events`, `reconstructed.wav`, `report.txt`, `run.meta` and `trace.events`) and printed the run's evaluations, restarts, initial and best loss, the best loss's four components, the stop reason, the wall time and `limited` as `key value` lines to stdout; or `generate-dataset` wrote `dataset.meta`, and the three `samples.meta`, `x.f32`, `y.f32` and `order.u32`, and printed `out` and the split and diagnostic counts as `key value` lines to stdout; or `--help` or `-h` (on the root or on any command) or `help [command]` printed clap's help to stdout; or `--version` or `-V` printed `patchback <version>` to stdout |
//! | 1 | `patch validate` refused (an unreadable file, a file the parser refuses, a graph validation refuses), with one line on stderr: `patchback: patch validate refused: <domain error>: "<file>": <line or node or edge>: <reason>`; or `render` refused (an unsupported sample rate, a block size of zero or less, a patch or note sequence not found, a patch not compiled, a render too long, a non-finite sample, an existing output or one it cannot write) and wrote nothing, with one line on stderr: `patchback: render refused: <domain error>: <message>` for a domain refusal; or `analyze` refused (unreadable or refused WAV, a refused analysis, an existing analysis file, an output it cannot write) and wrote nothing, with one line on stderr naming the reason: `patchback: analyze refused: <domain error>: <message>` for a domain refusal; or `compare` refused (a missing clip, a refused WAV, clips at different rates, lengths alignment cannot absorb), with one line on stderr: `patchback: compare refused: <domain error>: <message>` for a domain refusal; or `fit` refused (a bound of 0, a missing or refused reference or note sequence, a silent or unpitched reference, a run without a valid candidate, an `--out` that is not a directory, one of its six files already in `--out`, or one it cannot write) and wrote nothing, with one line on stderr: `patchback: fit refused: <domain error>: <message>` for a domain refusal; or `generate-dataset` refused (a split count that is not a positive multiple of 16, an existing `dataset.meta`, `samples.meta`, `x.f32`, `y.f32` or `order.u32`, an output it cannot write, a dense file that does not read back with its byte count and finite values, a generation that ran out of draws) and left nothing written, with one line on stderr: `patchback: generate-dataset refused: <domain error>: <message>` for a domain refusal |
//! | 2 | usage error, reported by clap on stderr: no arguments or `patch` alone (the help, to stderr), an unknown command or flag, `patch validate` without its file or with a second one, a missing `--patch`, `--notes`, `--out`, `--input`, `--reference`, `--candidate`, `--seed` or `--samples`, a `--block-size`, `--max-renders`, `--max-wall-time-ms`, `--max-restarts`, `--seed` or `--samples` that is not a non-negative integer, a `--samples` above 4294967295, an `--improvement-threshold` that is not a finite non-negative decimal, a `--block-size` below 1, or an argument a command does not take |
//!
//! Where clap's conventions differ from the std-only parser of wave a1: help is the help of the
//! command it follows, not one shared usage text; an unknown command is reported as clap's
//! `error: unrecognized subcommand '<name>'`; and an argument a command does not take, or a help
//! flag after such an argument, is a usage error (2).

mod analyze;
mod benchmark;
mod compare;
mod fit;
mod generate_dataset;
mod patch_validate;
mod render;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
    Render {
        /// The .patch file to load, validate, compile and render
        #[arg(long)]
        patch: PathBuf,
        /// The notes.events file that drives it, at 48 kHz
        #[arg(long)]
        notes: PathBuf,
        /// The mono 48 kHz float32 WAV to write; it must not exist yet
        #[arg(long)]
        out: PathBuf,
        /// Frames per processing block, a positive integer [default: 256]
        #[arg(long, allow_negative_numbers = true)]
        block_size: Option<i64>,
    },
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
    Compare {
        /// The recording to match: mono or stereo, PCM16 or float32, 48 kHz
        #[arg(long)]
        reference: PathBuf,
        /// The rendering to judge against it, in the same format
        #[arg(long)]
        candidate: PathBuf,
    },
    /// Recover a patch that reproduces a reference WAV
    Fit(fit::FitArgs),
    /// Generate the deterministic training, validation and test splits
    GenerateDataset(generate_dataset::GenerateDatasetArgs),
    /// Benchmark the fit against the default patch over a synthesized holdout
    Benchmark(benchmark::BenchmarkArgs),
}

#[derive(Subcommand)]
enum PatchCommand {
    /// Check a .patch file without rendering it
    Validate {
        /// The .patch file to load, validate and compile
        file: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match &cli.command {
        Command::Patch {
            command: PatchCommand::Validate { file },
        } => patch_validate::run(file),
        Command::Render {
            patch,
            notes,
            out,
            block_size,
        } => match render::run(patch, notes, out, *block_size) {
            Ok(written) => {
                print!("{}", render::report(out, &written.report));
                ExitCode::SUCCESS
            }
            Err(refusal) => {
                eprintln!("{}", refusal.line());
                ExitCode::from(REFUSED)
            }
        },
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
        Command::Compare {
            reference,
            candidate,
        } => match compare::run(reference, candidate) {
            Ok(comparison) => {
                print!("{}", compare::render(&comparison));
                ExitCode::SUCCESS
            }
            Err(refusal) => {
                eprintln!("{}", refusal.line());
                ExitCode::from(REFUSED)
            }
        },
        Command::Fit(args) => fit::run(args),
        Command::GenerateDataset(args) => generate_dataset::run(args),
        Command::Benchmark(args) => benchmark::run(args),
    }
}
