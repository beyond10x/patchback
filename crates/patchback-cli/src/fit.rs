//! `patchback fit --input <wav> --notes <events> --out <dir> [--max-renders <n>]
//! [--max-wall-time-ms <ms>] [--max-restarts <n>] [--improvement-threshold <loss>] [--seed <n>]
//! [--block-size <frames>]` (DESIGN.md § 9.2, § 11, `StartFit` of `spec/domains/fit.yaml`): the
//! bounded search for a patch of `subtractive-mono/1` that reproduces a reference recording,
//! driven by its known notes (D-a6-1), with `patchback_fit::fit` (its module documentation
//! states the search and its stopping rule exactly). The run starts from the default patch.
//!
//! On success the best patch is written as `<out>/patch.patch`, the canonical text of
//! `patchback_core::patch::write_patch`; `<out>` is created when absent. Nothing else is
//! written yet (`notes.events`, `reconstructed.wav`, `report.txt`, `run.meta` and
//! `trace.events` are story:fit-run-artifacts). A refused or failed fit writes nothing, not even
//! the directory.
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
//!
//! A bound of 0 parses and is refused as `InvalidSearchBound`; a negative bound, a NaN, infinite
//! or negative threshold, a negative seed and a block size of 0 or less are usage errors (2). A
//! bound above `i64::MAX` is taken as `i64::MAX`.
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
//! | `init` | `default`: the run started from the default patch |
//! | `evaluations` | the render evaluations counted, valid and rejected |
//! | `restarts` | the restarts that began |
//! | `initial_loss` | the loss of the first valid candidate (the default patch's, when it rendered) |
//! | `best_loss` | the loss of the best patch, the one written |
//! | `log_spectral` | its multi-resolution log-spectral component |
//! | `envelope` | its short-time RMS envelope component |
//! | `transient` | its onset envelope component |
//! | `band_energy` | its band-energy distribution component |
//! | `stop_reason` | `RenderBudgetExhausted`, `TimeBudgetExhausted`, `RestartsExhausted` or `ImprovementBelowThreshold` |
//! | `wall_time_ms` | the run's wall time in milliseconds, from the fit's start (the reference analysis included) to its end; it may exceed `--max-wall-time-ms` by the reference analysis and one evaluation |
//! | `limited` | `true` when the protective output limiter changed a sample of the best patch's render, else `false` |
//!
//! # Refusals
//!
//! One line on stderr, exit status 1, nothing written, `patchback: fit refused: <error>:
//! <message>`, where `<error>` is an error of `spec/domains/fit.yaml` (or the analysis error the
//! comparison passes on). They are answered in the order `StartFit` declares its outcomes:
//!
//! 1. `patchback.fit.InvalidSearchBound`: `--max-renders`, `--max-wall-time-ms` or
//!    `--max-restarts` of 0, named `RenderEvaluations`, `WallTime` or `Restarts`.
//! 2. `patchback.fit.ClipNotFound`: an `--input` that cannot be read or that the WAV importer
//!    refuses (its `patchback.audio` error follows): no clip was imported.
//! 3. `patchback.fit.NoteSequenceNotFound`: a `--notes` file that cannot be read or that the
//!    parser refuses (its `patchback.score` error follows).
//! 4. `patchback.fit.ReferenceSilent`, `patchback.fit.PitchAmbiguous`: the reference's analysis
//!    finds it silent, or cannot estimate its fundamental; any other analysis refusal of the
//!    reference is its `patchback.analysis` error (a clip past the 60 s analysis bound is
//!    `patchback.analysis.DurationOverBound`).
//! 5. `patchback.fit.NoValidCandidate`: the run ended without a valid candidate (every render or
//!    comparison refused, for example a note sequence at a rate other than 48 kHz) and failed.
//!
//! The topology is always `subtractive-mono/1`, so `TopologyNotFound` cannot be reached here.
//! The output is not part of the model. Its refusals read `patchback: fit refused: cannot write
//! "<path>": <message>`. Before the search, after `--notes` and before silence and pitch (the
//! order bounds, `--input`, `--notes`, `<out>`, silence and pitch, the search):
//!
//! - an `--out` that exists and is not a directory, or the nearest existing ancestor of an
//!   absent `--out` that is not one, a symbolic link counting as a directory only when it
//!   resolves to one (a dangling link is not): `cannot write "<that path>": is not a
//!   directory`, the file unchanged;
//! - an existing `<out>/patch.patch`: `cannot write "<out>/patch.patch": already exists; it is
//!   never overwritten`, the file unchanged.
//!
//! After the search, a directory or file that still cannot be created or written (permissions,
//! a full disk) is refused the same way; a partial `patch.patch` is removed.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Args;
use patchback_audio::{AudioClip, ReadError, read_wav};
use patchback_core::parameter::Topology;
use patchback_core::patch::write_patch;
use patchback_core::score::{NoteSequence, ReadEventsError, read_events_file};
use patchback_dsp::RenderConfig;
use patchback_fit::{
    DEFAULT_IMPROVEMENT_THRESHOLD, DEFAULT_MAX_RENDERS, DEFAULT_MAX_RESTARTS,
    DEFAULT_MAX_WALL_TIME_MS, FitBounds, FitConfig, FitError, FitOutcome, fit,
};

/// The command ran and refused; it wrote nothing.
const REFUSED: u8 = 1;

/// The name of the patch file in the results directory (DESIGN.md § 11).
const PATCH_FILE: &str = "patch.patch";

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

/// Fits and writes `patch.patch`; on success returns the completed fit.
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
    // 2. The reference, 3. the note sequence.
    let reference = read_reference(&args.input)?;
    let notes = read_notes(&args.notes)?;
    // `<out>`: a directory the patch can be created in, as far as can be known without writing.
    if let Some(blocking) = non_directory_on_the_way(&args.out) {
        return Err(Refusal::io(blocking, "is not a directory"));
    }
    let patch_path = args.out.join(PATCH_FILE);
    if patch_path.exists() {
        return Err(Refusal::io(
            &patch_path,
            "already exists; it is never overwritten",
        ));
    }
    // 4. Silence and pitch, then the search; 5. a failed run.
    let outcome = fit(&reference, &notes, &Topology::subtractive_mono(), &config)?;
    let text = write_patch(&outcome.patch).map_err(|error| {
        Refusal::io(
            &patch_path,
            format_args!("{}: {error}", error.reason.spec_error()),
        )
    })?;
    fs::create_dir_all(&args.out).map_err(|error| Refusal::io(&args.out, error))?;
    write_new(&patch_path, text.as_bytes()).map_err(|error| Refusal::io(&patch_path, error))?;
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

fn read_reference(path: &Path) -> Result<AudioClip, Refusal> {
    read_wav(path).map_err(|error| {
        Refusal::domain(
            "patchback.fit.ClipNotFound",
            match &error {
                ReadError::Io(io) => format!("no clip at {path:?}: {io}"),
                ReadError::Wav(wav) => format!("{path:?}: {}: {wav}", wav.domain_error()),
            },
        )
    })
}

fn read_notes(path: &Path) -> Result<NoteSequence, Refusal> {
    read_events_file(path).map_err(|error| {
        Refusal::domain(
            "patchback.fit.NoteSequenceNotFound",
            match &error {
                ReadEventsError::Io(io) => format!("no note sequence at {path:?}: {io}"),
                ReadEventsError::Format(format) => {
                    format!("{path:?}: {}: {format}", format.reason.spec_error())
                }
            },
        )
    })
}

/// Writes `bytes` to a file that must not exist yet; a failed write removes the partial file.
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if written.is_err() {
        let _ = fs::remove_file(path);
    }
    written
}

/// The stdout lines of a completed fit (module documentation).
pub fn report(out: &Path, outcome: &FitOutcome) -> String {
    let run = &outcome.run;
    let real = |value: Option<f64>| value.map_or_else(|| "none".to_owned(), |v| v.to_string());
    let components = run.best_components();
    let mut text = format!("out {out:?}\ninit default\n");
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
    text
}
