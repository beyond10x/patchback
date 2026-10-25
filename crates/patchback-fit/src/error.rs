//! The typed refusals of the comparison and of the fit. Each [`CompareError`] names one error of
//! `spec/domains/fit.yaml`, or, for an input analysis refuses, of `spec/domains/analysis.yaml`
//! ([`CompareError::domain_error`]); each [`FitError`] one error of `spec/domains/fit.yaml`, or
//! the comparison refusal it passes on ([`FitError::domain_error`]).

use std::fmt;

use patchback_analysis::AnalysisError;

use crate::ALIGNMENT_TOLERANCE_FRAMES;
use crate::benchmark::{BenchmarkKind, BenchmarkState};
use crate::run::{FitState, SearchBound};

/// Why nothing was compared. A refusal produces no partial result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompareError {
    /// `patchback.fit.SampleRateMismatch`: the two clips are at different rates; neither is
    /// converted.
    SampleRateMismatch {
        reference_hz: u32,
        candidate_hz: u32,
    },
    /// `patchback.fit.LengthOutsideAlignment`: the lengths differ by more than
    /// [`ALIGNMENT_TOLERANCE_FRAMES`].
    LengthOutsideAlignment {
        reference_frames: usize,
        candidate_frames: usize,
    },
    /// An input analysis refuses: an empty clip, a non-finite sample, a clip past the analysis
    /// duration bound, or a common rate other than 48 kHz; named by its analysis error.
    Analysis(AnalysisError),
}

impl CompareError {
    /// Every error of `spec/` this crate can emit: the two of `CompareAudio` it decides, and
    /// every analysis error, which [`CompareError::Analysis`] passes on.
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
        "patchback.fit.SampleRateMismatch",
        "patchback.fit.LengthOutsideAlignment",
        "patchback.analysis.EmptyAudio",
        "patchback.analysis.UnsupportedSampleRate",
        "patchback.analysis.InvalidHopLength",
        "patchback.analysis.FftLengthNotPowerOfTwo",
        "patchback.analysis.FftLengthOverBound",
        "patchback.analysis.DurationOverBound",
        "patchback.analysis.FrameCountOverBound",
        "patchback.analysis.NonFiniteSamples",
        "patchback.analysis.SilentInput",
        "patchback.analysis.ClipTooShortForPitch",
        "patchback.analysis.PitchAmbiguous",
        "patchback.analysis.PitchOutOfRange",
    ];

    /// The error of `spec/` this refusal is.
    pub fn domain_error(&self) -> &'static str {
        match self {
            Self::SampleRateMismatch { .. } => "patchback.fit.SampleRateMismatch",
            Self::LengthOutsideAlignment { .. } => "patchback.fit.LengthOutsideAlignment",
            Self::Analysis(error) => error.domain_error(),
        }
    }

    /// One value of every variant, and every analysis refusal. The `match` below has no
    /// wildcard, so a new variant does not compile until it is listed here, and the domain test
    /// then checks its name.
    pub fn examples() -> Vec<CompareError> {
        let mut examples = vec![
            Self::SampleRateMismatch {
                reference_hz: 48_000,
                candidate_hz: 44_100,
            },
            Self::LengthOutsideAlignment {
                reference_frames: 48_000,
                candidate_frames: 60_000,
            },
        ];
        examples.extend(AnalysisError::examples().into_iter().map(Self::Analysis));
        for example in &examples {
            match example {
                Self::SampleRateMismatch { .. }
                | Self::LengthOutsideAlignment { .. }
                | Self::Analysis(_) => {}
            }
        }
        examples
    }
}

impl From<AnalysisError> for CompareError {
    fn from(error: AnalysisError) -> Self {
        Self::Analysis(error)
    }
}

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SampleRateMismatch {
                reference_hz,
                candidate_hz,
            } => write!(
                f,
                "the reference is at {reference_hz} Hz and the candidate at {candidate_hz} Hz; \
                 clips are compared at one rate and never converted"
            ),
            Self::LengthOutsideAlignment {
                reference_frames,
                candidate_frames,
            } => write!(
                f,
                "the reference has {reference_frames} frames and the candidate \
                 {candidate_frames}; they differ by {}, more than the {ALIGNMENT_TOLERANCE_FRAMES} \
                 frames time alignment absorbs",
                reference_frames.abs_diff(*candidate_frames)
            ),
            Self::Analysis(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CompareError {}

/// Why a fit refused to start, why a command of a [`crate::FitRun`] was refused, why a run
/// ended without a patch, why a run's results were not written, or why a command of a
/// [`crate::Benchmark`] was refused. A refusal produces no partial result.
#[derive(Clone, Debug, PartialEq)]
pub enum FitError {
    /// `patchback.fit.InvalidSearchBound`: a render, wall-time or restart bound is not positive.
    InvalidSearchBound { bound: SearchBound, value: i64 },
    /// `patchback.fit.TopologyNotFound`: the renderer knows no topology of this name, so no
    /// patch of it can be generated.
    TopologyNotFound { topology: String },
    /// `patchback.fit.ReferenceSilent`: the reference's analysis found it silent
    /// (`patchback.analysis.SilentInput`).
    ReferenceSilent { cause: AnalysisError },
    /// `patchback.fit.PitchAmbiguous`: the reference's analysis could not estimate its
    /// fundamental (`patchback.analysis.PitchAmbiguous`, `ClipTooShortForPitch` or
    /// `PitchOutOfRange`, D-a6-4).
    PitchAmbiguous { cause: AnalysisError },
    /// `patchback.fit.FitNotRunning`: the run has ended, so it takes no further candidate and no
    /// second ending.
    FitNotRunning { state: FitState },
    /// `patchback.fit.RenderBudgetExhausted`: the run has spent its render evaluations; the
    /// candidate is not counted.
    RenderBudgetExhausted,
    /// `patchback.fit.NoValidCandidate`: the run holds no valid candidate, so it cannot complete;
    /// a fit that ends so has failed with `FailureReason::NoValidCandidate`.
    NoValidCandidate,
    /// A fit that failed with `FailureReason::RendererFault`: a generated patch was refused by
    /// graph validation (D-a6-6; unreachable while every point of the unit cube maps into the
    /// contracts' ranges). The run has no valid patch to write, so it is named
    /// `patchback.fit.NoValidCandidate`; the message names the validation error.
    RendererFault { message: String },
    /// The reference's analysis refused it for a reason the comparison shares (an empty clip, a
    /// rate other than 48 kHz, a clip past the analysis duration bound, a non-finite sample):
    /// every comparison against it would be refused, so the fit is.
    Compare(CompareError),
    /// `patchback.fit.FailedRunHasNoPatch`: the run failed, so it has no patch to write and its
    /// results are not written ([`crate::write_results`]).
    FailedRunHasNoPatch,
    /// `patchback.fit.FitStillRunning`: the run has not ended, so it has no final patch to write
    /// ([`crate::write_results`]).
    FitStillRunning,
    /// `patchback.fit.InvalidHoldoutSize`: a benchmark's holdout holds no example
    /// (`example_count <= 0`; [`crate::Benchmark::start`]).
    InvalidHoldoutSize { example_count: i64 },
    /// `patchback.fit.WeightArtifactRequired`: an ML-init benchmark names no weight artifact to
    /// predict from ([`crate::Benchmark::start`]); carries the benchmark's topology.
    WeightArtifactRequired { topology: String },
    /// `patchback.fit.BenchmarkKindMismatch`: the result recorded is for the other kind of
    /// benchmark ([`crate::Benchmark::record_baseline_result`]); carries the benchmark's kind.
    BenchmarkKindMismatch { kind: BenchmarkKind },
    /// `patchback.fit.BenchmarkStateConflict`: the benchmark already holds its result
    /// ([`crate::Benchmark::record_baseline_result`]); carries its state.
    BenchmarkStateConflict { state: BenchmarkState },
}

impl FitError {
    /// Every error of `spec/` this type can name: the errors of `spec/domains/fit.yaml` it
    /// decides, then every error [`FitError::Compare`] passes on ([`CompareError::DOMAIN_ERRORS`]).
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
        "patchback.fit.InvalidSearchBound",
        "patchback.fit.TopologyNotFound",
        "patchback.fit.ReferenceSilent",
        "patchback.fit.PitchAmbiguous",
        "patchback.fit.FitNotRunning",
        "patchback.fit.RenderBudgetExhausted",
        "patchback.fit.NoValidCandidate",
        "patchback.fit.FailedRunHasNoPatch",
        "patchback.fit.FitStillRunning",
        "patchback.fit.InvalidHoldoutSize",
        "patchback.fit.WeightArtifactRequired",
        "patchback.fit.BenchmarkKindMismatch",
        "patchback.fit.BenchmarkStateConflict",
        "patchback.fit.SampleRateMismatch",
        "patchback.fit.LengthOutsideAlignment",
        "patchback.analysis.EmptyAudio",
        "patchback.analysis.UnsupportedSampleRate",
        "patchback.analysis.InvalidHopLength",
        "patchback.analysis.FftLengthNotPowerOfTwo",
        "patchback.analysis.FftLengthOverBound",
        "patchback.analysis.DurationOverBound",
        "patchback.analysis.FrameCountOverBound",
        "patchback.analysis.NonFiniteSamples",
        "patchback.analysis.SilentInput",
        "patchback.analysis.ClipTooShortForPitch",
        "patchback.analysis.PitchAmbiguous",
        "patchback.analysis.PitchOutOfRange",
    ];

    /// The error of `spec/` this refusal is.
    pub fn domain_error(&self) -> &'static str {
        match self {
            Self::InvalidSearchBound { .. } => "patchback.fit.InvalidSearchBound",
            Self::TopologyNotFound { .. } => "patchback.fit.TopologyNotFound",
            Self::ReferenceSilent { .. } => "patchback.fit.ReferenceSilent",
            Self::PitchAmbiguous { .. } => "patchback.fit.PitchAmbiguous",
            Self::FitNotRunning { .. } => "patchback.fit.FitNotRunning",
            Self::RenderBudgetExhausted => "patchback.fit.RenderBudgetExhausted",
            Self::NoValidCandidate | Self::RendererFault { .. } => "patchback.fit.NoValidCandidate",
            Self::FailedRunHasNoPatch => "patchback.fit.FailedRunHasNoPatch",
            Self::FitStillRunning => "patchback.fit.FitStillRunning",
            Self::InvalidHoldoutSize { .. } => "patchback.fit.InvalidHoldoutSize",
            Self::WeightArtifactRequired { .. } => "patchback.fit.WeightArtifactRequired",
            Self::BenchmarkKindMismatch { .. } => "patchback.fit.BenchmarkKindMismatch",
            Self::BenchmarkStateConflict { .. } => "patchback.fit.BenchmarkStateConflict",
            Self::Compare(error) => error.domain_error(),
        }
    }

    /// One value of every variant, and every comparison refusal. The `match` below has no
    /// wildcard, so a new variant does not compile until it is listed here, and the domain test
    /// then checks its name.
    pub fn examples() -> Vec<FitError> {
        let mut examples = vec![
            Self::InvalidSearchBound {
                bound: SearchBound::RenderEvaluations,
                value: 0,
            },
            Self::InvalidSearchBound {
                bound: SearchBound::WallTime,
                value: 0,
            },
            Self::InvalidSearchBound {
                bound: SearchBound::Restarts,
                value: -1,
            },
            Self::TopologyNotFound {
                topology: "subtractive-mono/9".to_owned(),
            },
            Self::ReferenceSilent {
                cause: AnalysisError::SilentInput { rms_dbfs: -120.0 },
            },
            Self::PitchAmbiguous {
                cause: AnalysisError::PitchAmbiguous { confidence: 0.1 },
            },
            Self::FitNotRunning {
                state: FitState::Completed,
            },
            Self::FitNotRunning {
                state: FitState::Failed,
            },
            Self::RenderBudgetExhausted,
            Self::NoValidCandidate,
            Self::RendererFault {
                message: "a parameter outside its contract".to_owned(),
            },
            Self::FailedRunHasNoPatch,
            Self::FitStillRunning,
            Self::InvalidHoldoutSize { example_count: 0 },
            Self::WeightArtifactRequired {
                topology: "subtractive-mono/1".to_owned(),
            },
            Self::BenchmarkKindMismatch {
                kind: BenchmarkKind::MlInit,
            },
            Self::BenchmarkStateConflict {
                state: BenchmarkState::Completed,
            },
        ];
        examples.extend(CompareError::examples().into_iter().map(Self::Compare));
        for example in &examples {
            match example {
                Self::InvalidSearchBound { .. }
                | Self::TopologyNotFound { .. }
                | Self::ReferenceSilent { .. }
                | Self::PitchAmbiguous { .. }
                | Self::FitNotRunning { .. }
                | Self::RenderBudgetExhausted
                | Self::NoValidCandidate
                | Self::RendererFault { .. }
                | Self::Compare(_)
                | Self::FailedRunHasNoPatch
                | Self::FitStillRunning
                | Self::InvalidHoldoutSize { .. }
                | Self::WeightArtifactRequired { .. }
                | Self::BenchmarkKindMismatch { .. }
                | Self::BenchmarkStateConflict { .. } => {}
            }
        }
        examples
    }
}

impl From<CompareError> for FitError {
    fn from(error: CompareError) -> Self {
        Self::Compare(error)
    }
}

impl fmt::Display for FitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSearchBound { bound, value } => write!(
                f,
                "the {bound} bound is {value}; a fit runs only within positive bounds"
            ),
            Self::TopologyNotFound { topology } => write!(
                f,
                "no topology named {topology:?} is known to the renderer; no run starts"
            ),
            Self::ReferenceSilent { cause } => write!(
                f,
                "the reference holds no note above the silence threshold ({cause}); no run starts"
            ),
            Self::PitchAmbiguous { cause } => write!(
                f,
                "the fundamental of the reference cannot be estimated ({}: {cause}); no run starts",
                cause.domain_error()
            ),
            Self::FitNotRunning { state } => write!(
                f,
                "the fit run is {state}; it takes no further candidate and no second ending"
            ),
            Self::RenderBudgetExhausted => f.write_str(
                "the run has spent its render evaluations; the candidate is not evaluated",
            ),
            Self::NoValidCandidate => f.write_str(
                "the run holds no valid candidate: every render or comparison was refused or \
                 non-finite; the run failed and nothing is written",
            ),
            Self::RendererFault { message } => write!(
                f,
                "a generated patch was refused by validation ({message}); the run failed with \
                 RendererFault and nothing is written"
            ),
            Self::Compare(error) => write!(f, "the reference is refused: {error}"),
            Self::FailedRunHasNoPatch => f.write_str(
                "the fit run failed, so it has no patch to write; no results are written",
            ),
            Self::FitStillRunning => f.write_str(
                "the fit run has not ended, so it has no final patch to write; no results are \
                 written",
            ),
            Self::InvalidHoldoutSize { example_count } => write!(
                f,
                "the holdout of {example_count} examples holds no example; a benchmark needs at \
                 least one; no benchmark starts"
            ),
            Self::WeightArtifactRequired { topology } => write!(
                f,
                "an ML-init benchmark in {topology:?} names no weight artifact to predict from; \
                 no benchmark starts"
            ),
            Self::BenchmarkKindMismatch { kind } => write!(
                f,
                "the benchmark is {kind}; the result recorded is for the other kind of benchmark"
            ),
            Self::BenchmarkStateConflict { state } => {
                write!(f, "the benchmark is {state}; it already holds its result")
            }
        }
    }
}

impl std::error::Error for FitError {}
