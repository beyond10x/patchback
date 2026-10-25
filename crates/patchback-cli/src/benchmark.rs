//! `patchback benchmark --out <dir> --examples <n> [--seed <n>] [--topology <name>]
//! [--max-renders <n>] [--max-wall-time-ms <ms>] [--max-restarts <n>]
//! [--improvement-threshold <loss>] [--block-size <frames>] [--kind engine-baseline|ml-init]
//! [--weights <path> --normalization <path>]` (story:engine-baseline, story:ml-init-benchmark,
//! DESIGN.md § 9.2, § 12.2, § 13 Milestones A and B; `StartBenchmark`, `RecordBaselineResult`
//! and `RecordMlInitResult` of `spec/domains/fit.yaml`).
//!
//! The engine baseline (`--kind engine-baseline`, the default): over a seeded holdout of
//! reference clips Patchback synthesizes itself (approval-record:engine-baseline-holdout: not a
//! dataset split), the median loss of the fixed default patch is compared with the median best
//! loss of `patchback_fit::fit` started from it, and the benchmark records whether the go/no-go
//! target (a reduction of at least 20 %) was met, with the evaluations, the wall time and
//! listening examples.
//!
//! The ML-init comparison (`--kind ml-init`, with `--weights` and `--normalization`): over the
//! same holdout, four arms per clip (§ 13 B "default vs ML-init vs ML-init-plus-search", § 9.2
//! "benchmarked against both the default patch and the raw regressor prediction", § 12.2
//! "raw-model-only result is also reported"): the default patch, the fit from it, the
//! regressor's raw prediction and the regressor-seeded fit (`patchback_fit::fit_request`); their
//! four median losses and both searches' median evaluations are recorded, with the inputs that
//! reproduce them. No reduction and no verdict are recorded: § 13 B's exit sets no numeric
//! target, and reading it from the records is the owner's (Da12-9).
//!
//! The model is `patchback_fit::Benchmark`; this verb draws the holdout, runs the fits and
//! writes the files (Da8-1).
//!
//! # Flags
//!
//! | Flag | Value | Default |
//! |---|---|---|
//! | `--out` | the benchmark directory; created when absent | required |
//! | `--examples` | the holdout's clip count (`example_count`), 0 to 2^64 - 1; 0 is refused `InvalidHoldoutSize` | required |
//! | `--seed` | the holdout's seed, 0 to 2^64 - 1 | 0 |
//! | `--topology` | the topology the holdout is synthesized and fitted in, by name | `subtractive-mono/1` |
//! | `--max-renders`, `--max-wall-time-ms`, `--max-restarts`, `--improvement-threshold`, `--block-size` | every clip's fit bounds and render block size, as `patchback fit` takes them; both searches of an ML-init benchmark take the same | 2000, 60000, 4, 0.0001, 256 |
//! | `--kind` | `engine-baseline` or `ml-init` (`BenchmarkKind`) | `engine-baseline` |
//! | `--weights` | the regressor's `weights/1` artifact, as `patchback fit` takes it; requires `--normalization`; `--kind ml-init` only | none |
//! | `--normalization` | the `normalization.stats` file the artifact was trained with, read up to `patchback fit`'s `MAX_STATS_BYTES`; requires `--weights`; `--kind ml-init` only | none |
//!
//! A count or bound above `i64::MAX` is taken as `i64::MAX`. A negative or non-integer count,
//! seed or bound, a NaN, infinite or negative threshold, a block size below 1, a `--kind` other
//! than the two, `--weights` or `--normalization` without the other, and either with the engine
//! baseline (by default or named) are usage errors (2). `--kind ml-init` without them is not: it
//! reaches `StartBenchmark`, which refuses it `WeightArtifactRequired` (Da12-2).
//!
//! # The holdout (Da8-2, Da8-46)
//!
//! One stream, `SplitMix64::new(--seed ^ HOLDOUT_DOMAIN)` (`patchback_core::prng`), is read in
//! order; [`HOLDOUT_DOMAIN`] keeps it apart from `generate-dataset`'s stream of the same seed
//! and from a fit seeded with `--seed`. For clip `k = 0, 1, …`: the next output is a patch seed,
//! and its draw is
//! `patchback_dataset::PatchDraw::from_seed(patch_seed)` (eight uniform normalized parameters,
//! a MIDI note and a gate; one note from sample 0). The draw is classified by the dataset
//! generator's own rule, `patchback_dataset::Pipeline::run` (`crates/patchback-dataset/src/
//! generate.rs`, `Pipeline::run` and `Generator::run_with` step 2): a draw whose render is
//! refused for a diagnostic reason, whose render the protective limiter changed, or whose
//! `features/1` vector the contract refuses for a diagnostic reason (silence, noise, clipping)
//! is a diagnostic example; it is skipped and the next output is the next patch seed. Once a draw
//! is admitted, the next output is clip `k`'s fit seed. Without diagnostic draws the stream is so
//! `patch_seed_0, fit_seed_0, patch_seed_1, fit_seed_1, …`. As in the generator
//! (`MAX_DRAWS_PER_SAMPLE`, `Generator::new`), at most `4 × --examples` draws are made; a holdout
//! not drawn within them is refused.
//!
//! Clip `k`'s reference is its draw's patch (`PatchDraw::patch` in the topology) validated,
//! compiled and rendered against its notes (`PatchDraw::notes`) with the run's block size, whole:
//! the note, its release and `patchback_dsp::RENDER_TAIL_FRAMES`. The generator classifies at
//! the default block size; the block size never changes the samples (`patchback_dsp::render`).
//!
//! # The run
//!
//! 0. With `--weights` and `--normalization` (an ML-init benchmark), the inputs are read once,
//!    before anything else, as `patchback fit` reads them (Da11-6, Da12-3): the artifact
//!    (`patchback_model::Predictor::load`), then the statistics (`read_stats`), then both
//!    feature contracts against `features/1` (`patchback_fit::check_seed_inputs`). The artifact
//!    comes first because the start names it: `weight_artifact_id` is `sha256:` and the
//!    artifact's checksum (the SHA-256 of its payload, 64 lowercase hex digits). Each clip's
//!    seeded fit takes its own copy of what was read; no file is read again.
//! 1. `StartBenchmark` (`Benchmark::start`, the `--kind`): `InvalidHoldoutSize`, then
//!    `WeightArtifactRequired` for `--kind ml-init` without `--weights`, then `TopologyNotFound`
//!    for an unknown `--topology`.
//! 2. The fit bounds (`FitBounds::check`): `InvalidSearchBound` for a bound of 0.
//! 3. `<out>`: refused when it, or the nearest existing ancestor of an absent `<out>`, is not a
//!    directory; refused when `benchmark.txt`, `clips.txt` or `examples` exists in it (in that
//!    order; a symbolic link counts whether it resolves or not). Nothing is touched.
//! 4. The holdout is drawn (above).
//! 5. For each clip in order: its reference is rendered; the default arm's loss is
//!    `patchback_fit::default_arm` (the default patch's render cut or padded to the reference,
//!    compared with it; computed here, not read from the fit's log, Da8-5); the search arm is
//!    `patchback_fit::fit` of the reference with the clip's notes, the topology and the run's
//!    bounds and block size, seeded with the clip's fit seed: its best loss, its evaluations, its
//!    restarts and its stop reason. A clip whose reference or default arm cannot be rendered or
//!    compared, whose fit is refused or whose run fails ends the benchmark (Da8-3).
//!
//!    On an ML-init benchmark two more arms follow, in this order (Da12-4): the reference is
//!    analysed with the `features/1` configuration (`AnalysisConfig::default()`); arm 4, the
//!    regressor-seeded fit, is `patchback_fit::fit_request` with that analysis, the clip's copy
//!    of the inputs and the same bounds, block size and fit seed as arm 2, so its restarts 1 and
//!    later start where arm 2's do and only restart 0 differs (the prediction for the clip's
//!    note, from the note's own clip, Da11-21); arm 3, the raw regressor arm, is that seeded
//!    start (`SeededStart::point`, the prediction's values clamped to [0, 1]) scored once by
//!    `patchback_fit::predicted_arm`, the default arm's pattern at that point. A clip whose
//!    analysis is refused (Da12-7), whose seeded fit is refused or fails, or whose predicted
//!    patch cannot be rendered or compared ends the benchmark as Da8-3 does. The analysis
//!    refusal is a guard that no generated holdout reaches (Da12-15). The generator admits a
//!    draw only after analysing its render under the feature contract's configuration
//!    (`patchback_dataset::generate`, `analyze_and_extract`); `FeatureContract::analysis_config`
//!    builds that from `ANALYSIS_FFT_LENGTHS` with `fft_length / HOP_DIVISOR` hops, which is
//!    `AnalysisConfig::default()`; and a refused analysis is classed diagnostic and skipped
//!    (`DiagnosticCause::from_feature`). 40 of 40 admitted clips over 5 seeds were analysed and
//!    seeded-fitted without a refusal in the adversary's case
//!    `no_admitted_holdout_clip_reaches_the_analysis_or_seeded_fit_refusal`
//!    (`tests/adversary_benchmark_ml_init.rs`). For that reason the path has no end-to-end test.
//! 6. The default median and the search median are the lower medians of the per-clip losses
//!    (`patchback_fit::median`, Da8-4); the median evaluations the lower median of the per-clip
//!    counts; the wall time the whole run's so far, from the verb's start. On the engine
//!    baseline a default median of 0 is refused (no reduction is defined); the reduction is
//!    `patchback_fit::reduction_percent`; `RecordBaselineResult`
//!    (`Benchmark::record_baseline_result`) records them with the listening examples and gives
//!    the verdict: `Met` at a reduction of at least 20, else `NotMet`. Both complete the
//!    benchmark and exit 0 (Da8-11). On an ML-init benchmark the raw regressor median and the
//!    seeded median are the lower medians of arms 3 and 4, the seeded median evaluations the
//!    lower median of arm 4's counts; `RecordMlInitResult` (`Benchmark::record_ml_init_result`)
//!    records the four medians, both median evaluations, the wall time and the listening
//!    examples; no reduction is computed, so a default median of 0 is not refused.
//! 7. The files are written (below); a failed write removes every file and directory the run
//!    created.
//!
//! # Files (Da8-8)
//!
//! Written in this order, each created new, never overwritten: `<out>/clips.txt`; for each of
//! the first `patchback_fit::LISTENING_EXAMPLE_COUNT` (3) clips, fewer when the holdout is
//! smaller, `<out>/examples/<k>/reference.wav`, `default.wav` and `fit.wav` ([`EXAMPLE_FILES`]),
//! on an ML-init benchmark then `predicted.wav` and `seeded.wav` ([`ML_INIT_EXAMPLE_FILES`],
//! Da12-6); `<out>/benchmark.txt` last, so a `benchmark.txt` names a complete benchmark.
//!
//! The WAVs are mono float32 at 48 kHz: `reference.wav` is clip `k`'s reference, `default.wav` the
//! clip the default patch was scored on and `fit.wav` the clip the fit's best patch was scored on
//! (`FitOutcome::render`), `predicted.wav` the clip the raw prediction was scored on and
//! `seeded.wav` the clip the seeded fit's best patch was scored on, each cut or padded to the
//! reference, as `patchback fit`'s `reconstructed.wav` is. So `patchback compare --reference
//! examples/<k>/reference.wav --candidate examples/<k>/fit.wav` prints clip `k`'s best loss as
//! its `loss`; with `default.wav` its default loss, with `predicted.wav` its raw loss and with
//! `seeded.wav` its seeded best loss.
//!
//! `benchmark.txt` and `clips.txt` are line-oriented UTF-8 text like `run.meta`, each line ended
//! by `\n`, one space between fields. Reals are written as Rust prints an `f64` (the shortest text
//! that reads back to the same value). `wall_time_ms` is the one record that differs between two
//! runs of the same flags, unless a search stops on `TimeBudgetExhausted`: then that clip's
//! evaluations, losses, `fit.wav` and `seeded.wav` can differ as well, as the search module's
//! determinism note says. Otherwise every other byte of the three kinds of file is the same.
//!
//! `benchmark.txt`, one `<key> <value>` record per line, in this order; both kinds are format
//! version 1, the records a kind does not have absent from its file (Da12-5):
//!
//! | Record | Value |
//! |---|---|
//! | `patchback-benchmark` | the format version, `1` |
//! | `patchback` | the Patchback version |
//! | `kind` | `engine-baseline` or `ml-init` |
//! | `topology` | the topology's name |
//! | `seed` | `--seed` |
//! | `example_count` | `--examples` |
//! | `sample_rate` | `48000` |
//! | `block_size` | the frames per render block |
//! | `max_renders`, `max_wall_time_ms`, `max_restarts`, `improvement_threshold` | every fit's bounds |
//! | `prng` | `xoshiro256**`, the generator of the draws and of the restarts |
//! | `weights` | ML-init only: `sha256 <hex>`, the artifact's checksum (the SHA-256 of its payload) |
//! | `modelspec` | ML-init only: `<version> <commit>`, the ModelSpec release the artifact names |
//! | `normalization` | ML-init only: `sha256 <hex>`, the SHA-256 of the `--normalization` bytes as read |
//! | `feature_contract` | ML-init only: `features/1` |
//! | `default_median_loss` | the lower median of the default arm's losses |
//! | `default_search_median_loss` | the lower median of the fits' best losses |
//! | `raw_regressor_median_loss` | ML-init only: the lower median of the raw regressor arm's losses |
//! | `regressor_search_median_loss` | ML-init only: the lower median of the seeded fits' best losses |
//! | `median_loss_reduction_percent` | engine baseline only: `(default − search) / default × 100` of the two medians |
//! | `target_verdict` | engine baseline only: `met` at a reduction of at least 20, else `not-met` |
//! | `default_search_median_evaluations` | the lower median of the fits' render evaluations |
//! | `regressor_search_median_evaluations` | ML-init only: the lower median of the seeded fits' render evaluations |
//! | `clamped_predictions` | ML-init only: `<n> of <clips>`, the clips whose prediction had an output outside [0, 1] (`Prediction::clamped > 0`) of the holdout's clips |
//! | `wall_time_ms` | the whole run's wall time, from the verb's start to the result, in milliseconds |
//! | `listening_example` | zero or more, last: one per listening example, its path relative to `<out>`, in the order clip 0 `reference.wav`, `default.wav`, `fit.wav` (then `predicted.wav`, `seeded.wav` on ML-init), then clip 1, … |
//!
//! The version, sample rate, block size and bounds are the "fixed build and configuration" of
//! DESIGN.md § 12.2 that two compared runs must share; no device is recorded, the code has none
//! (Da8-9). On ML-init the inputs' records make the predictions reproducible, as `run.meta`'s do
//! for a seeded fit (Da11-10).
//!
//! `clips.txt`: the header line `patchback-benchmark-clips 1`; the line `columns index patch_seed
//! fit_seed default_loss best_loss evaluations restarts stop_reason`; then one line per clip, in
//! clip order, `clip` and those eight fields: the clip index from 0, its patch seed and fit seed,
//! the default arm's loss, the fit's best loss, its render evaluations, the restarts that began
//! and its stop reason (`RenderBudgetExhausted`, `TimeBudgetExhausted`, `RestartsExhausted` or
//! `ImprovementBelowThreshold`). On ML-init the columns line and every clip line go on with six
//! more: `raw_loss seeded_best_loss seeded_evaluations seeded_restarts seeded_stop_reason
//! clamped`, the raw regressor arm's loss, the seeded fit's best loss, render evaluations,
//! restarts that began and stop reason, and how many of the prediction's eight outputs were
//! clamped (0 to 8).
//!
//! # Output
//!
//! On success, exit 0 on either verdict and on ML-init, and on stdout the records of
//! `benchmark.txt` from `default_median_loss` on, in its order and form.
//!
//! # Refusals
//!
//! `--weights` or `--normalization` with the engine baseline is a usage error (2), reported by
//! clap as an argument conflict before anything is read. Otherwise one line on stderr, exit
//! status 1, nothing left written, `patchback: benchmark refused: …`:
//!
//! - on ML-init, the inputs, in step 0's order, as `patchback fit` refuses them:
//!   `patchback.model.UnknownWeightArtifact: …` for a `--weights` path with no file or one that
//!   cannot be read, `patchback.model.CorruptWeightArtifact: …` for a file `weights/1` refuses,
//!   `patchback.analysis.NormalizationStatsNotFound: …` for a `--normalization` file that is
//!   absent, unreadable, larger than `MAX_STATS_BYTES`, not UTF-8 or refused by `read_stats`,
//!   then `patchback.fit.FeatureContractMismatch: …` for an artifact, then statistics, of
//!   another feature contract than `features/1`;
//! - `patchback.fit.InvalidHoldoutSize: …`, `patchback.fit.WeightArtifactRequired: …` (the
//!   topology quoted; `--kind ml-init` without `--weights`), `patchback.fit.TopologyNotFound: …`
//!   (the name quoted), `patchback.fit.InvalidSearchBound: …`;
//! - `cannot write "<path>": <message>` for an `<out>` that is not a directory, an existing
//!   `benchmark.txt`, `clips.txt` or `examples`, or a file or directory that cannot be created or
//!   written;
//! - `holdout exhausted: …` when the draws reach their bound before the holdout is full; the
//!   model declares no error for it;
//! - `clip <k>: <error>: <message>` for clip `k` whose draw the generator's pipeline refuses for a
//!   reason that is no diagnostic, whose reference or default arm cannot be rendered or compared,
//!   whose fit is refused or whose run fails (`<error>` the error of `spec/` where there is one);
//!   on ML-init also a clip whose analysis is refused (`patchback.fit.ReferenceSilent`,
//!   `patchback.fit.PitchAmbiguous` or the analysis error, as `patchback fit` names it), whose
//!   seeded fit is refused or fails (a prediction that gives no start is
//!   `patchback.fit.NoValidCandidate`, Da11-12, Da11-22), or whose predicted patch cannot be
//!   rendered or compared (`the predicted patch's …`);
//! - on the engine baseline, `the default patch's median loss is 0: …`.

use std::fs;
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::error::ErrorKind;
use clap::{Args, ValueEnum};
use patchback_analysis::{AnalysisConfig, FEATURES_1_VERSION, analyze_clip, read_stats};
use patchback_audio::{AudioClip, WavSampleFormat, encode};
use patchback_core::digest::{hex, sha256};
use patchback_core::parameter::{SUBTRACTIVE_MONO, Topology};
use patchback_core::patch::{compile, known_topology, validate};
use patchback_core::prng::{NAME as PRNG_NAME, SplitMix64};
use patchback_dataset::{MAX_DRAWS_PER_SAMPLE, PatchDraw, Pipeline, Rendering};
use patchback_dsp::{RenderConfig, SAMPLE_RATE_HZ, render};
use patchback_fit::{
    BaselineResult, Benchmark, BenchmarkKind, DEFAULT_IMPROVEMENT_THRESHOLD, DEFAULT_MAX_RENDERS,
    DEFAULT_MAX_RESTARTS, DEFAULT_MAX_WALL_TIME_MS, FitBounds, FitConfig, FitError, FitRequest,
    LISTENING_EXAMPLE_COUNT, MlInitResult, PATCHBACK_VERSION, SeedInputs, TargetVerdict,
    check_seed_inputs, default_arm, fit, fit_request, median, median_count, predicted_arm,
    reduction_percent, reference_refusal,
};
use patchback_model::{PredictError, Predictor};

use crate::fit::MAX_STATS_BYTES;

/// The command ran and refused; it wrote nothing.
const REFUSED: u8 = 1;

/// The benchmark's record file.
pub const BENCHMARK_FILE: &str = "benchmark.txt";
/// The per-clip record file.
pub const CLIPS_FILE: &str = "clips.txt";
/// The directory of the listening examples.
pub const EXAMPLES_DIR: &str = "examples";
/// The listening example files of one clip of an engine baseline, in the order they are written
/// and recorded.
pub const EXAMPLE_FILES: [&str; 3] = ["reference.wav", "default.wav", "fit.wav"];
/// The listening example files of one clip of an ML-init benchmark, in the order they are
/// written and recorded (Da12-6): the engine baseline's three, then the raw regressor arm's
/// render and the regressor-seeded fit's. Provisional, as the count of clips is: DESIGN.md
/// § 12.2 names no listening examples.
pub const ML_INIT_EXAMPLE_FILES: [&str; 5] = [
    "reference.wav",
    "default.wav",
    "fit.wav",
    "predicted.wav",
    "seeded.wav",
];

/// The `columns` line of an engine baseline's `clips.txt`.
const BASELINE_COLUMNS: &str =
    "columns index patch_seed fit_seed default_loss best_loss evaluations restarts stop_reason";
/// The columns an ML-init benchmark's `clips.txt` appends to [`BASELINE_COLUMNS`].
const ML_INIT_COLUMNS: &str =
    "raw_loss seeded_best_loss seeded_evaluations seeded_restarts seeded_stop_reason clamped";

/// The first field of `benchmark.txt`'s header line.
pub const BENCHMARK_HEADER: &str = "patchback-benchmark";
/// The first field of `clips.txt`'s header line.
pub const CLIPS_HEADER: &str = "patchback-benchmark-clips";
/// The format version of both files.
pub const FORMAT_VERSION: u32 = 1;

/// The holdout stream's domain (Da8-46): the holdout reads `SplitMix64::new(--seed ^
/// HOLDOUT_DOMAIN)`, not `SplitMix64::new(--seed)`. `generate-dataset` draws its patch seeds from
/// `SplitMix64::new(--seed)`, so without it the same `--seed` would make holdout clip 0 training
/// sample 0 of that dataset; and a fit draws its restarts from its seed the same way
/// (`SplitMix64`, then `Xoshiro256StarStar`), so a fit seeded with `--seed` would start at clip
/// 0's own patch. The value is the ASCII bytes of `holdout1` read big-endian, a fixed odd
/// constant; changing it changes every holdout.
pub const HOLDOUT_DOMAIN: u64 = 0x686f_6c64_6f75_7431;

/// Which benchmark `--kind` selects (`BenchmarkKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Kind {
    /// The default patch against the search from it
    EngineBaseline,
    /// Also the regressor's raw prediction and the search from it; requires --weights and
    /// --normalization
    MlInit,
}

impl Kind {
    /// The model's kind.
    fn model(self) -> BenchmarkKind {
        match self {
            Self::EngineBaseline => BenchmarkKind::EngineBaseline,
            Self::MlInit => BenchmarkKind::MlInit,
        }
    }
}

/// The value of `benchmark.txt`'s `kind` record.
fn kind_record(kind: BenchmarkKind) -> &'static str {
    match kind {
        BenchmarkKind::EngineBaseline => "engine-baseline",
        BenchmarkKind::MlInit => "ml-init",
    }
}

/// The listening example files of one clip of a benchmark of `kind`.
fn example_files(kind: BenchmarkKind) -> &'static [&'static str] {
    match kind {
        BenchmarkKind::EngineBaseline => &EXAMPLE_FILES,
        BenchmarkKind::MlInit => &ML_INIT_EXAMPLE_FILES,
    }
}

/// The arguments of `patchback benchmark`.
#[derive(Args, Debug)]
pub struct BenchmarkArgs {
    /// The directory to write the benchmark into; created when absent
    #[arg(long)]
    pub out: PathBuf,
    /// The number of reference clips in the synthesized holdout
    #[arg(long)]
    pub examples: u64,
    /// The seed the holdout's patches and the fits' restarts are drawn from
    #[arg(long, default_value_t = 0)]
    pub seed: u64,
    /// The topology the holdout is synthesized and fitted in
    #[arg(long, default_value = SUBTRACTIVE_MONO)]
    pub topology: String,
    /// The most renders each clip's search may spend
    #[arg(long, default_value_t = DEFAULT_MAX_RENDERS as u64)]
    pub max_renders: u64,
    /// The wall time after which no render of a clip's search starts, in milliseconds
    #[arg(long, default_value_t = DEFAULT_MAX_WALL_TIME_MS as u64)]
    pub max_wall_time_ms: u64,
    /// The number of restarts of each clip's coordinate search
    #[arg(long, default_value_t = DEFAULT_MAX_RESTARTS as u64)]
    pub max_restarts: u64,
    /// A restart that lowers the best loss by less than this ends a clip's search
    #[arg(long, default_value_t = DEFAULT_IMPROVEMENT_THRESHOLD, value_parser = threshold)]
    pub improvement_threshold: f64,
    /// Frames per render block, a positive integer
    #[arg(long, default_value_t = RenderConfig::default().block_size_frames,
          value_parser = clap::value_parser!(i64).range(1..))]
    pub block_size: i64,
    /// The benchmark to run
    #[arg(long, value_enum, default_value_t = Kind::EngineBaseline)]
    pub kind: Kind,
    /// The regressor's weights/1 artifact (--kind ml-init); requires --normalization
    #[arg(long, requires = "normalization")]
    pub weights: Option<PathBuf>,
    /// The normalization.stats file the regressor was trained with (--kind ml-init); requires
    /// --weights
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

/// Why `benchmark` left nothing written: the one line it prints on stderr.
struct Refusal(String);

impl Refusal {
    fn new(message: impl std::fmt::Display) -> Self {
        Self::one_line(format!("patchback: benchmark refused: {message}"))
    }

    fn io(path: &Path, error: impl std::fmt::Display) -> Self {
        Self::new(format!("cannot write {path:?}: {error}"))
    }

    /// A refusal named by the error of `spec/` `error`.
    fn domain(error: &str, message: impl std::fmt::Display) -> Self {
        Self::new(format!("{error}: {message}"))
    }

    /// Clip `index` ended the benchmark with `error`, named by its error of `spec/` if any.
    fn clip(index: usize, domain: Option<&str>, error: impl std::fmt::Display) -> Self {
        match domain {
            Some(name) => Self::new(format!("clip {index}: {name}: {error}")),
            None => Self::new(format!("clip {index}: {error}")),
        }
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

impl From<FitError> for Refusal {
    fn from(error: FitError) -> Self {
        Self::new(format!("{}: {error}", error.domain_error()))
    }
}

/// Runs the verb: the report on stdout and exit 0, or one refusal line on stderr and exit 1;
/// `--weights` or `--normalization` with the engine baseline is a usage error (2), reported as
/// clap reports a conflict (Da12-2).
pub fn run(args: &BenchmarkArgs) -> ExitCode {
    if args.kind == Kind::EngineBaseline {
        let given = [
            ("--weights <WEIGHTS>", args.weights.is_some()),
            (
                "--normalization <NORMALIZATION>",
                args.normalization.is_some(),
            ),
        ];
        if let Some((flag, _)) = given.iter().find(|(_, given)| *given) {
            let mut command = BenchmarkArgs::augment_args(clap::Command::new("benchmark"))
                .bin_name("patchback benchmark");
            let error = command.error(
                ErrorKind::ArgumentConflict,
                format!(
                    "the argument '{flag}' cannot be used with '--kind engine-baseline'; it \
                     belongs to '--kind ml-init'"
                ),
            );
            let _ = error.print();
            return ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(2));
        }
    }
    match benchmark(args) {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(refusal) => {
            eprintln!("{}", refusal.0);
            ExitCode::from(REFUSED)
        }
    }
}

fn saturating(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// One clip of the holdout: its draw and the seed of its fit.
struct HoldoutClip {
    draw: PatchDraw,
    fit_seed: u64,
}

/// One line of `clips.txt`.
struct ClipRecord {
    patch_seed: u64,
    fit_seed: u64,
    default_loss: f64,
    best_loss: f64,
    evaluations: i64,
    restarts: u32,
    stop_reason: &'static str,
    /// The ML-init arms, on an ML-init benchmark only.
    ml_init: Option<MlInitClip>,
}

/// The ML-init columns of one line of `clips.txt`: arm 3 (the raw prediction's loss), arm 4
/// (the seeded fit's best loss, evaluations, restarts and stop reason) and the prediction's
/// clamp count.
struct MlInitClip {
    raw_loss: f64,
    seeded_best_loss: f64,
    seeded_evaluations: i64,
    seeded_restarts: u32,
    seeded_stop_reason: &'static str,
    clamped: usize,
}

/// The benchmark (module documentation, steps 1 to 7); on success the stdout report.
fn benchmark(args: &BenchmarkArgs) -> Result<String, Refusal> {
    let started = Instant::now();
    // 0. The ML-init inputs, read once and checked (Da12-3).
    let seed = match (&args.weights, &args.normalization) {
        (Some(weights), Some(normalization)) => {
            let seed = read_seed_inputs(weights, normalization)?;
            check_seed_inputs(&seed)?;
            Some(seed)
        }
        _ => None,
    };
    let weight_artifact_id = seed
        .as_ref()
        .map(|seed| format!("sha256:{}", seed.predictor.artifact().checksum()));
    // 1. StartBenchmark.
    let mut bench = Benchmark::start(
        args.kind.model(),
        &args.topology,
        args.seed,
        saturating(args.examples),
        weight_artifact_id.as_deref(),
    )?;
    // An ML-init benchmark started only with its inputs; an engine baseline takes none.
    let seed = seed.filter(|_| bench.kind() == BenchmarkKind::MlInit);
    let topology = known_topology(&args.topology).ok_or_else(|| FitError::TopologyNotFound {
        topology: args.topology.clone(),
    })?;
    // 2. The fit bounds.
    let config = FitConfig {
        bounds: FitBounds {
            max_renders: saturating(args.max_renders),
            max_wall_time_ms: saturating(args.max_wall_time_ms),
            max_restarts: saturating(args.max_restarts),
            improvement_threshold: args.improvement_threshold,
        },
        seed: 0,
        render: RenderConfig {
            block_size_frames: args.block_size,
        },
    };
    config.bounds.check()?;
    // 3. `<out>`.
    let out = &args.out;
    if let Some(blocking) = non_directory_on_the_way(out) {
        return Err(Refusal::io(blocking, "is not a directory"));
    }
    for name in [BENCHMARK_FILE, CLIPS_FILE, EXAMPLES_DIR] {
        let path = out.join(name);
        if fs::symlink_metadata(&path).is_ok() {
            return Err(Refusal::io(
                &path,
                "already exists; it is never overwritten",
            ));
        }
    }
    // 4. The holdout.
    let holdout = draw_holdout(args.seed, args.examples)?;
    // 5. The two arms of every clip, four on an ML-init benchmark.
    let mut records = Vec::with_capacity(holdout.len());
    let mut examples: Vec<Vec<Vec<u8>>> = Vec::new();
    for (index, clip) in holdout.iter().enumerate() {
        let reference = render_reference(index, &clip.draw, &topology, &config.render)?;
        let notes = clip.draw.notes();
        let arm = default_arm(&reference, &notes, &topology, &config.render)
            .map_err(|error| Refusal::clip(index, error.domain_error(), &error))?;
        let outcome = fit(
            &reference,
            &notes,
            &topology,
            &FitConfig {
                seed: clip.fit_seed,
                ..config
            },
        )
        .map_err(|error| Refusal::clip(index, Some(error.domain_error()), &error))?;
        let run = &outcome.run;
        let (Some(best_loss), Some(stop_reason)) = (run.best_loss(), run.stop_reason()) else {
            return Err(Refusal::clip(
                index,
                Some(FitError::NoValidCandidate.domain_error()),
                FitError::NoValidCandidate,
            ));
        };
        // Arms 4 and 3 of an ML-init benchmark (Da12-4, Da12-7).
        let ml_init = match &seed {
            None => None,
            Some(seed) => Some(ml_init_arms(
                index,
                &reference,
                &notes,
                &topology,
                &FitConfig {
                    seed: clip.fit_seed,
                    ..config
                },
                seed,
            )?),
        };
        records.push(ClipRecord {
            patch_seed: clip.draw.patch_seed,
            fit_seed: clip.fit_seed,
            default_loss: arm.comparison.total,
            best_loss,
            evaluations: run.evaluations(),
            restarts: outcome.restarts,
            stop_reason: stop_reason.name(),
            ml_init: ml_init.as_ref().map(|arms| arms.record()),
        });
        if index < LISTENING_EXAMPLE_COUNT {
            let mut files = vec![
                wav(index, &reference)?,
                wav(index, &arm.clip)?,
                wav(index, &outcome.render)?,
            ];
            if let Some(arms) = &ml_init {
                files.push(wav(index, &arms.predicted)?);
                files.push(wav(index, &arms.seeded)?);
            }
            examples.push(files);
        }
    }
    // 6. The medians, the reduction and RecordBaselineResult; or RecordMlInitResult.
    let default_losses: Vec<f64> = records.iter().map(|r| r.default_loss).collect();
    let search_losses: Vec<f64> = records.iter().map(|r| r.best_loss).collect();
    let evaluations: Vec<i64> = records.iter().map(|r| r.evaluations).collect();
    let (Some(default_median), Some(search_median), Some(median_evaluations)) = (
        median(&default_losses),
        median(&search_losses),
        median_count(&evaluations),
    ) else {
        return Err(FitError::InvalidHoldoutSize { example_count: 0 }.into());
    };
    let listening_examples: Vec<String> = (0..examples.len())
        .flat_map(|k| {
            example_files(bench.kind())
                .iter()
                .map(move |name| format!("{EXAMPLES_DIR}/{k}/{name}"))
        })
        .collect();
    let result = match &seed {
        None => {
            if default_median == 0.0 {
                return Err(Refusal::new(
                    "the default patch's median loss is 0: no reduction of it is defined",
                ));
            }
            let reduction = reduction_percent(default_median, search_median);
            let wall_time_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let verdict = bench.record_baseline_result(BaselineResult {
                default_median_loss: default_median,
                default_search_median_loss: search_median,
                median_loss_reduction_percent: reduction,
                default_search_median_evaluations: median_evaluations,
                wall_time_ms,
                listening_examples,
            })?;
            benchmark_result(&bench, verdict)
        }
        Some(_) => {
            let ml: Vec<&MlInitClip> = records.iter().filter_map(|r| r.ml_init.as_ref()).collect();
            let raw_losses: Vec<f64> = ml.iter().map(|r| r.raw_loss).collect();
            let seeded_losses: Vec<f64> = ml.iter().map(|r| r.seeded_best_loss).collect();
            let seeded_evaluations: Vec<i64> = ml.iter().map(|r| r.seeded_evaluations).collect();
            let (Some(raw_median), Some(seeded_median), Some(seeded_median_evaluations)) = (
                median(&raw_losses),
                median(&seeded_losses),
                median_count(&seeded_evaluations),
            ) else {
                return Err(FitError::InvalidHoldoutSize { example_count: 0 }.into());
            };
            let wall_time_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            bench.record_ml_init_result(MlInitResult {
                default_median_loss: default_median,
                default_search_median_loss: search_median,
                raw_regressor_median_loss: raw_median,
                regressor_search_median_loss: seeded_median,
                default_search_median_evaluations: median_evaluations,
                regressor_search_median_evaluations: seeded_median_evaluations,
                wall_time_ms,
                listening_examples,
            })?;
            let clamped = ml.iter().filter(|r| r.clamped > 0).count();
            ml_init_result(&bench, clamped, records.len())
        }
    };
    // 7. The files.
    let head = benchmark_head(&bench, &config, seed.as_ref());
    let mut created = Created::default();
    let written = write_all(
        out,
        &clips_text(&records),
        example_files(bench.kind()),
        &examples,
        &format!("{head}{result}"),
        &mut created,
    );
    if written.is_err() {
        created.remove();
    }
    written.map(|()| result)
}

/// Arms 4 and 3 of one clip of an ML-init benchmark: the clips they were scored on and what
/// `clips.txt` records of them.
struct MlInitArms {
    /// The raw prediction's render, cut or padded to the reference (`predicted.wav`).
    predicted: AudioClip,
    /// The seeded fit's best render, cut or padded to the reference (`seeded.wav`).
    seeded: AudioClip,
    raw_loss: f64,
    seeded_best_loss: f64,
    seeded_evaluations: i64,
    seeded_restarts: u32,
    seeded_stop_reason: &'static str,
    clamped: usize,
}

impl MlInitArms {
    fn record(&self) -> MlInitClip {
        MlInitClip {
            raw_loss: self.raw_loss,
            seeded_best_loss: self.seeded_best_loss,
            seeded_evaluations: self.seeded_evaluations,
            seeded_restarts: self.seeded_restarts,
            seeded_stop_reason: self.seeded_stop_reason,
            clamped: self.clamped,
        }
    }
}

/// Arms 4 and 3 of clip `index` (module documentation, step 5): the reference analysed with the
/// `features/1` configuration; the regressor-seeded fit (`fit_request` with that analysis and
/// the clip's own copy of the inputs, seeded with the clip's fit seed); then the seeded start,
/// the prediction's clamped values, scored once through `predicted_arm`. A refused analysis,
/// seeded fit or predicted render, and a seeded run without a valid candidate, refuse the clip.
fn ml_init_arms(
    index: usize,
    reference: &AudioClip,
    notes: &patchback_core::score::NoteSequence,
    topology: &Topology,
    config: &FitConfig,
    seed: &SeedInputs,
) -> Result<MlInitArms, Refusal> {
    let refused = |error: FitError| Refusal::clip(index, Some(error.domain_error()), &error);
    let analysis = analyze_clip(reference, &AnalysisConfig::default())
        .map_err(|error| refused(reference_refusal(error)))?;
    let outcome = fit_request(FitRequest {
        reference,
        notes,
        topology,
        config,
        analysis: Some(&analysis),
        seed: Some(seed.clone()),
    })
    .map_err(refused)?;
    let run = &outcome.run;
    let (Some(best_loss), Some(stop_reason)) = (run.best_loss(), run.stop_reason()) else {
        return Err(refused(FitError::NoValidCandidate));
    };
    let Some(start) = &outcome.seeded else {
        return Err(refused(FitError::PredictionFailed {
            message: "the regressor-seeded fit holds no prediction".to_owned(),
        }));
    };
    let arm = predicted_arm(reference, notes, topology, &start.point(), &config.render)
        .map_err(|error| Refusal::clip(index, error.domain_error(), &error))?;
    Ok(MlInitArms {
        predicted: arm.clip,
        seeded: outcome.render.clone(),
        raw_loss: arm.comparison.total,
        seeded_best_loss: best_loss,
        seeded_evaluations: run.evaluations(),
        seeded_restarts: outcome.restarts,
        seeded_stop_reason: stop_reason.name(),
        clamped: start.prediction.clamped,
    })
}

/// Reads the file at `path`, at most `limit + 1` bytes of it, so a longer file is seen to be
/// longer without being read whole (the rule of `patchback fit`).
fn read_bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The predictor of the `--weights` artifact and the `--normalization` statistics with the
/// SHA-256 of their bytes, each file read once, refused as `patchback fit` refuses them
/// (Da11-6, Da12-3): a weights file that is absent or cannot be read is
/// `patchback.model.UnknownWeightArtifact`, one that `weights/1` refuses
/// `patchback.model.CorruptWeightArtifact`; a statistics file that is absent, cannot be read,
/// is larger than `MAX_STATS_BYTES`, is not UTF-8 or that `read_stats` refuses is
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
            "{normalization:?}: the file is larger than the {MAX_STATS_BYTES} bytes a \
             benchmark reads"
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

/// Draws the holdout (module documentation, "The holdout"). The pipeline classifies draws in
/// `subtractive-mono/1`, the one topology `known_topology` admits.
fn draw_holdout(seed: u64, count: u64) -> Result<Vec<HoldoutClip>, Refusal> {
    let pipeline = Pipeline::new();
    let max_draws = count.saturating_mul(MAX_DRAWS_PER_SAMPLE);
    let mut stream = SplitMix64::new(seed ^ HOLDOUT_DOMAIN);
    let mut draws = 0_u64;
    let mut holdout = Vec::new();
    while (holdout.len() as u64) < count {
        if draws >= max_draws {
            return Err(Refusal::new(format!(
                "holdout exhausted: {draws} patch draws gave {} of the {count} clips asked for, \
                 every other draw a diagnostic example (at most {MAX_DRAWS_PER_SAMPLE} draws a \
                 clip)",
                holdout.len()
            )));
        }
        let draw = PatchDraw::from_seed(stream.next_u64());
        draws += 1;
        let rendering = pipeline
            .run(&draw)
            .map_err(|error| Refusal::clip(holdout.len(), error.domain_error(), &error))?;
        match rendering {
            Rendering::Sample(_) => {
                holdout.push(HoldoutClip {
                    draw,
                    fit_seed: stream.next_u64(),
                });
            }
            Rendering::Diagnostic(_) => {}
        }
    }
    Ok(holdout)
}

/// Clip `index`'s reference: its draw's patch rendered whole with the run's block size.
fn render_reference(
    index: usize,
    draw: &PatchDraw,
    topology: &Topology,
    config: &RenderConfig,
) -> Result<AudioClip, Refusal> {
    let validated = validate(&draw.patch(topology))
        .map_err(|error| Refusal::clip(index, Some(error.spec_error()), &error))?;
    let rendered = render(&compile(&validated), &draw.notes(), config)
        .map_err(|error| Refusal::clip(index, Some(error.domain_error()), error))?;
    AudioClip::from_mono(rendered.samples)
        .map_err(|error| Refusal::clip(index, Some(error.domain_error()), error))
}

/// The bytes of `clip` as a mono float32 48 kHz WAV.
fn wav(index: usize, clip: &AudioClip) -> Result<Vec<u8>, Refusal> {
    encode(clip, WavSampleFormat::Float32)
        .map_err(|error| Refusal::clip(index, Some(error.domain_error()), error))
}

/// The records of `benchmark.txt` before the result; on an ML-init benchmark (`seed`) the inputs'
/// records after `prng`.
fn benchmark_head(bench: &Benchmark, config: &FitConfig, seed: Option<&SeedInputs>) -> String {
    let bounds = &config.bounds;
    let mut text = format!("{BENCHMARK_HEADER} {FORMAT_VERSION}\n");
    text.push_str(&format!("patchback {PATCHBACK_VERSION}\n"));
    text.push_str(&format!("kind {}\n", kind_record(bench.kind())));
    text.push_str(&format!("topology {}\n", bench.topology()));
    text.push_str(&format!("seed {}\n", bench.seed()));
    text.push_str(&format!("example_count {}\n", bench.example_count()));
    text.push_str(&format!("sample_rate {SAMPLE_RATE_HZ}\n"));
    text.push_str(&format!("block_size {}\n", config.render.block_size_frames));
    text.push_str(&format!("max_renders {}\n", bounds.max_renders));
    text.push_str(&format!("max_wall_time_ms {}\n", bounds.max_wall_time_ms));
    text.push_str(&format!("max_restarts {}\n", bounds.max_restarts));
    text.push_str(&format!(
        "improvement_threshold {}\n",
        bounds.improvement_threshold
    ));
    text.push_str(&format!("prng {PRNG_NAME}\n"));
    if let Some(seed) = seed {
        let artifact = seed.predictor.artifact();
        text.push_str(&format!("weights sha256 {}\n", artifact.checksum()));
        text.push_str(&format!(
            "modelspec {} {}\n",
            artifact.modelspec_version(),
            artifact.modelspec_commit()
        ));
        text.push_str(&format!(
            "normalization sha256 {}\n",
            hex(&seed.normalization_sha256)
        ));
        text.push_str(&format!("feature_contract {FEATURES_1_VERSION}\n"));
    }
    text
}

/// The records of an ML-init `benchmark.txt` from `default_median_loss` on: also the stdout
/// report. `clamped` of the `clips` clips had a prediction with an output clamped. No reduction
/// and no verdict (Da12-9).
fn ml_init_result(bench: &Benchmark, clamped: usize, clips: usize) -> String {
    let real = |value: Option<f64>| value.map_or_else(|| "none".to_owned(), |v| v.to_string());
    let mut text = String::new();
    for (key, value) in [
        ("default_median_loss", bench.default_median_loss()),
        (
            "default_search_median_loss",
            bench.default_search_median_loss(),
        ),
        (
            "raw_regressor_median_loss",
            bench.raw_regressor_median_loss(),
        ),
        (
            "regressor_search_median_loss",
            bench.regressor_search_median_loss(),
        ),
    ] {
        text.push_str(&format!("{key} {}\n", real(value)));
    }
    text.push_str(&format!(
        "default_search_median_evaluations {}\n",
        bench.default_search_median_evaluations().unwrap_or(0)
    ));
    text.push_str(&format!(
        "regressor_search_median_evaluations {}\n",
        bench.regressor_search_median_evaluations().unwrap_or(0)
    ));
    text.push_str(&format!("clamped_predictions {clamped} of {clips}\n"));
    text.push_str(&format!(
        "wall_time_ms {}\n",
        bench.wall_time_ms().unwrap_or(0)
    ));
    for path in bench.listening_examples().unwrap_or_default() {
        text.push_str(&format!("listening_example {path}\n"));
    }
    text
}

/// The records of `benchmark.txt` from `default_median_loss` on: also the stdout report.
fn benchmark_result(bench: &Benchmark, verdict: TargetVerdict) -> String {
    let real = |value: Option<f64>| value.map_or_else(|| "none".to_owned(), |v| v.to_string());
    let mut text = format!(
        "default_median_loss {}\n",
        real(bench.default_median_loss())
    );
    text.push_str(&format!(
        "default_search_median_loss {}\n",
        real(bench.default_search_median_loss())
    ));
    text.push_str(&format!(
        "median_loss_reduction_percent {}\n",
        real(bench.median_loss_reduction_percent())
    ));
    text.push_str(&format!(
        "target_verdict {}\n",
        match verdict {
            TargetVerdict::Met => "met",
            TargetVerdict::NotMet => "not-met",
        }
    ));
    text.push_str(&format!(
        "default_search_median_evaluations {}\n",
        bench.default_search_median_evaluations().unwrap_or(0)
    ));
    text.push_str(&format!(
        "wall_time_ms {}\n",
        bench.wall_time_ms().unwrap_or(0)
    ));
    for path in bench.listening_examples().unwrap_or_default() {
        text.push_str(&format!("listening_example {path}\n"));
    }
    text
}

/// `clips.txt`.
fn clips_text(records: &[ClipRecord]) -> String {
    let mut text = format!("{CLIPS_HEADER} {FORMAT_VERSION}\n");
    text.push_str(BASELINE_COLUMNS);
    if records.iter().any(|r| r.ml_init.is_some()) {
        text.push_str(&format!(" {ML_INIT_COLUMNS}"));
    }
    text.push('\n');
    for (index, r) in records.iter().enumerate() {
        text.push_str(&format!(
            "clip {index} {} {} {} {} {} {} {}",
            r.patch_seed,
            r.fit_seed,
            r.default_loss,
            r.best_loss,
            r.evaluations,
            r.restarts,
            r.stop_reason
        ));
        if let Some(m) = &r.ml_init {
            text.push_str(&format!(
                " {} {} {} {} {} {}",
                m.raw_loss,
                m.seeded_best_loss,
                m.seeded_evaluations,
                m.seeded_restarts,
                m.seeded_stop_reason,
                m.clamped
            ));
        }
        text.push('\n');
    }
    text
}

/// The nearest of `out` and its ancestors that exists in any form, a symbolic link included,
/// when it is not a directory (a link counts as one only when it resolves to one); `None` when
/// it is a directory or nothing exists. The rule of `patchback fit`.
fn non_directory_on_the_way(out: &Path) -> Option<&Path> {
    out.ancestors()
        .filter(|path| !path.as_os_str().is_empty())
        .find(|path| fs::symlink_metadata(path).is_ok())
        .filter(|path| !path.is_dir())
}

/// Writes the files (module documentation, "Files"), recording what it creates.
fn write_all(
    out: &Path,
    clips: &str,
    names: &[&str],
    examples: &[Vec<Vec<u8>>],
    benchmark: &str,
    created: &mut Created,
) -> Result<(), Refusal> {
    created.dir(out).map_err(|error| Refusal::io(out, error))?;
    let path = out.join(CLIPS_FILE);
    write_new(&path, clips.as_bytes(), created).map_err(|error| Refusal::io(&path, error))?;
    for (k, files) in examples.iter().enumerate() {
        let dir = out.join(EXAMPLES_DIR).join(k.to_string());
        created
            .dir(&dir)
            .map_err(|error| Refusal::io(&dir, error))?;
        for (name, bytes) in names.iter().zip(files) {
            let path = dir.join(name);
            write_new(&path, bytes, created).map_err(|error| Refusal::io(&path, error))?;
        }
    }
    let path = out.join(BENCHMARK_FILE);
    write_new(&path, benchmark.as_bytes(), created).map_err(|error| Refusal::io(&path, error))
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

    fn remove(self) {
        for file in self.files.iter().rev() {
            let _ = fs::remove_file(file);
        }
        for dir in self.dirs.iter().rev() {
            let _ = fs::remove_dir(dir);
        }
    }
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
