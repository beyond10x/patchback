//! `patchback generate-dataset --seed <n> --samples <n> --out <dir>` (DESIGN.md § 7, § 11,
//! `GenerateDataset` of `spec/domains/dataset.yaml`): the deterministic training, validation and
//! test splits of `patchback_dataset`, written as `dataset.meta` and one `samples.meta` a split
//! (`patchback_dataset::meta`). The dense `x.f32`, `y.f32` and `order.u32` files are
//! story:dense-data-files.
//!
//! # The run
//!
//! 1. The split counts from `--samples` (`SplitCounts::from_samples`: 60,000 gives 50,016 /
//!    4,992 / 4,992, 48 gives 16 / 16 / 16); a count that is not a positive multiple of the batch
//!    size 16 is refused before anything is touched. `--samples` admits
//!    `0..=9223372036854775807` (`patchback_dataset::MAX_SAMPLES`, `i64::MAX`), so every count is
//!    the rule's; a larger value is a usage error (clap's message, exit 2).
//! 2. `<out>` must be a directory or absent, and none of `<out>/dataset.meta`,
//!    `<out>/train/samples.meta`, `<out>/val/samples.meta` and `<out>/test/samples.meta` may
//!    exist: an existing file is never overwritten. Checked before the generation starts, and
//!    again by every write. A split's directory (`SplitRole::dir`: `train`, `val`, `test`,
//!    DESIGN.md § 7.2) is not its record name (`SplitRole::key`: `train`, `validation`, `test`).
//! 3. The generation (`Generator::run`), which renders every drawn patch.
//! 4. The writes: `<out>` and the split directories are created when absent, each split's
//!    `samples.meta`, then `dataset.meta`, last, each as a new file. A failed write removes every
//!    file and directory this run created.
//!
//! # Output
//!
//! UTF-8 text on stdout, one `key value` line per record, each ending in `\n`: `out`, the
//! directory as given, quoted and escaped as a refusal quotes a path (`Path`'s `Debug`); then
//! `train`, `validation`, `test`, `diagnostic_silence`, `diagnostic_noise` and
//! `diagnostic_clipping`, the counts `dataset.meta` records.
//!
//! # Refusals
//!
//! One line on stderr, exit status 1, nothing left written:
//!
//! - `patchback: generate-dataset refused: patchback.dataset.SplitCountNotBatchMultiple: <message>`;
//! - `patchback: generate-dataset refused: cannot write "<path>": <message>` for an existing file
//!   or one that cannot be written;
//! - `patchback: generate-dataset refused: generation exhausted: <message>` when the draws reach
//!   their bound (`patchback_dataset::MAX_DRAWS_PER_SAMPLE` a sample) before the splits are full;
//!   the model declares no error for it;
//! - `patchback: generate-dataset refused: <domain error>: <message>` for a render or feature
//!   refusal that is a fault rather than a diagnostic example (none is reachable from the
//!   arguments).

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Args;
use patchback_dataset::meta::{
    DATASET_META_FILE, SAMPLES_META_FILE, count_records, dataset_meta, samples_meta,
};
use patchback_dataset::{GenerateError, Generated, Generator, MAX_SAMPLES, SplitCounts, SplitRole};

/// The command ran and refused; it wrote nothing.
const REFUSED: u8 = 1;

/// The arguments of `patchback generate-dataset`.
#[derive(Args, Debug)]
pub struct GenerateDatasetArgs {
    /// The dataset seed: the same seed, build and arguments give the same files
    #[arg(long)]
    pub seed: u64,
    /// How many samples to generate over the training, validation and test splits, at most
    /// 9223372036854775807
    #[arg(long, value_parser = clap::value_parser!(u64).range(..=MAX_SAMPLES))]
    pub samples: u64,
    /// The directory to write the dataset into; created when absent
    #[arg(long)]
    pub out: PathBuf,
}

/// Why `generate-dataset` wrote nothing: the one line it prints on stderr.
struct Refusal(String);

impl Refusal {
    fn domain(error: &str, message: impl std::fmt::Display) -> Self {
        Self::one_line(format!(
            "patchback: generate-dataset refused: {error}: {message}"
        ))
    }

    fn io(path: &Path, error: impl std::fmt::Display) -> Self {
        Self::one_line(format!(
            "patchback: generate-dataset refused: cannot write {path:?}: {error}"
        ))
    }

    /// A refusal is one line: any control character left in a message is escaped.
    fn one_line(text: String) -> Self {
        Self(
            text.chars()
                .flat_map(|c| {
                    let escaped: Vec<char> = if c.is_control() {
                        c.escape_default().collect()
                    } else {
                        vec![c]
                    };
                    escaped
                })
                .collect(),
        )
    }
}

impl From<GenerateError> for Refusal {
    fn from(error: GenerateError) -> Self {
        match error.domain_error() {
            Some(name) => Self::domain(name, &error),
            None => Self::one_line(format!("patchback: generate-dataset refused: {error}")),
        }
    }
}

/// Run the verb: the module documentation's steps, its output on stdout, a refusal on stderr.
pub fn run(args: &GenerateDatasetArgs) -> ExitCode {
    match generate(args) {
        Ok(generated) => {
            print!("out {:?}\n{}", args.out, count_records(&generated));
            ExitCode::SUCCESS
        }
        Err(refusal) => {
            eprintln!("{}", refusal.0);
            ExitCode::from(REFUSED)
        }
    }
}

/// The files of a dataset under `out`, in the order they are written: each split's
/// `samples.meta`, then `dataset.meta`.
fn targets(out: &Path) -> Vec<(Option<SplitRole>, PathBuf)> {
    let mut targets: Vec<(Option<SplitRole>, PathBuf)> = SplitRole::ALL
        .iter()
        .map(|&role| (Some(role), out.join(role.dir()).join(SAMPLES_META_FILE)))
        .collect();
    targets.push((None, out.join(DATASET_META_FILE)));
    targets
}

fn generate(args: &GenerateDatasetArgs) -> Result<Generated, Refusal> {
    let counts = SplitCounts::from_samples(args.samples);
    counts.check().map_err(GenerateError::Dataset)?;

    let out = &args.out;
    if out.exists() && !out.is_dir() {
        return Err(Refusal::io(out, "exists and is not a directory"));
    }
    // dataset.meta first: it names an existing dataset.
    let mut checks = targets(out);
    checks.rotate_right(1);
    for (_, path) in &checks {
        if path.exists() {
            return Err(Refusal::io(path, "already exists; it is never overwritten"));
        }
    }

    let generated = Generator::new(args.seed, counts, out.clone()).run()?;
    write(out, &generated)?;
    Ok(generated)
}

/// Writes the dataset's files (module documentation, step 4); on failure removes what it created.
fn write(out: &Path, generated: &Generated) -> Result<(), Refusal> {
    let mut created = Created::default();
    let result = write_all(out, generated, &mut created);
    if result.is_err() {
        created.remove();
    }
    result
}

/// What one run created, to be removed when a later write fails.
#[derive(Default)]
struct Created {
    dirs: Vec<PathBuf>,
    files: Vec<PathBuf>,
}

impl Created {
    /// Creates `dir` and its missing parents, recording each one created.
    fn dir(&mut self, dir: &Path) -> io::Result<()> {
        let mut missing = Vec::new();
        let mut current = Some(dir);
        while let Some(path) = current {
            if path.as_os_str().is_empty() || path.exists() {
                break;
            }
            missing.push(path.to_path_buf());
            current = path.parent();
        }
        for path in missing.into_iter().rev() {
            fs::create_dir(&path)?;
            self.dirs.push(path);
        }
        Ok(())
    }

    fn remove(self) {
        for file in self.files.iter().rev() {
            let _ = fs::remove_file(file);
        }
        for dir in self.dirs.iter().rev() {
            let _ = fs::remove_dir(dir);
        }
    }
}

fn write_all(out: &Path, generated: &Generated, created: &mut Created) -> Result<(), Refusal> {
    created.dir(out).map_err(|error| Refusal::io(out, error))?;
    for (role, path) in targets(out) {
        let text = match role {
            Some(role) => {
                let dir = out.join(role.dir());
                created
                    .dir(&dir)
                    .map_err(|error| Refusal::io(&dir, error))?;
                samples_meta(generated, role)
            }
            None => dataset_meta(generated),
        };
        write_new(&path, text.as_bytes(), created).map_err(|error| Refusal::io(&path, error))?;
    }
    Ok(())
}

/// Writes `bytes` to a file that must not exist yet, recording it once it exists.
fn write_new(path: &Path, bytes: &[u8], created: &mut Created) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| match error.kind() {
            io::ErrorKind::AlreadyExists => io::Error::new(
                io::ErrorKind::AlreadyExists,
                "already exists; it is never overwritten",
            ),
            _ => error,
        })?;
    created.files.push(path.to_path_buf());
    file.write_all(bytes).and_then(|()| file.sync_all())
}
