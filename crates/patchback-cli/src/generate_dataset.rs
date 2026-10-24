//! `patchback generate-dataset --seed <n> --samples <n> --out <dir>` (DESIGN.md § 7, § 11,
//! `GenerateDataset` of `spec/domains/dataset.yaml`): the deterministic training, validation
//! and test splits. The arguments parse; the generator is `story:dataset-generation`, and until
//! it lands the verb exits 3 with one stderr line.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

/// The command is recognised but not implemented yet.
const NOT_IMPLEMENTED: u8 = 3;

/// The arguments of `patchback generate-dataset`.
#[derive(Args, Debug)]
pub struct GenerateDatasetArgs {
    /// The dataset seed: the same seed, build and arguments give the same files
    #[arg(long)]
    pub seed: u64,
    /// How many samples to generate over the training, validation and test splits
    #[arg(long)]
    pub samples: u64,
    /// The directory to write the dataset into; created when absent
    #[arg(long)]
    pub out: PathBuf,
}

/// Run the verb: not implemented yet, one line on stderr, exit 3.
pub fn run(_args: &GenerateDatasetArgs) -> ExitCode {
    eprintln!("patchback: generate-dataset is not implemented yet");
    ExitCode::from(NOT_IMPLEMENTED)
}
