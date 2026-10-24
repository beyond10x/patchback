//! The versioned feature contract and feature extraction (DESIGN.md § 6.3, § 7.1, § 12.1,
//! § 12.3; `spec/domains/analysis.yaml` `FeatureContract`, `FeatureVector`).
//!
//! # The contract
//!
//! A [`FeatureContractDefinition`] lays out a vector: `band_count` band energies for each of its
//! `band_windows`, window-major (every band of the first window, then every band of the next),
//! then its `scalar_features` in order ([`FeatureContractDefinition::slots`]).
//! [`FeatureContractDefinition::features_1`] is `features/1`: 32 values, the eight bands of
//! [`crate::BAND_EDGES_HZ`] over attack, body and release (indices 0–7, 8–15, 16–23), then RMS,
//! peak, spectral centroid, rolloff, spectral flatness, zero-crossing rate, attack-envelope slope
//! and normalized MIDI pitch (indices 24–31), at the FFT lengths 512, 2048 and 8192.
//!
//! A [`FeatureRegistry`] holds contracts by version. A contract is defined as a draft
//! ([`FeatureRegistry::define`]), may have its FFT lengths amended while a draft
//! ([`FeatureRegistry::amend`]), and is published once ([`FeatureRegistry::publish`]), after which
//! nothing about it changes: a change is a new version.
//!
//! # The values
//!
//! [`extract_features`] reads one note of an [`Analysis`] and the samples it was computed from.
//! A note from `start` to `end` (its gate) has three windows ([`NoteSpan::window`]): attack,
//! `start` to `min(start + ATTACK_WINDOW_FRAMES, end)`; body, from there to `end`; release,
//! `end + RELEASE_OFFSET_FRAMES` for [`RELEASE_WINDOW_FRAMES`] samples; each cut at the clip's
//! end. Spectral values are read from the analysis' STFT of [`FEATURE_FFT_LENGTH`], from the
//! frames whose centre lies in the window. Amplitude is relative to full scale 1.0, as
//! [`Analysis::rms_dbfs`] is.
//!
//! - Band energy of a window: the mean power of its frames' [`crate::log_band_energies`],
//!   `10 log10(mean 10^(E / 10))`, in dB, floored at [`crate::LOG_FLOOR_DB`].
//! - RMS and peak: of the samples of the gate, in dBFS, floored at [`crate::LOG_FLOOR_DB`].
//! - Spectral centroid (Hz), rolloff (Hz) and flatness: the mean over the gate's frames.
//! - Zero-crossing rate: [`crate::zero_crossing_rate`] of the gate's samples, per sample.
//! - Attack-envelope slope: [`crate::attack_envelope_slope`] of the attack window's samples, in
//!   full scale per second.
//! - Normalized MIDI pitch: `midi / 127`, `midi = 69 + 12 log2(f0 / 440)`, from the analysis'
//!   f0 estimate.
//!
//! A value that cannot be computed reliably is refused, never estimated
//! ([`FeatureError::FeatureNotComputable`] with a [`NotComputableReason`]): a window holding no
//! frame centre, a window (or for the scalars, the gate or attack window) whose RMS level is
//! below [`SILENCE_THRESHOLD_DBFS`], an f0 below [`F0_CONFIDENCE_THRESHOLD`] or outside the
//! search range. A computed value that is NaN or infinite is refused as
//! [`FeatureError::NonFiniteFeature`]. Values are computed in contract order and the first
//! refusal is returned.
//!
//! Every window bound, the FFT length, the hop ([`FeatureContract::analysis_config`] takes
//! `fft_length / HOP_DIVISOR`; extraction reads an STFT at any hop, by its own hop) and the
//! definitions of zero-crossing rate and attack-envelope slope are provisional (OQ-15).

use std::fmt;
use std::ops::Range;

use crate::stft::magnitude_db;
use crate::{
    ANALYSIS_FFT_LENGTHS, ATTACK_WINDOW_FRAMES, Analysis, AnalysisConfig, AnalysisError,
    BAND_COUNT, F0_CONFIDENCE_THRESHOLD, F0_MAX_HZ, F0_MIN_HZ, F0_RANGE_TOLERANCE_CENTS,
    FEATURE_FFT_LENGTH, FeatureError, FrameDescriptors, HOP_DIVISOR, LOG_FLOOR_DB,
    NotComputableReason, RELEASE_OFFSET_FRAMES, RELEASE_WINDOW_FRAMES, SAMPLE_RATE_HZ,
    SILENCE_THRESHOLD_DBFS, StftAnalysis, StftConfig, analyze, attack_envelope_slope, check_finite,
    peak, rms, zero_crossing_rate,
};

/// The window of a note a band energy is computed over (DESIGN.md § 6.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BandWindow {
    Attack,
    Body,
    Release,
}

impl fmt::Display for BandWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Attack => "attack",
            Self::Body => "body",
            Self::Release => "release",
        })
    }
}

/// A scalar feature (DESIGN.md § 6.3), declared in the order `features/1` lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScalarFeature {
    Rms,
    Peak,
    SpectralCentroid,
    Rolloff,
    SpectralFlatness,
    ZeroCrossingRate,
    AttackEnvelopeSlope,
    NormalizedMidiPitch,
}

impl fmt::Display for ScalarFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Rms => "RMS",
            Self::Peak => "peak",
            Self::SpectralCentroid => "spectral centroid",
            Self::Rolloff => "spectral rolloff",
            Self::SpectralFlatness => "spectral flatness",
            Self::ZeroCrossingRate => "zero-crossing rate",
            Self::AttackEnvelopeSlope => "attack-envelope slope",
            Self::NormalizedMidiPitch => "normalized MIDI pitch",
        })
    }
}

/// One value of a contract's vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FeatureSlot {
    /// The energy of band `band` (of [`crate::BAND_EDGES_HZ`]) over `window`.
    Band {
        window: BandWindow,
        band: usize,
    },
    Scalar(ScalarFeature),
}

impl fmt::Display for FeatureSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Band { window, band } => write!(f, "{window} band {band} energy"),
            Self::Scalar(scalar) => write!(f, "{scalar}"),
        }
    }
}

/// The version of the feature contract DESIGN.md § 6.3 defines.
pub const FEATURES_1_VERSION: &str = "features/1";

/// The values of a `features/1` vector (DESIGN.md § 6.3).
pub const FEATURES_1_VALUE_COUNT: usize = 32;

/// The windows of the band energies of `features/1`, in vector order.
pub const FEATURES_1_BAND_WINDOWS: [BandWindow; 3] =
    [BandWindow::Attack, BandWindow::Body, BandWindow::Release];

/// The scalars of `features/1`, in vector order (DESIGN.md § 6.3).
pub const FEATURES_1_SCALARS: [ScalarFeature; 8] = [
    ScalarFeature::Rms,
    ScalarFeature::Peak,
    ScalarFeature::SpectralCentroid,
    ScalarFeature::Rolloff,
    ScalarFeature::SpectralFlatness,
    ScalarFeature::ZeroCrossingRate,
    ScalarFeature::AttackEnvelopeSlope,
    ScalarFeature::NormalizedMidiPitch,
];

/// The layout of a contract, as `DefineFeatureContract` takes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureContractDefinition {
    pub version: String,
    pub value_count: usize,
    pub band_count: usize,
    pub band_windows: Vec<BandWindow>,
    pub scalar_features: Vec<ScalarFeature>,
    /// The STFT lengths of the analysis the features are read from, in samples.
    pub fft_lengths: Vec<usize>,
}

impl FeatureContractDefinition {
    /// `features/1` (DESIGN.md § 6.3): 32 values, the eight bands over attack, body and release,
    /// the eight scalars of [`FEATURES_1_SCALARS`], at [`ANALYSIS_FFT_LENGTHS`].
    pub fn features_1() -> Self {
        Self {
            version: FEATURES_1_VERSION.to_string(),
            value_count: FEATURES_1_VALUE_COUNT,
            band_count: BAND_COUNT,
            band_windows: FEATURES_1_BAND_WINDOWS.to_vec(),
            scalar_features: FEATURES_1_SCALARS.to_vec(),
            fft_lengths: ANALYSIS_FFT_LENGTHS.to_vec(),
        }
    }

    /// The values of the vector in order: window-major band energies, then the scalars.
    pub fn slots(&self) -> Vec<FeatureSlot> {
        let bands = self.band_windows.iter().flat_map(|&window| {
            (0..self.band_count).map(move |band| FeatureSlot::Band { window, band })
        });
        bands
            .chain(self.scalar_features.iter().map(|&s| FeatureSlot::Scalar(s)))
            .collect()
    }
}

/// The identity of a contract in its [`FeatureRegistry`], in the order contracts were defined
/// from 1. The domain's identity is a Uuid; this crate holds contracts in memory only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FeatureContractId(pub u64);

impl fmt::Display for FeatureContractId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "feature contract #{}", self.0)
    }
}

/// The lifecycle state of a contract: a draft may be amended; a published contract is frozen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FeatureContractState {
    Draft,
    Published,
}

/// A contract held by a [`FeatureRegistry`]; only the registry changes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureContract {
    id: FeatureContractId,
    definition: FeatureContractDefinition,
    state: FeatureContractState,
}

impl FeatureContract {
    pub fn id(&self) -> FeatureContractId {
        self.id
    }

    pub fn definition(&self) -> &FeatureContractDefinition {
        &self.definition
    }

    pub fn state(&self) -> FeatureContractState {
        self.state
    }

    /// The configuration [`analyze`] computes this contract's STFTs with: each of its FFT
    /// lengths at the hop `fft_length / HOP_DIVISOR`.
    pub fn analysis_config(&self) -> AnalysisConfig {
        AnalysisConfig {
            stfts: self
                .definition
                .fft_lengths
                .iter()
                .map(|&fft_length| StftConfig {
                    fft_length,
                    hop_length: fft_length / HOP_DIVISOR,
                })
                .collect(),
        }
    }

    /// `CheckFeatureVector`: admits a vector of `version` holding `values` only when the version
    /// is this contract's, the count is its value count and every value is finite, refused in
    /// that order.
    pub fn check(&self, version: &str, values: &[f64]) -> Result<(), FeatureError> {
        if version != self.definition.version {
            return Err(FeatureError::FeatureVersionMismatch {
                version: version.to_string(),
            });
        }
        if values.len() != self.definition.value_count {
            return Err(FeatureError::FeatureLengthMismatch {
                value_count: values.len(),
            });
        }
        match values.iter().position(|v| !v.is_finite()) {
            Some(index) => Err(FeatureError::NonFiniteFeature {
                feature_contract_id: self.id,
                index,
            }),
            None => Ok(()),
        }
    }
}

/// One vector extracted under a published contract.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureVector {
    pub feature_contract_id: FeatureContractId,
    pub version: String,
    /// The contract's values in its order, each finite.
    pub values: Vec<f64>,
}

/// The contracts, by version, each defined once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeatureRegistry {
    contracts: Vec<FeatureContract>,
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.contracts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.contracts.is_empty()
    }

    /// The contract of `version`, draft or published.
    pub fn find(&self, version: &str) -> Option<&FeatureContract> {
        self.contracts
            .iter()
            .find(|c| c.definition.version == version)
    }

    pub fn get(&self, id: FeatureContractId) -> Result<&FeatureContract, FeatureError> {
        self.contracts
            .iter()
            .find(|c| c.id == id)
            .ok_or(FeatureError::FeatureContractNotFound {
                feature_contract_id: id,
            })
    }

    fn get_mut(&mut self, id: FeatureContractId) -> Result<&mut FeatureContract, FeatureError> {
        self.contracts.iter_mut().find(|c| c.id == id).ok_or(
            FeatureError::FeatureContractNotFound {
                feature_contract_id: id,
            },
        )
    }

    /// `DefineFeatureContract`: a draft of `definition`, or refused, in this order, when it is
    /// `features/1` with a count other than [`FEATURES_1_VALUE_COUNT`]
    /// ([`FeatureError::FeatureCountMismatch`]), and when its version is defined
    /// ([`FeatureError::FeatureVersionTaken`]). Any other version is defined with the count it
    /// carries.
    ///
    /// The model states the count rule for `features/1` only; a layout rule for other versions
    /// (the count equals the bands over the windows plus the scalars) is an open point for the
    /// spec. Until it is settled, a contract whose count is not its layout's is defined, and a
    /// vector [`extract_features`] reads under it holds the layout's values, which
    /// [`FeatureContract::check`] refuses as [`FeatureError::FeatureLengthMismatch`].
    pub fn define(
        &mut self,
        definition: FeatureContractDefinition,
    ) -> Result<FeatureContractId, FeatureError> {
        if definition.version == FEATURES_1_VERSION
            && definition.value_count != FEATURES_1_VALUE_COUNT
        {
            return Err(FeatureError::FeatureCountMismatch {
                version: definition.version,
                value_count: definition.value_count,
            });
        }
        if self.find(&definition.version).is_some() {
            return Err(FeatureError::FeatureVersionTaken {
                version: definition.version,
            });
        }
        let id = FeatureContractId(self.contracts.len() as u64 + 1);
        self.contracts.push(FeatureContract {
            id,
            definition,
            state: FeatureContractState::Draft,
        });
        Ok(id)
    }

    /// `AmendFeatureContract`: replaces the FFT lengths of a draft; a published contract is
    /// refused as frozen.
    pub fn amend(
        &mut self,
        id: FeatureContractId,
        fft_lengths: Vec<usize>,
    ) -> Result<(), FeatureError> {
        let contract = self.get_mut(id)?;
        if contract.state == FeatureContractState::Published {
            return Err(FeatureError::FeatureContractFrozen {
                feature_contract_id: id,
            });
        }
        contract.definition.fft_lengths = fft_lengths;
        Ok(())
    }

    /// `PublishFeatureContract`: freezes a draft; a published contract is refused as frozen.
    pub fn publish(&mut self, id: FeatureContractId) -> Result<(), FeatureError> {
        let contract = self.get_mut(id)?;
        if contract.state == FeatureContractState::Published {
            return Err(FeatureError::FeatureContractFrozen {
                feature_contract_id: id,
            });
        }
        contract.state = FeatureContractState::Published;
        Ok(())
    }

    /// `ExtractFeatures` under the contract `id`: [`extract_features`].
    pub fn extract(
        &self,
        id: FeatureContractId,
        analysis: &Analysis,
        samples: &[f32],
        note: NoteSpan,
    ) -> Result<FeatureVector, FeatureError> {
        extract_features(self.get(id)?, analysis, samples, note)
    }

    /// `CheckFeatureVector` against the contract `id`: [`FeatureContract::check`].
    pub fn check_vector(
        &self,
        id: FeatureContractId,
        version: &str,
        values: &[f64],
    ) -> Result<(), FeatureError> {
        self.get(id)?.check(version, values)
    }
}

/// A note of a clip: its gate from `start_sample` to `end_sample` (exclusive), as handed to
/// extraction (known for rendered notes, estimated for external audio).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NoteSpan {
    pub start_sample: usize,
    pub end_sample: usize,
}

impl NoteSpan {
    /// The samples of `window` in a clip of `frame_count` samples (see the module documentation);
    /// empty when the window has none.
    pub fn window(&self, window: BandWindow, frame_count: usize) -> Range<usize> {
        let end = self.end_sample.min(frame_count);
        let start = self.start_sample.min(end);
        let attack_end = start.saturating_add(ATTACK_WINDOW_FRAMES).min(end);
        match window {
            BandWindow::Attack => start..attack_end,
            BandWindow::Body => attack_end..end,
            BandWindow::Release => {
                let release = self
                    .end_sample
                    .saturating_add(RELEASE_OFFSET_FRAMES)
                    .min(frame_count);
                release
                    ..release
                        .saturating_add(RELEASE_WINDOW_FRAMES)
                        .min(frame_count)
            }
        }
    }

    fn gate(&self) -> Range<usize> {
        self.start_sample..self.end_sample
    }
}

/// `ExtractFeatures`: the vector of `note` in the clip `samples` under `contract`, read from
/// `analysis`, the analysis of exactly those samples.
///
/// Refused, in this order: a draft contract ([`FeatureError::FeatureContractNotPublished`]);
/// then, as [`FeatureError::FeatureNotComputable`] of the contract's first value, an analysis
/// not at [`SAMPLE_RATE_HZ`], samples whose count is not the analysis' frame count, a
/// non-finite sample, a note that does not start before it ends inside the clip, a band count
/// other than [`BAND_COUNT`], a contract that does not name [`FEATURE_FFT_LENGTH`], and an
/// analysis without an STFT of each of the contract's FFT lengths, at any hop (the model leaves
/// the hop open, so no hop is refused here; each STFT's frames are placed by its own hop, and a
/// hop of 0 places none). Of two STFTs with one FFT length, the first in `analysis.stfts` is
/// read. Then, value by value in contract order, a value that cannot be
/// computed ([`FeatureError::FeatureNotComputable`]) or is not finite
/// ([`FeatureError::NonFiniteFeature`]).
pub fn extract_features(
    contract: &FeatureContract,
    analysis: &Analysis,
    samples: &[f32],
    note: NoteSpan,
) -> Result<FeatureVector, FeatureError> {
    if contract.state != FeatureContractState::Published {
        return Err(FeatureError::FeatureContractNotPublished {
            feature_contract_id: contract.id,
        });
    }
    let slots = contract.definition.slots();
    let mut values = Vec::with_capacity(slots.len());
    if let Some(&first) = slots.first() {
        let frames = preconditions(contract, analysis, samples, note).map_err(|reason| {
            FeatureError::FeatureNotComputable {
                feature: first,
                index: 0,
                reason,
            }
        })?;
        let extractor = Extractor {
            analysis,
            samples,
            note,
            frames,
        };
        for (index, &slot) in slots.iter().enumerate() {
            let value =
                extractor
                    .value(slot)
                    .map_err(|reason| FeatureError::FeatureNotComputable {
                        feature: slot,
                        index,
                        reason,
                    })?;
            if !value.is_finite() {
                return Err(FeatureError::NonFiniteFeature {
                    feature_contract_id: contract.id,
                    index,
                });
            }
            values.push(value);
        }
    }
    Ok(FeatureVector {
        feature_contract_id: contract.id,
        version: contract.definition.version.clone(),
        values,
    })
}

/// [`analyze`] of `samples` with the contract's [`FeatureContract::analysis_config`], then
/// [`extract_features`] of `note`. A draft is refused before anything is analysed. An analysis
/// refusal is [`FeatureError::FeatureNotComputable`] with [`NotComputableReason::Analysis`]: of
/// the normalized MIDI pitch for a pitch refusal (an unpitched or too short clip) when the
/// contract has that value, otherwise of the contract's first value.
pub fn analyze_and_extract(
    contract: &FeatureContract,
    samples: &[f32],
    sample_rate_hz: u32,
    note: NoteSpan,
) -> Result<FeatureVector, FeatureError> {
    if contract.state != FeatureContractState::Published {
        return Err(FeatureError::FeatureContractNotPublished {
            feature_contract_id: contract.id,
        });
    }
    let slots = contract.definition.slots();
    if slots.is_empty() {
        // a contract of no values reads nothing, as in extract_features
        return Ok(FeatureVector {
            feature_contract_id: contract.id,
            version: contract.definition.version.clone(),
            values: Vec::new(),
        });
    }
    let analysis =
        analyze(samples, sample_rate_hz, &contract.analysis_config()).map_err(|error| {
            let pitch = FeatureSlot::Scalar(ScalarFeature::NormalizedMidiPitch);
            let pitch_error = matches!(
                error,
                AnalysisError::PitchAmbiguous { .. }
                    | AnalysisError::PitchOutOfRange { .. }
                    | AnalysisError::ClipTooShortForPitch { .. }
            );
            let index = slots
                .iter()
                .position(|s| *s == pitch)
                .filter(|_| pitch_error)
                .unwrap_or(0);
            FeatureError::FeatureNotComputable {
                feature: slots[index],
                index,
                reason: NotComputableReason::Analysis(error),
            }
        })?;
    extract_features(contract, &analysis, samples, note)
}

/// The checks every value rests on; the STFT of [`FEATURE_FFT_LENGTH`].
fn preconditions<'a>(
    contract: &FeatureContract,
    analysis: &'a Analysis,
    samples: &[f32],
    note: NoteSpan,
) -> Result<&'a StftAnalysis, NotComputableReason> {
    let definition = &contract.definition;
    if analysis.sample_rate_hz != SAMPLE_RATE_HZ {
        return Err(NotComputableReason::Analysis(
            AnalysisError::UnsupportedSampleRate {
                sample_rate_hz: analysis.sample_rate_hz,
            },
        ));
    }
    if samples.len() != analysis.frame_count {
        return Err(NotComputableReason::ClipMismatch {
            frame_count: samples.len(),
            analysis_frame_count: analysis.frame_count,
        });
    }
    check_finite(samples).map_err(NotComputableReason::Analysis)?;
    if note.start_sample >= note.end_sample || note.end_sample > samples.len() {
        return Err(NotComputableReason::NoteOutsideClip {
            start_sample: note.start_sample,
            end_sample: note.end_sample,
            frame_count: samples.len(),
        });
    }
    if !definition.band_windows.is_empty() && definition.band_count != BAND_COUNT {
        return Err(NotComputableReason::BandCountUnsupported {
            band_count: definition.band_count,
        });
    }
    if !definition.fft_lengths.contains(&FEATURE_FFT_LENGTH) {
        return Err(NotComputableReason::FftLengthNotInContract {
            fft_length: FEATURE_FFT_LENGTH,
        });
    }
    let stft_of = |fft_length: usize| {
        analysis
            .stfts
            .iter()
            .find(|s| s.fft_length == fft_length)
            .ok_or(NotComputableReason::StftMissing { fft_length })
    };
    for &fft_length in &definition.fft_lengths {
        stft_of(fft_length)?;
    }
    stft_of(FEATURE_FFT_LENGTH)
}

struct Extractor<'a> {
    analysis: &'a Analysis,
    samples: &'a [f32],
    note: NoteSpan,
    frames: &'a StftAnalysis,
}

impl Extractor<'_> {
    fn value(&self, slot: FeatureSlot) -> Result<f64, NotComputableReason> {
        match slot {
            FeatureSlot::Band { window, band } => self.band_energy(window, band),
            FeatureSlot::Scalar(scalar) => self.scalar(scalar),
        }
    }

    fn window(&self, window: BandWindow) -> Range<usize> {
        self.note.window(window, self.samples.len())
    }

    /// The frames whose centre `i * hop_length + fft_length / 2` lies in `range`, refused when
    /// there are none. A hop of 0 places no frame, nor does a centre that does not fit in `usize`.
    fn frames_in(
        &self,
        range: &Range<usize>,
    ) -> Result<Vec<&FrameDescriptors>, NotComputableReason> {
        let hop = self.frames.hop_length;
        let half = self.frames.fft_length / 2;
        let frames: Vec<_> = self
            .frames
            .frames
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                hop != 0
                    && i.checked_mul(hop)
                        .and_then(|offset| offset.checked_add(half))
                        .is_some_and(|centre| range.contains(&centre))
            })
            .map(|(_, frame)| frame)
            .collect();
        if frames.is_empty() {
            return Err(NotComputableReason::WindowTooShort {
                start_sample: range.start,
                end_sample: range.end,
            });
        }
        Ok(frames)
    }

    /// The samples of `range`, refused when their RMS level is below
    /// [`SILENCE_THRESHOLD_DBFS`].
    fn audible(&self, range: &Range<usize>) -> Result<&[f32], NotComputableReason> {
        let samples = &self.samples[range.clone()];
        let rms_dbfs = magnitude_db(rms(samples).map_err(NotComputableReason::Analysis)?);
        if rms_dbfs < SILENCE_THRESHOLD_DBFS {
            return Err(NotComputableReason::SilentWindow {
                start_sample: range.start,
                end_sample: range.end,
                rms_dbfs,
            });
        }
        Ok(samples)
    }

    fn band_energy(&self, window: BandWindow, band: usize) -> Result<f64, NotComputableReason> {
        let range = self.window(window);
        let frames = self.frames_in(&range)?;
        self.audible(&range)?;
        Ok(floored(power_mean_db(
            frames.iter().map(|f| f.band_energies_db[band]),
        )))
    }

    /// The mean of `field` over the frames of the gate.
    fn gate_mean(
        &self,
        field: impl Fn(&FrameDescriptors) -> f64,
    ) -> Result<f64, NotComputableReason> {
        let gate = self.note.gate();
        let frames = self.frames_in(&gate)?;
        self.audible(&gate)?;
        Ok(frames.iter().map(|f| field(f)).sum::<f64>() / frames.len() as f64)
    }

    fn scalar(&self, scalar: ScalarFeature) -> Result<f64, NotComputableReason> {
        let analysis_error = NotComputableReason::Analysis;
        match scalar {
            ScalarFeature::Rms => {
                let gate = self.audible(&self.note.gate())?;
                Ok(magnitude_db(rms(gate).map_err(analysis_error)?))
            }
            ScalarFeature::Peak => {
                let gate = self.audible(&self.note.gate())?;
                Ok(magnitude_db(peak(gate).map_err(analysis_error)?))
            }
            ScalarFeature::SpectralCentroid => self.gate_mean(|f| f.centroid_hz),
            ScalarFeature::Rolloff => self.gate_mean(|f| f.rolloff_hz),
            ScalarFeature::SpectralFlatness => self.gate_mean(|f| f.flatness),
            ScalarFeature::ZeroCrossingRate => {
                let gate = self.audible(&self.note.gate())?;
                zero_crossing_rate(gate).map_err(analysis_error)
            }
            ScalarFeature::AttackEnvelopeSlope => {
                let attack = self.audible(&self.window(BandWindow::Attack))?;
                attack_envelope_slope(attack, self.analysis.sample_rate_hz).map_err(analysis_error)
            }
            ScalarFeature::NormalizedMidiPitch => normalized_midi_pitch(self.analysis),
        }
    }
}

/// `midi / 127` of the analysis' f0, refused when the estimate is not confident or not a
/// frequency in the search range.
fn normalized_midi_pitch(analysis: &Analysis) -> Result<f64, NotComputableReason> {
    let estimate = analysis.f0;
    if estimate.confidence.is_nan() || estimate.confidence < F0_CONFIDENCE_THRESHOLD {
        return Err(NotComputableReason::PitchAmbiguous {
            confidence: estimate.confidence,
        });
    }
    let slack = 2f64.powf(F0_RANGE_TOLERANCE_CENTS / 1_200.0);
    let f0_hz = estimate.f0_hz;
    if f0_hz.is_nan() || f0_hz < F0_MIN_HZ / slack || f0_hz > F0_MAX_HZ * slack {
        return Err(NotComputableReason::PitchAbsent { f0_hz });
    }
    let midi = 69.0 + 12.0 * (f0_hz / 440.0).log2();
    Ok(midi / 127.0)
}

/// `10 log10(mean 10^(e / 10))` of the dB values `energies`, computed relative to their largest
/// so no power overflows; NaN when any value is not finite.
fn power_mean_db(energies: impl Iterator<Item = f64> + Clone) -> f64 {
    if energies.clone().any(|e| !e.is_finite()) {
        return f64::NAN;
    }
    let largest = energies.clone().fold(f64::NEG_INFINITY, f64::max);
    let (sum, count) = energies.fold((0.0, 0usize), |(sum, count), e| {
        (sum + 10f64.powf((e - largest) / 10.0), count + 1)
    });
    largest + 10.0 * (sum / count as f64).log10()
}

/// `value` floored at [`LOG_FLOOR_DB`]; a value that is not finite stays as it is, to be refused.
fn floored(value: f64) -> f64 {
    if value.is_finite() {
        value.max(LOG_FLOOR_DB)
    } else {
        value
    }
}
