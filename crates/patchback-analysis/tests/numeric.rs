//! The numeric obligations of story:analysis-core (DESIGN.md § 6.2, § 12.1, § 2 invariant 4),
//! one test per obligation. The tolerances are stated beside each test.

use std::f64::consts::PI;

use patchback_analysis::{
    AnalysisError, Complex, F0_MAX_HZ, F0_MIN_HZ, F0_RANGE_TOLERANCE_CENTS, F0_WINDOW,
    LOG_FLOOR_DB, MAX_STFT_VALUES, estimate_f0, fft, hann, ifft, log_band_energies, log_magnitude,
    peak, rms, spectral_centroid, spectral_flatness, spectral_flux, spectral_rolloff, stft,
};

const RATE: u32 = 48_000;

/// A deterministic sequence in `[-1, 1)` (64-bit LCG, Knuth's MMIX constants).
fn noise(count: usize, seed: u64) -> Vec<f64> {
    let mut state = seed;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((state >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        })
        .collect()
}

fn direct_dft(input: &[Complex]) -> Vec<Complex> {
    let n = input.len();
    (0..n)
        .map(|k| {
            input.iter().enumerate().fold(Complex::ZERO, |sum, (j, x)| {
                let angle = -2.0 * PI * (k * j % n) as f64 / n as f64;
                sum + *x * Complex::from_angle(angle)
            })
        })
        .collect()
}

fn complex_noise(n: usize, seed: u64) -> Vec<Complex> {
    let re = noise(n, seed);
    let im = noise(n, seed + 1);
    re.into_iter()
        .zip(im)
        .map(|(re, im)| Complex::new(re, im))
        .collect()
}

fn sine(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (amplitude * (2.0 * PI * frequency_hz * i as f64 / f64::from(RATE)).sin()) as f32)
        .collect()
}

/// A naive (not band-limited) sawtooth in `[-amplitude, amplitude)`.
fn saw(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            let phase = (frequency_hz * i as f64 / f64::from(RATE)).fract();
            (amplitude * (2.0 * phase - 1.0)) as f32
        })
        .collect()
}

fn cents(estimate: f64, truth: f64) -> f64 {
    1_200.0 * (estimate / truth).log2()
}

fn close(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance
}

/// Tolerance: `1e-9 * n` per component against a direct `O(n^2)` DFT in `f64`.
#[test]
fn fft_matches_a_direct_dft_for_every_power_of_two_up_to_64() {
    let mut n = 1;
    while n <= 64 {
        let input = complex_noise(n, n as u64);
        let expected = direct_dft(&input);
        let mut actual = input.clone();
        fft(&mut actual).unwrap();
        for (k, (a, e)) in actual.iter().zip(&expected).enumerate() {
            let tolerance = 1e-9 * n as f64;
            assert!(
                close(a.re, e.re, tolerance) && close(a.im, e.im, tolerance),
                "n = {n}, bin {k}: {a:?} vs {e:?}"
            );
        }
        n *= 2;
    }
}

/// Tolerance: `1e-12` absolute per component, inputs in `[-1, 1)`, lengths 1 to 8192.
#[test]
fn fft_then_ifft_roundtrips_within_1e_12() {
    let mut n = 1;
    while n <= 8_192 {
        let input = complex_noise(n, 7 + n as u64);
        let mut buffer = input.clone();
        fft(&mut buffer).unwrap();
        ifft(&mut buffer).unwrap();
        for (i, (a, e)) in buffer.iter().zip(&input).enumerate() {
            assert!(
                close(a.re, e.re, 1e-12) && close(a.im, e.im, 1e-12),
                "n = {n}, sample {i}: {a:?} vs {e:?}"
            );
        }
        n *= 2;
    }
}

/// A sine at exactly bin `k` of a 2048-point frame peaks at bin `k`, and its Hann-windowed
/// magnitude there is `A * N / 4` (the window's coherent gain of 1/2 times `N / 2`) within 1e-6
/// relative.
#[test]
fn a_pure_sine_lands_in_its_expected_bin() {
    let n = 2_048;
    for k in [1usize, 37, 100, 512, 1_000] {
        let frequency = k as f64 * f64::from(RATE) / n as f64;
        let samples = sine(frequency, 0.5, n);
        let spectrum = stft(&samples, n, n / 2).unwrap();
        let first = &spectrum.magnitudes[0];
        assert_eq!(first.len(), n / 2 + 1);
        let (argmax, max) =
            first.iter().enumerate().fold(
                (0, f64::MIN),
                |best, (i, m)| if *m > best.1 { (i, *m) } else { best },
            );
        assert_eq!(argmax, k, "{frequency} Hz");
        let expected = 0.5 * n as f64 / 4.0;
        assert!(
            (max - expected).abs() <= 1e-6 * expected,
            "{frequency} Hz: {max} vs {expected}"
        );
    }
}

/// The periodic Hann window's energy `sum w^2` is `3N / 8` and its sum `N / 2`, within 1e-9, for
/// `N >= 3` (at `N = 2` the window is `[0, 1]`, energy 1).
#[test]
fn hann_window_energy_matches_the_closed_form() {
    for n in [4usize, 8, 512, 2_048, 8_192] {
        let window = hann(n).unwrap();
        assert_eq!(window.len(), n);
        let energy: f64 = window.iter().map(|w| w * w).sum();
        let sum: f64 = window.iter().sum();
        assert!(
            close(energy, 3.0 * n as f64 / 8.0, 1e-9),
            "n = {n}: {energy}"
        );
        assert!(close(sum, n as f64 / 2.0, 1e-9), "n = {n}: {sum}");
    }
    assert_eq!(hann(2).unwrap(), [0.0, 1.0]);
    assert_eq!(hann(1).unwrap(), [1.0]);
}

/// Bins 6000 Hz apart (an 8-point FFT at 48 kHz); magnitudes 1 at 6 kHz and 3 at 18 kHz:
/// `(6000 * 1 + 18000 * 3) / 4 = 15000` Hz.
#[test]
fn spectral_centroid_matches_a_hand_computed_value() {
    let centroid = spectral_centroid(&[0.0, 1.0, 0.0, 3.0, 0.0], 6_000.0).unwrap();
    assert!(close(centroid, 15_000.0, 1e-9), "{centroid}");
    assert_eq!(spectral_centroid(&[0.0; 5], 6_000.0).unwrap(), 0.0);
}

/// Powers 4, 1, 1, 0, 0 total 6; 85 % is 5.1, first reached at bin 2: 12000 Hz.
#[test]
fn spectral_rolloff_matches_a_hand_computed_value() {
    let rolloff = spectral_rolloff(&[2.0, 1.0, 1.0, 0.0, 0.0], 6_000.0).unwrap();
    assert!(close(rolloff, 12_000.0, 1e-9), "{rolloff}");
    let flat = spectral_rolloff(&[1.0; 5], 6_000.0).unwrap();
    assert!(close(flat, 24_000.0, 1e-9), "{flat}");
    assert_eq!(spectral_rolloff(&[0.0; 5], 6_000.0).unwrap(), 0.0);
}

/// Equal powers are flat (1); powers 1 and 4 have geometric mean 2 and arithmetic mean 2.5, so
/// 0.8; a zero bin makes the geometric mean, and the flatness, 0.
#[test]
fn spectral_flatness_matches_hand_computed_values() {
    assert!(close(spectral_flatness(&[0.7; 9]).unwrap(), 1.0, 1e-12));
    assert!(close(spectral_flatness(&[1.0, 2.0]).unwrap(), 0.8, 1e-12));
    assert_eq!(spectral_flatness(&[1.0, 0.0, 1.0]).unwrap(), 0.0);
    assert_eq!(spectral_flatness(&[0.0; 4]).unwrap(), 0.0);
}

/// `rms([0.3, -0.4]) = sqrt(0.125)`, `rms([1, 0, 0, 0]) = 0.5`; peak is the largest `|x|`.
#[test]
fn rms_and_peak_match_hand_computed_values() {
    assert!(close(rms(&[0.3, -0.4]).unwrap(), 0.125f64.sqrt(), 1e-7));
    assert!(close(rms(&[1.0, 0.0, 0.0, 0.0]).unwrap(), 0.5, 1e-12));
    assert!(close(peak(&[0.3, -0.4]).unwrap(), 0.4, 1e-7));
    assert_eq!(peak(&[0.0, 0.0]).unwrap(), 0.0);
}

/// Half-wave rectified magnitude increase: `[1, 2, 3] -> [2, 1, 5]` is `1 + 0 + 2 = 3`; the
/// first frame's flux is its own magnitude sum.
#[test]
fn spectral_flux_matches_hand_computed_values() {
    assert!(close(
        spectral_flux(&[1.0, 2.0, 3.0], &[2.0, 1.0, 5.0]).unwrap(),
        3.0,
        1e-12
    ));
    assert!(close(
        spectral_flux(&[], &[2.0, 1.0, 5.0]).unwrap(),
        8.0,
        1e-12
    ));
    assert_eq!(spectral_flux(&[1.0, 1.0], &[0.5, 0.5]).unwrap(), 0.0);
}

/// A 16-point FFT at 48 kHz has bins 3000 Hz apart. With every magnitude 1: band 0 holds the DC
/// bin (1), bands 1 to 4 nothing, band 5 the 3 kHz bin (1), band 6 the 6 and 9 kHz bins (2),
/// band 7 the 12 to 24 kHz bins (5); energies in dB, the empty bands at the floor.
#[test]
fn log_band_energies_match_hand_computed_values() {
    let bands = log_band_energies(&[1.0; 9], 3_000.0).unwrap();
    let expected = [
        0.0,
        LOG_FLOOR_DB,
        LOG_FLOOR_DB,
        LOG_FLOOR_DB,
        LOG_FLOOR_DB,
        0.0,
        10.0 * 2f64.log10(),
        10.0 * 5f64.log10(),
    ];
    for (b, (a, e)) in bands.iter().zip(expected).enumerate() {
        assert!(close(*a, e, 1e-9), "band {b}: {a} vs {e}");
    }
}

/// `20 log10(m)`, floored at [`LOG_FLOOR_DB`].
#[test]
fn log_magnitude_is_decibels_with_a_floor() {
    assert!(close(log_magnitude(10.0).unwrap(), 20.0, 1e-12));
    assert!(close(log_magnitude(1.0).unwrap(), 0.0, 1e-12));
    assert_eq!(log_magnitude(0.0).unwrap(), LOG_FLOOR_DB);
}

/// Tolerance: 5 cents, one second of a 0.5-amplitude sine.
#[test]
fn f0_of_a_synthetic_sine_is_within_5_cents() {
    for frequency in [110.0, 261.63, 440.0, 1_000.0] {
        let estimate = estimate_f0(&sine(frequency, 0.5, 48_000), RATE).unwrap();
        let error = cents(estimate.f0_hz, frequency);
        assert!(
            error.abs() <= 5.0,
            "{frequency} Hz: {estimate:?}, {error} cents"
        );
        assert!(estimate.confidence >= 0.85 && estimate.confidence <= 1.0);
    }
}

/// Tolerance: 5 cents, one second of a 0.5-amplitude naive sawtooth, at 49 frequencies
/// log-spaced across the whole search range [`F0_MIN_HZ`] to [`F0_MAX_HZ`] and four musical ones.
#[test]
fn f0_of_a_synthetic_saw_is_within_5_cents() {
    let span = F0_MAX_HZ / F0_MIN_HZ;
    let swept = (0..=48).map(|i| F0_MIN_HZ * span.powf(f64::from(i) / 48.0));
    for frequency in swept.chain([55.0, 110.0, 220.0, 440.0]) {
        let estimate = estimate_f0(&saw(frequency, 0.5, 48_000), RATE).unwrap();
        let error = cents(estimate.f0_hz, frequency);
        assert!(
            error.abs() <= 5.0,
            "{frequency} Hz: {estimate:?}, {error} cents"
        );
    }
}

/// Noise has no pitch: refused as ambiguous with its confidence, below the threshold.
#[test]
fn f0_of_noise_is_refused_as_ambiguous() {
    let samples: Vec<f32> = noise(48_000, 3)
        .into_iter()
        .map(|x| (x * 0.5) as f32)
        .collect();
    match estimate_f0(&samples, RATE) {
        Err(AnalysisError::PitchAmbiguous { confidence }) => {
            assert!(confidence.is_finite() && confidence < 0.85, "{confidence}");
        }
        other => panic!("{other:?}"),
    }
}

/// Every function refuses NaN and both infinities with `NonFiniteSamples` at the bad index.
#[test]
fn nan_and_inf_input_is_refused_by_every_function() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let refused = |result: Result<(), AnalysisError>, what: &str| {
            assert_eq!(
                result,
                Err(AnalysisError::NonFiniteSamples { index: 1 }),
                "{what} with {bad}"
            );
        };
        let mut buffer = vec![Complex::ZERO, Complex::new(bad, 0.0)];
        refused(fft(&mut buffer), "fft");
        let mut buffer = vec![Complex::ZERO, Complex::new(0.0, bad)];
        refused(ifft(&mut buffer), "ifft");
        let samples = [0.0, bad as f32, 0.0, 0.0];
        refused(stft(&samples, 4, 2).map(drop), "stft");
        refused(rms(&samples).map(drop), "rms");
        refused(peak(&samples).map(drop), "peak");
        refused(estimate_f0(&samples, RATE).map(drop), "estimate_f0");
        let spectrum = [1.0, bad, 1.0];
        refused(spectral_centroid(&spectrum, 1.0).map(drop), "centroid");
        refused(spectral_rolloff(&spectrum, 1.0).map(drop), "rolloff");
        refused(spectral_flatness(&spectrum).map(drop), "flatness");
        refused(log_band_energies(&spectrum, 1.0).map(drop), "bands");
        refused(spectral_flux(&[1.0; 3], &spectrum).map(drop), "flux");
        refused(
            spectral_flux(&spectrum, &[1.0; 3]).map(drop),
            "flux previous",
        );
        assert_eq!(
            log_magnitude(bad),
            Err(AnalysisError::NonFiniteSamples { index: 0 })
        );
    }
}

/// A finite bin width is required too.
#[test]
fn a_non_finite_bin_width_is_refused() {
    assert!(spectral_centroid(&[1.0], f64::NAN).is_err());
    assert!(spectral_rolloff(&[1.0], f64::INFINITY).is_err());
    assert!(log_band_energies(&[1.0], f64::NAN).is_err());
}

/// A finite bin width whose product with a bin index overflows gives no infinite centroid or
/// rolloff: it is refused as a non-finite value at the bin width's position, index
/// `magnitudes.len()`.
#[test]
fn a_centroid_or_rolloff_that_overflows_is_refused() {
    let magnitudes = [0.0, 0.0, 1.0];
    let refused = Err(AnalysisError::NonFiniteSamples { index: 3 });
    assert_eq!(spectral_centroid(&magnitudes, f64::MAX), refused);
    assert_eq!(spectral_rolloff(&magnitudes, f64::MAX), refused);
    assert_eq!(spectral_centroid(&magnitudes, 1.0), Ok(2.0));
}

/// A pitch outside [`F0_MIN_HZ`, `F0_MAX_HZ`] is refused with the bound it lies beyond, never
/// reported an octave down or at the edge of the range.
#[test]
fn f0_outside_the_search_range_is_refused() {
    for frequency in [2_093.0, 3_000.0, 4_000.0] {
        assert_eq!(
            estimate_f0(&sine(frequency, 0.5, 48_000), RATE),
            Err(AnalysisError::PitchOutOfRange {
                bound_hz: F0_MAX_HZ
            }),
            "{frequency} Hz"
        );
    }
    for frequency in [48.0, 49.0] {
        assert_eq!(
            estimate_f0(&sine(frequency, 0.5, 48_000), RATE),
            Err(AnalysisError::PitchOutOfRange {
                bound_hz: F0_MIN_HZ
            }),
            "{frequency} Hz"
        );
    }
}

/// A clip that cannot hold one [`F0_WINDOW`] plus the shortest searched period and two lags is
/// refused as too short, not as an ambiguous pitch, naming 2074 samples at 48 kHz. That is enough
/// for a pitch near the top of the range; a pitch whose period does not fit in what the clip
/// leaves after the window is refused as too short as well, naming the length that searches the
/// whole lag range (3012 samples: the window, 963 lags, one more), which the clip does not have.
#[test]
fn f0_of_a_clip_too_short_for_the_window_is_refused_as_too_short() {
    let required = F0_WINDOW + 24 + 2;
    let full = F0_WINDOW + 963 + 1;
    for count in [0, 1, F0_WINDOW, required - 1] {
        assert_eq!(
            estimate_f0(&sine(1_000.0, 0.5, count), RATE),
            Err(AnalysisError::ClipTooShortForPitch {
                frame_count: count,
                required_frame_count: required
            }),
            "{count} samples"
        );
    }
    let estimate = estimate_f0(&sine(1_900.0, 0.5, required + 30), RATE).unwrap();
    assert!(cents(estimate.f0_hz, 1_900.0).abs() <= 5.0, "{estimate:?}");
    assert_eq!(
        estimate_f0(&sine(110.0, 0.5, required + 30), RATE),
        Err(AnalysisError::ClipTooShortForPitch {
            frame_count: required + 30,
            required_frame_count: full
        })
    );
    for frequency in [110.0, 50.0, 49.9] {
        let estimate = estimate_f0(&sine(frequency, 0.5, full), RATE).unwrap();
        assert!(
            cents(estimate.f0_hz, frequency).abs() <= 5.0,
            "{frequency} Hz: {estimate:?}"
        );
    }
}

/// An STFT that would hold more than [`MAX_STFT_VALUES`] magnitudes is refused before anything
/// is allocated: 1024 frames of an FFT of 65 536 (32 769 bins) are 33 555 456 values.
#[test]
fn an_stft_over_the_value_bound_is_refused() {
    assert_eq!(MAX_STFT_VALUES, 1 << 25);
    assert_eq!(
        stft(&[0.5; 1_024], 65_536, 1).map(drop),
        Err(AnalysisError::FrameCountOverBound {
            fft_length: 65_536,
            hop_length: 1,
            frame_count: 1_024
        })
    );
    assert_eq!(
        stft(&[0.5; 1_023], 65_536, 1023).unwrap().magnitudes.len(),
        1
    );
}

/// The radix-2 FFT refuses a length that is not a power of two (0 included) and one past
/// [`patchback_analysis::MAX_FFT_LENGTH`], and so does the STFT.
#[test]
fn fft_lengths_outside_the_radix_2_bound_are_refused() {
    for n in [0usize, 3, 500, 1_000] {
        let mut buffer = vec![Complex::ZERO; n];
        assert_eq!(
            fft(&mut buffer),
            Err(AnalysisError::FftLengthNotPowerOfTwo { fft_length: n })
        );
        assert_eq!(
            ifft(&mut buffer),
            Err(AnalysisError::FftLengthNotPowerOfTwo { fft_length: n })
        );
    }
    let over = patchback_analysis::MAX_FFT_LENGTH * 2;
    let mut buffer = vec![Complex::ZERO; over];
    assert_eq!(
        fft(&mut buffer),
        Err(AnalysisError::FftLengthOverBound { fft_length: over })
    );
    assert_eq!(
        stft(&[0.0; 4], over, 4),
        Err(AnalysisError::FftLengthOverBound { fft_length: over })
    );
    assert_eq!(
        stft(&[0.0; 4], 500, 4),
        Err(AnalysisError::FftLengthNotPowerOfTwo { fft_length: 500 })
    );
    assert_eq!(
        stft(&[0.0; 4], 4, 0),
        Err(AnalysisError::InvalidHopLength { hop_length: 0 })
    );
    assert_eq!(
        hann(over),
        Err(AnalysisError::FftLengthOverBound { fft_length: over })
    );
}

/// Frames start at `0, hop, 2 hop, ...` while the start is inside the clip; samples past the end
/// are zero. 10 samples, length 4, hop 4: frames at 0, 4 and 8.
#[test]
fn stft_frames_cover_the_clip_with_zero_padding() {
    let spectrum = stft(&[0.25; 10], 4, 4).unwrap();
    assert_eq!(spectrum.magnitudes.len(), 3);
    assert!(spectrum.magnitudes.iter().all(|m| m.len() == 3));
    assert_eq!(stft(&[], 4, 4).unwrap().magnitudes.len(), 0);
    for frame in &spectrum.magnitudes {
        assert!(frame.iter().all(|m| m.is_finite()));
    }
}

/// A naive pulse of duty cycle `duty`.
fn pulse(frequency_hz: f64, duty: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            let phase = (frequency_hz * i as f64 / f64::from(RATE)).fract();
            (if phase < duty { amplitude } else { -amplitude }) as f32
        })
        .collect()
}

/// A 25 % naive pulse at 401 log-spaced pitches over the whole range is reported within 5 cents
/// whenever it is reported, and it is reported everywhere.
#[test]
fn f0_of_a_quarter_pulse_on_a_dense_grid_is_within_5_cents() {
    let span = F0_MAX_HZ / F0_MIN_HZ;
    for i in 0..=400 {
        let frequency = F0_MIN_HZ * span.powf(f64::from(i) / 400.0);
        let estimate = estimate_f0(&pulse(frequency, 0.25, 0.5, 48_000), RATE)
            .unwrap_or_else(|e| panic!("{frequency} Hz: {e:?}"));
        assert!(
            cents(estimate.f0_hz, frequency).abs() <= 5.0,
            "{frequency} Hz: {estimate:?}"
        );
    }
}

/// A fundamental 12 to 19 dB under its second harmonic: the true period is reported, never the
/// octave above.
#[test]
fn f0_of_a_weak_fundamental_is_the_true_period() {
    for (fundamental, frequency) in [(0.2, 220.0), (0.1, 220.0), (0.09, 110.0), (0.2, 900.0)] {
        let samples: Vec<f32> = (0..48_000)
            .map(|i| {
                let t = i as f64 / f64::from(RATE);
                (fundamental * (2.0 * PI * frequency * t).sin()
                    + 0.8 * (4.0 * PI * frequency * t).sin()) as f32
            })
            .collect();
        let estimate = estimate_f0(&samples, RATE)
            .unwrap_or_else(|e| panic!("{fundamental} at {frequency} Hz: {e:?}"));
        assert!(
            cents(estimate.f0_hz, frequency).abs() <= 5.0,
            "{fundamental} at {frequency} Hz: {estimate:?}"
        );
    }
}

/// [`F0_RANGE_TOLERANCE_CENTS`] holds at both edges: a pitch inside the band is reported, one
/// past it refused.
#[test]
fn f0_inside_the_tolerance_band_is_reported_at_both_edges() {
    for frequency in [49.9, 49.97, 2_005.7] {
        let estimate = estimate_f0(&sine(frequency, 0.5, 48_000), RATE)
            .unwrap_or_else(|e| panic!("{frequency} Hz: {e:?}"));
        assert!(
            cents(estimate.f0_hz, frequency).abs() <= 5.0,
            "{estimate:?}"
        );
    }
    assert!(cents(49.8, F0_MIN_HZ) < -F0_RANGE_TOLERANCE_CENTS);
    assert_eq!(
        estimate_f0(&sine(49.8, 0.5, 48_000), RATE),
        Err(AnalysisError::PitchOutOfRange {
            bound_hz: F0_MIN_HZ
        })
    );
}

/// Each band is scaled by its own loudest bin, so a band far below another keeps its power: a
/// bin of magnitude 1 beside one of 1e300 is reported at 0 dB (normalising by the loudest bin of
/// the spectrum would underflow it to the floor); a band whose power is under [`LOG_FLOOR_DB`]
/// is floored.
#[test]
fn log_band_energies_keep_each_band_independent_of_the_others() {
    let bands = log_band_energies(&[1e300, 0.0, 0.0, 0.0, 0.0, 1.0], 4_000.0).unwrap();
    assert!((bands[0] - 6_000.0).abs() < 1e-9, "{bands:?}");
    assert!(bands[7].abs() < 1e-9, "{bands:?}");
    let bands = log_band_energies(&[1e300, 0.0, 0.0, 0.0, 0.0, 1e-7], 4_000.0).unwrap();
    assert_eq!(bands[7], LOG_FLOOR_DB);
}
