//! The iterative radix-2 complex FFT and its inverse (DESIGN.md § 6.2).
//!
//! Decimation in time: the input is permuted into bit-reversed order, then `log2 n` butterfly
//! passes combine transforms of length 2, 4, ... n. The twiddle factors `e^(-2 pi i k / n)` are
//! computed once per call, each directly from its angle, so their error does not accumulate.

use std::f64::consts::PI;

use crate::{AnalysisError, Complex, MAX_FFT_LENGTH};

/// Refuses a length the radix-2 FFT cannot take: not a power of two (0 included), or past
/// [`MAX_FFT_LENGTH`].
pub(crate) fn check_length(fft_length: usize) -> Result<(), AnalysisError> {
    if !fft_length.is_power_of_two() {
        return Err(AnalysisError::FftLengthNotPowerOfTwo { fft_length });
    }
    if fft_length > MAX_FFT_LENGTH {
        return Err(AnalysisError::FftLengthOverBound { fft_length });
    }
    Ok(())
}

fn check_buffer(buffer: &[Complex]) -> Result<(), AnalysisError> {
    check_length(buffer.len())?;
    match buffer.iter().position(|z| !z.is_finite()) {
        Some(index) => Err(AnalysisError::NonFiniteSamples { index }),
        None => Ok(()),
    }
}

/// The forward DFT of `buffer`, in place: `X_k = sum_n x_n e^(-2 pi i k n / N)`, unscaled.
///
/// Refuses a length that is not a power of two or is past [`MAX_FFT_LENGTH`], and a non-finite
/// component; the buffer is then left as it was.
pub fn fft(buffer: &mut [Complex]) -> Result<(), AnalysisError> {
    check_buffer(buffer)?;
    transform(buffer, -1.0);
    Ok(())
}

/// The inverse DFT of `buffer`, in place: `x_n = (1 / N) sum_k X_k e^(2 pi i k n / N)`, so
/// [`fft`] followed by `ifft` returns the input within rounding. Refuses what [`fft`] refuses.
pub fn ifft(buffer: &mut [Complex]) -> Result<(), AnalysisError> {
    check_buffer(buffer)?;
    transform(buffer, 1.0);
    let scale = 1.0 / buffer.len() as f64;
    for z in buffer.iter_mut() {
        *z = z.scale(scale);
    }
    Ok(())
}

/// The unscaled transform with twiddles `e^(sign 2 pi i k / n)`; `buffer.len()` is a power of
/// two.
fn transform(buffer: &mut [Complex], sign: f64) {
    let n = buffer.len();
    if n < 2 {
        return;
    }
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if i < j {
            buffer.swap(i, j);
        }
    }
    let twiddles: Vec<Complex> = (0..n / 2)
        .map(|k| Complex::from_angle(sign * 2.0 * PI * k as f64 / n as f64))
        .collect();
    let mut length = 2;
    while length <= n {
        let half = length / 2;
        let stride = n / length;
        for start in (0..n).step_by(length) {
            for k in 0..half {
                let w = twiddles[k * stride];
                let even = buffer[start + k];
                let odd = buffer[start + k + half] * w;
                buffer[start + k] = even + odd;
                buffer[start + k + half] = even - odd;
            }
        }
        length *= 2;
    }
}
