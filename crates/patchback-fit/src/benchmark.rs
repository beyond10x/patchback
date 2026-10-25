//! The benchmark of `spec/domains/fit.yaml` (`Benchmark`, lifecycle Running → Completed) and its
//! engine-baseline commands as methods, each answering its outcomes in the model's order
//! (story:engine-baseline, DESIGN.md § 12.2, § 13 Milestone A):
//!
//! | Method | Command | Outcomes, in order |
//! |---|---|---|
//! | [`Benchmark::start`] | `StartBenchmark` | `invalid-holdout-size`, `weight-artifact-required`, `unknown-topology`, `started` |
//! | [`Benchmark::record_baseline_result`] | `RecordBaselineResult` | `kind-mismatch`, `target-met`, `target-not-met`, `wrong-state` |
//!
//! A benchmark is a value, as a [`crate::FitRun`] is (Da8-10): `no-such-benchmark`
//! (`BenchmarkNotFound`) has no counterpart, since no call can name a benchmark that does not
//! exist, and the `Benchmarks` view is [`Benchmark::summary`]. `RecordMlInitResult` is
//! story:ml-init-benchmark's; its fields (`raw_regressor_median_loss`,
//! `regressor_search_median_loss`, `regressor_search_median_evaluations`) stay absent here.
//!
//! The topology is named, not identified by Uuid: the code knows topologies by name
//! (`patchback_core::patch::known_topology`), and [`FitError::TopologyNotFound`] carries the name,
//! as the fit's does (Da8-6). The weight artifact is an opaque identifier.
//!
//! # The numbers
//!
//! - [`median`] is the lower median: the element at index `(n − 1) / 2` of the values sorted
//!   ascending, `None` for no value (Da8-4). [`median_count`] is the same of integers.
//! - [`reduction_percent`] is `(default − search) / default × 100` (Da8-4). A default median of 0
//!   defines no reduction; the caller refuses it before asking.
//! - The verdict is [`TargetVerdict::Met`] at a reduction of at least
//!   [`TARGET_REDUCTION_PERCENT`], else [`TargetVerdict::NotMet`] (DESIGN.md § 12.2: "≥20%
//!   reduction", inclusive). Both complete the benchmark; neither is an error (Da8-11). The
//!   verdict is decided on the recorded `f64` reduction, the value [`reduction_percent`] computes
//!   and `benchmark.txt` prints, so the record and the verdict always agree; for two medians
//!   within rounding distance of exactly 20 % it may differ from exact rational arithmetic
//!   (Da8-35).
//!
//! # The default arm
//!
//! [`default_arm`] is the engine baseline's default arm on one clip (Da8-5): the default patch of
//! the topology ([`crate::default_point`], [`crate::patch_at`]), validated, compiled and rendered
//! against the clip's notes with the run's render configuration, the render cut to the
//! reference's length or padded with silence to it, compared with the reference
//! ([`crate::compare`]). That is the candidate [`crate::fit`] evaluates first (restart 0 starts at
//! the default point), so when that evaluation is valid its total is this loss; it is computed
//! here, apart from the fit's log, because the run's `initial_loss` is the first *valid*
//! candidate's and differs from the default's when the default render is rejected.

use std::fmt;

use patchback_audio::{AudioClip, WavError};
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, known_topology, validate};
use patchback_core::score::NoteSequence;
use patchback_dsp::{RenderConfig, RenderError, render};

use crate::{CompareError, Comparison, FitError, compare, default_point, patch_at};

/// The reduction of the median loss, in percent, at and above which the engine baseline's
/// go/no-go target is met (DESIGN.md § 12.2: "≥20% reduction in median normalized audio loss").
pub const TARGET_REDUCTION_PERCENT: f64 = 20.0;

/// How many clips of the holdout, the first ones, get listening examples (Da8-7): their
/// reference, default render and fitted render. Provisional: DESIGN.md § 12.2 asks for
/// listening examples and names no count.
pub const LISTENING_EXAMPLE_COUNT: usize = 3;

/// Which benchmark (`BenchmarkKind`): the engine baseline (default patch against default plus
/// search, DESIGN.md § 13 A) or the ML-init comparison (§ 13 B).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkKind {
    EngineBaseline,
    MlInit,
}

impl BenchmarkKind {
    /// The variant's name in `spec/domains/fit.yaml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::EngineBaseline => "EngineBaseline",
            Self::MlInit => "MlInit",
        }
    }
}

impl fmt::Display for BenchmarkKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Whether the engine baseline met its go/no-go target (`TargetVerdict`). A result, never a pass
/// or fail of the benchmark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetVerdict {
    Met,
    NotMet,
}

impl TargetVerdict {
    /// The variant's name in `spec/domains/fit.yaml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Met => "Met",
            Self::NotMet => "NotMet",
        }
    }

    /// The verdict of a reduction: [`Self::Met`] at [`TARGET_REDUCTION_PERCENT`] or more, else
    /// [`Self::NotMet`]; a NaN is [`Self::NotMet`].
    ///
    /// It compares the `f64` it is given, the reduction as recorded, so the recorded reduction
    /// and the verdict always agree. For a reduction [`reduction_percent`] computed from two
    /// medians whose exact reduction lies within rounding distance of 20 %, the verdict may
    /// differ from the one exact rational arithmetic gives (Da8-35): documented, not corrected.
    pub fn of(median_loss_reduction_percent: f64) -> Self {
        if median_loss_reduction_percent >= TARGET_REDUCTION_PERCENT {
            Self::Met
        } else {
            Self::NotMet
        }
    }
}

impl fmt::Display for TargetVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The lifecycle state of a benchmark (`Benchmark.State`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkState {
    Running,
    Completed,
}

impl BenchmarkState {
    /// The state's name in `spec/domains/fit.yaml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Completed => "Completed",
        }
    }
}

impl fmt::Display for BenchmarkState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The input of `RecordBaselineResult` besides the benchmark.
#[derive(Clone, Debug, PartialEq)]
pub struct BaselineResult {
    /// The median of the default patch's loss over the holdout.
    pub default_median_loss: f64,
    /// The median of the fit's best loss over the holdout.
    pub default_search_median_loss: f64,
    /// [`reduction_percent`] of the two medians.
    pub median_loss_reduction_percent: f64,
    /// The median of the fits' render evaluations.
    pub default_search_median_evaluations: i64,
    /// The wall time of the whole benchmark, in milliseconds.
    pub wall_time_ms: u64,
    /// The listening examples' paths.
    pub listening_examples: Vec<String>,
}

/// One benchmark: its inputs, its state and, once completed, its result.
#[derive(Clone, Debug, PartialEq)]
pub struct Benchmark {
    kind: BenchmarkKind,
    topology: String,
    seed: u64,
    example_count: i64,
    weight_artifact_id: Option<String>,
    state: BenchmarkState,
    default_median_loss: Option<f64>,
    default_search_median_loss: Option<f64>,
    raw_regressor_median_loss: Option<f64>,
    regressor_search_median_loss: Option<f64>,
    median_loss_reduction_percent: Option<f64>,
    target_verdict: Option<TargetVerdict>,
    default_search_median_evaluations: Option<i64>,
    regressor_search_median_evaluations: Option<i64>,
    wall_time_ms: Option<u64>,
    listening_examples: Option<Vec<String>>,
}

/// One row of the `Benchmarks` view: the fields it publishes, from the value (Da8-10). The
/// view's `benchmark_id` has no counterpart (a benchmark is a value) and its `topology_id` is the
/// topology's name.
#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkRow {
    pub state: BenchmarkState,
    pub kind: BenchmarkKind,
    pub topology: String,
    pub seed: u64,
    pub example_count: i64,
    pub target_verdict: Option<TargetVerdict>,
    pub default_search_median_evaluations: Option<i64>,
    pub regressor_search_median_evaluations: Option<i64>,
    pub wall_time_ms: Option<u64>,
    pub listening_examples: Option<Vec<String>>,
}

impl Benchmark {
    /// Starts a benchmark Running (`StartBenchmark`), refusing in the model's order: a holdout of
    /// no example ([`FitError::InvalidHoldoutSize`], `example_count <= 0`); an ML-init benchmark
    /// without a weight artifact ([`FitError::WeightArtifactRequired`]); a topology the renderer
    /// does not know ([`FitError::TopologyNotFound`]).
    pub fn start(
        kind: BenchmarkKind,
        topology: &str,
        seed: u64,
        example_count: i64,
        weight_artifact_id: Option<&str>,
    ) -> Result<Self, FitError> {
        if example_count <= 0 {
            return Err(FitError::InvalidHoldoutSize { example_count });
        }
        if kind == BenchmarkKind::MlInit && weight_artifact_id.is_none() {
            return Err(FitError::WeightArtifactRequired {
                topology: topology.to_owned(),
            });
        }
        if known_topology(topology).is_none() {
            return Err(FitError::TopologyNotFound {
                topology: topology.to_owned(),
            });
        }
        Ok(Self {
            kind,
            topology: topology.to_owned(),
            seed,
            example_count,
            weight_artifact_id: weight_artifact_id.map(str::to_owned),
            state: BenchmarkState::Running,
            default_median_loss: None,
            default_search_median_loss: None,
            raw_regressor_median_loss: None,
            regressor_search_median_loss: None,
            median_loss_reduction_percent: None,
            target_verdict: None,
            default_search_median_evaluations: None,
            regressor_search_median_evaluations: None,
            wall_time_ms: None,
            listening_examples: None,
        })
    }

    /// Records the engine baseline's result and completes the benchmark
    /// (`RecordBaselineResult`): refused for a benchmark of another kind
    /// ([`FitError::BenchmarkKindMismatch`]), then for one already Completed
    /// ([`FitError::BenchmarkStateConflict`]); otherwise every field of `result` is set, the
    /// verdict is [`TargetVerdict::of`] the reduction, and the benchmark is Completed on either
    /// verdict.
    ///
    /// Precondition: finite medians and `default_median_loss > 0`, so the reduction is a finite
    /// number. The method checks none of it and adds no refusal to the model's four outcomes
    /// (Da8-36): a non-finite reduction is recorded as given, the verdict is
    /// [`TargetVerdict::NotMet`] for a NaN (and [`TargetVerdict::Met`] for +∞), and the benchmark
    /// is Completed. `patchback benchmark` refuses a default median of 0 before it records, and
    /// the fit's losses are finite. What the model should answer to a non-finite result is the
    /// specification owner's.
    pub fn record_baseline_result(
        &mut self,
        result: BaselineResult,
    ) -> Result<TargetVerdict, FitError> {
        if self.kind != BenchmarkKind::EngineBaseline {
            return Err(FitError::BenchmarkKindMismatch { kind: self.kind });
        }
        if self.state != BenchmarkState::Running {
            return Err(FitError::BenchmarkStateConflict { state: self.state });
        }
        let verdict = TargetVerdict::of(result.median_loss_reduction_percent);
        self.default_median_loss = Some(result.default_median_loss);
        self.default_search_median_loss = Some(result.default_search_median_loss);
        self.median_loss_reduction_percent = Some(result.median_loss_reduction_percent);
        self.default_search_median_evaluations = Some(result.default_search_median_evaluations);
        self.wall_time_ms = Some(result.wall_time_ms);
        self.listening_examples = Some(result.listening_examples);
        self.target_verdict = Some(verdict);
        self.state = BenchmarkState::Completed;
        Ok(verdict)
    }

    /// The benchmark's row of the `Benchmarks` view.
    pub fn summary(&self) -> BenchmarkRow {
        BenchmarkRow {
            state: self.state,
            kind: self.kind,
            topology: self.topology.clone(),
            seed: self.seed,
            example_count: self.example_count,
            target_verdict: self.target_verdict,
            default_search_median_evaluations: self.default_search_median_evaluations,
            regressor_search_median_evaluations: self.regressor_search_median_evaluations,
            wall_time_ms: self.wall_time_ms,
            listening_examples: self.listening_examples.clone(),
        }
    }

    pub fn kind(&self) -> BenchmarkKind {
        self.kind
    }

    /// The name of the topology the holdout is synthesized in.
    pub fn topology(&self) -> &str {
        &self.topology
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn example_count(&self) -> i64 {
        self.example_count
    }

    pub fn weight_artifact_id(&self) -> Option<&str> {
        self.weight_artifact_id.as_deref()
    }

    pub fn state(&self) -> BenchmarkState {
        self.state
    }

    pub fn default_median_loss(&self) -> Option<f64> {
        self.default_median_loss
    }

    pub fn default_search_median_loss(&self) -> Option<f64> {
        self.default_search_median_loss
    }

    /// Recorded by `RecordMlInitResult` (story:ml-init-benchmark); absent here.
    pub fn raw_regressor_median_loss(&self) -> Option<f64> {
        self.raw_regressor_median_loss
    }

    /// Recorded by `RecordMlInitResult` (story:ml-init-benchmark); absent here.
    pub fn regressor_search_median_loss(&self) -> Option<f64> {
        self.regressor_search_median_loss
    }

    pub fn median_loss_reduction_percent(&self) -> Option<f64> {
        self.median_loss_reduction_percent
    }

    pub fn target_verdict(&self) -> Option<TargetVerdict> {
        self.target_verdict
    }

    pub fn default_search_median_evaluations(&self) -> Option<i64> {
        self.default_search_median_evaluations
    }

    /// Recorded by `RecordMlInitResult` (story:ml-init-benchmark); absent here.
    pub fn regressor_search_median_evaluations(&self) -> Option<i64> {
        self.regressor_search_median_evaluations
    }

    pub fn wall_time_ms(&self) -> Option<u64> {
        self.wall_time_ms
    }

    pub fn listening_examples(&self) -> Option<&[String]> {
        self.listening_examples.as_deref()
    }
}

/// The lower median of `values`: the element at index `(n − 1) / 2` of the values sorted
/// ascending (`f64::total_cmp`); `None` for no value (Da8-4).
pub fn median(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted.get(sorted.len().checked_sub(1)? / 2).copied()
}

/// The lower median of integer counts, as [`median`].
pub fn median_count(values: &[i64]) -> Option<i64> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len().checked_sub(1)? / 2).copied()
}

/// The reduction of `search_median` from `default_median`, in percent of the default:
/// `(default − search) / default × 100` (Da8-4). Positive when the search lowered the loss.
///
/// Computed in `f64` in exactly that order, each step rounded:
/// `(default_median - search_median) / default_median * 100.0`. The result is the value
/// `benchmark.txt` records and the verdict is decided on ([`TargetVerdict::of`]); for medians
/// whose exact reduction lies within rounding distance of 20 % it can fall on the other side of
/// 20 than the exact quotient (Da8-35). A default median of 0 gives an infinite reduction or NaN;
/// callers refuse it before asking.
pub fn reduction_percent(default_median: f64, search_median: f64) -> f64 {
    (default_median - search_median) / default_median * 100.0
}

/// The default arm on one clip (module documentation): the clip the default patch was scored on
/// and its comparison with the reference.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultArm {
    /// The default patch's render, cut to the reference's length or padded with silence to it.
    pub clip: AudioClip,
    pub comparison: Comparison,
}

/// Why the default arm of a clip has no loss.
#[derive(Clone, Debug, PartialEq)]
pub enum DefaultArmError {
    /// The topology's default patch was not built or not admitted by validation (unreachable for
    /// a known topology: the default point maps into every contract's range).
    Patch(String),
    /// The renderer refused the default patch with the notes.
    Render(RenderError),
    /// The rendered samples did not make a clip.
    Audio(WavError),
    /// The comparison refused the rendered clip.
    Compare(CompareError),
    /// The loss or a component was not finite and non-negative.
    NonFiniteLoss,
}

impl DefaultArmError {
    /// The error of `spec/` this refusal names, where it names one.
    pub fn domain_error(&self) -> Option<&'static str> {
        match self {
            Self::Patch(_) | Self::NonFiniteLoss => None,
            Self::Render(error) => Some(error.domain_error()),
            Self::Audio(error) => Some(error.domain_error()),
            Self::Compare(error) => Some(error.domain_error()),
        }
    }
}

impl fmt::Display for DefaultArmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Patch(message) => write!(f, "the default patch is refused: {message}"),
            Self::Render(error) => write!(f, "the default patch's render is refused: {error}"),
            Self::Audio(error) => write!(f, "the default patch's render is no clip: {error}"),
            Self::Compare(error) => {
                write!(f, "the default patch's render cannot be compared: {error}")
            }
            Self::NonFiniteLoss => {
                f.write_str("the default patch's loss is not finite and non-negative")
            }
        }
    }
}

impl std::error::Error for DefaultArmError {}

/// The default arm of one clip (module documentation, Da8-5).
pub fn default_arm(
    reference: &AudioClip,
    notes: &NoteSequence,
    topology: &Topology,
    render_config: &RenderConfig,
) -> Result<DefaultArm, DefaultArmError> {
    let patch = patch_at(topology, &default_point(topology))
        .map_err(|error| DefaultArmError::Patch(error.to_string()))?;
    let validated = validate(&patch)
        .map_err(|error| DefaultArmError::Patch(format!("{}: {error}", error.spec_error())))?;
    let rendered =
        render(&compile(&validated), notes, render_config).map_err(DefaultArmError::Render)?;
    let mut samples = rendered.samples;
    samples.resize(reference.frame_count(), 0.0);
    let clip = AudioClip::from_mono(samples).map_err(DefaultArmError::Audio)?;
    let comparison = compare(reference, &clip).map_err(DefaultArmError::Compare)?;
    let c = &comparison.components;
    let valid = [
        comparison.total,
        c.log_spectral,
        c.envelope,
        c.transient,
        c.band_energy,
    ]
    .iter()
    .all(|value| value.is_finite() && *value >= 0.0);
    if !valid {
        return Err(DefaultArmError::NonFiniteLoss);
    }
    Ok(DefaultArm { clip, comparison })
}
