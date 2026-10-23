//! Cross-correlation over a bounded lag range, by blocks of [`fft`] (DESIGN.md § 6.2, § 9.1:
//! the time alignment that precedes the comparison loss).

use crate::{AnalysisError, Complex, MAX_DURATION_FRAMES, MAX_FFT_LENGTH, check_finite, fft, ifft};

/// The shortest FFT [`cross_correlation`] uses, so a small lag range is not computed in many
/// tiny blocks.
const MIN_CORRELATION_FFT_LENGTH: usize = 1_024;

/// The FFT length [`cross_correlation`] uses for lags up to `max_lag`: the larger of 1 024 and
/// twice the power of two at or above `2 max_lag + 1`, so each block of the reference is at
/// least half the transform. `usize::MAX` when that length does not fit in a `usize`.
pub fn cross_correlation_fft_length(max_lag: usize) -> usize {
    max_lag
        .checked_mul(2)
        .and_then(|span| span.checked_add(1))
        .and_then(usize::checked_next_power_of_two)
        .and_then(|span| span.checked_mul(2))
        .map_or(usize::MAX, |length| length.max(MIN_CORRELATION_FFT_LENGTH))
}

/// The cross-correlation `r(d) = sum_n reference[n] candidate[n + d]` for every lag `d` from
/// `-max_lag` to `max_lag`, at index `d + max_lag`; a term whose `n + d` lies outside
/// `candidate` is 0. A positive `d` means `candidate` is late: `candidate[n + d]` lines up with
/// `reference[n]`. The inputs are samples (`f32`) or any sequence of `f64` values, such as an
/// energy envelope, and are computed with in `f64`.
///
/// Computed by overlap-save: the reference is cut into blocks of `L - 2 max_lag` samples, `L`
/// being [`cross_correlation_fft_length`], and each block is correlated with the stretch of the
/// candidate it can meet through `ifft(conj(fft(block)) fft(stretch))`. The values agree with
/// the direct sum within the rounding of the FFT.
///
/// Refuses, in this order and before anything is allocated, an FFT length past
/// [`MAX_FFT_LENGTH`], either input past [`MAX_DURATION_FRAMES`] samples, and a non-finite
/// sample of `reference`, then of `candidate` (at its index in that input). Empty input
/// correlates to all zeros.
pub fn cross_correlation<T: Copy + Into<f64>>(
    reference: &[T],
    candidate: &[T],
    max_lag: usize,
) -> Result<Vec<f64>, AnalysisError> {
    let fft_length = cross_correlation_fft_length(max_lag);
    if fft_length > MAX_FFT_LENGTH {
        return Err(AnalysisError::FftLengthOverBound { fft_length });
    }
    for input in [reference, candidate] {
        if input.len() > MAX_DURATION_FRAMES {
            return Err(AnalysisError::DurationOverBound {
                frame_count: input.len(),
            });
        }
    }
    check_finite(reference)?;
    check_finite(candidate)?;

    let span = 2 * max_lag + 1;
    let block = fft_length - 2 * max_lag;
    let mut correlation = vec![0.0; span];
    let mut x = vec![Complex::ZERO; fft_length];
    let mut y = vec![Complex::ZERO; fft_length];
    let mut start = 0;
    while start < reference.len() {
        let end = (start + block).min(reference.len());
        if reference[start..end].iter().all(|s| (*s).into() == 0.0) {
            start = end;
            continue;
        }
        for (k, slot) in x.iter_mut().enumerate() {
            let sample = if start + k < end {
                reference[start + k].into()
            } else {
                0.0
            };
            *slot = Complex::new(sample, 0.0);
        }
        // y[k] = candidate[start - max_lag + k] for k < (end - start) + 2 max_lag, else 0
        let reach = end - start + 2 * max_lag;
        for (k, slot) in y.iter_mut().enumerate() {
            let index = (start + k).checked_sub(max_lag);
            let sample = match index {
                Some(i) if k < reach && i < candidate.len() => candidate[i].into(),
                _ => 0.0,
            };
            *slot = Complex::new(sample, 0.0);
        }
        fft(&mut x)?;
        fft(&mut y)?;
        for (a, b) in x.iter_mut().zip(&y) {
            *a = a.conj() * *b;
        }
        ifft(&mut x)?;
        // x[m] = sum_n block[n] y[n + m]; lag d = m - max_lag for m in 0..span, no wrap
        for (value, z) in correlation.iter_mut().zip(&x[..span]) {
            *value += z.re;
        }
        start = end;
    }
    Ok(correlation)
}
