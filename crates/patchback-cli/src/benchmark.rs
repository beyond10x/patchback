//! `patchback benchmark --out <dir> --examples <n> [--seed <n>] [--topology <name>]
//! [--max-renders <n>] [--max-wall-time-ms <ms>] [--max-restarts <n>]
//! [--improvement-threshold <loss>] [--block-size <frames>]` (story:engine-baseline, DESIGN.md
//! § 9.2, § 12.2, § 13 Milestone A; `StartBenchmark` and `RecordBaselineResult` of
//! `spec/domains/fit.yaml`): the engine baseline. Over a seeded holdout of reference clips
//! Patchback synthesizes itself (approval-record:engine-baseline-holdout: not a dataset split),
//! the median loss of the fixed default patch is compared with the median best loss of
//! `patchback_fit::fit` started from it, and the benchmark records whether the go/no-go target
//! (a reduction of at least 20 %) was met, with the evaluations, the wall time and listening
//! examples. The model is `patchback_fit::Benchmark`; this verb draws the holdout, runs the fits
//! and writes the files (Da8-1).
//!
//! # Flags
//!
//! | Flag | Value | Default |
//! |---|---|---|
//! | `--out` | the benchmark directory; created when absent | required |
//! | `--examples` | the holdout's clip count (`example_count`), 0 to 2^64 - 1; 0 is refused `InvalidHoldoutSize` | required |
//! | `--seed` | the holdout's seed, 0 to 2^64 - 1 | 0 |
//! | `--topology` | the topology the holdout is synthesized and fitted in, by name | `subtractive-mono/1` |
//! | `--max-renders`, `--max-wall-time-ms`, `--max-restarts`, `--improvement-threshold`, `--block-size` | every clip's fit bounds and render block size, as `patchback fit` takes them | 2000, 60000, 4, 0.0001, 256 |
//!
//! A count or bound above `i64::MAX` is taken as `i64::MAX`. A negative or non-integer count,
//! seed or bound, a NaN, infinite or negative threshold and a block size below 1 are usage
//! errors (2).
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
//! 1. `StartBenchmark` (`Benchmark::start`, kind `EngineBaseline`): `InvalidHoldoutSize`, then
//!    `TopologyNotFound` for an unknown `--topology`.
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
//! 6. The default median and the search median are the lower medians of the per-clip losses
//!    (`patchback_fit::median`, Da8-4); a default median of 0 is refused (no reduction is
//!    defined); the reduction is `patchback_fit::reduction_percent`; the median evaluations the
//!    lower median of the per-clip counts; the wall time the whole run's so far, from the verb's
//!    start. `RecordBaselineResult` (`Benchmark::record_baseline_result`) records them with the
//!    listening examples and gives the verdict: `Met` at a reduction of at least 20, else
//!    `NotMet`. Both complete the benchmark and exit 0 (Da8-11).
//! 7. The files are written (below); a failed write removes every file and directory the run
//!    created.
//!
//! # Files (Da8-8)
//!
//! Written in this order, each created new, never overwritten: `<out>/clips.txt`; for each of
//! the first `patchback_fit::LISTENING_EXAMPLE_COUNT` (3) clips, fewer when the holdout is
//! smaller, `<out>/examples/<k>/reference.wav`, `default.wav` and `fit.wav`; `<out>/benchmark.txt`
//! last, so a `benchmark.txt` names a complete benchmark.
//!
//! The WAVs are mono float32 at 48 kHz: `reference.wav` is clip `k`'s reference, `default.wav` the
//! clip the default patch was scored on and `fit.wav` the clip the fit's best patch was scored on
//! (`FitOutcome::render`), each cut or padded to the reference, as `patchback fit`'s
//! `reconstructed.wav` is. So `patchback compare --reference examples/<k>/reference.wav
//! --candidate examples/<k>/fit.wav` prints clip `k`'s best loss as its `loss`, and with
//! `default.wav` its default loss.
//!
//! `benchmark.txt` and `clips.txt` are line-oriented UTF-8 text like `run.meta`, each line ended
//! by `\n`, one space between fields. Reals are written as Rust prints an `f64` (the shortest text
//! that reads back to the same value). `wall_time_ms` is the one record that differs between two
//! runs of the same flags; every other byte of the three kinds of file is the same.
//!
//! `benchmark.txt`, one `<key> <value>` record per line, in this order:
//!
//! | Record | Value |
//! |---|---|
//! | `patchback-benchmark` | the format version, `1` |
//! | `patchback` | the Patchback version |
//! | `kind` | `engine-baseline` |
//! | `topology` | the topology's name |
//! | `seed` | `--seed` |
//! | `example_count` | `--examples` |
//! | `sample_rate` | `48000` |
//! | `block_size` | the frames per render block |
//! | `max_renders`, `max_wall_time_ms`, `max_restarts`, `improvement_threshold` | every fit's bounds |
//! | `prng` | `xoshiro256**`, the generator of the draws and of the restarts |
//! | `default_median_loss` | the lower median of the default arm's losses |
//! | `default_search_median_loss` | the lower median of the fits' best losses |
//! | `median_loss_reduction_percent` | `(default − search) / default × 100` of the two medians |
//! | `target_verdict` | `met` at a reduction of at least 20, else `not-met` |
//! | `default_search_median_evaluations` | the lower median of the fits' render evaluations |
//! | `wall_time_ms` | the whole run's wall time, from the verb's start to the result, in milliseconds |
//! | `listening_example` | zero or more, last: one per listening example, its path relative to `<out>`, in the order clip 0 `reference.wav`, `default.wav`, `fit.wav`, then clip 1, … |
//!
//! The version, sample rate, block size and bounds are the "fixed build and configuration" of
//! DESIGN.md § 12.2 that two compared runs must share; no device is recorded, the code has none
//! (Da8-9).
//!
//! `clips.txt`: the header line `patchback-benchmark-clips 1`; the line `columns index patch_seed
//! fit_seed default_loss best_loss evaluations restarts stop_reason`; then one line per clip, in
//! clip order, `clip` and those eight fields: the clip index from 0, its patch seed and fit seed,
//! the default arm's loss, the fit's best loss, its render evaluations, the restarts that began
//! and its stop reason (`RenderBudgetExhausted`, `TimeBudgetExhausted`, `RestartsExhausted` or
//! `ImprovementBelowThreshold`).
//!
//! # Output
//!
//! On success, exit 0 on either verdict, and on stdout the records of `benchmark.txt` from
//! `default_median_loss` on, in its order and form.
//!
//! # Refusals
//!
//! One line on stderr, exit status 1, nothing left written, `patchback: benchmark refused: …`:
//!
//! - `patchback.fit.InvalidHoldoutSize: …`, `patchback.fit.TopologyNotFound: …` (the name
//!   quoted), `patchback.fit.InvalidSearchBound: …`;
//! - `cannot write "<path>": <message>` for an `<out>` that is not a directory, an existing
//!   `benchmark.txt`, `clips.txt` or `examples`, or a file or directory that cannot be created or
//!   written;
//! - `holdout exhausted: …` when the draws reach their bound before the holdout is full; the
//!   model declares no error for it;
//! - `clip <k>: <error>: <message>` for clip `k` whose draw the generator's pipeline refuses for a
//!   reason that is no diagnostic, whose reference or default arm cannot be rendered or compared,
//!   whose fit is refused or whose run fails (`<error>` the error of `spec/` where there is one);
//! - `the default patch's median loss is 0: …`.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::Args;
use patchback_audio::{AudioClip, WavSampleFormat, encode};
use patchback_core::parameter::{SUBTRACTIVE_MONO, Topology};
use patchback_core::patch::{compile, known_topology, validate};
use patchback_core::prng::{NAME as PRNG_NAME, SplitMix64};
use patchback_dataset::{MAX_DRAWS_PER_SAMPLE, PatchDraw, Pipeline, Rendering};
use patchback_dsp::{RenderConfig, SAMPLE_RATE_HZ, render};
use patchback_fit::{
    BaselineResult, Benchmark, BenchmarkKind, DEFAULT_IMPROVEMENT_THRESHOLD, DEFAULT_MAX_RENDERS,
    DEFAULT_MAX_RESTARTS, DEFAULT_MAX_WALL_TIME_MS, FitBounds, FitConfig, FitError,
    LISTENING_EXAMPLE_COUNT, PATCHBACK_VERSION, TargetVerdict, default_arm, fit, median,
    median_count, reduction_percent,
};

/// The command ran and refused; it wrote nothing.
const REFUSED: u8 = 1;

/// The benchmark's record file.
pub const BENCHMARK_FILE: &str = "benchmark.txt";
/// The per-clip record file.
pub const CLIPS_FILE: &str = "clips.txt";
/// The directory of the listening examples.
pub const EXAMPLES_DIR: &str = "examples";
/// The listening example files of one clip, in the order they are written and recorded.
pub const EXAMPLE_FILES: [&str; 3] = ["reference.wav", "default.wav", "fit.wav"];

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

/// Runs the verb: the report on stdout and exit 0, or one refusal line on stderr and exit 1.
pub fn run(args: &BenchmarkArgs) -> ExitCode {
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
}

/// The benchmark (module documentation, steps 1 to 7); on success the stdout report.
fn benchmark(args: &BenchmarkArgs) -> Result<String, Refusal> {
    let started = Instant::now();
    // 1. StartBenchmark.
    let mut bench = Benchmark::start(
        BenchmarkKind::EngineBaseline,
        &args.topology,
        args.seed,
        saturating(args.examples),
        None,
    )?;
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
    // 5. The two arms of every clip.
    let mut records = Vec::with_capacity(holdout.len());
    let mut examples: Vec<[Vec<u8>; 3]> = Vec::new();
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
        records.push(ClipRecord {
            patch_seed: clip.draw.patch_seed,
            fit_seed: clip.fit_seed,
            default_loss: arm.comparison.total,
            best_loss,
            evaluations: run.evaluations(),
            restarts: outcome.restarts,
            stop_reason: stop_reason.name(),
        });
        if index < LISTENING_EXAMPLE_COUNT {
            examples.push([
                wav(index, &reference)?,
                wav(index, &arm.clip)?,
                wav(index, &outcome.render)?,
            ]);
        }
    }
    // 6. The medians, the reduction and RecordBaselineResult.
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
    if default_median == 0.0 {
        return Err(Refusal::new(
            "the default patch's median loss is 0: no reduction of it is defined",
        ));
    }
    let reduction = reduction_percent(default_median, search_median);
    let listening_examples: Vec<String> = (0..examples.len())
        .flat_map(|k| EXAMPLE_FILES.map(|name| format!("{EXAMPLES_DIR}/{k}/{name}")))
        .collect();
    let wall_time_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let verdict = bench.record_baseline_result(BaselineResult {
        default_median_loss: default_median,
        default_search_median_loss: search_median,
        median_loss_reduction_percent: reduction,
        default_search_median_evaluations: median_evaluations,
        wall_time_ms,
        listening_examples: listening_examples.clone(),
    })?;
    // 7. The files.
    let head = benchmark_head(&bench, &config);
    let result = benchmark_result(&bench, verdict);
    let mut created = Created::default();
    let written = write_all(
        out,
        &clips_text(&records),
        &examples,
        &format!("{head}{result}"),
        &mut created,
    );
    if written.is_err() {
        created.remove();
    }
    written.map(|()| result)
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

/// The records of `benchmark.txt` before the result.
fn benchmark_head(bench: &Benchmark, config: &FitConfig) -> String {
    let bounds = &config.bounds;
    let mut text = format!("{BENCHMARK_HEADER} {FORMAT_VERSION}\n");
    text.push_str(&format!("patchback {PATCHBACK_VERSION}\n"));
    text.push_str("kind engine-baseline\n");
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
    text.push_str(
        "columns index patch_seed fit_seed default_loss best_loss evaluations restarts \
         stop_reason\n",
    );
    for (index, r) in records.iter().enumerate() {
        text.push_str(&format!(
            "clip {index} {} {} {} {} {} {} {}\n",
            r.patch_seed,
            r.fit_seed,
            r.default_loss,
            r.best_loss,
            r.evaluations,
            r.restarts,
            r.stop_reason
        ));
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
    examples: &[[Vec<u8>; 3]],
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
        for (name, bytes) in EXAMPLE_FILES.iter().zip(files) {
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
