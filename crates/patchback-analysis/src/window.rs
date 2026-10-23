//! The Hann window (DESIGN.md § 6.2).

use std::f64::consts::PI;

use crate::{AnalysisError, MAX_FFT_LENGTH};

/// The periodic Hann window of `length` samples: `w[k] = 0.5 - 0.5 cos(2 pi k / length)`.
///
/// Periodic, not symmetric, so that frames at 50 % overlap sum to a constant and the energy
/// `sum w[k]^2` is exactly `3 length / 8` for `length >= 3` (its sum is `length / 2`). A window
/// of one sample is `[1.0]`, and of zero samples empty. A length past [`MAX_FFT_LENGTH`] is
/// refused.
pub fn hann(length: usize) -> Result<Vec<f64>, AnalysisError> {
    if length > MAX_FFT_LENGTH {
        return Err(AnalysisError::FftLengthOverBound { fft_length: length });
    }
    if length == 1 {
        return Ok(vec![1.0]);
    }
    Ok((0..length)
        .map(|k| 0.5 - 0.5 * (2.0 * PI * k as f64 / length as f64).cos())
        .collect())
}
