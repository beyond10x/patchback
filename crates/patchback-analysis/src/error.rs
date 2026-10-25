//! The typed refusals of analysis. Each [`AnalysisError`] names one error of
//! `spec/domains/analysis.yaml` ([`AnalysisError::domain_error`]), and so does each
//! [`FeatureError`] of the feature contract ([`FeatureError::domain_error`]) and each
//! [`NormalizationError`] of the normalization statistics ([`NormalizationError::domain_error`]).

use std::fmt;

use patchback_core::split::SplitRole;

use crate::{FeatureContractId, FeatureSlot, ScalarFeature};

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

/// Why a value of a feature contract cannot be computed reliably
/// ([`FeatureError::FeatureNotComputable`]); nothing is estimated in its place (DESIGN.md § 6.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NotComputableReason {
    /// The analysis of the clip refused it, or the clip or analysis holds what analysis refuses
    /// (a sample rate other than [`crate::SAMPLE_RATE_HZ`], a non-finite sample).
    Analysis(AnalysisError),
    /// The samples handed to extraction are not those of the analysis: `frame_count` samples
    /// against the analysis' `analysis_frame_count`.
    ClipMismatch {
        frame_count: usize,
        analysis_frame_count: usize,
    },
    /// The note does not start before it ends, or ends past the clip's `frame_count` samples.
    NoteOutsideClip {
        start_sample: usize,
        end_sample: usize,
        frame_count: usize,
    },
    /// The contract's band count is not [`crate::BAND_COUNT`], the bands of
    /// [`crate::BAND_EDGES_HZ`].
    BandCountUnsupported { band_count: usize },
    /// The contract does not name [`crate::FEATURE_FFT_LENGTH`], the FFT length the band
    /// energies and spectral descriptors are read at.
    FftLengthNotInContract { fft_length: usize },
    /// The analysis holds no STFT of this length, one the contract names, at any hop.
    StftMissing { fft_length: usize },
    /// No frame of [`crate::FEATURE_FFT_LENGTH`] has its centre in the window from
    /// `start_sample` to `end_sample` (exclusive); an empty window has none.
    WindowTooShort {
        start_sample: usize,
        end_sample: usize,
    },
    /// The window's RMS level, `rms_dbfs`, is below [`crate::SILENCE_THRESHOLD_DBFS`].
    SilentWindow {
        start_sample: usize,
        end_sample: usize,
        rms_dbfs: f64,
    },
    /// The analysis' f0 confidence is below [`crate::F0_CONFIDENCE_THRESHOLD`] (or not a number).
    PitchAmbiguous { confidence: f64 },
    /// The analysis' f0 is not a positive finite frequency within [`crate::F0_MIN_HZ`] to
    /// [`crate::F0_MAX_HZ`], each widened by [`crate::F0_RANGE_TOLERANCE_CENTS`].
    PitchAbsent { f0_hz: f64 },
}

impl fmt::Display for NotComputableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(error) => write!(f, "analysis refused: {error}"),
            Self::ClipMismatch {
                frame_count,
                analysis_frame_count,
            } => write!(
                f,
                "the clip holds {frame_count} samples and its analysis {analysis_frame_count}"
            ),
            Self::NoteOutsideClip {
                start_sample,
                end_sample,
                frame_count,
            } => write!(
                f,
                "the note from sample {start_sample} to {end_sample} does not lie in the clip of \
                 {frame_count} samples"
            ),
            Self::BandCountUnsupported { band_count } => write!(
                f,
                "{band_count} bands; the band edges give {}",
                crate::BAND_COUNT
            ),
            Self::FftLengthNotInContract { fft_length } => {
                write!(f, "the contract does not name the FFT length {fft_length}")
            }
            Self::StftMissing { fft_length } => {
                write!(f, "the analysis holds no STFT of length {fft_length}")
            }
            Self::WindowTooShort {
                start_sample,
                end_sample,
            } => write!(
                f,
                "the window from sample {start_sample} to {end_sample} holds no frame centre"
            ),
            Self::SilentWindow {
                start_sample,
                end_sample,
                rms_dbfs,
            } => write!(
                f,
                "the window from sample {start_sample} to {end_sample} is silent: {rms_dbfs:.1} \
                 dBFS is below {} dBFS",
                crate::SILENCE_THRESHOLD_DBFS
            ),
            Self::PitchAmbiguous { confidence } => write!(
                f,
                "f0 confidence {confidence:.3} is below the threshold of {}",
                crate::F0_CONFIDENCE_THRESHOLD
            ),
            Self::PitchAbsent { f0_hz } => write!(
                f,
                "f0 of {f0_hz} Hz is not a frequency from {} to {} Hz",
                crate::F0_MIN_HZ,
                crate::F0_MAX_HZ
            ),
        }
    }
}

/// Why a feature contract command was refused. Each [`FeatureError`] names one error of
/// `spec/domains/analysis.yaml` ([`FeatureError::domain_error`]); a refusal changes nothing and
/// produces no partial vector.
#[derive(Clone, Debug, PartialEq)]
pub enum FeatureError {
    /// `patchback.analysis.FeatureVersionTaken`: a contract of this version exists, draft or
    /// published; a change is a new version.
    FeatureVersionTaken { version: String },
    /// `patchback.analysis.FeatureCountMismatch`: `features/1` has exactly
    /// [`crate::FEATURES_1_VALUE_COUNT`] values; only `features/1` is refused this way.
    FeatureCountMismatch { version: String, value_count: usize },
    /// `patchback.analysis.FeatureContractFrozen`: the contract is published; it is neither
    /// amended nor published again.
    FeatureContractFrozen {
        feature_contract_id: FeatureContractId,
    },
    /// `patchback.analysis.FeatureContractNotFound`: no contract has this identity.
    FeatureContractNotFound {
        feature_contract_id: FeatureContractId,
    },
    /// `patchback.analysis.FeatureContractNotPublished`: features are extracted only under a
    /// published contract.
    FeatureContractNotPublished {
        feature_contract_id: FeatureContractId,
    },
    /// `patchback.analysis.FeatureNotComputable`: the value at `index` of the contract, `feature`,
    /// cannot be computed reliably, for `reason`; the first such value in contract order.
    FeatureNotComputable {
        feature: FeatureSlot,
        index: usize,
        reason: NotComputableReason,
    },
    /// `patchback.analysis.NonFiniteFeature`: the value at `index` is NaN or infinite; the first
    /// such value in contract order.
    NonFiniteFeature {
        feature_contract_id: FeatureContractId,
        index: usize,
    },
    /// `patchback.analysis.FeatureVersionMismatch`: the vector's `version` is not the contract's.
    FeatureVersionMismatch { version: String },
    /// `patchback.analysis.FeatureLengthMismatch`: the vector holds `value_count` values, not the
    /// contract's count.
    FeatureLengthMismatch { value_count: usize },
}

impl FeatureError {
    /// Every error of `spec/domains/analysis.yaml` the feature contract can emit.
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
        "patchback.analysis.FeatureVersionTaken",
        "patchback.analysis.FeatureCountMismatch",
        "patchback.analysis.FeatureContractFrozen",
        "patchback.analysis.FeatureContractNotFound",
        "patchback.analysis.FeatureContractNotPublished",
        "patchback.analysis.FeatureNotComputable",
        "patchback.analysis.NonFiniteFeature",
        "patchback.analysis.FeatureVersionMismatch",
        "patchback.analysis.FeatureLengthMismatch",
    ];

    /// The error of `spec/domains/analysis.yaml` this refusal is.
    pub fn domain_error(&self) -> &'static str {
        match self {
            Self::FeatureVersionTaken { .. } => "patchback.analysis.FeatureVersionTaken",
            Self::FeatureCountMismatch { .. } => "patchback.analysis.FeatureCountMismatch",
            Self::FeatureContractFrozen { .. } => "patchback.analysis.FeatureContractFrozen",
            Self::FeatureContractNotFound { .. } => "patchback.analysis.FeatureContractNotFound",
            Self::FeatureContractNotPublished { .. } => {
                "patchback.analysis.FeatureContractNotPublished"
            }
            Self::FeatureNotComputable { .. } => "patchback.analysis.FeatureNotComputable",
            Self::NonFiniteFeature { .. } => "patchback.analysis.NonFiniteFeature",
            Self::FeatureVersionMismatch { .. } => "patchback.analysis.FeatureVersionMismatch",
            Self::FeatureLengthMismatch { .. } => "patchback.analysis.FeatureLengthMismatch",
        }
    }

    /// One value of every variant. The `match` below has no wildcard, so a new variant does not
    /// compile until it is listed here, and the domain test then checks its name.
    pub fn examples() -> Vec<FeatureError> {
        let id = FeatureContractId(1);
        let examples = vec![
            Self::FeatureVersionTaken {
                version: crate::FEATURES_1_VERSION.to_string(),
            },
            Self::FeatureCountMismatch {
                version: crate::FEATURES_1_VERSION.to_string(),
                value_count: 31,
            },
            Self::FeatureContractFrozen {
                feature_contract_id: id,
            },
            Self::FeatureContractNotFound {
                feature_contract_id: id,
            },
            Self::FeatureContractNotPublished {
                feature_contract_id: id,
            },
            Self::FeatureNotComputable {
                feature: FeatureSlot::Scalar(ScalarFeature::NormalizedMidiPitch),
                index: 31,
                reason: NotComputableReason::PitchAmbiguous { confidence: 0.2 },
            },
            Self::NonFiniteFeature {
                feature_contract_id: id,
                index: 26,
            },
            Self::FeatureVersionMismatch {
                version: "features/2".to_string(),
            },
            Self::FeatureLengthMismatch { value_count: 31 },
        ];
        for example in &examples {
            match example {
                Self::FeatureVersionTaken { .. }
                | Self::FeatureCountMismatch { .. }
                | Self::FeatureContractFrozen { .. }
                | Self::FeatureContractNotFound { .. }
                | Self::FeatureContractNotPublished { .. }
                | Self::FeatureNotComputable { .. }
                | Self::NonFiniteFeature { .. }
                | Self::FeatureVersionMismatch { .. }
                | Self::FeatureLengthMismatch { .. } => {}
            }
        }
        examples
    }
}

impl fmt::Display for FeatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FeatureVersionTaken { version } => write!(
                f,
                "feature contract {version} is already defined; a change is a new version"
            ),
            Self::FeatureCountMismatch {
                version,
                value_count,
            } => write!(
                f,
                "feature contract {version} with {value_count} values: {version} has exactly {}",
                crate::FEATURES_1_VALUE_COUNT
            ),
            Self::FeatureContractFrozen {
                feature_contract_id,
            } => write!(
                f,
                "{feature_contract_id} is published and frozen; nothing changed"
            ),
            Self::FeatureContractNotFound {
                feature_contract_id,
            } => write!(f, "{feature_contract_id} does not exist"),
            Self::FeatureContractNotPublished {
                feature_contract_id,
            } => write!(
                f,
                "{feature_contract_id} is a draft; features are extracted only under a published \
                 contract"
            ),
            Self::FeatureNotComputable {
                feature,
                index,
                reason,
            } => write!(
                f,
                "feature {index} ({feature}) cannot be computed reliably: {reason}"
            ),
            Self::NonFiniteFeature {
                feature_contract_id,
                index,
            } => write!(
                f,
                "value {index} of a {feature_contract_id} vector is not finite"
            ),
            Self::FeatureVersionMismatch { version } => write!(
                f,
                "the vector is of feature contract version {version}, not the contract's"
            ),
            Self::FeatureLengthMismatch { value_count } => write!(
                f,
                "the vector holds {value_count} values, not the contract's count"
            ),
        }
    }
}

impl std::error::Error for FeatureError {}

/// Why normalization statistics were not computed or not applied (`ComputeNormalizationStats`,
/// `ApplyNormalization`, story:normalization-stats). Each names one error of
/// `spec/domains/analysis.yaml` ([`NormalizationError::domain_error`]); a refusal produces
/// nothing.
///
/// The two commands declare seven errors; `NormalizationStatsNotFound` has no variant, because
/// statistics are a value ([`crate::NormalizationStats`]) and no call can name statistics that do
/// not exist. Two variants go beyond the two commands and are `CheckFeatureVector`'s errors:
/// [`Self::NonFiniteFeature`] and [`Self::FeatureLengthMismatch`] (DESIGN.md § 2 invariant 4).
#[derive(Clone, Debug, PartialEq)]
pub enum NormalizationError {
    /// `patchback.analysis.DatasetSplitNotFound`: the caller found no split of this identity
    /// (`patchback_dataset::DatasetSplitId`'s number).
    DatasetSplitNotFound { dataset_split_id: u64 },
    /// `patchback.analysis.FeatureContractNotFound`: no contract has this identity.
    FeatureContractNotFound {
        feature_contract_id: FeatureContractId,
    },
    /// `patchback.analysis.StatsFromNonTrainingSplit`: the split is the validation or test split.
    StatsFromNonTrainingSplit { split_role: SplitRole },
    /// `patchback.analysis.FeatureContractNotPublished`: statistics need a published contract.
    FeatureContractNotPublished {
        feature_contract_id: FeatureContractId,
    },
    /// `patchback.analysis.FeatureVersionMismatch`: the `version` named is not the contract's.
    FeatureVersionMismatch { version: String },
    /// `patchback.analysis.NormalizationContractMismatch`: the features to normalize are of
    /// `feature_contract_version`, not the version the statistics were computed under.
    NormalizationContractMismatch { feature_contract_version: String },
    /// `patchback.analysis.NonFiniteFeature`: the value at `index` of a training row or of the
    /// features to normalize is NaN or infinite, or the mean, standard deviation or normalized
    /// value at `index` would be.
    NonFiniteFeature {
        feature_contract_id: FeatureContractId,
        index: usize,
    },
    /// `patchback.analysis.FeatureLengthMismatch`: a training row holds `value_count` values (32),
    /// not the contract's count.
    FeatureLengthMismatch { value_count: usize },
}

impl NormalizationError {
    /// Every error of `spec/domains/analysis.yaml` the normalization statistics can emit.
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
        "patchback.analysis.DatasetSplitNotFound",
        "patchback.analysis.FeatureContractNotFound",
        "patchback.analysis.StatsFromNonTrainingSplit",
        "patchback.analysis.FeatureContractNotPublished",
        "patchback.analysis.FeatureVersionMismatch",
        "patchback.analysis.NormalizationContractMismatch",
        "patchback.analysis.NonFiniteFeature",
        "patchback.analysis.FeatureLengthMismatch",
    ];

    /// The error of `spec/domains/analysis.yaml` this refusal is.
    pub fn domain_error(&self) -> &'static str {
        match self {
            Self::DatasetSplitNotFound { .. } => "patchback.analysis.DatasetSplitNotFound",
            Self::FeatureContractNotFound { .. } => "patchback.analysis.FeatureContractNotFound",
            Self::StatsFromNonTrainingSplit { .. } => {
                "patchback.analysis.StatsFromNonTrainingSplit"
            }
            Self::FeatureContractNotPublished { .. } => {
                "patchback.analysis.FeatureContractNotPublished"
            }
            Self::FeatureVersionMismatch { .. } => "patchback.analysis.FeatureVersionMismatch",
            Self::NormalizationContractMismatch { .. } => {
                "patchback.analysis.NormalizationContractMismatch"
            }
            Self::NonFiniteFeature { .. } => "patchback.analysis.NonFiniteFeature",
            Self::FeatureLengthMismatch { .. } => "patchback.analysis.FeatureLengthMismatch",
        }
    }

    /// One value of every variant. The `match` below has no wildcard, so a new variant does not
    /// compile until it is listed here, and the domain test then checks its name.
    pub fn examples() -> Vec<NormalizationError> {
        let id = FeatureContractId(1);
        let examples = vec![
            Self::DatasetSplitNotFound {
                dataset_split_id: 99,
            },
            Self::FeatureContractNotFound {
                feature_contract_id: id,
            },
            Self::StatsFromNonTrainingSplit {
                split_role: SplitRole::Test,
            },
            Self::FeatureContractNotPublished {
                feature_contract_id: id,
            },
            Self::FeatureVersionMismatch {
                version: "features/2".to_string(),
            },
            Self::NormalizationContractMismatch {
                feature_contract_version: "features/2".to_string(),
            },
            Self::NonFiniteFeature {
                feature_contract_id: id,
                index: 3,
            },
            Self::FeatureLengthMismatch {
                value_count: crate::FEATURES_1_VALUE_COUNT,
            },
        ];
        for example in &examples {
            match example {
                Self::DatasetSplitNotFound { .. }
                | Self::FeatureContractNotFound { .. }
                | Self::StatsFromNonTrainingSplit { .. }
                | Self::FeatureContractNotPublished { .. }
                | Self::FeatureVersionMismatch { .. }
                | Self::NormalizationContractMismatch { .. }
                | Self::NonFiniteFeature { .. }
                | Self::FeatureLengthMismatch { .. } => {}
            }
        }
        examples
    }
}

impl fmt::Display for NormalizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DatasetSplitNotFound { dataset_split_id } => {
                write!(f, "split #{dataset_split_id} does not exist")
            }
            Self::FeatureContractNotFound {
                feature_contract_id,
            } => write!(f, "{feature_contract_id} does not exist"),
            Self::StatsFromNonTrainingSplit { split_role } => write!(
                f,
                "normalization statistics come from the training split only, not the {split_role} \
                 split"
            ),
            Self::FeatureContractNotPublished {
                feature_contract_id,
            } => write!(
                f,
                "{feature_contract_id} is a draft; statistics need a published contract"
            ),
            Self::FeatureVersionMismatch { version } => {
                write!(f, "{version} is not the contract's version")
            }
            Self::NormalizationContractMismatch {
                feature_contract_version,
            } => write!(
                f,
                "the features are of {feature_contract_version}, not the version the statistics \
                 were computed under"
            ),
            Self::NonFiniteFeature {
                feature_contract_id,
                index,
            } => write!(
                f,
                "value {index} under {feature_contract_id} is not finite, or its statistic or \
                 normalized value would not be"
            ),
            Self::FeatureLengthMismatch { value_count } => write!(
                f,
                "a row holds {value_count} values, not the contract's count"
            ),
        }
    }
}

impl std::error::Error for NormalizationError {}
