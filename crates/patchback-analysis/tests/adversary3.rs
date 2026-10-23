//! Adversarial cases, pass 3, against story:analysis-core (wave a3), after correction round 3:
//! the multiple-period refinement, the octave-doubling rule with its sub-harmonic power test, the
//! lag range down to 963, the too-short lengths, and the summed STFT bound.

use std::f64::consts::PI;

use patchback_analysis::{
    AnalysisConfig, AnalysisError, F0_MAX_HZ, F0_MIN_HZ, MAX_STFT_VALUES, SAMPLE_RATE_HZ,
    StftConfig, analyze, estimate_f0,
};

const RATE: f64 = 48_000.0;

fn cents(estimate: f64, truth: f64) -> f64 {
    1_200.0 * (estimate / truth).log2()
}

fn phase(frequency_hz: f64, i: usize) -> f64 {
    (frequency_hz * i as f64 / RATE).fract()
}

fn saw(frequency_hz: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (0.5 * (2.0 * phase(frequency_hz, i) - 1.0)) as f32)
        .collect()
}

fn square(frequency_hz: f64, count: usize) -> Vec<f32> {
    pulse(frequency_hz, 0.5, count)
}

fn quarter_pulse(frequency_hz: f64, count: usize) -> Vec<f32> {
    pulse(frequency_hz, 0.25, count)
}

fn tenth_pulse(frequency_hz: f64, count: usize) -> Vec<f32> {
    pulse(frequency_hz, 0.1, count)
}

fn pulse(frequency_hz: f64, duty: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            if phase(frequency_hz, i) < duty {
                0.5
            } else {
                -0.5
            }
        })
        .collect()
}

fn triangle(frequency_hz: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (0.5 * (4.0 * (phase(frequency_hz, i) - 0.5).abs() - 1.0)) as f32)
        .collect()
}

/// A sum of sines `(amplitude, frequency, phase)`.
fn partials(parts: &[(f64, f64, f64)], count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            let t = i as f64 / RATE;
            parts
                .iter()
                .map(|(a, f, p)| a * (2.0 * PI * f * t + p).sin())
                .sum::<f64>() as f32
        })
        .collect()
}

/// The pitches among `frequencies` whose estimate is reported more than `tolerance` cents from
/// the truth. A refusal is not counted.
fn reported_wrong(
    shape: fn(f64, usize) -> Vec<f32>,
    frequencies: impl Iterator<Item = f64>,
    tolerance: f64,
) -> Vec<String> {
    frequencies
        .filter_map(|f| match estimate_f0(&shape(f, 8_192), SAMPLE_RATE_HZ) {
            Ok(e) if cents(e.f0_hz, f).abs() > tolerance => Some(format!(
                "{f:.2} Hz (period {:.3}) reported as {:.2} Hz ({:+.0} cents, confidence {:.3})",
                RATE / f,
                e.f0_hz,
                cents(e.f0_hz, f),
                e.confidence
            )),
            _ => None,
        })
        .collect()
}

/// Every period of `k + 0.5` samples in the range: the samples repeat only every two periods,
/// which is the case the sub-harmonic power test exists for.
fn half_integer_pitches() -> impl Iterator<Item = f64> {
    (24..=960u32)
        .filter(|k| *k < 200 || k % 7 == 0)
        .map(|k| RATE / (f64::from(k) + 0.5))
        .filter(|f| *f <= F0_MAX_HZ * 1.002 && *f >= F0_MIN_HZ)
}

fn log_grid(count: u32) -> impl Iterator<Item = f64> {
    let span = F0_MAX_HZ / F0_MIN_HZ;
    (0..=count).map(move |i| F0_MIN_HZ * span.powf(f64::from(i) / f64::from(count)))
}

// --- Half-integer periods and the octave-doubling rule ------------------------------------------

/// `pitch.rs` step 6: a naive waveform whose period is a half-integer number of samples must not
/// double. Saw, square, 25 % pulse and triangle at every such period: never reported an octave
/// (or any amount over 5 cents) off.
#[test]
fn adversary3_half_integer_periods_are_not_reported_an_octave_down() {
    let mut wrong = Vec::new();
    for (name, shape) in [
        ("saw", saw as fn(f64, usize) -> Vec<f32>),
        ("square", square),
        ("25% pulse", quarter_pulse),
        ("triangle", triangle),
    ] {
        for line in reported_wrong(shape, half_integer_pitches(), 5.0) {
            wrong.push(format!("{name}: {line}"));
        }
    }
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// A naive 10 % pulse, a narrower width of the same oscillator, on the 401-pitch grid: never
/// reported more than 5 cents off.
#[test]
fn adversary3_narrow_pulse_on_a_dense_grid_is_not_reported_an_octave_off() {
    let wrong = reported_wrong(tenth_pulse, log_grid(400), 5.0);
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// A naive 10 % pulse at half-integer periods.
#[test]
fn adversary3_narrow_pulse_at_half_integer_periods_is_not_reported_an_octave_down() {
    let wrong = reported_wrong(tenth_pulse, half_integer_pitches(), 5.0);
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// A naive triangle on the 401-pitch grid. Green expected: its harmonics fall as 1/n^2.
#[test]
fn adversary3_triangle_on_a_dense_grid_is_within_5_cents() {
    let wrong = reported_wrong(triangle, log_grid(400), 5.0);
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

// --- Weak and missing fundamentals ----------------------------------------------------------------

/// Routing of pass 2, row 4: a weak fundamental is reported at its true period or refused as
/// ambiguous, never the octave above with a confidence over the threshold. The doc holds 12 to
/// 19 dB; this goes to 20 to 30 dB under the second harmonic, at five pitches.
#[test]
fn adversary3_very_weak_fundamental_is_never_reported_an_octave_up() {
    let mut wrong = Vec::new();
    for db in [20.0, 22.0, 25.0, 30.0] {
        let fundamental = 0.8 * 10f64.powf(-db / 20.0);
        for f in [55.0, 110.0, 220.0, 440.0, 900.0] {
            let samples = partials(&[(fundamental, f, 0.0), (0.8, 2.0 * f, 0.0)], 8_192);
            if let Ok(e) = estimate_f0(&samples, SAMPLE_RATE_HZ)
                && cents(e.f0_hz, f).abs() > 5.0
            {
                wrong.push(format!(
                    "{f} Hz, fundamental {db} dB under: {:.2} Hz, confidence {:.3}",
                    e.f0_hz, e.confidence
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// The claimed band, 12 to 19 dB, at the top of the range where the implementor estimates the
/// margin over aliasing at 2.6 dB; with the fundamental at several phases.
#[test]
fn adversary3_weak_fundamental_near_the_top_of_the_range_is_the_true_period() {
    let mut wrong = Vec::new();
    for db in [12.0, 15.0, 19.0] {
        let fundamental = 0.8 * 10f64.powf(-db / 20.0);
        for f in [700.0, 850.0, 990.0] {
            for p in [0.0, PI / 2.0, PI] {
                let samples = partials(&[(fundamental, f, p), (0.8, 2.0 * f, 0.0)], 8_192);
                match estimate_f0(&samples, SAMPLE_RATE_HZ) {
                    Ok(e) if cents(e.f0_hz, f).abs() <= 5.0 => {}
                    other => wrong.push(format!("{f} Hz, {db} dB, phase {p:.2}: {other:?}")),
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// Harmonics 2 to 6 of 200 Hz without the fundamental: the period is 1/200 s.
#[test]
fn adversary3_missing_fundamental_is_the_true_period() {
    let parts: Vec<(f64, f64, f64)> = (2..=6)
        .map(|n| (0.3 / f64::from(n), 200.0 * f64::from(n), 0.3 * f64::from(n)))
        .collect();
    let samples = partials(&parts, 8_192);
    match estimate_f0(&samples, SAMPLE_RATE_HZ) {
        Ok(e) => assert!(cents(e.f0_hz, 200.0).abs() <= 5.0, "{e:?}"),
        Err(AnalysisError::PitchAmbiguous { .. }) => {}
        Err(other) => panic!("{other:?}"),
    }
}

/// A tone with a sub-octave at -10 dB has the period of the sub-octave.
#[test]
fn adversary3_sub_octave_tone_is_the_lower_period() {
    for f in [220.0, 1_000.0, 1_900.0] {
        let samples = partials(&[(0.25, f / 2.0, 0.0), (0.8, f, 0.0)], 8_192);
        match estimate_f0(&samples, SAMPLE_RATE_HZ) {
            Ok(e) => assert!(cents(e.f0_hz, f / 2.0).abs() <= 5.0, "{f} Hz: {e:?}"),
            Err(AnalysisError::PitchAmbiguous { .. }) => {}
            Err(other) => panic!("{f} Hz: {other:?}"),
        }
    }
}

/// Two tones a fifth apart, 440 and 660 Hz: the period is 1/220 s; reporting either tone is a
/// guess.
#[test]
fn adversary3_two_tones_a_fifth_apart_are_the_common_period_or_ambiguous() {
    let samples = partials(&[(0.4, 440.0, 0.0), (0.4, 660.0, 1.0)], 8_192);
    match estimate_f0(&samples, SAMPLE_RATE_HZ) {
        Ok(e) => assert!(cents(e.f0_hz, 220.0).abs() <= 5.0, "{e:?}"),
        Err(AnalysisError::PitchAmbiguous { .. }) => {}
        Err(other) => panic!("{other:?}"),
    }
}

/// A saw with 30-cent vibrato at 6 Hz stays within the vibrato of its centre; the multiple
/// refinement must not lock onto a neighbouring cycle.
#[test]
fn adversary3_vibrato_saw_stays_within_its_vibrato() {
    for centre in [110.0, 440.0, 1_760.0] {
        let mut ph = 0.0f64;
        let samples: Vec<f32> = (0..48_000)
            .map(|i| {
                let t = i as f64 / RATE;
                let f = centre * 2f64.powf(30.0 / 1_200.0 * (2.0 * PI * 6.0 * t).sin());
                ph = (ph + f / RATE).fract();
                (0.5 * (2.0 * ph - 1.0)) as f32
            })
            .collect();
        match estimate_f0(&samples, SAMPLE_RATE_HZ) {
            Ok(e) => assert!(cents(e.f0_hz, centre).abs() <= 35.0, "{centre} Hz: {e:?}"),
            Err(AnalysisError::PitchAmbiguous { .. }) => {}
            Err(other) => panic!("{centre} Hz: {other:?}"),
        }
    }
}

// --- The lag range and the too-short lengths -----------------------------------------------------

/// `pitch.rs` step 1 and 4: 2073 samples are refused naming 2074; 2074 to 2900 samples of a
/// 49.9 Hz sine (period 961.9, past what they search) are refused naming 3012; 3012 are reported.
/// (3011 samples search 962 lags, which hold that period, so they may report it.)
#[test]
fn adversary3_too_short_lengths_are_2074_and_3012() {
    let sine = partials(&[(0.5, 49.9, 0.0)], 3_012);
    assert_eq!(
        estimate_f0(&sine[..2_073], SAMPLE_RATE_HZ),
        Err(AnalysisError::ClipTooShortForPitch {
            frame_count: 2_073,
            required_frame_count: 2_074
        })
    );
    for len in [2_074, 2_500, 2_900] {
        assert_eq!(
            estimate_f0(&sine[..len], SAMPLE_RATE_HZ),
            Err(AnalysisError::ClipTooShortForPitch {
                frame_count: len,
                required_frame_count: 3_012
            }),
            "{len}"
        );
    }
    let e = estimate_f0(&sine, SAMPLE_RATE_HZ).unwrap();
    assert!(cents(e.f0_hz, 49.9).abs() <= 5.0, "{e:?}");
}

// --- The summed STFT bound -----------------------------------------------------------------------

/// Two STFTs each under [`MAX_STFT_VALUES`] and over it together are refused, naming the second.
#[test]
fn adversary3_stft_bound_is_summed_over_the_configuration() {
    let samples = partials(&[(0.5, 440.0, 0.0)], 48_000);
    let one = StftConfig {
        fft_length: 8_192,
        hop_length: 6,
    };
    let each = 48_000usize.div_ceil(one.hop_length) * (one.fft_length / 2 + 1);
    assert!(
        each <= MAX_STFT_VALUES && 2 * each > MAX_STFT_VALUES,
        "{each}"
    );
    let config = AnalysisConfig {
        stfts: vec![one, one],
    };
    assert_eq!(
        analyze(&samples, SAMPLE_RATE_HZ, &config).map(|_| ()),
        Err(AnalysisError::FrameCountOverBound {
            fft_length: 8_192,
            hop_length: 6,
            frame_count: 8_000
        })
    );
}
