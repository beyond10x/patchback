//! `cross_correlation`, added for the time alignment of story:audio-comparison (DESIGN.md
//! § 9.1): the FFT result agrees with the direct sum `sum_n a[n] b[n + d]` at every lag, and it
//! refuses what every analysis function refuses.

use patchback_analysis::{
    AnalysisError, MAX_DURATION_FRAMES, MAX_FFT_LENGTH, cross_correlation,
    cross_correlation_fft_length,
};

/// A deterministic sequence in `[-1, 1)` (64-bit LCG, Knuth's MMIX constants).
fn noise(count: usize, seed: u64) -> Vec<f32> {
    let mut state = seed;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (((state >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0) as f32
        })
        .collect()
}

fn direct(a: &[f32], b: &[f32], max_lag: usize) -> Vec<f64> {
    let max_lag = max_lag as isize;
    (-max_lag..=max_lag)
        .map(|d| {
            a.iter()
                .enumerate()
                .filter_map(|(n, x)| {
                    let j = n as isize + d;
                    (0..b.len() as isize)
                        .contains(&j)
                        .then(|| f64::from(*x) * f64::from(b[j as usize]))
                })
                .sum()
        })
        .collect()
}

/// Tolerance: `1e-9` of `sqrt(sum a^2 sum b^2)`, the bound on every `|r(d)|`.
fn assert_agrees(a: &[f32], b: &[f32], max_lag: usize) {
    let fast = cross_correlation(a, b, max_lag).unwrap();
    let slow = direct(a, b, max_lag);
    assert_eq!(fast.len(), 2 * max_lag + 1);
    let energy = |x: &[f32]| x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
    let scale = (energy(a) * energy(b)).sqrt().max(f64::MIN_POSITIVE);
    for (index, (f, s)) in fast.iter().zip(&slow).enumerate() {
        assert!(
            (f - s).abs() <= 1e-9 * scale,
            "lengths {} {}, max_lag {max_lag}, lag {}: {f} vs {s}",
            a.len(),
            b.len(),
            index as isize - max_lag as isize
        );
    }
}

#[test]
fn cross_correlation_agrees_with_the_direct_sum_at_every_lag() {
    for (a_len, b_len, max_lag) in [
        (1, 1, 0),
        (1, 1, 3),
        (7, 5, 4),
        (100, 100, 10),
        (300, 280, 40),
        (2_000, 2_100, 150),
        (5_000, 4_900, 600),
        (40, 3_000, 700),
    ] {
        let a = noise(a_len, a_len as u64);
        let b = noise(b_len, b_len as u64 + 1);
        assert_agrees(&a, &b, max_lag);
    }
}

#[test]
fn f64_sequences_correlate_like_their_f32_values() {
    let a = noise(900, 21);
    let b = noise(1_000, 22);
    let wide = |x: &[f32]| x.iter().map(|v| f64::from(*v)).collect::<Vec<f64>>();
    assert_eq!(
        cross_correlation(&wide(&a), &wide(&b), 64).unwrap(),
        cross_correlation(&a, &b, 64).unwrap()
    );
    assert_eq!(
        cross_correlation(&[0.0, f64::INFINITY], &[1.0], 1),
        Err(AnalysisError::NonFiniteSamples { index: 1 })
    );
}

#[test]
fn a_delayed_copy_peaks_at_its_delay() {
    let a = noise(6_000, 11);
    for delay in [0usize, 1, 37, 480] {
        let mut b = vec![0.0f32; delay];
        b.extend_from_slice(&a);
        let r = cross_correlation(&a, &b, 500).unwrap();
        let best = r
            .iter()
            .enumerate()
            .max_by(|x, y| x.1.total_cmp(y.1))
            .unwrap()
            .0;
        assert_eq!(best, 500 + delay, "delay {delay}");
    }
}

#[test]
fn empty_input_correlates_to_zero() {
    assert_eq!(cross_correlation(&[], &[1.0], 2).unwrap(), vec![0.0; 5]);
    assert_eq!(cross_correlation(&[1.0], &[], 0).unwrap(), vec![0.0]);
}

#[test]
fn non_finite_input_is_refused() {
    assert_eq!(
        cross_correlation(&[0.0, f32::NAN], &[0.0], 1),
        Err(AnalysisError::NonFiniteSamples { index: 1 })
    );
    assert_eq!(
        cross_correlation(&[0.0], &[0.0, 0.0, f32::INFINITY], 1),
        Err(AnalysisError::NonFiniteSamples { index: 2 })
    );
}

#[test]
fn a_lag_range_past_the_fft_bound_is_refused_before_anything_is_allocated() {
    let fft_length = cross_correlation_fft_length(MAX_FFT_LENGTH);
    assert!(fft_length > MAX_FFT_LENGTH);
    assert_eq!(
        cross_correlation(&[1.0], &[1.0], MAX_FFT_LENGTH),
        Err(AnalysisError::FftLengthOverBound { fft_length })
    );
    assert_eq!(
        cross_correlation(&[1.0], &[1.0], usize::MAX),
        Err(AnalysisError::FftLengthOverBound {
            fft_length: usize::MAX
        })
    );
}

#[test]
fn input_past_the_duration_bound_is_refused() {
    let long = vec![0.0f32; MAX_DURATION_FRAMES + 1];
    assert_eq!(
        cross_correlation(&long, &[0.0], 1),
        Err(AnalysisError::DurationOverBound {
            frame_count: MAX_DURATION_FRAMES + 1
        })
    );
    assert_eq!(
        cross_correlation(&[0.0], &long, 1),
        Err(AnalysisError::DurationOverBound {
            frame_count: MAX_DURATION_FRAMES + 1
        })
    );
}

#[test]
fn extreme_finite_input_correlates_to_finite_values() {
    let a = vec![f32::MAX; 3_000];
    let b = vec![-f32::MAX; 3_000];
    for value in cross_correlation(&a, &b, 100).unwrap() {
        assert!(value.is_finite() && value < 0.0, "{value}");
    }
}
