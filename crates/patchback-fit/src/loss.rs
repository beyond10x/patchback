//! Time alignment and the four components of the comparison loss (DESIGN.md § 9.1); the
//! definitions are in the crate documentation.

use patchback_analysis::{
    AnalysisError, BAND_COUNT, HOP_DIVISOR, LOG_FLOOR_DB, MAX_DURATION_FRAMES, SAMPLE_RATE_HZ,
    cross_correlation, log_band_energies, stft,
};
use patchback_audio::AudioClip;

use crate::{
    ALIGNMENT_SMOOTHING_FRAMES, ALIGNMENT_TOLERANCE_FRAMES, BAND_ENERGY_FFT_LENGTH, CompareError,
    ENVELOPE_FFT_LENGTH, EPSILON, LOG_SPECTRAL_FFT_LENGTHS, LOSS_WEIGHTS, LossWeights,
    TRANSIENT_FFT_LENGTH,
};

/// Mono samples and the rate they were taken at.
#[derive(Clone, Copy, Debug)]
pub struct Signal<'a> {
    pub samples: &'a [f32],
    pub sample_rate_hz: u32,
}

/// The four component values of the loss (`spec/domains/fit.yaml` `LossComponents`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LossComponents {
    pub log_spectral: f64,
    pub envelope: f64,
    pub transient: f64,
    pub band_energy: f64,
}

/// One comparison of a candidate with a reference (`spec/domains/fit.yaml` `Comparison`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Comparison {
    /// The components, on the reference and the aligned candidate.
    pub components: LossComponents,
    /// The weights the total used: [`LOSS_WEIGHTS`].
    pub weights: LossWeights,
    /// `weights.total(&components)`.
    pub total: f64,
    /// The lag the candidate was aligned by, in samples; positive when the candidate was late.
    pub lag_frames: isize,
}

/// Compares `candidate` with `reference`, two imported clips.
pub fn compare(reference: &AudioClip, candidate: &AudioClip) -> Result<Comparison, CompareError> {
    compare_signals(
        Signal {
            samples: reference.samples(),
            sample_rate_hz: reference.sample_rate_hz(),
        },
        Signal {
            samples: candidate.samples(),
            sample_rate_hz: candidate.sample_rate_hz(),
        },
    )
}

/// Compares `candidate` with `reference`: checks, aligns, then computes the loss, in the order
/// of the crate documentation.
pub fn compare_signals(
    reference: Signal<'_>,
    candidate: Signal<'_>,
) -> Result<Comparison, CompareError> {
    if reference.sample_rate_hz != candidate.sample_rate_hz {
        return Err(CompareError::SampleRateMismatch {
            reference_hz: reference.sample_rate_hz,
            candidate_hz: candidate.sample_rate_hz,
        });
    }
    if reference.sample_rate_hz != SAMPLE_RATE_HZ {
        return Err(AnalysisError::UnsupportedSampleRate {
            sample_rate_hz: reference.sample_rate_hz,
        }
        .into());
    }
    check_input(reference.samples)?;
    check_input(candidate.samples)?;
    let (reference, candidate) = (reference.samples, candidate.samples);
    if reference.len().abs_diff(candidate.len()) > ALIGNMENT_TOLERANCE_FRAMES {
        return Err(CompareError::LengthOutsideAlignment {
            reference_frames: reference.len(),
            candidate_frames: candidate.len(),
        });
    }
    let lag_frames = alignment_lag(reference, candidate)?;
    let aligned = shifted(candidate, lag_frames, reference.len());
    let components = components(reference, &aligned)?;
    Ok(Comparison {
        components,
        weights: LOSS_WEIGHTS,
        total: LOSS_WEIGHTS.total(&components),
        lag_frames,
    })
}

/// The four components of `candidate` against `reference` as they lie, without alignment; the
/// shorter is read as zero past its end. Refuses what [`compare_signals`] refuses of each clip.
pub fn loss_components(
    reference: &[f32],
    candidate: &[f32],
) -> Result<LossComponents, CompareError> {
    check_input(reference)?;
    check_input(candidate)?;
    let length = reference.len().max(candidate.len());
    let padded = |samples: &[f32]| {
        let mut padded = samples.to_vec();
        padded.resize(length, 0.0);
        padded
    };
    Ok(components(&padded(reference), &padded(candidate))?)
}

/// Refuses an empty clip, one past the analysis duration bound, and a non-finite sample.
fn check_input(samples: &[f32]) -> Result<(), AnalysisError> {
    if samples.is_empty() {
        return Err(AnalysisError::EmptyAudio);
    }
    if samples.len() > MAX_DURATION_FRAMES {
        return Err(AnalysisError::DurationOverBound {
            frame_count: samples.len(),
        });
    }
    match samples.iter().position(|s| !s.is_finite()) {
        Some(index) => Err(AnalysisError::NonFiniteSamples { index }),
        None => Ok(()),
    }
}

/// The lag that best aligns `candidate` with `reference` (crate documentation, step 4).
///
/// The energy envelope `e[k]` of a clip is `(x / max |x|)^2` smoothed twice by a moving mean of
/// [`ALIGNMENT_SMOOTHING_FRAMES`] samples, for every `k` at which it is not 0 (samples outside
/// the clip count as 0), so a delayed copy of a clip has the same envelope, delayed, and the
/// ripple of a squared tone is left at a fraction of a percent. The correlation of two envelopes
/// at lag `d` is not built from the envelopes, which can be longer than the analysis duration
/// bound allows: it is the correlation of the squared samples smoothed four times by the same
/// moving mean along the lags, which is the same sum up to a constant factor.
fn alignment_lag(reference: &[f32], candidate: &[f32]) -> Result<isize, AnalysisError> {
    let width = ALIGNMENT_SMOOTHING_FRAMES;
    let reach = ALIGNMENT_TOLERANCE_FRAMES + 2 * (width - 1);
    let squares = cross_correlation(
        &normalised_square(reference),
        &normalised_square(candidate),
        reach,
    )?;
    let boxed = |x: &[f64]| -> Vec<f64> {
        let mut prefix = Vec::with_capacity(x.len() + 1);
        prefix.push(0.0);
        let mut sum = 0.0;
        for value in x {
            sum += value;
            prefix.push(sum);
        }
        (0..=x.len() - width)
            .map(|m| prefix[m + width] - prefix[m])
            .collect()
    };
    let correlation = boxed(&boxed(&boxed(&boxed(&squares))));
    let best = correlation.iter().copied().fold(0.0, f64::max);
    let threshold = best * (1.0 - 1e-12);
    let max_lag = ALIGNMENT_TOLERANCE_FRAMES as isize;
    let lag = (-max_lag..=max_lag)
        .zip(&correlation)
        .filter(|(_, r)| **r >= threshold)
        .map(|(lag, _)| lag)
        .min_by_key(|lag| (lag.unsigned_abs(), *lag))
        .unwrap_or(0);
    Ok(lag)
}

/// `(x / max |x|)^2` for every sample, in `[0, 1]`; all 0 for silence.
fn normalised_square(samples: &[f32]) -> Vec<f64> {
    let peak = samples
        .iter()
        .map(|s| f64::from(s.abs()))
        .fold(0.0, f64::max);
    if peak == 0.0 {
        return vec![0.0; samples.len()];
    }
    samples
        .iter()
        .map(|s| {
            let x = f64::from(*s) / peak;
            x * x
        })
        .collect()
}

/// `candidate[n + lag]` for `n` in `0..length`, 0 outside the candidate.
fn shifted(candidate: &[f32], lag: isize, length: usize) -> Vec<f32> {
    (0..length)
        .map(|n| {
            n.checked_add_signed(lag)
                .and_then(|i| candidate.get(i))
                .copied()
                .unwrap_or(0.0)
        })
        .collect()
}

/// The four components of two checked clips of equal length.
fn components(reference: &[f32], candidate: &[f32]) -> Result<LossComponents, AnalysisError> {
    Ok(LossComponents {
        log_spectral: log_spectral(reference, candidate)?,
        envelope: normalised_difference(
            &levels(reference, ENVELOPE_FFT_LENGTH)?,
            &levels(candidate, ENVELOPE_FFT_LENGTH)?,
        ),
        transient: normalised_difference(&onsets(reference)?, &onsets(candidate)?),
        band_energy: band_energy(reference, candidate)?,
    })
}

fn hop(fft_length: usize) -> usize {
    fft_length / HOP_DIVISOR
}

/// For each FFT length, the normalised difference of the log magnitudes above the floor,
/// `ln(1 + |X| / (n / 2) / EPSILON) = ln(EPSILON + |X| / (n / 2)) - ln(EPSILON)`, over every frame
/// and bin; the mean over the lengths.
fn log_spectral(reference: &[f32], candidate: &[f32]) -> Result<f64, AnalysisError> {
    let mut sum = 0.0;
    for fft_length in LOG_SPECTRAL_FFT_LENGTHS {
        let window_sum = fft_length as f64 / 2.0;
        let above_floor = |samples: &[f32]| -> Result<Vec<f64>, AnalysisError> {
            Ok(stft(samples, fft_length, hop(fft_length))?
                .magnitudes
                .iter()
                .flatten()
                .map(|m| (m / window_sum / EPSILON).ln_1p())
                .collect())
        };
        sum += normalised_difference(&above_floor(reference)?, &above_floor(candidate)?);
    }
    Ok(sum / LOG_SPECTRAL_FFT_LENGTHS.len() as f64)
}

/// The level of each frame of the STFT at `fft_length`: `sqrt(sum_k |X_k|^2) / (n / 2)`, the
/// Hann-windowed short-time RMS in the unit of the log-spectral magnitudes. Each frame is divided
/// by its largest magnitude before squaring, so no sum overflows.
fn levels(samples: &[f32], fft_length: usize) -> Result<Vec<f64>, AnalysisError> {
    let spectra = stft(samples, fft_length, hop(fft_length))?;
    let window_sum = fft_length as f64 / 2.0;
    Ok(spectra
        .magnitudes
        .iter()
        .map(|frame| {
            let scale = frame.iter().copied().fold(0.0, f64::max);
            if scale == 0.0 {
                return 0.0;
            }
            let power: f64 = frame.iter().map(|m| (m / scale).powi(2)).sum();
            scale * power.sqrt() / window_sum
        })
        .collect())
}

/// The onset envelope: the rise `max(0, ln(EPSILON + l_t) - ln(EPSILON + l_{t-1}))` of the
/// frame levels at [`TRANSIENT_FFT_LENGTH`], the level before the first frame being 0.
fn onsets(samples: &[f32]) -> Result<Vec<f64>, AnalysisError> {
    let mut previous = EPSILON.ln();
    Ok(levels(samples, TRANSIENT_FFT_LENGTH)?
        .into_iter()
        .map(|level| {
            let current = (EPSILON + level).ln();
            let rise = (current - previous).max(0.0);
            previous = current;
            rise
        })
        .collect())
}

/// `sum |a - b| / (sum a + sum b)` over two non-negative curves of equal length, each value
/// divided by the largest first; 0 when both are all zero.
fn normalised_difference(a: &[f64], b: &[f64]) -> f64 {
    let scale = a.iter().chain(b).copied().fold(0.0, f64::max);
    if scale == 0.0 {
        return 0.0;
    }
    let (mut difference, mut total) = (0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        let (x, y) = (x / scale, y / scale);
        difference += (x - y).abs();
        total += x + y;
    }
    difference / total
}

/// The energy of each band, summed over the frames of the STFT at [`BAND_ENERGY_FFT_LENGTH`].
fn band_energies(samples: &[f32]) -> Result<[f64; BAND_COUNT], AnalysisError> {
    let spectra = stft(samples, BAND_ENERGY_FFT_LENGTH, hop(BAND_ENERGY_FFT_LENGTH))?;
    let bin_hz = f64::from(SAMPLE_RATE_HZ) / BAND_ENERGY_FFT_LENGTH as f64;
    let mut energies = [0.0; BAND_COUNT];
    for frame in &spectra.magnitudes {
        for (energy, db) in energies.iter_mut().zip(log_band_energies(frame, bin_hz)?) {
            if db > LOG_FLOOR_DB {
                *energy += 10f64.powf(db / 10.0);
            }
        }
    }
    Ok(energies)
}

fn band_energy(reference: &[f32], candidate: &[f32]) -> Result<f64, AnalysisError> {
    let distribution = |energies: [f64; BAND_COUNT]| {
        let scale = energies.iter().copied().fold(0.0, f64::max);
        if scale == 0.0 {
            return None;
        }
        let total: f64 = energies.iter().map(|e| e / scale).sum();
        Some(energies.map(|e| e / scale / total))
    };
    Ok(
        match (
            distribution(band_energies(reference)?),
            distribution(band_energies(candidate)?),
        ) {
            (None, None) => 0.0,
            (Some(_), None) | (None, Some(_)) => 1.0,
            (Some(p), Some(q)) => 0.5 * p.iter().zip(&q).map(|(a, b)| (a - b).abs()).sum::<f64>(),
        },
    )
}
