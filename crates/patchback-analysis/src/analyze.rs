//! `patchback analyze` over one clip (DESIGN.md § 6.2, § 11; `spec/domains/analysis.yaml`
//! `AnalyzeAudio`).

use patchback_audio::AudioClip;

use crate::fft::check_length;
use crate::stft::magnitude_db;
use crate::{
    ANALYSIS_FFT_LENGTHS, AnalysisError, BAND_COUNT, HOP_DIVISOR, MAX_DURATION_FRAMES,
    MAX_STFT_VALUES, PitchEstimate, SAMPLE_RATE_HZ, SILENCE_THRESHOLD_DBFS, check_finite,
    estimate_f0, log_band_energies, peak, rms, spectral_centroid, spectral_flatness, spectral_flux,
    spectral_rolloff, stft,
};

/// One STFT to compute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StftConfig {
    pub fft_length: usize,
    pub hop_length: usize,
}

/// What [`analyze`] computes: one STFT per entry, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisConfig {
    pub stfts: Vec<StftConfig>,
}

impl Default for AnalysisConfig {
    /// [`ANALYSIS_FFT_LENGTHS`], each with the hop `fft_length / HOP_DIVISOR` (50 % overlap).
    fn default() -> Self {
        Self {
            stfts: ANALYSIS_FFT_LENGTHS
                .iter()
                .map(|&fft_length| StftConfig {
                    fft_length,
                    hop_length: fft_length / HOP_DIVISOR,
                })
                .collect(),
        }
    }
}

/// The descriptors of one STFT frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameDescriptors {
    /// [`rms`] of the frame's samples inside the clip (the RMS envelope).
    pub rms: f64,
    /// [`peak`] of the frame's samples inside the clip (the peak envelope).
    pub peak: f64,
    pub centroid_hz: f64,
    pub rolloff_hz: f64,
    pub flatness: f64,
    /// [`spectral_flux`] from the previous frame; the first frame's is against no spectrum.
    pub flux: f64,
    pub band_energies_db: [f64; BAND_COUNT],
}

/// The descriptors at one FFT length.
#[derive(Clone, Debug, PartialEq)]
pub struct StftAnalysis {
    pub fft_length: usize,
    pub hop_length: usize,
    pub frames: Vec<FrameDescriptors>,
}

/// The analysis of one clip.
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    pub sample_rate_hz: u32,
    pub frame_count: usize,
    /// The clip's RMS level in dBFS (full scale 1.0), floored at [`crate::LOG_FLOOR_DB`].
    pub rms_dbfs: f64,
    /// The clip's largest `|x|`.
    pub peak: f64,
    pub f0: PitchEstimate,
    pub stfts: Vec<StftAnalysis>,
}

/// [`analyze`] of an imported clip, at its sample rate.
pub fn analyze_clip(clip: &AudioClip, config: &AnalysisConfig) -> Result<Analysis, AnalysisError> {
    analyze(clip.samples(), clip.sample_rate_hz(), config)
}

/// Refuses, before anything is allocated, a configuration whose STFTs of a clip of `len` samples
/// hold more than [`MAX_STFT_VALUES`] magnitudes together: each STFT holds `ceil(len / hop)`
/// frames of `fft_length / 2 + 1` bins, and the STFT at which the running total passes the bound
/// is named. Hops are positive here.
fn check_config_size(len: usize, config: &AnalysisConfig) -> Result<(), AnalysisError> {
    let mut total: usize = 0;
    for s in &config.stfts {
        let frame_count = len.div_ceil(s.hop_length);
        total = total.saturating_add(frame_count.saturating_mul(s.fft_length / 2 + 1));
        if total > MAX_STFT_VALUES {
            return Err(AnalysisError::FrameCountOverBound {
                fft_length: s.fft_length,
                hop_length: s.hop_length,
                frame_count,
            });
        }
    }
    Ok(())
}

/// Analyses `samples` at `sample_rate_hz` with `config`, or refuses with the first of, in the
/// order of the `AnalyzeAudio` outcomes: no samples; a rate other than [`SAMPLE_RATE_HZ`]; a
/// zero hop; an FFT length that is not a power of two; one past [`crate::MAX_FFT_LENGTH`]; more
/// than [`MAX_DURATION_FRAMES`] samples; STFTs holding more than [`MAX_STFT_VALUES`]
/// magnitudes together; a non-finite sample; an RMS level below [`SILENCE_THRESHOLD_DBFS`]; a
/// clip too short for the f0 search; an f0 outside the search range; an f0 confidence below
/// [`crate::F0_CONFIDENCE_THRESHOLD`]. A refusal returns nothing else: when the f0 estimate is
/// refused, no descriptor is returned.
pub fn analyze(
    samples: &[f32],
    sample_rate_hz: u32,
    config: &AnalysisConfig,
) -> Result<Analysis, AnalysisError> {
    if samples.is_empty() {
        return Err(AnalysisError::EmptyAudio);
    }
    if sample_rate_hz != SAMPLE_RATE_HZ {
        return Err(AnalysisError::UnsupportedSampleRate { sample_rate_hz });
    }
    if let Some(s) = config.stfts.iter().find(|s| s.hop_length == 0) {
        return Err(AnalysisError::InvalidHopLength {
            hop_length: s.hop_length,
        });
    }
    if let Some(s) = config
        .stfts
        .iter()
        .find(|s| !s.fft_length.is_power_of_two())
    {
        return Err(AnalysisError::FftLengthNotPowerOfTwo {
            fft_length: s.fft_length,
        });
    }
    for s in &config.stfts {
        check_length(s.fft_length)?;
    }
    if samples.len() > MAX_DURATION_FRAMES {
        return Err(AnalysisError::DurationOverBound {
            frame_count: samples.len(),
        });
    }
    check_config_size(samples.len(), config)?;
    check_finite(samples)?;
    let rms_dbfs = magnitude_db(rms(samples)?);
    if rms_dbfs < SILENCE_THRESHOLD_DBFS {
        return Err(AnalysisError::SilentInput { rms_dbfs });
    }
    let f0 = estimate_f0(samples, sample_rate_hz)?;
    let stfts = config
        .stfts
        .iter()
        .map(|s| describe(samples, sample_rate_hz, *s))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Analysis {
        sample_rate_hz,
        frame_count: samples.len(),
        rms_dbfs,
        peak: peak(samples)?,
        f0,
        stfts,
    })
}

fn describe(
    samples: &[f32],
    sample_rate_hz: u32,
    config: StftConfig,
) -> Result<StftAnalysis, AnalysisError> {
    let spectra = stft(samples, config.fft_length, config.hop_length)?;
    let bin_hz = f64::from(sample_rate_hz) / config.fft_length as f64;
    let mut frames = Vec::with_capacity(spectra.magnitudes.len());
    let mut previous: &[f64] = &[];
    for (index, magnitudes) in spectra.magnitudes.iter().enumerate() {
        let start = index * config.hop_length;
        let end = (start + config.fft_length).min(samples.len());
        let frame = &samples[start..end];
        frames.push(FrameDescriptors {
            rms: rms(frame)?,
            peak: peak(frame)?,
            centroid_hz: spectral_centroid(magnitudes, bin_hz)?,
            rolloff_hz: spectral_rolloff(magnitudes, bin_hz)?,
            flatness: spectral_flatness(magnitudes)?,
            flux: spectral_flux(previous, magnitudes)?,
            band_energies_db: log_band_energies(magnitudes, bin_hz)?,
        });
        previous = magnitudes;
    }
    Ok(StftAnalysis {
        fft_length: config.fft_length,
        hop_length: config.hop_length,
        frames,
    })
}
