//! Adversary pass 1 on story:audio-comparison: hostile inputs, alignment boundaries, literal
//! component values, the cross-correlation against a direct sum, and the acceptance scenarios
//! read as the contract they claim to be.

use std::f64::consts::PI;
use std::fs;
use std::path::Path;

use patchback_analysis::{MAX_DURATION_FRAMES, cross_correlation};
use patchback_fit::{
    ALIGNMENT_TOLERANCE_FRAMES, Comparison, LOSS_WEIGHTS, LossComponents, Signal, compare_signals,
};

const RATE: u32 = 48_000;

fn signal(samples: &[f32]) -> Signal<'_> {
    Signal {
        samples,
        sample_rate_hz: RATE,
    }
}

fn noise(seed: u64, amplitude: f64, count: usize) -> Vec<f32> {
    let mut state = seed;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (amplitude * 2.0 * (((state >> 11) as f64 / (1u64 << 53) as f64) - 0.5)) as f32
        })
        .collect()
}

fn sine(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (amplitude * (2.0 * PI * frequency_hz * i as f64 / f64::from(RATE)).sin()) as f32)
        .collect()
}

fn parts(c: &LossComponents) -> [(&'static str, f64); 4] {
    [
        ("log_spectral", c.log_spectral),
        ("envelope", c.envelope),
        ("transient", c.transient),
        ("band_energy", c.band_energy),
    ]
}

fn assert_zero(name: &str, comparison: &Comparison) {
    for (component, value) in parts(&comparison.components) {
        assert_eq!(value, 0.0, "{name}: {component} = {value}");
    }
    assert_eq!(comparison.total, 0.0, "{name}: total");
}

fn assert_finite_unit(name: &str, comparison: &Comparison) {
    for (component, value) in parts(&comparison.components) {
        assert!(
            value.is_finite() && (0.0..=1.0).contains(&value),
            "{name}: {component} = {value}"
        );
    }
    assert!(
        comparison.total.is_finite() && (0.0..=1.0 + 1e-15).contains(&comparison.total),
        "{name}: total = {}",
        comparison.total
    );
}

fn hostile() -> Vec<(&'static str, Vec<f32>)> {
    let n = RATE as usize / 2;
    let square: Vec<f32> = (0..n)
        .map(|i| if (i / 240) % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let max_square: Vec<f32> = square.iter().map(|s| s * f32::MAX).collect();
    let alternating: Vec<f32> = (0..n)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let mut impulse_first = vec![0.0f32; n];
    impulse_first[0] = 1.0;
    let mut impulse_last = vec![0.0f32; n];
    impulse_last[n - 1] = 1.0;
    let denormal: Vec<f32> = (0..n)
        .map(|i| {
            if i % 3 == 0 {
                f32::from_bits(1)
            } else {
                -f32::from_bits(7)
            }
        })
        .collect();
    vec![
        ("all zero", vec![0.0; n]),
        ("dc +1", vec![1.0; n]),
        ("dc -1", vec![-1.0; n]),
        ("full-scale square", square),
        ("f32::MAX square", max_square),
        ("alternating +-1", alternating),
        ("impulse first", impulse_first),
        ("impulse last", impulse_last),
        ("denormal", denormal),
        ("min positive dc", vec![f32::MIN_POSITIVE; n]),
        ("one sample", vec![1.0]),
        ("two samples", vec![1.0, -1.0]),
        ("255 noise", noise(11, 1.0, 255)),
        ("511 noise", noise(12, 1.0, 511)),
        ("513 noise", noise(13, 1.0, 513)),
        ("2047 noise", noise(14, 1.0, 2_047)),
        ("8191 noise", noise(15, 1.0, 8_191)),
        ("8193 noise", noise(16, 1.0, 8_193)),
    ]
}

/// Identical clips give exactly 0 for every component and the total, at lag 0, for every hostile
/// input, including clips shorter than every FFT length.
#[test]
fn adversary_identical_hostile_clips_give_exactly_zero() {
    for (name, samples) in hostile() {
        let comparison = compare_signals(signal(&samples), signal(&samples)).unwrap();
        assert_eq!(comparison.lag_frames, 0, "{name}: lag");
        assert_zero(name, &comparison);
    }
}

/// Every pair of hostile inputs of equal length gives finite components in `[0, 1]` (cli.md:
/// "Every term is between 0 and 1").
#[test]
fn adversary_every_hostile_pair_is_finite_and_within_unit_range() {
    let inputs: Vec<_> = hostile()
        .into_iter()
        .filter(|(_, s)| s.len() == RATE as usize / 2)
        .collect();
    for (a_name, a) in &inputs {
        for (b_name, b) in &inputs {
            let comparison = compare_signals(signal(a), signal(b)).unwrap();
            assert_finite_unit(&format!("{a_name} vs {b_name}"), &comparison);
        }
    }
    // short against short, lengths differing by less than the tolerance
    let short: Vec<_> = hostile()
        .into_iter()
        .filter(|(_, s)| s.len() <= ALIGNMENT_TOLERANCE_FRAMES)
        .collect();
    for (a_name, a) in &short {
        for (b_name, b) in &short {
            let comparison = compare_signals(signal(a), signal(b)).unwrap();
            assert_finite_unit(&format!("{a_name} vs {b_name}"), &comparison);
        }
    }
}

/// Identical 60 s clips, at exactly the analysis bound: accepted, lag 0, total exactly 0. Long
/// clips push the four-fold prefix-sum smoothing of the correlation towards its rounding limit.
#[test]
fn adversary_identical_clips_at_the_duration_bound_give_zero() {
    let n = MAX_DURATION_FRAMES;
    let cases: [(&str, Vec<f32>); 2] = [("dc", vec![0.5; n]), ("noise", noise(21, 0.9, n))];
    for (name, samples) in cases {
        let comparison = compare_signals(signal(&samples), signal(&samples)).unwrap();
        assert_eq!(comparison.lag_frames, 0, "{name}: lag");
        assert_zero(name, &comparison);
    }
}

/// `count` samples: `onset` of silence, then a 1 kHz-period pulse train (one full-scale sample
/// every `period`), so the waveform is periodic well inside the alignment tolerance.
fn pulse_train(onset: usize, period: usize, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            if i >= onset && (i - onset).is_multiple_of(period) {
                1.0
            } else {
                0.0
            }
        })
        .collect()
}

/// A noise burst after `onset` samples of silence, decaying.
fn burst(onset: usize, count: usize) -> Vec<f32> {
    let raw = noise(5, 0.8, count);
    (0..count)
        .map(|i| {
            if i < onset {
                0.0
            } else {
                raw[i] * (-((i - onset) as f32) / 9_600.0).exp()
            }
        })
        .collect()
}

/// `reference` delayed by `delay` samples (advanced when negative): leading zeros added, or
/// leading samples dropped, so the candidate's length differs by `delay`.
fn delayed(reference: &[f32], delay: isize) -> Vec<f32> {
    if delay >= 0 {
        let mut out = vec![0.0; delay as usize];
        out.extend_from_slice(reference);
        out
    } else {
        reference[delay.unsigned_abs()..].to_vec()
    }
}

/// A delayed copy aligns back to its exact delay at the tolerance and one sample inside it, in
/// both directions; the aligned candidate is then the reference itself, so the loss is 0.
#[test]
fn adversary_shifts_at_the_tolerance_align_exactly_in_both_directions() {
    let t = ALIGNMENT_TOLERANCE_FRAMES as isize;
    let references: [(&str, Vec<f32>); 2] = [
        ("burst", burst(6_000, RATE as usize)),
        ("pulse train", pulse_train(6_000, 1_000, RATE as usize)),
    ];
    for (name, reference) in &references {
        for delay in [t, t - 1, 1, -1, -(t - 1), -t] {
            let candidate = delayed(reference, delay);
            let comparison = compare_signals(signal(reference), signal(&candidate)).unwrap();
            assert_eq!(comparison.lag_frames, delay, "{name}: delay {delay}");
            assert_zero(&format!("{name} delay {delay}"), &comparison);
        }
        for delay in [t + 1, -(t + 1)] {
            let candidate = delayed(reference, delay);
            let refused = compare_signals(signal(reference), signal(&candidate)).unwrap_err();
            assert_eq!(
                refused.domain_error(),
                "patchback.fit.LengthOutsideAlignment",
                "{name}: delay {delay}"
            );
        }
    }
}

/// A periodic signal (pulse period 300 to 2 300 samples, all inside the tolerance) delayed by a
/// fraction or a multiple of its period aligns to the true delay, not to a neighbouring period.
#[test]
fn adversary_periodic_signals_do_not_align_to_a_wrong_period() {
    for period in [300usize, 1_000, 2_300] {
        let reference = pulse_train(5_000, period, RATE as usize);
        for delay in [period as isize / 2, period as isize, 4_000] {
            let candidate = delayed(&reference, delay);
            let comparison = compare_signals(signal(&reference), signal(&candidate)).unwrap();
            assert_eq!(
                comparison.lag_frames, delay,
                "period {period}: delay {delay}"
            );
        }
    }
}

/// Silence against a sound and a sound against silence: every normalised difference is 1, the
/// band-energy distance is 1, and the loss is the sum of the weights.
#[test]
fn adversary_silence_against_sound_scores_one_per_component() {
    let sound = noise(31, 0.5, RATE as usize / 2);
    let silence = vec![0.0f32; RATE as usize / 2];
    for (name, reference, candidate) in [
        ("silence vs sound", &silence, &sound),
        ("sound vs silence", &sound, &silence),
    ] {
        let comparison = compare_signals(signal(reference), signal(candidate)).unwrap();
        for (component, value) in parts(&comparison.components) {
            assert_eq!(value, 1.0, "{name}: {component}");
        }
        assert!(
            (comparison.total - 1.0).abs() <= 1e-15,
            "{name}: total {}",
            comparison.total
        );
    }
}

/// A copy at twice the amplitude: the envelope component is exactly `|l - 2l| / (l + 2l) = 1/3`
/// (levels are linear in amplitude), the band-energy distributions are equal (0), and the lag 0.
#[test]
fn adversary_a_louder_copy_has_envelope_one_third_and_band_energy_zero() {
    let reference = burst(2_000, RATE as usize / 2);
    let louder: Vec<f32> = reference.iter().map(|s| s * 2.0).collect();
    for (name, a, b) in [
        ("x vs 2x", &reference, &louder),
        ("2x vs x", &louder, &reference),
    ] {
        let comparison = compare_signals(signal(a), signal(b)).unwrap();
        assert_eq!(comparison.lag_frames, 0, "{name}");
        let c = comparison.components;
        assert!(
            (c.envelope - 1.0 / 3.0).abs() < 1e-9,
            "{name}: envelope {}",
            c.envelope
        );
        assert!(c.band_energy.abs() < 1e-9, "{name}: band {}", c.band_energy);
        assert!(c.log_spectral > 0.0 && c.log_spectral < 1.0, "{name}");
        assert!(c.transient >= 0.0 && c.transient < 1.0, "{name}");
        let expected =
            0.45 * c.log_spectral + 0.25 * c.envelope + 0.15 * c.transient + 0.15 * c.band_energy;
        assert_eq!(comparison.total, expected, "{name}: total");
        assert_eq!(comparison.weights, LOSS_WEIGHTS);
    }
}

fn direct(a: &[f64], b: &[f64], max_lag: usize) -> Vec<f64> {
    let lag = max_lag as isize;
    (-lag..=lag)
        .map(|d| {
            a.iter()
                .enumerate()
                .filter_map(|(n, x)| {
                    let i = n as isize + d;
                    (i >= 0 && (i as usize) < b.len()).then(|| x * b[i as usize])
                })
                .sum()
        })
        .collect()
}

/// `cross_correlation` agrees with the O(n^2) sum on inputs that span several blocks, carry an
/// all-zero block in the middle (the skipped path), are shorter than the lag range, and at lag 0.
#[test]
fn adversary_cross_correlation_matches_the_direct_sum() {
    let to64 = |v: Vec<f32>| v.into_iter().map(f64::from).collect::<Vec<f64>>();
    let mut gapped = to64(noise(41, 1.0, 3_000));
    for value in &mut gapped[900..2_100] {
        *value = 0.0;
    }
    let cases: Vec<(&str, Vec<f64>, Vec<f64>, usize)> = vec![
        (
            "lag 0",
            to64(noise(42, 1.0, 700)),
            to64(noise(43, 1.0, 650)),
            0,
        ),
        (
            "multi-block",
            to64(noise(44, 1.0, 3_000)),
            to64(noise(45, 1.0, 2_900)),
            7,
        ),
        ("gapped", gapped.clone(), to64(noise(46, 1.0, 3_000)), 100),
        ("gapped candidate", to64(noise(47, 1.0, 2_000)), gapped, 300),
        ("short", vec![1.0, 2.0, 3.0], vec![4.0, 5.0], 9),
        (
            "candidate longer",
            to64(noise(48, 1.0, 50)),
            to64(noise(49, 1.0, 2_500)),
            600,
        ),
    ];
    for (name, a, b, max_lag) in cases {
        let fast = cross_correlation(&a, &b, max_lag).unwrap();
        let slow = direct(&a, &b, max_lag);
        assert_eq!(fast.len(), slow.len(), "{name}");
        let scale = a.iter().map(|x| x * x).sum::<f64>().sqrt()
            * b.iter().map(|x| x * x).sum::<f64>().sqrt();
        for (i, (f, s)) in fast.iter().zip(&slow).enumerate() {
            assert!(
                (f - s).abs() <= 1e-10 * scale.max(1.0),
                "{name}: lag {} fast {f} direct {s}",
                i as isize - max_lag as isize
            );
        }
    }
}

fn frame_counts(scenario: &str) -> Vec<usize> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec/scenarios")
        .join(scenario);
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|line| line.trim().strip_prefix("frame_count:"))
        .map(|value| value.trim().parse().unwrap())
        .collect()
}

/// The acceptance scenario for LengthOutsideAlignment, read as the contract it claims to be: the
/// two clips it imports must differ by more than the tolerance, or the implementation compares
/// them and the scenario's refusal never happens.
#[test]
fn adversary_length_outside_alignment_scenario_has_lengths_outside_the_tolerance() {
    let counts = frame_counts("audio-comparison-length-outside-alignment.yaml");
    assert_eq!(counts.len(), 2, "{counts:?}");
    let reference = vec![0.1f32; counts[0]];
    let candidate = vec![0.1f32; counts[1]];
    let outcome = compare_signals(signal(&reference), signal(&candidate));
    assert!(
        matches!(&outcome, Err(e) if e.domain_error() == "patchback.fit.LengthOutsideAlignment"),
        "the scenario imports {} and {} frames, a difference of {}; the implementation \
         returns {:?} instead of LengthOutsideAlignment",
        counts[0],
        counts[1],
        counts[0].abs_diff(counts[1]),
        outcome.map(|c| c.total)
    );
}

/// The records-components scenario expects `compared` for its two clips: the implementation
/// compares clips of those lengths.
#[test]
fn adversary_records_components_scenario_lengths_are_compared() {
    let counts = frame_counts("audio-comparison-records-components.yaml");
    assert_eq!(counts.len(), 2, "{counts:?}");
    let reference = sine(220.0, 0.5, counts[0]);
    let candidate = sine(330.0, 0.5, counts[1]);
    assert!(compare_signals(signal(&reference), signal(&candidate)).is_ok());
}
