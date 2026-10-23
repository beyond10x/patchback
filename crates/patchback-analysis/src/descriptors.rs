//! Envelopes and spectral descriptors (DESIGN.md § 6.2).
//!
//! The spectral descriptors take one magnitude spectrum, bin `k` at `k * bin_hz` Hz, and use
//! `|m|` of each value. They are computed on the spectrum divided by its largest `|m|` (the band
//! energies on each band divided by its own), so no intermediate sum overflows for any finite
//! input; the scale-invariant ones (centroid, rolloff, flatness) do not depend on that division. A spectrum without energy (empty or all
//! zero) has centroid, rolloff and flatness 0 and every band at [`LOG_FLOOR_DB`].

use crate::{AnalysisError, BAND_EDGES_HZ, LOG_FLOOR_DB, ROLLOFF_FRACTION, check_finite};

/// The number of logarithmic bands of [`log_band_energies`].
pub const BAND_COUNT: usize = BAND_EDGES_HZ.len() - 1;

/// The root mean square of `samples`; 0 for none.
pub fn rms(samples: &[f32]) -> Result<f64, AnalysisError> {
    check_finite(samples)?;
    if samples.is_empty() {
        return Ok(0.0);
    }
    let sum: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    Ok((sum / samples.len() as f64).sqrt())
}

/// The largest `|x|` of `samples`; 0 for none.
pub fn peak(samples: &[f32]) -> Result<f64, AnalysisError> {
    check_finite(samples)?;
    Ok(samples
        .iter()
        .map(|s| f64::from(s.abs()))
        .fold(0.0, f64::max))
}

/// `|m|` of each value divided by the largest, and that largest; `None` without energy.
fn normalised(magnitudes: &[f64]) -> Option<(Vec<f64>, f64)> {
    let scale = magnitudes.iter().map(|m| m.abs()).fold(0.0, f64::max);
    if scale > 0.0 {
        Some((magnitudes.iter().map(|m| m.abs() / scale).collect(), scale))
    } else {
        None
    }
}

fn check_spectrum(magnitudes: &[f64], bin_hz: f64) -> Result<(), AnalysisError> {
    check_finite(magnitudes)?;
    if !bin_hz.is_finite() {
        return Err(AnalysisError::NonFiniteSamples {
            index: magnitudes.len(),
        });
    }
    Ok(())
}

/// A frequency computed from a finite `bin_hz` that overflowed is refused like a non-finite
/// `bin_hz`, at index `len`: the result is finite or a typed refusal.
fn finite_frequency(hz: f64, len: usize) -> Result<f64, AnalysisError> {
    if hz.is_finite() {
        Ok(hz)
    } else {
        Err(AnalysisError::NonFiniteSamples { index: len })
    }
}

/// The magnitude-weighted mean frequency `sum (k bin_hz) |m_k| / sum |m_k|`, in Hz.
///
/// A non-finite `bin_hz`, or one so large that the centroid overflows, is refused as a
/// non-finite value at index `magnitudes.len()`.
pub fn spectral_centroid(magnitudes: &[f64], bin_hz: f64) -> Result<f64, AnalysisError> {
    check_spectrum(magnitudes, bin_hz)?;
    let Some((m, _)) = normalised(magnitudes) else {
        return Ok(0.0);
    };
    let total: f64 = m.iter().sum();
    let weighted: f64 = m.iter().enumerate().map(|(k, v)| k as f64 * v).sum();
    finite_frequency(weighted / total * bin_hz, magnitudes.len())
}

/// The frequency of the lowest bin `k` at which the cumulative power `sum_{j <= k} m_j^2`
/// reaches [`ROLLOFF_FRACTION`] of the total, in Hz.
///
/// A non-finite `bin_hz`, or one so large that the rolloff overflows, is refused as a
/// non-finite value at index `magnitudes.len()`.
pub fn spectral_rolloff(magnitudes: &[f64], bin_hz: f64) -> Result<f64, AnalysisError> {
    check_spectrum(magnitudes, bin_hz)?;
    let Some((m, _)) = normalised(magnitudes) else {
        return Ok(0.0);
    };
    let total: f64 = m.iter().map(|v| v * v).sum();
    let target = ROLLOFF_FRACTION * total;
    let mut cumulative = 0.0;
    let mut bin = m.len() - 1;
    for (k, v) in m.iter().enumerate() {
        cumulative += v * v;
        if cumulative >= target {
            bin = k;
            break;
        }
    }
    finite_frequency(bin as f64 * bin_hz, magnitudes.len())
}

/// The geometric mean of the power `m_k^2` over its arithmetic mean, in `[0, 1]`: 1 for a flat
/// spectrum, 0 when any bin is zero.
pub fn spectral_flatness(magnitudes: &[f64]) -> Result<f64, AnalysisError> {
    check_finite(magnitudes)?;
    let Some((m, _)) = normalised(magnitudes) else {
        return Ok(0.0);
    };
    if m.contains(&0.0) {
        return Ok(0.0);
    }
    let count = m.len() as f64;
    let log_mean = m.iter().map(|v| (v * v).ln()).sum::<f64>() / count;
    let mean = m.iter().map(|v| v * v).sum::<f64>() / count;
    Ok((log_mean.exp() / mean).clamp(0.0, 1.0))
}

/// The power `sum m_k^2` of the bins in each band of [`crate::BAND_EDGES_HZ`], in dB
/// (`10 log10`), floored at [`LOG_FLOOR_DB`]. Band `b` holds the bins at
/// `BAND_EDGES_HZ[b] <= k bin_hz < BAND_EDGES_HZ[b + 1]`; the last band also holds the bins at or
/// above its upper edge.
///
/// Dynamic range: each band is divided by its own largest `|m|` before squaring, so a band is
/// reported at its power however far below another band it lies (from the largest finite
/// magnitude down to the floor), and no sum overflows for finite input.
pub fn log_band_energies(
    magnitudes: &[f64],
    bin_hz: f64,
) -> Result<[f64; BAND_COUNT], AnalysisError> {
    check_spectrum(magnitudes, bin_hz)?;
    let mut bands = [LOG_FLOOR_DB; BAND_COUNT];
    let band_of = |k: usize| {
        let frequency = k as f64 * bin_hz;
        (0..BAND_COUNT)
            .find(|&b| frequency < BAND_EDGES_HZ[b + 1])
            .unwrap_or(BAND_COUNT - 1)
    };
    let mut scale = [0.0f64; BAND_COUNT];
    for (k, v) in magnitudes.iter().enumerate() {
        let band = band_of(k);
        scale[band] = scale[band].max(v.abs());
    }
    let mut power = [0.0f64; BAND_COUNT];
    for (k, v) in magnitudes.iter().enumerate() {
        let band = band_of(k);
        if scale[band] > 0.0 {
            let ratio = v.abs() / scale[band];
            power[band] += ratio * ratio;
        }
    }
    // the scale in dB unfloored: a quiet band reports its true power down to the floor
    for ((band, p), s) in bands.iter_mut().zip(power).zip(scale) {
        if p > 0.0 {
            *band = (20.0 * s.log10() + 10.0 * p.log10()).max(LOG_FLOOR_DB);
        }
    }
    Ok(bands)
}

/// The half-wave rectified magnitude increase `sum_k max(0, |c_k| - |p_k|)` from `previous` to
/// `current`; a bin missing from the shorter spectrum counts as 0, so the first frame's flux,
/// against an empty `previous`, is its magnitude sum. Saturates at `f64::MAX`.
pub fn spectral_flux(previous: &[f64], current: &[f64]) -> Result<f64, AnalysisError> {
    check_finite(previous)?;
    check_finite(current)?;
    let scale = previous
        .iter()
        .chain(current)
        .map(|m| m.abs())
        .fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok(0.0);
    }
    let bins = previous.len().max(current.len());
    let at = |spectrum: &[f64], k: usize| spectrum.get(k).map_or(0.0, |m| m.abs() / scale);
    let sum: f64 = (0..bins)
        .map(|k| (at(current, k) - at(previous, k)).max(0.0))
        .sum();
    Ok((sum * scale).min(f64::MAX))
}
