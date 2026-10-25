//! `patchback fit --input <wav> --notes <events> --out <dir> [--max-renders <n>]
//! [--max-wall-time-ms <ms>] [--max-restarts <n>] [--improvement-threshold <loss>] [--seed <n>]
//! [--block-size <frames>] [--weights <path> --normalization <path>]` (DESIGN.md § 9.2, § 11,
//! `StartFit` of `spec/domains/fit.yaml`): the bounded search for a patch of
//! `subtractive-mono/1` that reproduces a reference recording, driven by its known notes
//! (D-a6-1), with `patchback_fit::fit` (its module documentation states the search and its
//! stopping rule exactly). The run starts from the default patch; with `--weights` and
//! `--normalization` its first restart starts from the regressor's prediction for the
//! reference's first note instead, through `patchback_fit::fit_request`
//! (story:regressor-seeded-fit, Da11-2), and the other restarts are unchanged.
//!
//! On success the run's six files (DESIGN.md § 11) are written into `<out>` by
//! `patchback_fit::write_results`, whose module documentation states each file's records;
//! `<out>` and its missing parents are created when absent:
//!
//! | File | Contents |
//! |---|---|
//! | `patch.patch` | the best patch, the canonical text of `patchback_core::patch::write_patch` |
//! | `notes.events` | the `--notes` sequence in its canonical form |
//! | `reconstructed.wav` | the clip the best patch was scored on, mono float32 48 kHz: `patchback compare --reference <input> --candidate <out>/reconstructed.wav` prints the `best_loss` below as its `loss` |
//! | `report.txt` | `patchback-fit-report 1`: the version, the values printed below but `out`, `init` and `limited`, and one `warning` line per warning (`limiter engaged on <n> of <m> rendered samples` when the limiter changed `<n>` of the `<m>` samples of the best patch's uncut render; then `clamped <n> of 8 predictions` when `<n>` of a seeded run's eight predicted values were outside [0, 1]) |
//! | `run.meta` | `patchback-fit-run-meta 1`: the version, `init`, on a seeded run the artifact's payload SHA-256 (`weights sha256`), its ModelSpec version and commit (`weights_modelspec`) and the SHA-256 of the `--normalization` bytes as read (`normalization sha256`), then the seed, topology, feature contract, generator, the ModelSpec pin's version, the SHA-256 of the `--input` and `--notes` bytes as read, the bounds and the block size |
//! | `trace.events` | `patchback-fit-trace 1`: one line per evaluation, in order (`patchback_fit::trace`) |
//!
//! The `--input` and `--notes` files are each read once, into memory: the digest is of the bytes
//! the run used. `--input` is read up to [`MAX_INPUT_BYTES`]. A refused or failed fit writes
//! nothing, not even the directory.
//!
//! # Flags
//!
//! | Flag | Value | Default |
//! |---|---|---|
//! | `--input` | the reference WAV: mono or stereo, PCM16 or float32, 48 kHz | required |
//! | `--notes` | the `notes.events` file of its known notes, at 48 kHz | required |
//! | `--out` | the results directory | required |
//! | `--max-renders` | the most render evaluations, an integer of at least 0 | 2000 |
//! | `--max-wall-time-ms` | the wall time after which no render starts but the run's first, in ms, at least 0 | 60000 |
//! | `--max-restarts` | the restarts of the coordinate search, at least 0 | 4 |
//! | `--improvement-threshold` | a finite, non-negative loss | 0.0001 |
//! | `--seed` | the seed of the restarts' starting points, 0 to 2^64 - 1 | 0 |
//! | `--block-size` | frames per render block, at least 1 | 256 |
//! | `--weights` | the regressor's `weights/1` artifact; requires `--normalization` | none: the default start |
//! | `--normalization` | the `normalization.stats` file the artifact was trained with, read up to [`MAX_STATS_BYTES`]; requires `--weights` | none: the default start |
//!
//! A bound of 0 parses and is refused as `InvalidSearchBound`; a negative bound, a NaN, infinite
//! or negative threshold, a negative seed, a block size of 0 or less, and `--weights` or
//! `--normalization` without the other are usage errors (2). A bound above `i64::MAX` is taken
//! as `i64::MAX`.
//!
//! # Output
//!
//! UTF-8 text on stdout, one `key value` line per record, each ending in `\n`, in this order.
//! Reals are printed as Rust prints an `f64` (the shortest form that reads back to the same
//! value).
//!
//! | Key | Value |
//! |---|---|
//! | `out` | the results directory, as given, quoted and escaped as a refusal quotes a path (`Path`'s `Debug`), for example `out "results"` |
//! | `init` | `default`: the run started from the default patch; `regressor-prediction`: its first restart started from the regressor's prediction |
//! | `evaluations` | the render evaluations counted, valid and rejected |
//! | `restarts` | the restarts that began |
//! | `initial_loss` | the loss of the first valid candidate (the default patch's, when it rendered) |
//! | `best_loss` | the loss of the best patch, the one written |
//! | `log_spectral` | its multi-resolution log-spectral component |
//! | `envelope` | its short-time RMS envelope component |
//! | `transient` | its onset envelope component |
//! | `band_energy` | its band-energy distribution component |
//! | `stop_reason` | `RenderBudgetExhausted`, `TimeBudgetExhausted`, `RestartsExhausted` or `ImprovementBelowThreshold` |
//! | `wall_time_ms` | the run's wall time in milliseconds, from the fit's start to its end; it may exceed `--max-wall-time-ms` by what the fit's clock holds before the search and one evaluation: on a default run the reference analysis; on a seeded run the first note's analysis and the prediction, while the CLI's reference analysis is made before the clock starts and is outside it (Da11-23) |
//! | `limited` | `true` when the protective output limiter changed a sample of the best patch's uncut render, else `false`; `reconstructed.wav` is that render cut or padded to the reference's length, so it can hold fewer of the limited samples, or none |
//! | `warning` | only on a seeded run whose prediction had `<n>` of its eight values outside [0, 1]: `warning clamped <n> of 8 predictions`, the last line (Da11-11) |
//!
//! # Refusals
//!
//! One line on stderr, exit status 1, nothing written, `patchback: fit refused: <error>:
//! <message>`, where `<error>` is an error of `spec/domains/fit.yaml` (or the analysis error the
//! comparison passes on). They are answered in the order `StartFit` declares its outcomes:
//!
//! 1. `patchback.fit.InvalidSearchBound`: `--max-renders`, `--max-wall-time-ms` or
//!    `--max-restarts` of 0, named `RenderEvaluations`, `WallTime` or `Restarts`.
//! 2. `patchback.fit.ClipNotFound`: an `--input` that cannot be read, that is larger than
//!    [`MAX_INPUT_BYTES`], or that the WAV importer refuses (its `patchback.audio` error
//!    follows): no clip was imported.
//! 3. `patchback.fit.NoteSequenceNotFound`: a `--notes` file that cannot be read or that the
//!    parser refuses (its `patchback.score` error follows).
//! 4. On a seeded run, the inputs as files (Da11-6): `patchback.model.UnknownWeightArtifact`
//!    for a `--weights` path with no file or one that cannot be read;
//!    `patchback.model.CorruptWeightArtifact` for a file `weights/1` refuses (its reason
//!    follows); `patchback.analysis.NormalizationStatsNotFound` for a `--normalization` file
//!    that is absent, cannot be read, is larger than [`MAX_STATS_BYTES`], is not UTF-8 or that
//!    `patchback_analysis::read_stats` refuses (its reason follows).
//! 5. On a seeded run, `patchback.fit.FeatureContractMismatch`: the artifact's feature contract,
//!    then the statistics', is not `features/1`, the analysis's; answered before silence and
//!    pitch (Da11-5).
//! 6. `patchback.fit.ReferenceSilent`, `patchback.fit.PitchAmbiguous`: the reference's analysis
//!    finds it silent, or cannot estimate its fundamental; any other analysis refusal of the
//!    reference is its `patchback.analysis` error (a clip past the 60 s analysis bound is
//!    `patchback.analysis.DurationOverBound`).
//! 7. `patchback.fit.NoValidCandidate`: the run ended without a valid candidate (every render or
//!    comparison refused, for example a note sequence at a rate other than 48 kHz) and failed;
//!    on a seeded run also a prediction that gave no start (a normalized feature or an output
//!    that is not finite), the predictor's error in the message (Da11-12), and a first note
//!    whose own clip, from its start to the end of its release window, the analysis refuses,
//!    the analysis crate's error in the message: a first note that reaches the recording's end
//!    has no release window, so its release features cannot be computed
//!    (`patchback.analysis.FeatureNotComputable`, "holds no frame centre"; Da11-22), and a
//!    first-note clip that is silent or whose pitch is ambiguous is refused so too (Da11-26).
//!    None of these refusals is the fit's without `--weights` and `--normalization`, which
//!    analyses the whole reference (step 6), not the first note's clip.
//!
//! The topology is always `subtractive-mono/1`, so `TopologyNotFound` cannot be reached here.
//! The CLI analyses the reference itself on a seeded run and passes the analysis in, so
//! `AnalysisRequired` cannot be reached here either.
//! `WriteFitResults` is answered by `patchback_fit::write_results` after a completed run only, so
//! its `FailedRunHasNoPatch` and `FitStillRunning` cannot be reached here either (a failed run is
//! refused with its reason above, before anything is written), nor its `FitRunNotFound` (the
//! run is a value).
//!
//! The output is not part of the model. Its refusals read `patchback: fit refused: cannot write
//! "<path>": <message>`. Before the search, after `--notes` and the seeded run's files and
//! before the feature contract, silence and pitch (the order bounds, `--input`, `--notes`,
//! `--weights`, `--normalization`, `<out>`, the ModelSpec pin, the feature contract, silence and
//! pitch, the search):
//!
//! - an `--out` that exists and is not a directory, or the nearest existing ancestor of an
//!   absent `--out` that is not one, a symbolic link counting as a directory only when it
//!   resolves to one (a dangling link is not): `cannot write "<that path>": is not a
//!   directory`, the file unchanged;
//! - any of the six files existing in `<out>`, a symbolic link there counting whether it resolves
//!   or not, the first in the order `patch.patch`, `notes.events`, `reconstructed.wav`,
//!   `report.txt`, `run.meta`, `trace.events`: `cannot write "<out>/<file>": already exists; it
//!   is never overwritten`, every file unchanged (Da7-6).
//!
//! Then the compiled-in ModelSpec pin (`models/modelspec.pin`, `patchback_model::ModelSpecPin`)
//! is read for `run.meta`; a pin that does not parse is refused `patchback: fit refused: the
//! ModelSpec pin is refused: <PinError>`. The repository's pin parses
//! (`crates/patchback-model/tests/pinned_binary.rs` and `tests/fit_results.rs` unwrap it), so
//! this refusal is not reachable from a binary built from the repository.
//!
//! After the search, a file the writers refuse, or a directory or file that cannot be created or
//! written (permissions, a full disk, a file created in `<out>` during the search), is refused
//! `cannot write "<path>": <message>`, and every file and directory the run created is removed.

use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Args;
use patchback_analysis::{AnalysisConfig, analyze_clip, read_stats};
use patchback_audio::{AudioClip, MAX_FRAMES, decode};
use patchback_core::digest::sha256;
use patchback_core::parameter::Topology;
use patchback_core::score::{
    MAX_FILE_BYTES as MAX_EVENTS_FILE_BYTES, NoteSequence, ReadEventsError, read_events,
};
use patchback_dsp::RenderConfig;
use patchback_fit::{
    DEFAULT_IMPROVEMENT_THRESHOLD, DEFAULT_MAX_RENDERS, DEFAULT_MAX_RESTARTS,
    DEFAULT_MAX_WALL_TIME_MS, FitBounds, FitConfig, FitError, FitOutcome, FitRequest, RESULT_FILES,
    ResultsError, RunMeta, SeedInputs, check_seed_inputs, fit, fit_request, reference_refusal,
    warnings, write_results,
};
use patchback_model::{ModelSpecPin, PredictError, Predictor};

/// The command ran and refused; it wrote nothing.
const REFUSED: u8 = 1;

/// The most bytes of `--input` a fit reads: room for the samples of the longest clip the WAV
/// importer admits (`patchback_audio::MAX_FRAMES`, ten minutes at 48 kHz) as stereo float32,
/// 8 bytes a frame, and 16 MiB for its headers and other chunks. The file is held in memory once,
/// so its digest is of the bytes the clip was decoded from. Provisional: a larger file is refused
/// `ClipNotFound` although its audio might be within the import bound; any reference a fit
/// analyses (at most 60 s) is far below it.
pub const MAX_INPUT_BYTES: u64 = MAX_FRAMES as u64 * 8 + (1 << 24);

/// The most bytes of `--normalization` a fit reads, 1 MiB. Provisional: a `features/1`
/// statistics text is 37 lines of at most `patchback_analysis::STATS_MAX_LINE_BYTES` each, far
/// below it; a larger file is refused `NormalizationStatsNotFound` without being read whole.
pub const MAX_STATS_BYTES: u64 = 1 << 20;

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
    /// The most renders the search may spend
    #[arg(long, default_value_t = DEFAULT_MAX_RENDERS as u64)]
    pub max_renders: u64,
    /// The wall time after which no render starts, in milliseconds
    #[arg(long, default_value_t = DEFAULT_MAX_WALL_TIME_MS as u64)]
    pub max_wall_time_ms: u64,
    /// The number of restarts of the coordinate search
    #[arg(long, default_value_t = DEFAULT_MAX_RESTARTS as u64)]
    pub max_restarts: u64,
    /// A restart that lowers the best loss by less than this ends the search
    #[arg(long, default_value_t = DEFAULT_IMPROVEMENT_THRESHOLD, value_parser = threshold)]
    pub improvement_threshold: f64,
    /// The seed of the restarts' starting points
    #[arg(long, default_value_t = 0)]
    pub seed: u64,
    /// Frames per render block, a positive integer
    #[arg(long, default_value_t = RenderConfig::default().block_size_frames,
          value_parser = clap::value_parser!(i64).range(1..))]
    pub block_size: i64,
    /// The regressor's weights/1 artifact: the run starts from its prediction; requires
    /// --normalization
    #[arg(long, requires = "normalization")]
    pub weights: Option<PathBuf>,
    /// The normalization.stats file the regressor was trained with; requires --weights
    #[arg(long, requires = "weights")]
    pub normalization: Option<PathBuf>,
}

/// A finite, non-negative decimal.
fn threshold(value: &str) -> Result<f64, String> {
    match value.parse::<f64>() {
        Ok(parsed) if parsed.is_finite() && parsed >= 0.0 => Ok(parsed),
        _ => Err(format!("{value:?} is not a finite, non-negative decimal")),
    }
}

/// Why `fit` wrote nothing: the one line it prints on stderr.
pub struct Refusal(String);

impl Refusal {
    pub fn line(&self) -> &str {
        &self.0
    }

    fn domain(error: &str, message: impl std::fmt::Display) -> Self {
        Self::one_line(format!("patchback: fit refused: {error}: {message}"))
    }

    fn io(path: &Path, error: impl std::fmt::Display) -> Self {
        Self::one_line(format!(
            "patchback: fit refused: cannot write {path:?}: {error}"
        ))
    }

    /// A refusal is one line: any control character left in a message is escaped. Paths are
    /// quoted with `Path`'s `Debug`, which escapes them too.
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

impl From<FitError> for Refusal {
    fn from(error: FitError) -> Self {
        Self::domain(error.domain_error(), error)
    }
}

/// Runs the verb: the report on stdout and exit 0, or one refusal line on stderr and exit 1.
pub fn run(args: &FitArgs) -> ExitCode {
    match fit_and_write(args) {
        Ok(outcome) => {
            print!("{}", report(&args.out, &outcome));
            ExitCode::SUCCESS
        }
        Err(refusal) => {
            eprintln!("{}", refusal.line());
            ExitCode::from(REFUSED)
        }
    }
}

fn saturating(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Fits and writes the six files of the run; on success returns the completed fit.
pub fn fit_and_write(args: &FitArgs) -> Result<FitOutcome, Refusal> {
    let config = FitConfig {
        bounds: FitBounds {
            max_renders: saturating(args.max_renders),
            max_wall_time_ms: saturating(args.max_wall_time_ms),
            max_restarts: saturating(args.max_restarts),
            improvement_threshold: args.improvement_threshold,
        },
        seed: args.seed,
        render: RenderConfig {
            block_size_frames: args.block_size,
        },
    };
    // 1. The bounds, before any input is read.
    config.bounds.check()?;
    // 2. The reference, 3. the note sequence, each with the digest of its bytes.
    let (reference, reference_sha256) = read_reference(&args.input)?;
    let (notes, notes_sha256) = read_notes(&args.notes)?;
    // The weight artifact and its statistics, when given (each requires the other).
    let seed = match (&args.weights, &args.normalization) {
        (Some(weights), Some(normalization)) => Some(read_seed_inputs(weights, normalization)?),
        _ => None,
    };
    // `<out>`: a directory the files can be created in, as far as can be known without writing.
    if let Some(blocking) = non_directory_on_the_way(&args.out) {
        return Err(Refusal::io(blocking, "is not a directory"));
    }
    for name in RESULT_FILES {
        let path = args.out.join(name);
        if fs::symlink_metadata(&path).is_ok() {
            return Err(Refusal::io(
                &path,
                "already exists; it is never overwritten",
            ));
        }
    }
    let pin = ModelSpecPin::repository().map_err(|error| {
        Refusal::one_line(format!(
            "patchback: fit refused: the ModelSpec pin is refused: {error}"
        ))
    })?;
    // 4. Silence and pitch, then the search; 5. a failed run. A seeded fit answers the inputs'
    // feature contract first, then analyses the reference once and passes the analysis in.
    let topology = Topology::subtractive_mono();
    let outcome = match seed {
        None => fit(&reference, &notes, &topology, &config)?,
        Some(seed) => {
            check_seed_inputs(&seed)?;
            let analysis =
                analyze_clip(&reference, &AnalysisConfig::default()).map_err(reference_refusal)?;
            fit_request(FitRequest {
                reference: &reference,
                notes: &notes,
                topology: &topology,
                config: &config,
                analysis: Some(&analysis),
                seed: Some(seed),
            })?
        }
    };
    let meta = RunMeta {
        seed: args.seed,
        block_size_frames: args.block_size,
        modelspec_version: pin.version().to_owned(),
        reference_sha256,
        notes_sha256,
    };
    write_results(&args.out, &outcome, &notes, &meta).map_err(|error| match error {
        ResultsError::Run(error) => Refusal::from(error),
        error => Refusal::one_line(format!("patchback: fit refused: {error}")),
    })?;
    Ok(outcome)
}

/// The nearest of `out` and its ancestors that exists in any form, a symbolic link included
/// (`fs::symlink_metadata`), when it is not a directory: `out` could then never be created or
/// written as one. A link counts as a directory only when it resolves to one, so a dangling
/// link or a link to a file is not a directory (Da6-30). `None` when it is a directory or
/// nothing exists.
fn non_directory_on_the_way(out: &Path) -> Option<&Path> {
    out.ancestors()
        .filter(|path| !path.as_os_str().is_empty())
        .find(|path| fs::symlink_metadata(path).is_ok())
        .filter(|path| !path.is_dir())
}

/// Reads the file at `path`, at most `limit + 1` bytes of it, so a longer file is seen to be
/// longer without being read whole.
fn read_bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The reference clip and the SHA-256 of the bytes it was decoded from.
fn read_reference(path: &Path) -> Result<(AudioClip, [u8; 32]), Refusal> {
    let refused = |message: String| Refusal::domain("patchback.fit.ClipNotFound", message);
    let bytes = read_bounded(path, MAX_INPUT_BYTES)
        .map_err(|io| refused(format!("no clip at {path:?}: {io}")))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_INPUT_BYTES {
        return Err(refused(format!(
            "{path:?}: the file is larger than the {MAX_INPUT_BYTES} bytes a fit reads"
        )));
    }
    let clip = decode(&bytes)
        .map_err(|wav| refused(format!("{path:?}: {}: {wav}", wav.domain_error())))?;
    Ok((clip, sha256(&bytes)))
}

/// The note sequence and the SHA-256 of the bytes it was parsed from.
fn read_notes(path: &Path) -> Result<(NoteSequence, [u8; 32]), Refusal> {
    let refused = |message: String| Refusal::domain("patchback.fit.NoteSequenceNotFound", message);
    let bytes = read_bounded(path, MAX_EVENTS_FILE_BYTES as u64)
        .map_err(|io| refused(format!("no note sequence at {path:?}: {io}")))?;
    let notes = read_events(bytes.as_slice()).map_err(|error| {
        refused(match &error {
            ReadEventsError::Io(io) => format!("no note sequence at {path:?}: {io}"),
            ReadEventsError::Format(format) => {
                format!("{path:?}: {}: {format}", format.reason.spec_error())
            }
        })
    })?;
    Ok((notes, sha256(&bytes)))
}

/// The predictor of the `--weights` artifact and the `--normalization` statistics with the
/// SHA-256 of their bytes (Da11-6). A weights file that is absent or cannot be read is
/// `patchback.model.UnknownWeightArtifact`, one that `weights/1` refuses
/// `patchback.model.CorruptWeightArtifact`; a statistics file that is absent, cannot be read,
/// is larger than [`MAX_STATS_BYTES`], is not UTF-8 or that `read_stats` refuses is
/// `patchback.analysis.NormalizationStatsNotFound`.
fn read_seed_inputs(weights: &Path, normalization: &Path) -> Result<SeedInputs, Refusal> {
    let predictor = Predictor::load(weights).map_err(|error| match error {
        PredictError::UnknownWeightArtifact { .. } => Refusal::domain(
            "patchback.model.UnknownWeightArtifact",
            format!("no weight artifact at {weights:?}"),
        ),
        PredictError::Io { error, .. } => Refusal::domain(
            "patchback.model.UnknownWeightArtifact",
            format!("no weight artifact at {weights:?}: {error}"),
        ),
        error => Refusal::domain(
            error
                .domain_error()
                .unwrap_or("patchback.model.CorruptWeightArtifact"),
            format!("{weights:?}: {error}"),
        ),
    })?;
    let refused =
        |message: String| Refusal::domain("patchback.analysis.NormalizationStatsNotFound", message);
    let bytes = read_bounded(normalization, MAX_STATS_BYTES).map_err(|io| {
        refused(format!(
            "no normalization statistics at {normalization:?}: {io}"
        ))
    })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_STATS_BYTES {
        return Err(refused(format!(
            "{normalization:?}: the file is larger than the {MAX_STATS_BYTES} bytes a fit reads"
        )));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| refused(format!("{normalization:?}: not UTF-8: {error}")))?;
    let table = read_stats(text).map_err(|error| refused(format!("{normalization:?}: {error}")))?;
    Ok(SeedInputs {
        predictor,
        normalization: table,
        normalization_sha256: sha256(&bytes),
    })
}

/// The stdout lines of a completed fit (module documentation).
pub fn report(out: &Path, outcome: &FitOutcome) -> String {
    let run = &outcome.run;
    let real = |value: Option<f64>| value.map_or_else(|| "none".to_owned(), |v| v.to_string());
    let components = run.best_components();
    let mut text = format!("out {out:?}\ninit {}\n", outcome.init.record());
    text.push_str(&format!("evaluations {}\n", run.evaluations()));
    text.push_str(&format!("restarts {}\n", outcome.restarts));
    text.push_str(&format!("initial_loss {}\n", real(run.initial_loss())));
    text.push_str(&format!("best_loss {}\n", real(run.best_loss())));
    for (key, value) in [
        ("log_spectral", components.map(|c| c.log_spectral)),
        ("envelope", components.map(|c| c.envelope)),
        ("transient", components.map(|c| c.transient)),
        ("band_energy", components.map(|c| c.band_energy)),
    ] {
        text.push_str(&format!("{key} {}\n", real(value)));
    }
    text.push_str(&format!(
        "stop_reason {}\n",
        run.stop_reason().map_or("none", |reason| reason.name())
    ));
    text.push_str(&format!(
        "wall_time_ms {}\n",
        run.wall_time_ms().unwrap_or(0)
    ));
    text.push_str(&format!("limited {}\n", outcome.render_report.limited));
    for warning in warnings(outcome)
        .into_iter()
        .filter(|warning| warning.starts_with("clamped "))
    {
        text.push_str(&format!("warning {warning}\n"));
    }
    text
}
