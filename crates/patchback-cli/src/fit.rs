//! `patchback fit --input <wav> --notes <events> --out <dir> [--max-renders <n>]` (DESIGN.md
//! § 11, `StartFit` of `spec/domains/fit.yaml`): the bounded search for a patch that reproduces
//! a reference recording. The arguments parse; the search is `story:bounded-search-fit`, and
//! until it lands the verb exits 3 with one stderr line.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

/// The command is recognised but not implemented yet.
const NOT_IMPLEMENTED: u8 = 3;

/// The arguments of `patchback fit`.
#[derive(Args, Debug)]
pub struct FitArgs {
    /// The reference WAV to match: mono or stereo, PCM16 or float32, 48 kHz
    #[arg(long)]
    pub input: PathBuf,
    /// The notes.events file holding the reference's known notes, at 48 kHz
    #[arg(long)]
    pub notes: PathBuf,
    /// The directory to write the run's files into; created when absent
    #[arg(long)]
    pub out: PathBuf,
    /// The most renders the search may spend [default: 2000]
    #[arg(long, default_value_t = 2_000)]
    pub max_renders: u64,
}

/// Run the verb: not implemented yet, one line on stderr, exit 3.
pub fn run(_args: &FitArgs) -> ExitCode {
    eprintln!("patchback: fit is not implemented yet");
    ExitCode::from(NOT_IMPLEMENTED)
}
