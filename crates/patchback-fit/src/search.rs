//! The bounded derivative-free fit of DESIGN.md § 9.2 within one known topology: [`fit`] from
//! the default patch, and [`fit_request`], which mirrors `StartFit` and starts from the native
//! regressor's prediction when it is given a weight artifact (§ 9.2 step 1; story
//! regressor-seeded-fit).
//!
//! # Refusals
//!
//! In the order of `StartFit`'s outcomes (`spec/domains/fit.yaml`), before anything is rendered:
//!
//! 1. A bound that is not positive: [`FitError::InvalidSearchBound`] naming it
//!    ([`FitBounds::check`]: renders, then wall time, then restarts). Every note sequence here is
//!    known, so `NoteConfidenceMissing` is not reached.
//! 2. Seed inputs ([`FitRequest::seed`]) without an analysis ([`FitRequest::analysis`]):
//!    [`FitError::AnalysisRequired`] naming the artifact's checksum.
//! 3. A topology the renderer does not know (`patchback_core::patch::known_topology`):
//!    [`FitError::TopologyNotFound`]. The reference, the note sequence, the analysis and the
//!    weight artifact are values here; their existence is the caller's check (`patchback fit`
//!    answers it by path).
//! 4. Seed inputs of another feature contract than the analysis, `features/1`
//!    ([`check_seed_inputs`]): the weight artifact's contract, then the statistics', each
//!    [`FitError::FeatureContractMismatch`] naming the input, `features/1` and the contract it
//!    holds (Da11-5).
//! 5. The reference is analysed (`patchback_analysis::analyze_clip`, the default
//!    configuration): `SilentInput` is [`FitError::ReferenceSilent`]; `PitchAmbiguous`,
//!    `ClipTooShortForPitch` and `PitchOutOfRange` are [`FitError::PitchAmbiguous`] (D-a6-4); any
//!    other refusal (an empty clip, a rate other than 48 kHz, a clip past the analysis duration
//!    bound, a non-finite sample) would refuse every comparison too and is passed on as
//!    [`FitError::Compare`] ([`reference_refusal`]). A given analysis is used instead when it
//!    can be the reference's, the same frame count at the same sample rate: it admits the
//!    reference (an analysis exists only for a clip analysis admitted, so none of these refusals
//!    follows from it), and the prediction does not read it (step 6). With seed inputs, a given
//!    analysis of another frame count or rate is refused as [`FitError::PredictionFailed`],
//!    named `patchback.fit.NoValidCandidate`, its message naming both frame counts and rates
//!    (Da11-24); without them it is not used, and the reference is analysed as above.
//! 6. Seed inputs: the start is predicted ([`SeededStart`], Da11-7, Da11-8, Da11-21, Da11-25).
//!    The `features/1` vector of the first note of the sequence, its `start_sample..end_sample`
//!    clamped to the reference's length, is computed as the dataset computes a training example's
//!    (`patchback-dataset`'s generator): `patchback_analysis::analyze_and_extract` over the
//!    reference's samples from the note's start to the end of its release window, `end_sample +
//!    RELEASE_OFFSET_FRAMES + RELEASE_WINDOW_FRAMES` clamped to the reference's length, with the
//!    span `0..end_sample - start_sample`, so neither the audio before the note nor a later note
//!    changes it; then normalized by the statistics
//!    (`NormalizationTable::normalize`); every normalized value must be finite as `f64` and after
//!    the cast to `f32`, the predictor's own check; then `patchback_model::Predictor::predict`. A
//!    refused analysis or extraction, a value that is not finite or a refused prediction is
//!    [`FitError::PredictionFailed`] naming it (the error of `spec/` when it has one), named
//!    `patchback.fit.NoValidCandidate` (Da11-12); the predictor's `FeatureContractMismatch`,
//!    which step 4 leaves unreachable, would be [`FitError::FeatureContractMismatch`]. A first
//!    note that reaches the reference's end has no release window: its release features cannot
//!    be computed (`patchback.analysis.FeatureNotComputable`, "holds no frame centre") and the
//!    seeded fit is refused so, while [`fit`] of the same inputs runs (Da11-22). A sequence of
//!    more than one note is not refused; the prediction is of its first.
//!
//! # The search
//!
//! The wall-time clock starts when [`fit_request`] (or [`fit`]) is entered, before the
//! refusals, so it covers the reference analysis made here and, on a seeded run, the first
//! note's analysis and the prediction; an analysis the caller made and gave is outside it
//! (Da11-13, Da11-23). The run starts ([`FitRun::start`]) once the
//! refusals have passed. A point is a vector of normalized values in `[0, 1]`, one per parameter
//! contract, at the contract's index.
//!
//! - **Restarts.** Restart 0 starts at the run's start ([`FitOutcome::init`]): the default point
//!   ([`default_point`]: every parameter at its contract default, the fixed safe default patch of
//!   § 9.2 step 1), or, from seed inputs, the prediction's clamped values, each `f32` as `f64`,
//!   output `k` at index `k` ([`SeededStart::point`], Da11-9); a seeded run evaluates no default
//!   point. Restart `r >= 1` starts
//!   at the point whose value at index `i` is the `i`-th draw of
//!   `Xoshiro256StarStar::new(s_r).next_f64()` (`patchback_core::prng`), in index order, where
//!   `s_r` is the `r`-th output of `SplitMix64::new(seed)`, numbered from 1: restart 1 takes the
//!   first `next_u64`, restart 2 the second ([`restart_point`]). Nearby seeds and restarts so
//!   start apart (the earlier `seed ^ r` gave seed 1 restart 2 and seed 2 restart 1 one start).
//!   At most `max_restarts` restarts run. A restart `r >= 1` begins only while no bound is
//!   reached (see [Stopping](#stopping)), and its start is then evaluated without a second
//!   check, so every restart that began rendered at least its start.
//! - **Evaluation.** A point becomes the patch [`patch_at`] builds, validated and compiled
//!   (`patchback_core::patch`); it is rendered with `config.render` against the note sequence
//!   (`patchback_dsp::render`), the render is cut to the reference's length or padded with
//!   silence to it, and that clip is compared with the reference ([`crate::compare`]). The run
//!   counts the result: a valid loss through [`FitRun::evaluate`], a refused render or
//!   comparison through [`FitRun::reject`] (D-a6-6). A patch that validation refuses fails the
//!   run with `RendererFault` (unreachable: [`patch_at`] keeps every value in its contract's
//!   range).
//! - **Coordinate search.** From the restart's start, a sweep visits the parameters in index
//!   order; for each it tries the current point with that value raised by the step, then
//!   lowered by it, each clamped to `[0, 1]` (a value the clamp leaves unchanged is not
//!   rendered). The first candidate whose loss is strictly lower than the current point's
//!   becomes the current point and the sweep moves to the next parameter. The step starts at
//!   [`INITIAL_STEP`]. After a sweep:
//!   - no candidate was better: the step halves, and the restart ends when it falls below
//!     [`MIN_STEP`];
//!   - the sweep lowered the current loss by less than `improvement_threshold`: the restart
//!     ends;
//!   - otherwise the next sweep runs with the same step.
//!
//!   A rejected start counts as no loss, which any valid loss improves on.
//! - **Retention.** The run retains a candidate only when its loss is strictly lower than the
//!   best so far ([`FitRun::evaluate`]), whatever the restart; an improvement below the
//!   threshold is still retained (OQ-14 stays open).
//!
//! # Stopping
//!
//! The bounds are checked before every render but the run's first and before every restart
//! `r >= 1` begins. The run stops at the first of:
//!
//! - **`RenderBudgetExhausted`**: a render is due and the run has counted `max_renders`
//!   evaluations. Then `evaluations == max_renders`.
//! - **`TimeBudgetExhausted`**: a render is due, it is not the run's first candidate, and the
//!   wall time since the fit was entered has reached `max_wall_time_ms`: no render starts once
//!   the bound is reached. The run's first candidate (the start of restart 0) is always
//!   evaluated, so a time bound stops a run instead of failing it, and the run holds a patch
//!   whenever that candidate was valid. A render in progress is not interrupted. The recorded
//!   `wall_time_ms` is the whole run's, from entry to completion, the reference analysis made
//!   here, and on a seeded run the first note's analysis and the prediction, included, so it may
//!   exceed the bound by at most those and one evaluation (the first, or the one running when
//!   the bound passed; Da6-29). The model (`FitRun` in
//!   `spec/domains/fit.yaml`) bounds `evaluations`, not `wall_time_ms`; DESIGN.md § 9.2 calls
//!   the budget an offline upper bound, not a real-time guarantee (Da6-22).
//! - **`ImprovementBelowThreshold`**: the early end of the restart loop: a restart `r >= 1`
//!   ended having lowered the run's best loss by less than `improvement_threshold` (by nothing
//!   at all included); later restarts do not run.
//! - **`RestartsExhausted`**: the loop completed its `max_restarts` restarts without that. With
//!   `max_restarts` 1 a natural end is always `RestartsExhausted`, whatever the last sweep
//!   improved (Da6-23).
//!
//! The run then completes with that reason ([`FitRun::complete`]). A run that retained no valid
//! candidate cannot complete; it fails with `NoValidCandidate` and [`fit`] returns
//! [`FitError::NoValidCandidate`].
//!
//! # Determinism
//!
//! The search reads no clock except for the wall-time bound, and no other state: the same
//! reference, note sequence, topology, configuration and seed inputs (the predictor runs in
//! `f32` in a fixed order) give the same log, patch and render on
//! every run and platform the renderer and the comparison are bit-exact on (§ 2 invariant 5),
//! unless the wall-time bound stops the run.

use std::fmt;
use std::time::Instant;

use patchback_analysis::{
    Analysis, AnalysisConfig, AnalysisError, FEATURES_1_VALUE_COUNT, FEATURES_1_VERSION,
    FeatureContractDefinition, FeatureRegistry, NormalizationTable, NoteSpan,
    RELEASE_OFFSET_FRAMES, RELEASE_WINDOW_FRAMES, SAMPLE_RATE_HZ, analyze_and_extract,
    analyze_clip,
};
use patchback_audio::{AudioClip, WavError};
use patchback_core::digest::DIGEST_BYTES;
use patchback_core::parameter::{SetParameterError, Topology};
use patchback_core::patch::validate::{FIXED_CHAIN_EDGES, FIXED_CHAIN_KINDS};
use patchback_core::patch::{
    EdgeDecl, FORMAT_VERSION, NodeDecl, NodeKind, ParameterValue, Patch, Port, PortRate,
    SUPPORTED_SAMPLE_RATE_HZ, compile, known_topology, validate,
};
use patchback_core::prng::{SplitMix64, Xoshiro256StarStar};
use patchback_core::score::NoteSequence;
use patchback_dsp::{RenderConfig, RenderError, RenderReport, render};
use patchback_model::{PredictError, Prediction, Predictor};

use crate::run::{EvaluationOutcome, FailureReason, FitBounds, FitRun, StopReason};
use crate::{
    CompareError, FitError, INIT_DEFAULT, INIT_REGRESSOR_PREDICTION, LossComponents, compare,
};

/// The step of the first sweep of every restart, in normalized units (D-a6-2).
pub const INITIAL_STEP: f64 = 0.25;

/// A restart ends when its step halves below this: the steps are 0.25, 0.125, 0.0625 and
/// 0.03125. Provisional: DESIGN.md § 9.2 names no smallest step.
pub const MIN_STEP: f64 = 1.0 / 32.0;

/// Everything a fit is configured with besides its inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FitConfig {
    pub bounds: FitBounds,
    /// The seed of the restarts' starting points.
    pub seed: u64,
    /// The render configuration of every candidate.
    pub render: RenderConfig,
}

impl Default for FitConfig {
    /// The default bounds, seed 0 and the default render configuration (256-frame blocks).
    fn default() -> Self {
        Self {
            bounds: FitBounds::default(),
            seed: 0,
            render: RenderConfig::default(),
        }
    }
}

/// Where a run starts (`patchback.fit.InitSource`, DESIGN.md § 9.2 step 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitSource {
    /// The fixed safe default patch, [`default_point`]: no weight artifact was given.
    DefaultPatch,
    /// The native regressor's prediction from the reference's features ([`SeededStart`]).
    RegressorPrediction,
}

impl InitSource {
    /// The variant's name in `spec/domains/fit.yaml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::DefaultPatch => "DefaultPatch",
            Self::RegressorPrediction => "RegressorPrediction",
        }
    }

    /// The value of the `init` record of `run.meta` and of `patchback fit`'s stdout:
    /// [`INIT_DEFAULT`] or [`INIT_REGRESSOR_PREDICTION`].
    pub fn record(self) -> &'static str {
        match self {
            Self::DefaultPatch => INIT_DEFAULT,
            Self::RegressorPrediction => INIT_REGRESSOR_PREDICTION,
        }
    }
}

impl fmt::Display for InitSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What a regressor-seeded fit predicts its start from (`StartFit`'s `weight_artifact_id`;
/// Da11-3, Da11-4): a loaded weight artifact and the normalization statistics of the training
/// split it was trained on. The caller reads both; [`fit_request`] reads no file.
#[derive(Clone, Debug)]
pub struct SeedInputs {
    /// The weight artifact's predictor (`patchback_model::Predictor::load` or `from_artifact`).
    pub predictor: Predictor,
    /// The statistics (`patchback_analysis::read_stats` of a dataset's `normalization.stats`).
    pub normalization: NormalizationTable,
    /// The SHA-256 of the bytes the statistics were read from; `run.meta` records it.
    pub normalization_sha256: [u8; DIGEST_BYTES],
}

/// A fit as `StartFit` names it (Da11-4): what [`fit`] takes, and the two inputs of a
/// regressor-seeded fit. With neither, it is [`fit`]'s run exactly.
#[derive(Clone, Debug)]
pub struct FitRequest<'a> {
    pub reference: &'a AudioClip,
    pub notes: &'a NoteSequence,
    pub topology: &'a Topology,
    pub config: &'a FitConfig,
    /// The reference's analysis (`analysis_id`), made with the `features/1` configuration
    /// (`patchback_analysis::AnalysisConfig::default()`); it admits the reference instead of the
    /// fit's own analysis, and must be of the reference's frame count and sample rate: with
    /// [`Self::seed`] another is refused as [`FitError::PredictionFailed`], without it it is not
    /// used (module documentation, step 5; Da11-24). Required with [`Self::seed`]; the prediction
    /// does not read it, but analyses the first note's own clip (step 6; Da11-21, Da11-25).
    pub analysis: Option<&'a Analysis>,
    /// The weight artifact and statistics (`weight_artifact_id`): the run starts from their
    /// prediction ([`InitSource::RegressorPrediction`]).
    pub seed: Option<SeedInputs>,
}

/// The start of a regressor-seeded run and what reproduces it (`run.meta`, Da11-10).
#[derive(Clone, Debug, PartialEq)]
pub struct SeededStart {
    /// The normalized `features/1` vector the predictor ran on.
    pub features: Vec<f64>,
    /// The predictor's output: `raw`, the `values` clamped to `[0, 1]`, and how many were
    /// clamped (`clamped > 0` is the report's warning, Da11-11).
    pub prediction: Prediction,
    /// The weight artifact's checksum: the SHA-256 of its payload, 64 lowercase hex digits.
    pub weights_sha256: String,
    /// The ModelSpec version and commit the artifact names.
    pub modelspec_version: String,
    pub modelspec_commit: String,
    /// The SHA-256 of the statistics' bytes ([`SeedInputs::normalization_sha256`]).
    pub normalization_sha256: [u8; DIGEST_BYTES],
}

impl SeededStart {
    /// Restart 0's start: the clamped values, each `f32` as `f64`, output `k` at index `k`.
    pub fn point(&self) -> Vec<f64> {
        self.prediction
            .values
            .iter()
            .map(|v| f64::from(*v))
            .collect()
    }
}

/// Why a candidate was counted as rejected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rejection {
    /// The renderer refused the patch with the note sequence.
    Render(RenderError),
    /// The rendered samples did not make a clip.
    Audio(WavError),
    /// The comparison refused the rendered clip.
    Compare(CompareError),
    /// The loss or a component was not finite and non-negative.
    NonFiniteLoss,
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Render(error) => write!(f, "{}: {error}", error.domain_error()),
            Self::Audio(error) => write!(f, "{}: {error}", error.domain_error()),
            Self::Compare(error) => write!(f, "{}: {error}", error.domain_error()),
            Self::NonFiniteLoss => f.write_str("the loss is not finite and non-negative"),
        }
    }
}

/// One counted evaluation, as § 9.1 asks every candidate to be logged.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    /// 1 for the first evaluation; also the run's evaluation count after it.
    pub sequence: u64,
    /// The restart it belongs to, from 0.
    pub restart: u32,
    pub outcome: EvaluationOutcome,
    /// The candidate's normalized values, at their contracts' indices; [`patch_at`] gives its
    /// patch.
    pub normalized: Vec<f64>,
    /// The loss components, for a candidate that was not rejected.
    pub components: Option<LossComponents>,
    /// The weighted total, for a candidate that was not rejected.
    pub total: Option<f64>,
    /// The run's best loss after this evaluation.
    pub best_after: Option<f64>,
    /// Why the candidate was rejected, for a rejected one.
    pub rejection: Option<Rejection>,
}

/// A completed fit: the run, its best patch and render, and the log of every evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct FitOutcome {
    /// The completed run; its candidate is the [`Evaluation::sequence`] of the retained best.
    pub run: FitRun<u64>,
    /// The restarts that began; each rendered at least its start.
    pub restarts: u32,
    /// The best candidate's normalized values.
    pub normalized: Vec<f64>,
    /// The best patch, in physical values ([`patch_at`] of [`Self::normalized`]).
    pub patch: Patch,
    /// The clip the best patch was scored on: its render cut or padded to the reference's
    /// length. Comparing it with the reference gives the best loss again.
    pub render: AudioClip,
    /// What the renderer reported for the best patch's render.
    pub render_report: RenderReport,
    /// Every counted evaluation, in order.
    pub log: Vec<Evaluation>,
    /// Where restart 0 started.
    pub init: InitSource,
    /// The prediction restart 0 started from, for [`InitSource::RegressorPrediction`].
    pub seeded: Option<SeededStart>,
}

/// The default point of `topology`: every parameter at its contract's normalized default.
pub fn default_point(topology: &Topology) -> Vec<f64> {
    let mut point = vec![0.0; topology.contracts().len()];
    for contract in topology.contracts() {
        if let Some(value) = point.get_mut(contract.index) {
            *value = contract.default_normalized;
        }
    }
    point
}

/// The starting point of restart `restart >= 1` of a run seeded with `seed`, `len` values: the
/// first `len` draws of `Xoshiro256StarStar::new(s)`, where `s` is the `restart`-th output of
/// `SplitMix64::new(seed)` (module documentation). Restart 0 starts at [`default_point`] or at
/// the prediction ([`SeededStart::point`]), never here.
pub fn restart_point(seed: u64, restart: u32, len: usize) -> Vec<f64> {
    let mut mix = SplitMix64::new(seed);
    let mut restart_seed = 0;
    for _ in 0..restart {
        restart_seed = mix.next_u64();
    }
    let mut prng = Xoshiro256StarStar::new(restart_seed);
    (0..len).map(|_| prng.next_f64()).collect()
}

/// The patch of the normalized point `normalized`: the fixed chain of the topology
/// (`FIXED_CHAIN_KINDS` as nodes 1 to 7 in their order, `FIXED_CHAIN_EDGES` between them, each
/// port's rate the one its kind declares), each parameter at the physical value its contract
/// maps `normalized[contract.index]` to. A value outside `[0, 1]` is clamped
/// (`ParameterContract::apply`); a missing one is the contract's default; the physical value is
/// held inside the contract's range against rounding at its ends. Refused only for a NaN or
/// infinite value, or a node parameter with no contract in `topology`.
pub fn patch_at(topology: &Topology, normalized: &[f64]) -> Result<Patch, SetParameterError> {
    let value_of = |path: &str| -> Result<f64, SetParameterError> {
        let contract =
            topology
                .contract(path)
                .ok_or_else(|| SetParameterError::UnknownParameterPath {
                    path: path.to_owned(),
                })?;
        let p = normalized
            .get(contract.index)
            .copied()
            .unwrap_or(contract.default_normalized);
        let physical = contract.apply(p)?.physical;
        let low = contract.physical_at_zero.min(contract.physical_at_one);
        let high = contract.physical_at_zero.max(contract.physical_at_one);
        Ok(physical.clamp(low, high))
    };
    let id_of = |kind: NodeKind| -> u32 {
        FIXED_CHAIN_KINDS
            .iter()
            .position(|k| *k == kind)
            .map_or(0, |position| position as u32 + 1)
    };
    let rate_of = |ports: &[Port], name: &str| -> PortRate {
        ports
            .iter()
            .find(|port| port.name == name)
            .map_or(PortRate::Audio, |port| port.rate)
    };
    let mut nodes = Vec::with_capacity(FIXED_CHAIN_KINDS.len());
    for kind in FIXED_CHAIN_KINDS {
        let mut parameters = Vec::new();
        for path in kind.parameter_paths() {
            parameters.push(ParameterValue {
                name: (*path).to_owned(),
                value: value_of(path)?,
            });
        }
        nodes.push(NodeDecl {
            id: id_of(kind),
            kind,
            parameters,
        });
    }
    let edges = FIXED_CHAIN_EDGES
        .iter()
        .map(|edge| EdgeDecl {
            from_node: id_of(edge.from_kind),
            from_port: edge.from_port.to_owned(),
            from_rate: rate_of(edge.from_kind.output_ports(), edge.from_port),
            to_node: id_of(edge.to_kind),
            to_port: edge.to_port.to_owned(),
            to_rate: rate_of(edge.to_kind.input_ports(), edge.to_port),
        })
        .collect();
    Ok(Patch {
        format_version: FORMAT_VERSION,
        sample_rate_hz: SUPPORTED_SAMPLE_RATE_HZ,
        topology_id: topology.name().to_owned(),
        nodes,
        edges,
    })
}

/// The fit's refusal of a reference whose analysis refused it with `error` (module
/// documentation, step 5): `SilentInput` is [`FitError::ReferenceSilent`], a pitch refusal
/// [`FitError::PitchAmbiguous`], any other [`FitError::Compare`]. For a caller that analyses
/// the reference itself (`patchback fit` with a weight artifact).
pub fn reference_refusal(error: AnalysisError) -> FitError {
    match error {
        cause @ AnalysisError::SilentInput { .. } => FitError::ReferenceSilent { cause },
        cause @ (AnalysisError::PitchAmbiguous { .. }
        | AnalysisError::ClipTooShortForPitch { .. }
        | AnalysisError::PitchOutOfRange { .. }) => FitError::PitchAmbiguous { cause },
        error => FitError::Compare(CompareError::Analysis(error)),
    }
}

/// Whether `analysis` can be the analysis of `reference`: the same frame count at the same rate
/// (module documentation, step 5; Da11-24).
fn is_of(analysis: &Analysis, reference: &AudioClip) -> bool {
    analysis.frame_count == reference.frame_count()
        && analysis.sample_rate_hz == reference.sample_rate_hz()
}

/// Refuses a reference the fit cannot run against (module documentation, step 5).
fn check_reference(reference: &AudioClip) -> Result<(), FitError> {
    analyze_clip(reference, &AnalysisConfig::default())
        .map(|_| ())
        .map_err(reference_refusal)
}

/// Refuses seed inputs of another feature contract than the analysis, `features/1` (module
/// documentation, step 4): the weight artifact's, then the statistics'.
pub fn check_seed_inputs(seed: &SeedInputs) -> Result<(), FitError> {
    let mismatch = |input: &'static str, found: &str| FitError::FeatureContractMismatch {
        input,
        expected: FEATURES_1_VERSION.to_owned(),
        found: found.to_owned(),
    };
    let artifact = seed.predictor.artifact().feature_contract();
    if artifact != FEATURES_1_VERSION {
        return Err(mismatch("weight artifact", artifact));
    }
    let statistics = seed.normalization.feature_contract_version();
    if statistics != FEATURES_1_VERSION {
        return Err(mismatch("normalization statistics", statistics));
    }
    Ok(())
}

/// The fit's refusal of a prediction the predictor refused (Da11-12).
fn prediction_refusal(error: PredictError) -> FitError {
    match error {
        PredictError::FeatureContractMismatch { artifact, input } => {
            FitError::FeatureContractMismatch {
                input: "weight artifact",
                expected: input,
                found: artifact,
            }
        }
        error => FitError::PredictionFailed {
            message: match error.domain_error() {
                Some(name) => format!("{name}: {error}"),
                None => format!("the prediction failed: {error}"),
            },
        },
    }
}

/// Predicts the start of a regressor-seeded run of `topology` from the first note of `notes` in
/// `reference`, analysed on its own clip as the dataset analyses a training example (module
/// documentation, step 6; Da11-21).
fn predict_start(
    reference: &AudioClip,
    notes: &NoteSequence,
    topology: &Topology,
    seed: SeedInputs,
) -> Result<SeededStart, FitError> {
    let failed = |message: String| FitError::PredictionFailed { message };
    let feature = |error: patchback_analysis::FeatureError| {
        failed(format!("{}: {error}", error.domain_error()))
    };
    let note = notes
        .notes
        .first()
        .ok_or_else(|| failed("the note sequence holds no note to read features from".into()))?;
    let frames = reference.frame_count();
    let clamped = |sample: u64| usize::try_from(sample).map_or(frames, |s| s.min(frames));
    let (start, end) = (clamped(note.start_sample), clamped(note.end_sample));
    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .map_err(feature)?;
    registry.publish(id).map_err(feature)?;
    let contract = registry.get(id).map_err(feature)?;
    let tail_end = end
        .saturating_add(RELEASE_OFFSET_FRAMES)
        .saturating_add(RELEASE_WINDOW_FRAMES)
        .min(frames);
    let span = NoteSpan {
        start_sample: 0,
        end_sample: end.saturating_sub(start),
    };
    let vector = analyze_and_extract(
        contract,
        &reference.samples()[start.min(tail_end)..tail_end],
        SAMPLE_RATE_HZ,
        span,
    )
    .map_err(feature)?;
    let raw: [f64; FEATURES_1_VALUE_COUNT] = vector.values.as_slice().try_into().map_err(|_| {
        failed(format!(
            "the feature vector holds {} values, not {FEATURES_1_VALUE_COUNT}",
            vector.values.len()
        ))
    })?;
    let features = seed.normalization.normalize(&raw);
    if let Some(index) = features
        .iter()
        .position(|value| !value.is_finite() || !(*value as f32).is_finite())
    {
        return Err(failed(format!(
            "normalized feature {index} is {}, not a finite f32",
            features[index]
        )));
    }
    let mut predictor = seed.predictor;
    let prediction = predictor
        .predict(FEATURES_1_VERSION, &features)
        .map_err(prediction_refusal)?;
    if prediction.values.len() != topology.contracts().len() {
        return Err(failed(format!(
            "the prediction holds {} values; the topology has {} parameters",
            prediction.values.len(),
            topology.contracts().len()
        )));
    }
    let artifact = predictor.artifact();
    Ok(SeededStart {
        features: features.to_vec(),
        prediction,
        weights_sha256: artifact.checksum().to_owned(),
        modelspec_version: artifact.modelspec_version().to_owned(),
        modelspec_commit: artifact.modelspec_commit().to_owned(),
        normalization_sha256: seed.normalization_sha256,
    })
}

/// Why the search stopped early: a bound, or a renderer fault with its message.
enum Halt {
    Stop(StopReason),
    Fault(String),
}

/// How much `after` lowers `before`: infinite from no loss to a loss, 0 without a loss after.
fn gain(before: Option<f64>, after: Option<f64>) -> f64 {
    match (before, after) {
        (Some(before), Some(after)) => before - after,
        (None, Some(_)) => f64::INFINITY,
        (_, None) => 0.0,
    }
}

/// The best candidate's patch and render, kept beside the run.
struct Best {
    normalized: Vec<f64>,
    patch: Patch,
    render: AudioClip,
    report: RenderReport,
}

/// The state of one fit while it searches.
struct Search<'a> {
    reference: &'a AudioClip,
    notes: &'a NoteSequence,
    topology: &'a Topology,
    config: &'a FitConfig,
    started: Instant,
    /// Restart 0's start: the default point or the prediction.
    start: Vec<f64>,
    run: FitRun<u64>,
    restart: u32,
    log: Vec<Evaluation>,
    best: Option<Best>,
}

impl Search<'_> {
    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// The bound that refuses the next render or the next restart, if one does. The run's first
    /// candidate is never asked about (module documentation).
    fn bound_reached(&self) -> Option<StopReason> {
        let bounds = &self.config.bounds;
        if self.run.evaluations() >= bounds.max_renders {
            return Some(StopReason::RenderBudgetExhausted);
        }
        let limit = u64::try_from(bounds.max_wall_time_ms).unwrap_or(0);
        if self.elapsed_ms() >= limit {
            return Some(StopReason::TimeBudgetExhausted);
        }
        None
    }

    /// Renders and scores `point` unless a bound refuses it; its loss, or `None` when it was
    /// rejected.
    fn evaluate(&mut self, point: &[f64]) -> Result<Option<f64>, Halt> {
        if let Some(reason) = self.bound_reached() {
            return Err(Halt::Stop(reason));
        }
        self.measure(point)
    }

    /// Renders and scores `point` without asking the bounds: the run's first candidate, and the
    /// start of a restart that [`Self::restarts`] has just let begin.
    fn measure(&mut self, point: &[f64]) -> Result<Option<f64>, Halt> {
        let patch =
            patch_at(self.topology, point).map_err(|error| Halt::Fault(error.to_string()))?;
        let validated = validate(&patch)
            .map_err(|error| Halt::Fault(format!("{}: {error}", error.spec_error())))?;
        let plan = compile(&validated);
        let sequence = u64::try_from(self.run.evaluations()).unwrap_or(0) + 1;
        let measured = render(&plan, self.notes, &self.config.render)
            .map_err(Rejection::Render)
            .and_then(|rendered| {
                let mut samples = rendered.samples;
                samples.resize(self.reference.frame_count(), 0.0);
                let clip = AudioClip::from_mono(samples).map_err(Rejection::Audio)?;
                let comparison = compare(self.reference, &clip).map_err(Rejection::Compare)?;
                Ok((comparison, clip, rendered.report))
            });
        let (outcome, scored, loss) = match measured {
            Ok((comparison, clip, report)) => {
                let outcome = self
                    .run
                    .evaluate(sequence, comparison.components, comparison.total)
                    .map_err(halt)?;
                if outcome.retained() {
                    self.best = Some(Best {
                        normalized: point.to_vec(),
                        patch,
                        render: clip,
                        report,
                    });
                }
                if outcome == EvaluationOutcome::Rejected {
                    (outcome, Err(Rejection::NonFiniteLoss), None)
                } else {
                    (outcome, Ok(comparison), Some(comparison.total))
                }
            }
            Err(rejection) => (self.run.reject().map_err(halt)?, Err(rejection), None),
        };
        self.log.push(Evaluation {
            sequence,
            restart: self.restart,
            outcome,
            normalized: point.to_vec(),
            components: scored.as_ref().ok().map(|c| c.components),
            total: scored.as_ref().ok().map(|c| c.total),
            best_after: self.run.best_loss(),
            rejection: scored.err(),
        });
        Ok(loss)
    }

    /// One restart's coordinate search from `start`, which is evaluated unasked (module
    /// documentation).
    fn descend(&mut self, start: Vec<f64>) -> Result<(), Halt> {
        let threshold = self.config.bounds.improvement_threshold;
        let mut current_loss = self.measure(&start)?;
        let mut current = start;
        let mut step = INITIAL_STEP;
        loop {
            let sweep_start = current_loss;
            for index in 0..current.len() {
                for direction in [1.0, -1.0] {
                    let value = (current[index] + direction * step).clamp(0.0, 1.0);
                    if value == current[index] {
                        continue;
                    }
                    let mut candidate = current.clone();
                    candidate[index] = value;
                    let loss = self.evaluate(&candidate)?;
                    let lower = match (loss, current_loss) {
                        (Some(loss), Some(current)) => loss < current,
                        (Some(_), None) => true,
                        (None, _) => false,
                    };
                    if lower {
                        current = candidate;
                        current_loss = loss;
                        break;
                    }
                }
            }
            let improvement = gain(sweep_start, current_loss);
            if improvement > 0.0 {
                if improvement < threshold {
                    return Ok(());
                }
            } else {
                step /= 2.0;
                if step < MIN_STEP {
                    return Ok(());
                }
            }
        }
    }

    /// The restarts, until a bound stops the run or they end (module documentation).
    fn restarts(&mut self) -> Result<StopReason, Halt> {
        let bounds = self.config.bounds;
        let len = self.topology.contracts().len();
        let count = u32::try_from(bounds.max_restarts).unwrap_or(u32::MAX);
        for restart in 0..count {
            if restart >= 1
                && let Some(reason) = self.bound_reached()
            {
                return Ok(reason);
            }
            self.restart = restart;
            let before = self.run.best_loss();
            let start = if restart == 0 {
                self.start.clone()
            } else {
                restart_point(self.config.seed, restart, len)
            };
            self.descend(start)?;
            if restart >= 1 && gain(before, self.run.best_loss()) < bounds.improvement_threshold {
                return Ok(StopReason::ImprovementBelowThreshold);
            }
        }
        Ok(StopReason::RestartsExhausted)
    }
}

/// A command the search admitted beforehand was refused: only the render budget can be, and
/// [`Search::bound_reached`] checks it first.
fn halt(error: FitError) -> Halt {
    match error {
        FitError::RenderBudgetExhausted => Halt::Stop(StopReason::RenderBudgetExhausted),
        error => Halt::Fault(error.to_string()),
    }
}

/// Fits a patch of `topology` to `reference`, driven by `notes`, within `config`'s bounds,
/// from the default patch (module documentation): [`fit_request`] with neither an analysis nor
/// seed inputs. Returns the completed run with its best patch, render and log, or the refusal;
/// a run that fails returns [`FitError::NoValidCandidate`] or [`FitError::RendererFault`].
pub fn fit(
    reference: &AudioClip,
    notes: &NoteSequence,
    topology: &Topology,
    config: &FitConfig,
) -> Result<FitOutcome, FitError> {
    fit_request(FitRequest {
        reference,
        notes,
        topology,
        config,
        analysis: None,
        seed: None,
    })
}

/// `StartFit` (module documentation): the fit of [`fit`], started from the regressor's
/// prediction when the request carries seed inputs and the reference's analysis. Refused in the
/// order of the module documentation; a run that fails returns [`FitError::NoValidCandidate`]
/// or [`FitError::RendererFault`].
pub fn fit_request(request: FitRequest<'_>) -> Result<FitOutcome, FitError> {
    let started = Instant::now();
    let FitRequest {
        reference,
        notes,
        topology,
        config,
        analysis,
        seed,
    } = request;
    config.bounds.check()?;
    if let Some(seed) = &seed
        && analysis.is_none()
    {
        return Err(FitError::AnalysisRequired {
            weights_sha256: seed.predictor.artifact().checksum().to_owned(),
        });
    }
    if known_topology(topology.name()).is_none() {
        return Err(FitError::TopologyNotFound {
            topology: topology.name().to_owned(),
        });
    }
    if let Some(seed) = &seed {
        check_seed_inputs(seed)?;
    }
    let admitted = analysis.filter(|analysis| is_of(analysis, reference));
    if let (Some(_), Some(analysis), None) = (&seed, analysis, admitted) {
        return Err(FitError::PredictionFailed {
            message: format!(
                "the analysis holds {} frames at {} Hz; the reference {} frames at {} Hz",
                analysis.frame_count,
                analysis.sample_rate_hz,
                reference.frame_count(),
                reference.sample_rate_hz()
            ),
        });
    }
    if admitted.is_none() {
        check_reference(reference)?;
    }
    let seeded = match (seed, admitted) {
        (Some(seed), Some(_)) => Some(predict_start(reference, notes, topology, seed)?),
        _ => None,
    };
    let (init, start) = match &seeded {
        Some(seeded) => (InitSource::RegressorPrediction, seeded.point()),
        None => (InitSource::DefaultPatch, default_point(topology)),
    };
    let mut search = Search {
        reference,
        notes,
        topology,
        config,
        started,
        start,
        run: FitRun::start(config.bounds)?,
        restart: 0,
        log: Vec::new(),
        best: None,
    };
    let ended = search.restarts();
    let wall_time_ms = search.elapsed_ms();
    let reason = match ended {
        Ok(reason) | Err(Halt::Stop(reason)) => reason,
        Err(Halt::Fault(message)) => {
            search
                .run
                .fail(FailureReason::RendererFault, wall_time_ms)?;
            return Err(FitError::RendererFault { message });
        }
    };
    match search.run.complete(reason, wall_time_ms) {
        Ok(()) => {}
        Err(FitError::NoValidCandidate) => {
            search
                .run
                .fail(FailureReason::NoValidCandidate, wall_time_ms)?;
            return Err(FitError::NoValidCandidate);
        }
        Err(error) => return Err(error),
    }
    let Some(best) = search.best else {
        return Err(FitError::NoValidCandidate);
    };
    Ok(FitOutcome {
        run: search.run,
        restarts: search.restart + 1,
        normalized: best.normalized,
        patch: best.patch,
        render: best.render,
        render_report: best.report,
        log: search.log,
        init,
        seeded,
    })
}
