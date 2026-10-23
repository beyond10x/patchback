//! The typed refusals of the comparison. Each [`CompareError`] names one error of
//! `spec/domains/fit.yaml`, or, for an input analysis refuses, of `spec/domains/analysis.yaml`
//! ([`CompareError::domain_error`]).

use std::fmt;

use patchback_analysis::AnalysisError;

use crate::ALIGNMENT_TOLERANCE_FRAMES;

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
