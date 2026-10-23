//! The Hann-windowed short-time Fourier transform (DESIGN.md § 6.2, § 6.3).

use crate::fft::check_length;
use crate::{AnalysisError, Complex, LOG_FLOOR_DB, MAX_STFT_VALUES, check_finite, fft, hann};

/// The magnitude spectra of one STFT.
#[derive(Clone, Debug, PartialEq)]
pub struct Stft {
    pub fft_length: usize,
    pub hop_length: usize,
    /// One spectrum per frame, of `fft_length / 2 + 1` unnormalised magnitudes `|X_k|`, bin `k`
    /// at `k * sample_rate / fft_length` Hz.
    pub magnitudes: Vec<Vec<f64>>,
}

/// The STFT of `samples`: frames of `fft_length` samples starting at `0, hop, 2 hop, ...` while
/// the start lies inside the input (so `ceil(len / hop)` frames, none for empty input), samples
/// past the end read as zero, each frame multiplied by [`hann`] and transformed by [`fft`].
///
/// Refuses, in this order, a zero hop, an FFT length the radix-2 FFT cannot take, more
/// magnitudes than [`MAX_STFT_VALUES`] (before anything is allocated), and a non-finite sample.
pub fn stft(samples: &[f32], fft_length: usize, hop_length: usize) -> Result<Stft, AnalysisError> {
    if hop_length == 0 {
        return Err(AnalysisError::InvalidHopLength { hop_length });
    }
    check_length(fft_length)?;
    check_size(samples.len(), fft_length, hop_length)?;
    check_finite(samples)?;
    let window = hann(fft_length)?;
    let bins = fft_length / 2 + 1;
    let mut buffer = vec![Complex::ZERO; fft_length];
    let mut magnitudes = Vec::with_capacity(samples.len().div_ceil(hop_length));
    let mut start = 0;
    while start < samples.len() {
        for (k, slot) in buffer.iter_mut().enumerate() {
            let sample = samples.get(start + k).copied().map_or(0.0, f64::from);
            *slot = Complex::new(sample * window[k], 0.0);
        }
        fft(&mut buffer)?;
        magnitudes.push(buffer[..bins].iter().map(|z| z.norm()).collect());
        start += hop_length;
    }
    Ok(Stft {
        fft_length,
        hop_length,
        magnitudes,
    })
}

/// Refuses an STFT of `len` samples at `fft_length` and `hop_length` (both positive) whose
/// `ceil(len / hop)` frames of `fft_length / 2 + 1` magnitudes exceed [`MAX_STFT_VALUES`].
fn check_size(len: usize, fft_length: usize, hop_length: usize) -> Result<(), AnalysisError> {
    let frame_count = len.div_ceil(hop_length);
    if frame_count > MAX_STFT_VALUES / (fft_length / 2 + 1) {
        return Err(AnalysisError::FrameCountOverBound {
            fft_length,
            hop_length,
            frame_count,
        });
    }
    Ok(())
}

impl Stft {
    /// Every magnitude in dB, by [`log_magnitude`].
    pub fn log_magnitudes(&self) -> Vec<Vec<f64>> {
        self.magnitudes
            .iter()
            .map(|frame| frame.iter().map(|m| magnitude_db(*m)).collect())
            .collect()
    }
}

/// `20 log10(|magnitude|)`, floored at [`LOG_FLOOR_DB`]; `magnitude` is finite, so the result is.
pub(crate) fn magnitude_db(magnitude: f64) -> f64 {
    let magnitude = magnitude.abs();
    if magnitude > 0.0 {
        (20.0 * magnitude.log10()).max(LOG_FLOOR_DB)
    } else {
        LOG_FLOOR_DB
    }
}

/// `20 log10(|magnitude|)` in dB, floored at [`LOG_FLOOR_DB`]; a non-finite magnitude is
/// refused.
pub fn log_magnitude(magnitude: f64) -> Result<f64, AnalysisError> {
    check_finite(&[magnitude])?;
    Ok(magnitude_db(magnitude))
}
