//! The numeric obligations of story:audio-comparison (DESIGN.md § 9.1, § 2 invariant 4), one
//! named test each: identical clips give 0 for every component; every component is finite on
//! noise, silence-adjacent and clipped inputs; the weights are the § 9.1 values and the total is
//! their weighted sum; alignment brings a shifted copy back to a lower loss than the unaligned
//! comparison; the loss grows monotonically with the detuning of a sine. The refusals of
//! `CompareAudio` (`spec/domains/fit.yaml`) are held here too.

use std::f64::consts::PI;

use patchback_audio::AudioClip;
use patchback_fit::{
    ALIGNMENT_TOLERANCE_FRAMES, CompareError, LOSS_WEIGHTS, LossComponents, Signal,
    WEIGHT_BAND_ENERGY, WEIGHT_ENVELOPE, WEIGHT_LOG_SPECTRAL, WEIGHT_TRANSIENT, compare,
    compare_signals, loss_components,
};

const RATE: u32 = 48_000;

fn sine(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (amplitude * (2.0 * PI * frequency_hz * i as f64 / f64::from(RATE)).sin()) as f32)
        .collect()
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

/// A plucked note: `onset` samples of silence, then a 330 Hz tone with a fast attack and an
/// exponential decay, `count` samples in all.
fn note(onset: usize, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            if i < onset {
                return 0.0;
            }
            let t = (i - onset) as f64 / f64::from(RATE);
            let attack = (t / 0.005).min(1.0);
            let decay = (-t / 0.25).exp();
            let tone = (2.0 * PI * 330.0 * t).sin() + 0.3 * (2.0 * PI * 990.0 * t).sin();
            (0.6 * attack * decay * tone) as f32
        })
        .collect()
}

fn signal(samples: &[f32]) -> Signal<'_> {
    Signal {
        samples,
        sample_rate_hz: RATE,
    }
}

fn each(components: &LossComponents) -> [(&'static str, f64); 4] {
    [
        ("log_spectral", components.log_spectral),
        ("envelope", components.envelope),
        ("transient", components.transient),
        ("band_energy", components.band_energy),
    ]
}

#[test]
fn identical_clips_give_zero_for_every_component() {
    let cases: [(&str, Vec<f32>); 5] = [
        ("sine", sine(440.0, 0.5, RATE as usize)),
        ("noise", noise(7, 0.8, RATE as usize)),
        ("note", note(4_000, RATE as usize)),
        ("silence", vec![0.0; 24_000]),
        ("one sample", vec![0.25]),
    ];
    for (name, samples) in cases {
        let comparison = compare_signals(signal(&samples), signal(&samples)).unwrap();
        for (component, value) in each(&comparison.components) {
            assert_eq!(value, 0.0, "{name}: {component}");
        }
        assert_eq!(comparison.total, 0.0, "{name}");
        assert_eq!(comparison.lag_frames, 0, "{name}");
    }
}

#[test]
fn identical_clips_compare_to_zero_through_audio_clips() {
    let clip = AudioClip::from_mono(note(2_000, 30_000)).unwrap();
    let comparison = compare(&clip, &clip).unwrap();
    assert_eq!(comparison.total, 0.0);
}

#[test]
fn every_component_is_finite_on_noise_silence_adjacent_and_clipped_inputs() {
    let n = RATE as usize / 2;
    let clipped: Vec<f32> = sine(220.0, 4.0, n)
        .into_iter()
        .map(|s| s.clamp(-1.0, 1.0))
        .collect();
    let extreme: Vec<f32> = sine(220.0, 1.0, n)
        .into_iter()
        .map(|s| if s >= 0.0 { f32::MAX } else { -f32::MAX })
        .collect();
    let subnormal: Vec<f32> = noise(3, 1e-40, n);
    let near_silent: Vec<f32> = noise(4, 1e-30, n);
    let mut one_click = vec![0.0f32; n];
    one_click[n / 2] = 1.0;
    let inputs: [(&str, Vec<f32>); 8] = [
        ("noise", noise(1, 1.0, n)),
        ("other noise", noise(2, 0.1, n)),
        ("silence", vec![0.0; n]),
        ("subnormal", subnormal),
        ("near silent", near_silent),
        ("one click", one_click),
        ("clipped", clipped),
        ("f32::MAX square", extreme),
    ];
    for (reference_name, reference) in &inputs {
        for (candidate_name, candidate) in &inputs {
            let comparison = compare_signals(signal(reference), signal(candidate)).unwrap();
            for (component, value) in each(&comparison.components) {
                assert!(
                    value.is_finite() && value >= 0.0,
                    "{reference_name} vs {candidate_name}: {component} = {value}"
                );
            }
            assert!(
                comparison.total.is_finite() && comparison.total >= 0.0,
                "{reference_name} vs {candidate_name}: total = {}",
                comparison.total
            );
            let unaligned = loss_components(reference, candidate).unwrap();
            for (component, value) in each(&unaligned) {
                assert!(
                    value.is_finite() && value >= 0.0,
                    "{reference_name} vs {candidate_name} unaligned: {component} = {value}"
                );
            }
        }
    }
}

#[test]
fn the_weights_are_the_design_values_and_sum_to_one() {
    assert_eq!(WEIGHT_LOG_SPECTRAL, 0.45);
    assert_eq!(WEIGHT_ENVELOPE, 0.25);
    assert_eq!(WEIGHT_TRANSIENT, 0.15);
    assert_eq!(WEIGHT_BAND_ENERGY, 0.15);
    assert_eq!(LOSS_WEIGHTS.log_spectral, WEIGHT_LOG_SPECTRAL);
    assert_eq!(LOSS_WEIGHTS.envelope, WEIGHT_ENVELOPE);
    assert_eq!(LOSS_WEIGHTS.transient, WEIGHT_TRANSIENT);
    assert_eq!(LOSS_WEIGHTS.band_energy, WEIGHT_BAND_ENERGY);
    assert!(
        (LOSS_WEIGHTS.sum() - 1.0).abs() < 1e-15,
        "{}",
        LOSS_WEIGHTS.sum()
    );
}

#[test]
fn the_total_is_the_weighted_sum_of_the_components() {
    let reference = note(1_000, RATE as usize);
    let candidate = sine(300.0, 0.3, RATE as usize);
    let comparison = compare_signals(signal(&reference), signal(&candidate)).unwrap();
    let c = comparison.components;
    let expected =
        0.45 * c.log_spectral + 0.25 * c.envelope + 0.15 * c.transient + 0.15 * c.band_energy;
    assert_eq!(comparison.weights, LOSS_WEIGHTS);
    assert!(
        (comparison.total - expected).abs() <= 1e-12 * expected.max(1.0),
        "{} != {expected}",
        comparison.total
    );
    for (component, value) in each(&c) {
        assert!(value > 0.0, "{component} of different clips is {value}");
    }
}

#[test]
fn a_shifted_copy_within_the_tolerance_aligns_back_to_a_lower_loss() {
    let count = RATE as usize;
    let reference = note(4_000, count);
    for shift in [1_000isize, -1_000, 2_400, -3_000] {
        // the candidate is the reference delayed by `shift` samples (advanced when negative)
        let candidate: Vec<f32> = (0..count as isize)
            .map(|i| {
                let source = i - shift;
                if (0..count as isize).contains(&source) {
                    reference[source as usize]
                } else {
                    0.0
                }
            })
            .collect();
        let aligned = compare_signals(signal(&reference), signal(&candidate)).unwrap();
        let unaligned = loss_components(&reference, &candidate).unwrap();
        let unaligned_total = LOSS_WEIGHTS.total(&unaligned);
        assert_eq!(aligned.lag_frames, shift, "shift {shift}");
        assert!(
            aligned.total < unaligned_total,
            "shift {shift}: aligned {} is not below unaligned {unaligned_total}",
            aligned.total
        );
        assert!(
            aligned.total < 0.5 * unaligned_total,
            "shift {shift}: aligned {} vs unaligned {unaligned_total}",
            aligned.total
        );
    }
}

/// A 1 s sine of amplitude 0.5 with 10 ms raised-cosine fades at both ends.
///
/// The fades matter: a bare sine stops wherever its phase is at the last sample, and the splatter
/// of that cut decides the loss. Measured on bare 440 Hz sines, 1 cent scored 0.0045 and 2 cents
/// 0.0004, because the first ends a quarter cycle off zero and the second almost on it.
fn faded_sine(frequency_hz: f64) -> Vec<f32> {
    let count = RATE as usize;
    let fade = 480.0;
    sine(frequency_hz, 0.5, count)
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let edge = i.min(count - 1 - i) as f64;
            let gain = if edge < fade {
                0.5 - 0.5 * (PI * edge / fade).cos()
            } else {
                1.0
            };
            (f64::from(s) * gain) as f32
        })
        .collect()
}

/// The range: sines of 110 to 1760 Hz, each against itself detuned upwards by 0, 3.125, 6.25,
/// 12.5, 25, 50, 100, 200, 400 and 800 cents, at the same amplitude. The loss is 0 at 0 cents
/// and strictly increasing over those steps. Between finer steps it is not held: the
/// log-spectral term follows the Hann window's leakage as well as the pitch
/// (`patchback_fit::EPSILON`). The narrowest margins measured are 1.5 % (261.63 Hz, 12.5 to 25
/// cents) and 4 % (220 Hz, 3.125 to 6.25 cents).
/// How far two steady tones of different pitch may be aligned apart: 1 ms at 48 kHz. Their
/// energy envelopes differ only in a ripple the smoothing leaves at a fraction of a percent, and
/// near lag 0 their correlation is flat, so that ripple can move the maximum by a few samples
/// (measured: at most 2, at 110 Hz).
const MAX_TONE_LAG_FRAMES: usize = 48;

#[test]
fn the_loss_grows_monotonically_with_detuning_from_0_to_800_cents() {
    for base in [110.0, 220.0, 261.63, 330.0, 440.0, 660.0, 1_000.0, 1_760.0] {
        let reference = faded_sine(base);
        let mut previous = -1.0;
        for cents in [
            0.0, 3.125, 6.25, 12.5, 25.0, 50.0, 100.0, 200.0, 400.0, 800.0,
        ] {
            let candidate = faded_sine(base * (cents / 1200.0f64).exp2());
            let comparison = compare_signals(signal(&reference), signal(&candidate)).unwrap();
            let loss = comparison.total;
            if cents == 0.0 {
                assert_eq!(loss, 0.0, "{base} Hz");
            }
            assert!(
                comparison.lag_frames.unsigned_abs() <= MAX_TONE_LAG_FRAMES,
                "{base} Hz, {cents} cents: aligned at {} samples",
                comparison.lag_frames
            );
            assert!(
                loss > previous,
                "{base} Hz, {cents} cents: loss {loss} is not above {previous}"
            );
            previous = loss;
        }
    }
}

#[test]
fn clips_at_different_sample_rates_are_refused() {
    let samples = sine(440.0, 0.5, 4_800);
    let refused = compare_signals(
        signal(&samples),
        Signal {
            samples: &samples,
            sample_rate_hz: 44_100,
        },
    )
    .unwrap_err();
    assert_eq!(
        refused,
        CompareError::SampleRateMismatch {
            reference_hz: 48_000,
            candidate_hz: 44_100,
        }
    );
    assert_eq!(refused.domain_error(), "patchback.fit.SampleRateMismatch");
}

#[test]
fn two_clips_at_one_unsupported_rate_are_refused_as_analysis_does() {
    let samples = sine(440.0, 0.5, 4_800);
    let at = |rate| Signal {
        samples: &samples,
        sample_rate_hz: rate,
    };
    let refused = compare_signals(at(44_100), at(44_100)).unwrap_err();
    assert_eq!(
        refused.domain_error(),
        "patchback.analysis.UnsupportedSampleRate"
    );
}

#[test]
fn lengths_beyond_the_alignment_tolerance_are_refused() {
    let reference = note(1_000, RATE as usize);
    let inside = note(1_000, RATE as usize + ALIGNMENT_TOLERANCE_FRAMES);
    assert!(compare_signals(signal(&reference), signal(&inside)).is_ok());
    let inside_short = note(1_000, RATE as usize - ALIGNMENT_TOLERANCE_FRAMES);
    assert!(compare_signals(signal(&reference), signal(&inside_short)).is_ok());
    for candidate_frames in [
        RATE as usize + ALIGNMENT_TOLERANCE_FRAMES + 1,
        RATE as usize - ALIGNMENT_TOLERANCE_FRAMES - 1,
    ] {
        let candidate = note(1_000, candidate_frames);
        let refused = compare_signals(signal(&reference), signal(&candidate)).unwrap_err();
        assert_eq!(
            refused,
            CompareError::LengthOutsideAlignment {
                reference_frames: RATE as usize,
                candidate_frames,
            }
        );
        assert_eq!(
            refused.domain_error(),
            "patchback.fit.LengthOutsideAlignment"
        );
    }
}

#[test]
fn the_rate_is_checked_before_the_length() {
    let reference = vec![0.1f32; 100];
    let candidate = vec![0.1f32; 100 + ALIGNMENT_TOLERANCE_FRAMES + 1];
    let refused = compare_signals(
        signal(&reference),
        Signal {
            samples: &candidate,
            sample_rate_hz: 96_000,
        },
    )
    .unwrap_err();
    assert_eq!(refused.domain_error(), "patchback.fit.SampleRateMismatch");
}

#[test]
fn empty_and_non_finite_input_is_refused() {
    let some = [0.1f32; 10];
    for (reference, candidate, expected) in [
        (&[][..], &some[..], "patchback.analysis.EmptyAudio"),
        (&some[..], &[][..], "patchback.analysis.EmptyAudio"),
        (
            &[f32::NAN][..],
            &[0.0][..],
            "patchback.analysis.NonFiniteSamples",
        ),
        (
            &[0.0][..],
            &[f32::INFINITY][..],
            "patchback.analysis.NonFiniteSamples",
        ),
    ] {
        let refused = compare_signals(signal(reference), signal(candidate)).unwrap_err();
        assert_eq!(refused.domain_error(), expected, "{refused}");
        let refused = loss_components(reference, candidate).unwrap_err();
        assert_eq!(refused.domain_error(), expected, "{refused}");
    }
}

#[test]
fn clips_past_the_analysis_duration_bound_are_refused() {
    let long = vec![0.0f32; patchback_analysis::MAX_DURATION_FRAMES + 1];
    let refused = compare_signals(signal(&long), signal(&long)).unwrap_err();
    assert_eq!(
        refused.domain_error(),
        "patchback.analysis.DurationOverBound"
    );
}

#[test]
fn every_refusal_message_is_one_line_naming_its_numbers() {
    let refusals = [
        CompareError::SampleRateMismatch {
            reference_hz: 48_000,
            candidate_hz: 44_100,
        },
        CompareError::LengthOutsideAlignment {
            reference_frames: 48_000,
            candidate_frames: 60_000,
        },
    ];
    let messages: Vec<String> = refusals.iter().map(ToString::to_string).collect();
    assert!(messages[0].contains("48000") && messages[0].contains("44100"));
    assert!(messages[1].contains("48000") && messages[1].contains("60000"));
    assert!(messages[1].contains(&ALIGNMENT_TOLERANCE_FRAMES.to_string()));
    for message in messages {
        assert_eq!(message.lines().count(), 1, "{message}");
    }
}
