//! The typed refusals of analysis. Each [`AnalysisError`] names one error of
//! `spec/domains/analysis.yaml` ([`AnalysisError::domain_error`]).

use std::fmt;

/// Why nothing was analysed. A refusal produces no partial result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnalysisError {
    /// `patchback.analysis.EmptyAudio`: the clip holds no samples.
    EmptyAudio,
    /// `patchback.analysis.UnsupportedSampleRate`: not [`crate::SAMPLE_RATE_HZ`]; never converted.
    UnsupportedSampleRate { sample_rate_hz: u32 },
    /// `patchback.analysis.InvalidHopLength`: the STFT hop is zero samples.
    InvalidHopLength { hop_length: usize },
    /// `patchback.analysis.FftLengthNotPowerOfTwo`: the radix-2 FFT needs a power of two.
    FftLengthNotPowerOfTwo { fft_length: usize },
    /// `patchback.analysis.FftLengthOverBound`: longer than [`crate::MAX_FFT_LENGTH`].
    FftLengthOverBound { fft_length: usize },
    /// `patchback.analysis.DurationOverBound`: more than [`crate::MAX_DURATION_FRAMES`] samples.
    DurationOverBound { frame_count: usize },
    /// `patchback.analysis.FrameCountOverBound`: `frame_count` STFT frames of `fft_length / 2 + 1`
    /// magnitudes each would hold more than [`crate::MAX_STFT_VALUES`] values, together with the
    /// STFTs of the configuration before it.
    FrameCountOverBound {
        fft_length: usize,
        hop_length: usize,
        frame_count: usize,
    },
    /// `patchback.analysis.NonFiniteSamples`: a NaN or infinite value at `index` of the input.
    NonFiniteSamples { index: usize },
    /// `patchback.analysis.SilentInput`: the clip's RMS level, in dBFS, is below
    /// [`crate::SILENCE_THRESHOLD_DBFS`].
    SilentInput { rms_dbfs: f64 },
    /// `patchback.analysis.PitchAmbiguous`: the f0 confidence is below
    /// [`crate::F0_CONFIDENCE_THRESHOLD`].
    PitchAmbiguous { confidence: f64 },
    /// `patchback.analysis.PitchOutOfRange`: the pitch lies beyond `bound_hz`, which is
    /// [`crate::F0_MAX_HZ`] or [`crate::F0_MIN_HZ`]; it is not reported at another octave or at
    /// the edge of the range.
    PitchOutOfRange { bound_hz: f64 },
    /// `patchback.analysis.ClipTooShortForPitch`: the clip's `frame_count` samples do not hold
    /// the f0 window and the lags the pitch needs; `required_frame_count`, always more than
    /// `frame_count`, is the fewest samples any estimate needs when the clip holds fewer, and
    /// otherwise the length that searches the whole lag range.
    ClipTooShortForPitch {
        frame_count: usize,
        required_frame_count: usize,
    },
}

impl AnalysisError {
    /// Every error of `spec/domains/analysis.yaml` this crate can emit.
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
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

    /// The error of `spec/domains/analysis.yaml` this refusal is.
    pub fn domain_error(&self) -> &'static str {
        match self {
            Self::EmptyAudio => "patchback.analysis.EmptyAudio",
            Self::UnsupportedSampleRate { .. } => "patchback.analysis.UnsupportedSampleRate",
            Self::InvalidHopLength { .. } => "patchback.analysis.InvalidHopLength",
            Self::FftLengthNotPowerOfTwo { .. } => "patchback.analysis.FftLengthNotPowerOfTwo",
            Self::FftLengthOverBound { .. } => "patchback.analysis.FftLengthOverBound",
            Self::DurationOverBound { .. } => "patchback.analysis.DurationOverBound",
            Self::FrameCountOverBound { .. } => "patchback.analysis.FrameCountOverBound",
            Self::NonFiniteSamples { .. } => "patchback.analysis.NonFiniteSamples",
            Self::SilentInput { .. } => "patchback.analysis.SilentInput",
            Self::PitchAmbiguous { .. } => "patchback.analysis.PitchAmbiguous",
            Self::PitchOutOfRange { .. } => "patchback.analysis.PitchOutOfRange",
            Self::ClipTooShortForPitch { .. } => "patchback.analysis.ClipTooShortForPitch",
        }
    }

    /// One value of every variant. The `match` below has no wildcard, so a new variant does not
    /// compile until it is listed here, and the domain test then checks its name.
    pub fn examples() -> Vec<AnalysisError> {
        let examples = vec![
            Self::EmptyAudio,
            Self::UnsupportedSampleRate {
                sample_rate_hz: 44_100,
            },
            Self::InvalidHopLength { hop_length: 0 },
            Self::FftLengthNotPowerOfTwo { fft_length: 500 },
            Self::FftLengthOverBound {
                fft_length: 1 << 20,
            },
            Self::DurationOverBound {
                frame_count: usize::MAX,
            },
            Self::NonFiniteSamples { index: 0 },
            Self::SilentInput { rms_dbfs: -120.0 },
            Self::PitchAmbiguous { confidence: 0.1 },
            Self::FrameCountOverBound {
                fft_length: 65_536,
                hop_length: 1,
                frame_count: 1_024,
            },
            Self::PitchOutOfRange {
                bound_hz: crate::F0_MAX_HZ,
            },
            Self::ClipTooShortForPitch {
                frame_count: 0,
                required_frame_count: 2_074,
            },
        ];
        for example in &examples {
            match example {
                Self::EmptyAudio
                | Self::UnsupportedSampleRate { .. }
                | Self::InvalidHopLength { .. }
                | Self::FftLengthNotPowerOfTwo { .. }
                | Self::FftLengthOverBound { .. }
                | Self::DurationOverBound { .. }
                | Self::NonFiniteSamples { .. }
                | Self::SilentInput { .. }
                | Self::PitchAmbiguous { .. }
                | Self::FrameCountOverBound { .. }
                | Self::PitchOutOfRange { .. }
                | Self::ClipTooShortForPitch { .. } => {}
            }
        }
        examples
    }
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAudio => write!(f, "the audio holds no samples"),
            Self::UnsupportedSampleRate { sample_rate_hz } => write!(
                f,
                "unsupported sample rate {sample_rate_hz} Hz; only {} Hz",
                crate::SAMPLE_RATE_HZ
            ),
            Self::InvalidHopLength { hop_length } => {
                write!(f, "STFT hop of {hop_length} samples is not positive")
            }
            Self::FftLengthNotPowerOfTwo { fft_length } => {
                write!(f, "FFT length {fft_length} is not a power of two")
            }
            Self::FftLengthOverBound { fft_length } => write!(
                f,
                "FFT length {fft_length} exceeds the bound of {}",
                crate::MAX_FFT_LENGTH
            ),
            Self::DurationOverBound { frame_count } => write!(
                f,
                "{frame_count} frames exceed the analysis bound of {} frames",
                crate::MAX_DURATION_FRAMES
            ),
            Self::NonFiniteSamples { index } => {
                write!(f, "non-finite value at index {index}")
            }
            Self::SilentInput { rms_dbfs } => write!(
                f,
                "silent input: RMS level {rms_dbfs:.1} dBFS is below the silence threshold of {} dBFS",
                crate::SILENCE_THRESHOLD_DBFS
            ),
            Self::PitchAmbiguous { confidence } => write!(
                f,
                "ambiguous pitch: f0 confidence {confidence:.3} is below the threshold of {}",
                crate::F0_CONFIDENCE_THRESHOLD
            ),
            Self::FrameCountOverBound {
                fft_length,
                hop_length,
                frame_count,
            } => write!(
                f,
                "{frame_count} STFT frames of FFT length {fft_length} at hop {hop_length}, with the \
                 STFTs before them, exceed the bound of {} magnitudes",
                crate::MAX_STFT_VALUES
            ),
            Self::PitchOutOfRange { bound_hz } => write!(
                f,
                "pitch out of range: the f0 lies beyond the search bound of {bound_hz} Hz \
                 (range {} to {} Hz)",
                crate::F0_MIN_HZ,
                crate::F0_MAX_HZ
            ),
            Self::ClipTooShortForPitch {
                frame_count,
                required_frame_count,
            } => write!(
                f,
                "clip too short for pitch: {frame_count} samples do not hold the f0 window and the \
                 periods it searches (search needs {required_frame_count} samples)"
            ),
        }
    }
}

impl std::error::Error for AnalysisError {}
