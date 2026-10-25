//! The results of a fit run (DESIGN.md § 11, § 15; `spec/domains/fit.yaml` `WriteFitResults`,
//! `FitResults`): [`write_results`] writes the six files of a completed run into a directory.
//!
//! # Refusals
//!
//! In the order `WriteFitResults` declares its outcomes, before anything is touched
//! ([`admit_results`]):
//!
//! 1. `no-such-run` (`FitRunNotFound`) has no counterpart: a run is a value, so no call can name
//!    a run that does not exist (Da7-7).
//! 2. `failed-run`: a Failed run is refused [`FitError::FailedRunHasNoPatch`]; nothing is written,
//!    not even the directory (Da7-2; whether a failed run writes a diagnostic report or keeps its
//!    trace is OQ-12).
//! 3. `still-running`: a Running run is refused [`FitError::FitStillRunning`].
//! 4. `written`: a Completed run's files are written.
//!
//! Every file's bytes are then made before any is written; a file that cannot be made is
//! [`ResultsError::Encode`], nothing written. The directory and its missing parents are created,
//! then each file is created new, never overwritten, in the order of [`RESULT_FILES`], and synced.
//! A creation or write that fails is [`ResultsError::Io`], and every file and directory this call
//! created is removed again; whatever existed before is left as it was.
//!
//! # The files
//!
//! | File | Contents |
//! |---|---|
//! | [`PATCH_FILE`] | the best patch, `patchback_core::patch::write_patch` of [`FitOutcome::patch`] |
//! | [`NOTES_FILE`] | the note sequence the run was driven by, in its canonical form (`patchback_core::score::write_events`) |
//! | [`RECONSTRUCTED_WAV_FILE`] | [`FitOutcome::render`], the clip the best patch was scored on, mono float32 at 48 kHz (`patchback_audio::encode`): comparing it with the reference gives the best loss again (Da7-5) |
//! | [`REPORT_FILE`] | the report ([`report_text`]) |
//! | [`RUN_META_FILE`] | what reproduces the run ([`run_meta_text`]) |
//! | [`TRACE_FILE`] | the per-candidate log ([`crate::write_trace`], [`crate::trace`]) |
//!
//! `report.txt` and `run.meta` are line-oriented UTF-8 text like `dataset.meta`: a header line
//! naming the format and its version, then one `<key> <value>` record per line, one space between,
//! each line ended by `\n`, the records once each in the order below. Reals are written as Rust
//! prints an `f64` (the shortest text that reads back to the same value, never in exponent form);
//! an absent value, which a completed run never has, is `none`.
//!
//! `report.txt` (Da7-4):
//!
//! | Record | Value |
//! |---|---|
//! | `patchback-fit-report` | the format version, `1` |
//! | `patchback` | the Patchback version, [`PATCHBACK_VERSION`] |
//! | `initial_loss` | the loss of the run's first valid candidate |
//! | `best_loss` | the loss of the best patch, the one written |
//! | `log_spectral`, `envelope`, `transient`, `band_energy` | the best patch's four components |
//! | `evaluations` | the render evaluations counted, valid and rejected |
//! | `restarts` | the restarts that began |
//! | `stop_reason` | why the run stopped |
//! | `wall_time_ms` | the run's wall time in milliseconds |
//! | `warning` | zero or more, last, one per warning ([`warnings`]), in this order: `limiter engaged on <n> of <m> rendered samples` when the protective limiter changed `<n>` of the `<m>` samples of the best patch's uncut render; `clamped <n> of 8 predictions` when `<n>` of the eight outputs of a seeded run's prediction were outside [0, 1] and clamped into it (Da11-11) |
//!
//! `initial_loss` is the loss of the run's start, the default patch or the prediction, when it
//! rendered. `wall_time_ms` is the one record that differs between two runs of the same inputs.
//! There is no `confidence` record: DESIGN.md § 11 names one and defines it nowhere (Da7-4, for
//! the owner).
//!
//! `run.meta` (Da7-3):
//!
//! | Record | Value |
//! |---|---|
//! | `patchback-fit-run-meta` | the format version, `1` |
//! | `patchback` | the Patchback version, [`PATCHBACK_VERSION`] |
//! | `init` | [`INIT_DEFAULT`], `default`: the run started from the default patch; or [`INIT_REGRESSOR_PREDICTION`], `regressor-prediction`: from the regressor's prediction ([`crate::FitOutcome::init`]) |
//! | `weights` | seeded runs only: `sha256 <64 hex digits>`, the weight artifact's checksum (the SHA-256 of its payload, [`crate::SeededStart::weights_sha256`]) |
//! | `weights_modelspec` | seeded runs only: `<version> <commit>`, the ModelSpec version and commit the artifact names |
//! | `normalization` | seeded runs only: `sha256 <64 hex digits>`, the digest of the statistics' bytes the caller gave ([`crate::SeedInputs::normalization_sha256`]) |
//! | `seed` | the seed of the restarts' starting points |
//! | `topology` | the topology of the best patch, `subtractive-mono/1` |
//! | `feature_contract` | `patchback_analysis::FEATURES_1_VERSION`, `features/1` |
//! | `prng` | `patchback_core::prng::NAME`, `xoshiro256**` |
//! | `modelspec` | the version of the ModelSpec pin the caller names ([`RunMeta::modelspec_version`]) |
//! | `reference` | `sha256 <64 hex digits>`: the digest of the reference file's bytes |
//! | `notes` | `sha256 <64 hex digits>`: the digest of the notes file's bytes |
//! | `max_renders`, `max_wall_time_ms`, `max_restarts`, `improvement_threshold` | the run's bounds |
//! | `block_size` | the frames per render block |
//!
//! The three seeded records follow `init` and are written only for a seeded run, so a default
//! run's `run.meta` is the format-1 file it was before them; the format version stays 1
//! (Da11-10). The artifact's ModelSpec record is `weights_modelspec`, not a second `modelspec`:
//! `modelspec` is the caller's pin, and every key appears once (DESIGN.md § 11).
//!
//! Every file but `report.txt` is the same, byte for byte, for the same inputs, seed, bounds and
//! block size (§ 2 invariant 5), unless the wall-time bound stopped the run.

use std::fmt;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use patchback_audio::{WavSampleFormat, encode};
use patchback_core::digest::{self, hex};
use patchback_core::patch::{Patch, known_topology, write_patch};
use patchback_core::score::{NoteSequence, write_events};

use crate::run::{FitRun, FitState};
use crate::{FitError, FitOutcome, write_trace};

/// The best patch.
pub const PATCH_FILE: &str = "patch.patch";
/// The canonical note sequence.
pub const NOTES_FILE: &str = "notes.events";
/// The clip the best patch was scored on.
pub const RECONSTRUCTED_WAV_FILE: &str = "reconstructed.wav";
/// The report.
pub const REPORT_FILE: &str = "report.txt";
/// What reproduces the run.
pub const RUN_META_FILE: &str = "run.meta";
/// The per-candidate log.
pub const TRACE_FILE: &str = "trace.events";

/// The six files of a fit run (DESIGN.md § 11), in the order they are written.
pub const RESULT_FILES: [&str; 6] = [
    PATCH_FILE,
    NOTES_FILE,
    RECONSTRUCTED_WAV_FILE,
    REPORT_FILE,
    RUN_META_FILE,
    TRACE_FILE,
];

/// The first field of `report.txt`'s header line.
pub const REPORT_HEADER: &str = "patchback-fit-report";
/// The `report.txt` format version.
pub const REPORT_FORMAT_VERSION: u32 = 1;
/// The first field of `run.meta`'s header line.
pub const RUN_META_HEADER: &str = "patchback-fit-run-meta";
/// The `run.meta` format version.
pub const RUN_META_FORMAT_VERSION: u32 = 1;

/// The Patchback version `report.txt` and `run.meta` name: this crate's, which every crate of
/// the workspace inherits.
pub const PATCHBACK_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `init` of a run that started from the default patch ([`crate::fit`], or
/// [`crate::fit_request`] without seed inputs).
pub const INIT_DEFAULT: &str = "default";

/// The `init` of a run that started from the regressor's prediction ([`crate::fit_request`]
/// with seed inputs; `InitSource::RegressorPrediction`).
pub const INIT_REGRESSOR_PREDICTION: &str = "regressor-prediction";

/// What `run.meta` records that the outcome does not hold; the caller fills it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunMeta {
    /// The seed of the run ([`crate::FitConfig::seed`]).
    pub seed: u64,
    /// The frames per render block of the run.
    pub block_size_frames: i64,
    /// The ModelSpec version the run is recorded against: `patchback fit` names its compiled-in
    /// pin. One token: no whitespace, no control character.
    pub modelspec_version: String,
    /// The SHA-256 of the reference file's bytes.
    pub reference_sha256: [u8; digest::DIGEST_BYTES],
    /// The SHA-256 of the notes file's bytes.
    pub notes_sha256: [u8; digest::DIGEST_BYTES],
}

/// The written results of a completed run (`spec/domains/fit.yaml` `FitResults`; its
/// `fit_run_id` is the caller's).
#[derive(Clone, Debug, PartialEq)]
pub struct FitResults {
    /// The directory the files were written into.
    pub out_dir: PathBuf,
    /// The run's retained best candidate ([`FitRun::best`], the [`crate::Evaluation::sequence`]
    /// that scored it): the model's `recovered_patch_id`.
    pub recovered_candidate: Option<u64>,
    /// The recovered patch, the one written.
    pub recovered_patch: Patch,
    /// The reconstructed WAV: the model's `reconstructed_wav_id`, here its path.
    pub reconstructed_wav: PathBuf,
    /// The report's warnings, each the text of one `warning` line of `report.txt`: `limiter
    /// engaged on <n> of <m> rendered samples` ([`warnings`], Da7-15).
    pub warnings: Vec<String>,
    /// The six files, in the order written.
    pub files: Vec<PathBuf>,
}

/// Why no results were written.
#[derive(Debug)]
pub enum ResultsError {
    /// The run has no results to write: [`FitError::FailedRunHasNoPatch`] or
    /// [`FitError::FitStillRunning`].
    Run(FitError),
    /// The file at `path` could not be made: its writer refused the value (the message names
    /// the error of `spec/`). Nothing was written.
    Encode { path: PathBuf, message: String },
    /// The directory or the file at `path` could not be created or written; everything this
    /// call created has been removed.
    Io { path: PathBuf, error: io::Error },
}

impl fmt::Display for ResultsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Run(error) => write!(f, "{}: {error}", error.domain_error()),
            Self::Encode { path, message } => write!(f, "cannot write {path:?}: {message}"),
            Self::Io { path, error } if error.kind() == io::ErrorKind::AlreadyExists => {
                write!(
                    f,
                    "cannot write {path:?}: already exists; it is never overwritten"
                )
            }
            Self::Io { path, error } => write!(f, "cannot write {path:?}: {error}"),
        }
    }
}

impl std::error::Error for ResultsError {}

/// Whether the results of `run` may be written, answered in the model's order (module
/// documentation): a Failed run is refused [`FitError::FailedRunHasNoPatch`], a Running one
/// [`FitError::FitStillRunning`].
pub fn admit_results<C>(run: &FitRun<C>) -> Result<(), FitError> {
    match run.state() {
        FitState::Failed => Err(FitError::FailedRunHasNoPatch),
        FitState::Running => Err(FitError::FitStillRunning),
        FitState::Completed => Ok(()),
    }
}

/// The report's warnings: `limiter engaged on <n> of <m> rendered samples` when the protective
/// limiter changed samples of the best patch's render, `<n>` the samples it changed and `<m>`
/// the frames of that render ([`FitOutcome::render_report`]: `limited_samples`, `frame_count`).
/// Both count the whole render the best patch was scored from, before it is cut or padded to the
/// reference's length; [`RECONSTRUCTED_WAV_FILE`] holds the cut clip, so it may hold fewer of the
/// limited samples (Da7-15). Then, for a seeded run whose prediction had outputs outside
/// [0, 1], `clamped <n> of 8 predictions`, `<n>` the outputs clamped (`Prediction::clamped`,
/// DESIGN.md § 5.2, Da11-11).
pub fn warnings(outcome: &FitOutcome) -> Vec<String> {
    let mut warnings = Vec::new();
    if outcome.render_report.limited {
        warnings.push(format!(
            "limiter engaged on {} of {} rendered samples",
            outcome.render_report.limited_samples, outcome.render_report.frame_count
        ));
    }
    if let Some(seeded) = &outcome.seeded
        && seeded.prediction.clamped > 0
    {
        warnings.push(format!(
            "clamped {} of {} predictions",
            seeded.prediction.clamped,
            seeded.prediction.values.len()
        ));
    }
    warnings
}

fn real(value: Option<f64>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| value.to_string())
}

/// The text of `report.txt` (module documentation).
pub fn report_text(outcome: &FitOutcome) -> String {
    let run = &outcome.run;
    let components = run.best_components();
    let mut records = vec![
        (REPORT_HEADER, REPORT_FORMAT_VERSION.to_string()),
        ("patchback", PATCHBACK_VERSION.to_owned()),
        ("initial_loss", real(run.initial_loss())),
        ("best_loss", real(run.best_loss())),
        ("log_spectral", real(components.map(|c| c.log_spectral))),
        ("envelope", real(components.map(|c| c.envelope))),
        ("transient", real(components.map(|c| c.transient))),
        ("band_energy", real(components.map(|c| c.band_energy))),
        ("evaluations", run.evaluations().to_string()),
        ("restarts", outcome.restarts.to_string()),
        (
            "stop_reason",
            run.stop_reason()
                .map_or("none", |reason| reason.name())
                .to_owned(),
        ),
        (
            "wall_time_ms",
            run.wall_time_ms()
                .map_or_else(|| "none".to_owned(), |ms| ms.to_string()),
        ),
    ];
    records.extend(warnings(outcome).into_iter().map(|w| ("warning", w)));
    lines(&records)
}

/// The text of `run.meta` (module documentation).
pub fn run_meta_text(outcome: &FitOutcome, meta: &RunMeta) -> String {
    let bounds = outcome.run.bounds();
    let digest = |bytes: &[u8; digest::DIGEST_BYTES]| format!("{} {}", digest::NAME, hex(bytes));
    let mut records = vec![
        (RUN_META_HEADER, RUN_META_FORMAT_VERSION.to_string()),
        ("patchback", PATCHBACK_VERSION.to_owned()),
        ("init", outcome.init.record().to_owned()),
    ];
    if let Some(seeded) = &outcome.seeded {
        records.extend([
            (
                "weights",
                format!("{} {}", digest::NAME, seeded.weights_sha256),
            ),
            (
                "weights_modelspec",
                format!("{} {}", seeded.modelspec_version, seeded.modelspec_commit),
            ),
            ("normalization", digest(&seeded.normalization_sha256)),
        ]);
    }
    records.extend([
        ("seed", meta.seed.to_string()),
        ("topology", outcome.patch.topology_id.clone()),
        (
            "feature_contract",
            patchback_analysis::FEATURES_1_VERSION.to_owned(),
        ),
        ("prng", patchback_core::prng::NAME.to_owned()),
        ("modelspec", meta.modelspec_version.clone()),
        ("reference", digest(&meta.reference_sha256)),
        ("notes", digest(&meta.notes_sha256)),
        ("max_renders", bounds.max_renders.to_string()),
        ("max_wall_time_ms", bounds.max_wall_time_ms.to_string()),
        ("max_restarts", bounds.max_restarts.to_string()),
        (
            "improvement_threshold",
            bounds.improvement_threshold.to_string(),
        ),
        ("block_size", meta.block_size_frames.to_string()),
    ]);
    lines(&records)
}

fn lines(records: &[(&str, String)]) -> String {
    let mut text = String::new();
    for (key, value) in records {
        text.push_str(key);
        text.push(' ');
        text.push_str(value);
        text.push('\n');
    }
    text
}

/// Whether `value` is one record field: not empty, no whitespace, no control character.
fn one_token(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Writes the results of `outcome`, a run driven by `notes`, into `out` (module documentation).
pub fn write_results(
    out: &Path,
    outcome: &FitOutcome,
    notes: &NoteSequence,
    meta: &RunMeta,
) -> Result<FitResults, ResultsError> {
    admit_results(&outcome.run).map_err(ResultsError::Run)?;
    let stop_reason = outcome
        .run
        .stop_reason()
        .ok_or(ResultsError::Run(FitError::FitStillRunning))?;
    let encoding = |name: &str, message: String| ResultsError::Encode {
        path: out.join(name),
        message,
    };
    let topology = known_topology(&outcome.patch.topology_id).ok_or_else(|| {
        let error = FitError::TopologyNotFound {
            topology: outcome.patch.topology_id.clone(),
        };
        encoding(TRACE_FILE, format!("{}: {error}", error.domain_error()))
    })?;
    if !one_token(&meta.modelspec_version) {
        return Err(encoding(
            RUN_META_FILE,
            format!(
                "the ModelSpec version {:?} is not one token",
                meta.modelspec_version
            ),
        ));
    }
    let patch = write_patch(&outcome.patch).map_err(|error| {
        encoding(
            PATCH_FILE,
            format!("{}: {error}", error.reason.spec_error()),
        )
    })?;
    let events = write_events(notes).map_err(|error| {
        encoding(
            NOTES_FILE,
            format!("{}: {error}", error.reason.spec_error()),
        )
    })?;
    let wav = encode(&outcome.render, WavSampleFormat::Float32).map_err(|error| {
        encoding(
            RECONSTRUCTED_WAV_FILE,
            format!("{}: {error}", error.domain_error()),
        )
    })?;
    let trace = write_trace(&topology, &outcome.log, stop_reason)
        .map_err(|error| encoding(TRACE_FILE, error.to_string()))?;
    let contents: [(&str, Vec<u8>); 6] = [
        (PATCH_FILE, patch.into_bytes()),
        (NOTES_FILE, events.into_bytes()),
        (RECONSTRUCTED_WAV_FILE, wav),
        (REPORT_FILE, report_text(outcome).into_bytes()),
        (RUN_META_FILE, run_meta_text(outcome, meta).into_bytes()),
        (TRACE_FILE, trace.into_bytes()),
    ];
    let mut created = Created::default();
    if let Err(error) = write_all(out, &contents, &mut created) {
        created.remove();
        return Err(error);
    }
    Ok(FitResults {
        out_dir: out.to_path_buf(),
        recovered_candidate: outcome.run.best().copied(),
        recovered_patch: outcome.patch.clone(),
        reconstructed_wav: out.join(RECONSTRUCTED_WAV_FILE),
        warnings: warnings(outcome),
        files: created.files,
    })
}

fn write_all(
    out: &Path,
    contents: &[(&str, Vec<u8>)],
    created: &mut Created,
) -> Result<(), ResultsError> {
    created.dir(out).map_err(|error| ResultsError::Io {
        path: out.to_path_buf(),
        error,
    })?;
    for (name, bytes) in contents {
        let path = out.join(name);
        created
            .file(&path, bytes)
            .map_err(|error| ResultsError::Io { path, error })?;
    }
    Ok(())
}

/// What one call created, to be removed when a later step fails.
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
            if path.as_os_str().is_empty() || fs::symlink_metadata(path).is_ok() {
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

    /// Creates `path`, which must not exist (a symbolic link there, dangling or not, exists),
    /// records it, and writes and syncs `bytes`.
    fn file(&mut self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        self.files.push(path.to_path_buf());
        file.write_all(bytes).and_then(|()| file.sync_all())
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
