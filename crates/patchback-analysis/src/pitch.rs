//! Monophonic f0 estimation (DESIGN.md § 6.2): a YIN-style cumulative mean normalised
//! difference function with a confidence. A pitch that cannot be identified is refused, never
//! guessed: below [`F0_CONFIDENCE_THRESHOLD`], outside [`F0_MIN_HZ`] to [`F0_MAX_HZ`] (each
//! widened by [`F0_RANGE_TOLERANCE_CENTS`]), or in a clip too short to search for it.
//!
//! With `rate` the sample rate and `slack = 2^(F0_RANGE_TOLERANCE_CENTS / 1200)`,
//! `min_lag = floor(rate / F0_MAX_HZ)` and `max_lag = ceil(rate * slack / F0_MIN_HZ)` (963 at
//! 48 kHz, so the tolerance band below [`F0_MIN_HZ`] is searched as well as the one above
//! [`F0_MAX_HZ`]):
//!
//! 1. The lags: a clip of `len` samples is searched up to `top = min(max_lag, len - F0_WINDOW -
//!    1)`. A clip of fewer than `F0_WINDOW + min_lag + 2` samples (2074 at 48 kHz) searches
//!    nothing and is refused as [`AnalysisError::ClipTooShortForPitch`] naming that length.
//! 2. The segment: the [`F0_WINDOW`] samples, at a start that is a multiple of half the window
//!    (or the last possible start), with the most energy among those followed by `top + 1`
//!    samples.
//! 3. `d(tau) = sum_{j < W} (x_j - x_{j + tau})^2` for `tau` in `1 ..= top + 1`, and
//!    `d'(tau) = d(tau) tau / sum_{1 <= t <= tau} d(t)` (`d'` is 1 where that sum is 0).
//! 4. The dip: the first `tau` in `2 ..= top` with `d'(tau) < 1 - F0_CONFIDENCE_THRESHOLD`,
//!    moved down to the bottom of its dip within `top`. The search starts below `min_lag`, so a
//!    pitch above the range is seen at its own period rather than at a multiple of it.
//!    - No dip: refused as too short when `top < max_lag`, otherwise as
//!      [`AnalysisError::PitchAmbiguous`] with confidence `1 - min d'` over
//!      `min_lag ..= max_lag`, clamped to `[0, 1]`.
//!    - A dip whose bottom is `top` while `d'` still falls after it: the period lies past what
//!      was searched; refused as too short when `top < max_lag`, otherwise as
//!      [`AnalysisError::PitchOutOfRange`] beyond [`F0_MIN_HZ`].
//!
//!    "Too short" past step 1 names `F0_WINDOW + max_lag + 1` samples (3012 at 48 kHz), the
//!    length that searches the whole lag range; such a clip is never refused as too short.
//! 5. The period: a parabola through the raw difference `d` at `tau - 1`, `tau`, `tau + 1`
//!    refines the lag to `T`. The divisors: while `T / 2` or `T / 3` lies in the range (at most
//!    [`F0_MAX_HZ`] widened by the tolerance) and the subharmonic test (below) between it and `T`
//!    says the shorter, `T` becomes it. A narrow pulse whose period is not a whole number of
//!    samples can dip first at two or three periods, where its edges align better. Then the
//!    multiples: for `m = 2, 4, 8, ...` while `m * period` lies within `top`, the bottom of `d`
//!    within one lag of `round(m * period)` is refined by the same parabola and divided by `m`.
//!    The parabola's error (up to about 0.1 lag where `d` is V-shaped, as for a sawtooth's or a
//!    square's edges) is divided by `m`, so a short period is measured over many cycles.
//!    Refinement stops at a multiple whose bottom is not a dip under the threshold.
//! 6. The octave: while `2 * period` lies within `top` and the subharmonic test between `period`
//!    and `2 * period` says the longer, the period becomes the parabola's vertex at the lowest `d`
//!    within two percent of `2 * period`, refined over its multiples as in step 5.
//!
//!    The subharmonic test (`subharmonic_test`) between a period `S` and `q S` measures, on the
//!    comb `x_j - x_{j + round(S)}` over the window, the first `SUBHARMONIC_COUNT` (8) harmonics
//!    of `1 / (q S)` that are not harmonics of `1 / S`; the comb removes what repeats every `S`
//!    samples. Their power as a share of the window's power decides: at least
//!    `SUBHARMONIC_LONG_SHARE` (-19.6 dB), the longer period; under `SUBHARMONIC_SHORT_SHARE`
//!    (-40 dB), the shorter; between, undecided, and the pitch is refused as
//!    [`AnalysisError::PitchAmbiguous`] with a confidence of at most `UNDECIDED_CONFIDENCE`
//!    (0.5). Before that, aliasing: a naive waveform whose period is not a whole number of samples
//!    folds its harmonics past the Nyquist frequency onto those frequencies (a period of `k + 0.5`
//!    samples repeats only every `2k + 1`), each at most about the level of its harmonics just
//!    below the Nyquist frequency. When the strongest measured harmonic is at most `ALIAS_FACTOR`
//!    (4) times the window's power between a quarter of the rate and the Nyquist frequency per
//!    harmonic of `1 / S` in that band, it is taken as aliasing and the period is the shorter.
//! 7. The confidence is `1 - d'` at the bottom nearest the final period, or `1 - d'` at the
//!    lowest within one lag of twice the period if that is less and within `top`: a period is
//!    trusted only as far as it repeats. `f0 = rate / period`. An undecided octave is refused as
//!    [`AnalysisError::PitchAmbiguous`]; an `f0` more than [`F0_RANGE_TOLERANCE_CENTS`] above
//!    [`F0_MAX_HZ`] or below [`F0_MIN_HZ`] as [`AnalysisError::PitchOutOfRange`]; a confidence
//!    below the threshold as [`AnalysisError::PitchAmbiguous`].
//!
//! # Accuracy domain
//!
//! Held by `tests/numeric.rs`, `tests/adversary.rs`, `tests/adversary2.rs` and
//! `tests/adversary3.rs`, on clips of 8192 samples or one second at 48 kHz:
//!
//! - Within 5 cents and never refused: a sine; a naive sawtooth, square (50 % pulse), 25 % pulse
//!   and triangle at 401 log-spaced pitches across the range and at every period of `k + 0.5`
//!   samples.
//! - A naive 10 % pulse: within 5 cents or refused as [`AnalysisError::PitchAmbiguous`]; it is
//!   refused at some periods near `k + 0.5` samples and at the top of the range, where its
//!   aliases are as strong as a real subharmonic.
//! - A fundamental 12 to 19 dB under its second harmonic: the true period. 20 to 30 dB under:
//!   refused as [`AnalysisError::PitchAmbiguous`], never the octave above. From about 40 dB
//!   under (a share under -40 dB): the second harmonic's period, the fundamental taken as
//!   absent. A fundamental under about 20 dB in a signal with strong content above a quarter of
//!   the sample rate may be taken as aliasing and reported at the octave above.
//! - A missing fundamental (harmonics 2 to 6): the true period or refused. A tone with a
//!   sub-octave at -10 dB: the sub-octave's period or refused. Two tones a fifth apart: their
//!   common period or refused. A sawtooth with 30-cent vibrato: within its vibrato or refused.
//! - Noise: refused as [`AnalysisError::PitchAmbiguous`].
//!
//! Every constant of steps 5 and 6 is provisional; nothing past these signals is claimed.

use crate::{
    AnalysisError, Complex, F0_CONFIDENCE_THRESHOLD, F0_MAX_HZ, F0_MIN_HZ, F0_WINDOW, check_finite,
};

/// How far past [`F0_MIN_HZ`] or [`F0_MAX_HZ`] an estimate may lie and still be reported: the
/// estimator's own tolerance, so a pitch on the edge of the range is not refused for rounding.
pub const F0_RANGE_TOLERANCE_CENTS: f64 = 5.0;

/// The share of the window's power, in the harmonics a longer period has and the shorter one
/// lacks, from which the period is the longer one (the subharmonic test): -19.6 dB, between a
/// fundamental 19 dB under its second harmonic (a share of -19.1 dB) and one 20 dB under
/// (-20.0 dB). Provisional.
const SUBHARMONIC_LONG_SHARE: f64 = 0.011;

/// The share under which those harmonics are taken as absent and the period is the shorter one
/// (the subharmonic test): -40 dB. Between this and [`SUBHARMONIC_LONG_SHARE`] the period is
/// undecided and the pitch refused. Provisional.
const SUBHARMONIC_SHORT_SHARE: f64 = 1e-4;

/// How many times the mean power per harmonic of the shorter period between a quarter of the
/// sample rate and the Nyquist frequency the strongest harmonic it lacks may reach and still be
/// taken as aliasing (the subharmonic test). Provisional.
const ALIAS_FACTOR: f64 = 4.0;

/// How many of the harmonics the longer period has and the shorter one lacks the subharmonic
/// test measures, from the lowest. Provisional.
const SUBHARMONIC_COUNT: usize = 8;

/// The confidence an undecided period is refused with, at most: neither period is favoured.
const UNDECIDED_CONFIDENCE: f64 = 0.5;

/// An accepted f0 estimate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PitchEstimate {
    /// The fundamental frequency in Hz.
    pub f0_hz: f64,
    /// `1 - d'` at the chosen lag, in `[F0_CONFIDENCE_THRESHOLD, 1]`.
    pub confidence: f64,
}

/// The f0 of the monophonic `samples` at `sample_rate_hz`.
///
/// Refuses a non-finite sample, a sample rate of 0, a clip too short to search
/// ([`AnalysisError::ClipTooShortForPitch`]), a pitch outside the search range
/// ([`AnalysisError::PitchOutOfRange`]), and an estimate whose confidence is below
/// [`F0_CONFIDENCE_THRESHOLD`] ([`AnalysisError::PitchAmbiguous`]).
pub fn estimate_f0(samples: &[f32], sample_rate_hz: u32) -> Result<PitchEstimate, AnalysisError> {
    check_finite(samples)?;
    if sample_rate_hz == 0 {
        return Err(AnalysisError::UnsupportedSampleRate { sample_rate_hz });
    }
    let rate = f64::from(sample_rate_hz);
    let slack = 2f64.powf(F0_RANGE_TOLERANCE_CENTS / 1_200.0);
    let min_lag = ((rate / F0_MAX_HZ).floor() as usize).max(2);
    let max_lag = ((rate * slack / F0_MIN_HZ).ceil() as usize).max(min_lag + 1);
    let too_short = |required_frame_count| AnalysisError::ClipTooShortForPitch {
        frame_count: samples.len(),
        required_frame_count,
    };
    let least = F0_WINDOW + min_lag + 2;
    if samples.len() < least {
        return Err(too_short(least));
    }
    let top = max_lag.min(samples.len() - F0_WINDOW - 1);
    let truncated = top < max_lag;
    let whole_range = F0_WINDOW + max_lag + 1;
    let span = F0_WINDOW + top + 1;
    let start = loudest_segment(samples, span);
    let x: Vec<f64> = samples[start..start + span]
        .iter()
        .map(|s| f64::from(*s))
        .collect();

    // d(tau) and d'(tau) for tau in 0 ..= top + 1; d(0) = 0, d'(0) = 1
    let mut diff = vec![0.0; top + 2];
    let mut cmnd = vec![1.0; top + 2];
    let mut running = 0.0;
    for tau in 1..=top + 1 {
        let d: f64 = (0..F0_WINDOW)
            .map(|j| {
                let delta = x[j] - x[j + tau];
                delta * delta
            })
            .sum();
        diff[tau] = d;
        running += d;
        cmnd[tau] = if running > 0.0 {
            d * tau as f64 / running
        } else {
            1.0
        };
    }

    let dip = 1.0 - F0_CONFIDENCE_THRESHOLD;
    let Some(first) = (2..=top).find(|&tau| cmnd[tau] < dip) else {
        if truncated {
            return Err(too_short(whole_range));
        }
        let best = (min_lag..=max_lag)
            .map(|tau| cmnd[tau])
            .fold(f64::INFINITY, f64::min);
        return Err(AnalysisError::PitchAmbiguous {
            confidence: (1.0 - best).clamp(0.0, 1.0),
        });
    };
    let mut lag = first;
    while lag < top && cmnd[lag + 1] < cmnd[lag] {
        lag += 1;
    }
    if lag == top && cmnd[top + 1] < cmnd[top] {
        return Err(if truncated {
            too_short(whole_range)
        } else {
            AnalysisError::PitchOutOfRange {
                bound_hz: F0_MIN_HZ,
            }
        });
    }

    let mut period = refine(&diff, lag);
    let upper_band = upper_band_power(&x[..F0_WINDOW]);
    let mut undecided;
    'divide: loop {
        let mut divisor_undecided = false;
        for factor in [2u32, 3] {
            let short = period / f64::from(factor);
            if rate / short > F0_MAX_HZ * slack {
                continue;
            }
            match subharmonic_test(&x, upper_band, short, factor) {
                Period::Short => {
                    period = short;
                    continue 'divide;
                }
                Period::Long => {}
                Period::Undecided => divisor_undecided = true,
            }
        }
        undecided = divisor_undecided;
        break;
    }
    period = refine_over_multiples(&diff, &cmnd, period, top);
    while !undecided && 2.0 * period <= top as f64 {
        match subharmonic_test(&x, upper_band, period, 2) {
            Period::Long => {
                let doubled = lowest_near(&diff, 2.0 * period, top)
                    .map_or(2.0 * period, |b| refine(&diff, b));
                period = refine_over_multiples(&diff, &cmnd, doubled, top);
            }
            Period::Short => break,
            Period::Undecided => undecided = true,
        }
    }
    let at = bottom_near(&diff, period, top).unwrap_or(lag);
    let mut confidence = (1.0 - cmnd[at]).clamp(0.0, 1.0);
    let twice = (2.0 * period).round() as usize;
    if twice < top {
        let lowest = (twice - 1..=twice + 1)
            .map(|tau| cmnd[tau])
            .fold(f64::INFINITY, f64::min);
        confidence = confidence.min((1.0 - lowest).clamp(0.0, 1.0));
    }
    if undecided {
        return Err(AnalysisError::PitchAmbiguous {
            confidence: confidence.min(UNDECIDED_CONFIDENCE),
        });
    }

    let f0_hz = rate / period;
    if f0_hz > F0_MAX_HZ * slack {
        return Err(AnalysisError::PitchOutOfRange {
            bound_hz: F0_MAX_HZ,
        });
    }
    if f0_hz < F0_MIN_HZ / slack {
        return Err(AnalysisError::PitchOutOfRange {
            bound_hz: F0_MIN_HZ,
        });
    }
    if confidence < F0_CONFIDENCE_THRESHOLD {
        return Err(AnalysisError::PitchAmbiguous { confidence });
    }
    Ok(PitchEstimate { f0_hz, confidence })
}

/// The lag of `tau`'s parabola vertex through `d` at `tau - 1`, `tau`, `tau + 1`, moved by at
/// most half a lag. `1 <= tau` and `tau + 1 < d.len()`.
fn refine(diff: &[f64], tau: usize) -> f64 {
    let (before, at, after) = (diff[tau - 1], diff[tau], diff[tau + 1]);
    let curvature = before - 2.0 * at + after;
    let shift = if curvature > 0.0 {
        (0.5 * (before - after) / curvature).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    tau as f64 + shift
}

/// The lag of the lowest `d` within one lag of `round(lag)`, if it is a local minimum of `d`
/// strictly inside the searched lags `2 ..= top`; `None` past `top`.
fn bottom_near(diff: &[f64], lag: f64, top: usize) -> Option<usize> {
    let centre = lag.round() as usize;
    if centre < 3 || centre + 1 > top {
        return None;
    }
    let bottom = (centre - 1..=centre + 1).min_by(|a, b| diff[*a].total_cmp(&diff[*b]))?;
    (diff[bottom - 1] >= diff[bottom] && diff[bottom + 1] >= diff[bottom]).then_some(bottom)
}

/// The lag of the lowest `d` within two percent of `lag` (at least one lag either side), if it
/// is a local minimum of `d` strictly inside the searched lags; `None` otherwise.
fn lowest_near(diff: &[f64], lag: f64, top: usize) -> Option<usize> {
    let width = (lag / 50.0).ceil().max(1.0);
    let low = ((lag - width).round().max(2.0)) as usize;
    let high = ((lag + width).round() as usize).min(top);
    if low >= high {
        return None;
    }
    let bottom = (low..=high).min_by(|a, b| diff[*a].total_cmp(&diff[*b]))?;
    (bottom > low && bottom < high).then_some(bottom)
}

/// `period` refined over its multiples (step 5): for `m = 2, 4, 8, ...` while the bottom of `d`
/// within one lag of `round(m * period)` is a dip under the threshold, that bottom refined by
/// the parabola and divided by `m`.
fn refine_over_multiples(diff: &[f64], cmnd: &[f64], mut period: f64, top: usize) -> f64 {
    let dip = 1.0 - F0_CONFIDENCE_THRESHOLD;
    let mut m = 2;
    while let Some(bottom) = bottom_near(diff, period * m as f64, top) {
        if cmnd[bottom] >= dip {
            break;
        }
        period = refine(diff, bottom) / m as f64;
        m *= 2;
    }
    period
}

/// The subharmonic test's verdict between a period `short` and a multiple of it.
enum Period {
    Short,
    Long,
    Undecided,
}

/// The subharmonic test between the period `short` and `factor * short` (steps 5 and 6), on
/// the window that `d(round(short))` compares: the first [`F0_WINDOW`] samples of `x` and the
/// same shifted by `round(short)`.
///
/// The longer period's harmonics that the shorter one lacks, at `k / (factor * short)` cycles per
/// sample for `k` not a multiple of `factor` (the first [`SUBHARMONIC_COUNT`] below the Nyquist
/// frequency), are measured on the comb `x_j - x_{j + round(short)}`. The comb removes what
/// repeats every `short` samples, and with it the window's leakage from the shorter period's
/// harmonics; each measurement is divided by the comb's gain at its frequency. Their sum, as a
/// share of the window's power, is the evidence for the longer period:
///
/// - the strongest of them at most [`ALIAS_FACTOR`] times `upper_band` (the window's power
///   between a quarter of the sample rate and the Nyquist frequency) divided by the number of
///   harmonics of `1 / short` in that band: aliasing, the shorter period. A naive waveform whose
///   period is not a whole number of samples folds its harmonics past the Nyquist frequency onto
///   those frequencies, each at most about the level of its harmonics just below the Nyquist
///   frequency;
/// - otherwise a share of at least [`SUBHARMONIC_LONG_SHARE`]: the longer period;
/// - under [`SUBHARMONIC_SHORT_SHARE`]: the shorter period;
/// - between the two: undecided.
fn subharmonic_test(x: &[f64], upper_band: f64, short: f64, factor: u32) -> Period {
    let shift = short.round() as usize;
    if shift == 0 || F0_WINDOW + shift > x.len() {
        return Period::Short;
    }
    let window = &x[..F0_WINDOW];
    let power = mean_power(window);
    if power <= 0.0 {
        return Period::Short;
    }
    let comb: Vec<f64> = (0..F0_WINDOW).map(|j| x[j] - x[j + shift]).collect();
    let long = f64::from(factor) * short;
    let (mut lacking, mut strongest) = (0.0, 0.0f64);
    for k in (1..).filter(|k| k % factor != 0).take(SUBHARMONIC_COUNT) {
        let frequency = f64::from(k) / long;
        if frequency >= 0.5 {
            break;
        }
        let gain = 2.0 * (std::f64::consts::PI * frequency * shift as f64).sin();
        if gain.abs() < 1.0 {
            continue;
        }
        let p = component_power(&comb, frequency) / (gain * gain);
        lacking += p;
        strongest = strongest.max(p);
    }
    let first = (short / 4.0).floor() as usize + 1;
    let last = (short / 2.0).ceil() as usize - 1;
    let alias = if last >= first {
        upper_band / (last - first + 1) as f64
    } else {
        0.0
    };
    if strongest <= ALIAS_FACTOR * alias {
        Period::Short
    } else if lacking >= SUBHARMONIC_LONG_SHARE * power {
        Period::Long
    } else if lacking < SUBHARMONIC_SHORT_SHARE * power {
        Period::Short
    } else {
        Period::Undecided
    }
}

/// The power of `window` (whose length is a power of two) between a quarter of the sample rate
/// and the Nyquist frequency, both excluded, in the units of [`mean_power`]: `2 sum |X_b|^2 / (N
/// sum w^2)` over those bins `b` of the Hann-windowed DFT, by Parseval's theorem.
fn upper_band_power(window: &[f64]) -> f64 {
    let n = window.len();
    let Ok(hann) = crate::window::hann(n) else {
        return 0.0;
    };
    let mut buffer: Vec<Complex> = window
        .iter()
        .zip(&hann)
        .map(|(v, w)| Complex::new(v * w, 0.0))
        .collect();
    if crate::fft::fft(&mut buffer).is_err() {
        return 0.0;
    }
    let energy: f64 = hann.iter().map(|w| w * w).sum();
    if energy <= 0.0 {
        return 0.0;
    }
    let band: f64 = buffer[n / 4 + 1..n / 2].iter().map(|z| z.norm_sqr()).sum();
    2.0 * band / (n as f64 * energy)
}

/// The Hann-weighted mean power of `x`.
fn mean_power(x: &[f64]) -> f64 {
    let n = x.len() as f64;
    let (mut sum, mut weight) = (0.0, 0.0);
    for (j, v) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * j as f64 / n).cos();
        sum += w * v * v;
        weight += w;
    }
    if weight > 0.0 { sum / weight } else { 0.0 }
}

/// The power of a sinusoid at `frequency` cycles per sample in `x`, from the Hann-windowed DFT at
/// that frequency: `2 |X|^2 / (sum w)^2`, which is `A^2 / 2` for a sinusoid of amplitude `A` far
/// from other components.
fn component_power(x: &[f64], frequency: f64) -> f64 {
    let n = x.len() as f64;
    let (step_sin, step_cos) = (2.0 * std::f64::consts::PI * frequency).sin_cos();
    let (window_sin, window_cos) = (2.0 * std::f64::consts::PI / n).sin_cos();
    let (mut re, mut im, mut weight) = (0.0, 0.0, 0.0);
    let (mut pr, mut pi) = (1.0f64, 0.0f64);
    let (mut qr, mut qi) = (1.0f64, 0.0f64);
    for v in x {
        let w = 0.5 - 0.5 * qr;
        re += w * v * pr;
        im += w * v * pi;
        weight += w;
        (pr, pi) = (pr * step_cos + pi * step_sin, pi * step_cos - pr * step_sin);
        (qr, qi) = (
            qr * window_cos - qi * window_sin,
            qi * window_cos + qr * window_sin,
        );
    }
    if weight > 0.0 {
        2.0 * (re * re + im * im) / (weight * weight)
    } else {
        0.0
    }
}

/// The start of the [`F0_WINDOW`] samples with the most energy, among starts at multiples of
/// half the window (and the last possible start) that leave `span` samples. The caller has
/// checked that `samples` holds at least `span`.
fn loudest_segment(samples: &[f32], span: usize) -> usize {
    let last = samples.len() - span;
    let energy = |start: usize| -> f64 {
        samples[start..start + F0_WINDOW]
            .iter()
            .map(|s| f64::from(*s) * f64::from(*s))
            .sum()
    };
    let starts = (0..=last).step_by(F0_WINDOW / 2).chain([last]);
    let mut best = (0, f64::MIN);
    for start in starts {
        let e = energy(start);
        if e > best.1 {
            best = (start, e);
        }
    }
    best.0
}
