//! Adversarial cases, pass 2, against story:analysis-core (wave a3), after correction round 2.
//! Each case names what it attacks.

use std::f64::consts::PI;

use patchback_analysis::{
    AnalysisConfig, AnalysisError, F0_MAX_HZ, F0_MIN_HZ, F0_RANGE_TOLERANCE_CENTS, MAX_STFT_VALUES,
    SAMPLE_RATE_HZ, StftConfig, analyze, estimate_f0, log_band_energies,
};

const RATE: f64 = 48_000.0;

fn sine(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (amplitude * (2.0 * PI * frequency_hz * i as f64 / RATE).sin()) as f32)
        .collect()
}

/// The naive saw of `tests/numeric.rs` and `tests/adversary.rs`, unchanged.
fn saw(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            let phase = (frequency_hz * i as f64 / RATE).fract();
            (amplitude * (2.0 * phase - 1.0)) as f32
        })
        .collect()
}

fn square(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            let phase = (frequency_hz * i as f64 / RATE).fract();
            (if phase < 0.5 { amplitude } else { -amplitude }) as f32
        })
        .collect()
}

fn cents(estimate: f64, truth: f64) -> f64 {
    1_200.0 * (estimate / truth).log2()
}

/// Every pitch of `count + 1` log-spaced from `F0_MIN_HZ` to `F0_MAX_HZ` whose estimate is
/// reported more than 5 cents off. A refusal is not counted: only a wrong reported pitch.
fn off_by_more_than_5_cents(shape: fn(f64, f64, usize) -> Vec<f32>, count: u32) -> Vec<String> {
    let span = F0_MAX_HZ / F0_MIN_HZ;
    (0..=count)
        .map(|i| F0_MIN_HZ * span.powf(f64::from(i) / f64::from(count)))
        .filter_map(
            |f| match estimate_f0(&shape(f, 0.5, 48_000), SAMPLE_RATE_HZ) {
                Ok(e) if cents(e.f0_hz, f).abs() > 5.0 => Some(format!(
                    "{f:.2} Hz reported as {:.2} Hz ({:+.1} cents)",
                    e.f0_hz,
                    cents(e.f0_hz, f)
                )),
                _ => None,
            },
        )
        .collect()
}

// --- The 5-cent claim ------------------------------------------------------------------------

/// `pitch.rs` claims "within 5 cents for one second of a sine or a naive sawtooth anywhere in the
/// range"; the suite holds it at 49 log-spaced pitches. 401 log-spaced pitches of the same saw
/// find pitches above 1.5 kHz reported up to 6.7 cents off with confidence above 0.85.
#[test]
fn adversary2_f0_of_saws_on_a_dense_grid_is_within_5_cents() {
    let wrong = off_by_more_than_5_cents(saw, 400);
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// The same for a naive square, a shape the suite never estimates.
#[test]
fn adversary2_f0_of_squares_on_a_dense_grid_is_within_5_cents() {
    let wrong = off_by_more_than_5_cents(square, 400);
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

// --- The range tolerance ---------------------------------------------------------------------

/// `F0_RANGE_TOLERANCE_CENTS` is "how far past F0_MIN_HZ or F0_MAX_HZ an estimate may lie and
/// still be reported ... so a pitch on the edge of the range is not refused for rounding". At the
/// top edge 2005.7 Hz (4.9 cents above) is reported; at the bottom edge 49.9 Hz (3.5 cents below)
/// and 49.97 Hz (1.0 cent below) are refused as `PitchOutOfRange`, because the lag search stops
/// at `ceil(rate / F0_MIN_HZ)` = 960 samples before the tolerance is applied.
#[test]
fn adversary2_f0_inside_the_tolerance_below_the_range_is_reported() {
    for f in [49.9, 49.97] {
        assert!(cents(f, F0_MIN_HZ).abs() < F0_RANGE_TOLERANCE_CENTS);
        let estimate = estimate_f0(&sine(f, 0.5, 48_000), SAMPLE_RATE_HZ).unwrap_or_else(|e| {
            panic!(
                "{f} Hz, {:.2} cents below 50 Hz: {e:?}",
                cents(F0_MIN_HZ, f)
            )
        });
        assert!(
            cents(estimate.f0_hz, f).abs() <= 5.0,
            "{f} Hz: {estimate:?}"
        );
    }
}

/// The top edge, for contrast: inside the tolerance is reported, past it refused. Green.
#[test]
fn adversary2_f0_at_the_top_edge_respects_the_tolerance() {
    let estimate = estimate_f0(&sine(2_005.7, 0.5, 48_000), SAMPLE_RATE_HZ).unwrap();
    assert!(cents(estimate.f0_hz, 2_005.7).abs() <= 5.0, "{estimate:?}");
    assert_eq!(
        estimate_f0(&sine(2_006.5, 0.5, 48_000), SAMPLE_RATE_HZ),
        Err(AnalysisError::PitchOutOfRange {
            bound_hz: F0_MAX_HZ
        })
    );
}

/// Far above the range, sine and saw up to near Nyquist, are refused, never folded into the
/// range. Green.
#[test]
fn adversary2_f0_far_above_the_range_is_refused() {
    for f in [3_000.0, 8_000.0, 12_000.0, 20_000.0, 23_900.0] {
        for samples in [sine(f, 0.5, 48_000), saw(f, 0.5, 48_000)] {
            assert_eq!(
                estimate_f0(&samples, SAMPLE_RATE_HZ),
                Err(AnalysisError::PitchOutOfRange {
                    bound_hz: F0_MAX_HZ
                }),
                "{f} Hz"
            );
        }
    }
}

// --- ClipTooShortForPitch --------------------------------------------------------------------

/// The refusal says "<n> samples do not hold the f0 window and the periods it searches (at least
/// <required> samples)", and the domain says the clip "is too short for the f0 estimator to
/// search its whole lag range". 2500 samples of a 100 Hz sine are refused with
/// `required_frame_count` 2074, fewer than the clip already has; a clip of the length the
/// refusal names is refused again.
#[test]
fn adversary2_too_short_refusal_names_a_length_that_suffices() {
    let count = 2_500;
    let Err(AnalysisError::ClipTooShortForPitch {
        frame_count,
        required_frame_count,
    }) = estimate_f0(&sine(100.0, 0.5, count), SAMPLE_RATE_HZ)
    else {
        panic!("2500 samples of 100 Hz are expected to be refused as too short");
    };
    assert_eq!(frame_count, count);
    assert!(
        required_frame_count > frame_count,
        "refused as too short with frame_count {frame_count} >= required_frame_count \
         {required_frame_count}: \"{}\"",
        AnalysisError::ClipTooShortForPitch {
            frame_count,
            required_frame_count
        }
    );
    let retry = estimate_f0(&sine(100.0, 0.5, required_frame_count), SAMPLE_RATE_HZ);
    assert!(
        !matches!(retry, Err(AnalysisError::ClipTooShortForPitch { .. })),
        "a clip of the advised {required_frame_count} samples: {retry:?}"
    );
}

/// The boundary itself: one sample under the minimum is too short, the minimum is enough for a
/// pitch at the top of the range. Green.
#[test]
fn adversary2_too_short_boundary_is_2074_samples() {
    assert_eq!(
        estimate_f0(&sine(2_000.0, 0.5, 2_073), SAMPLE_RATE_HZ),
        Err(AnalysisError::ClipTooShortForPitch {
            frame_count: 2_073,
            required_frame_count: 2_074
        })
    );
    let estimate = estimate_f0(&sine(2_000.0, 0.5, 2_074), SAMPLE_RATE_HZ).unwrap();
    assert!(cents(estimate.f0_hz, 2_000.0).abs() <= 5.0, "{estimate:?}");
    let estimate = estimate_f0(&sine(50.0, 0.5, 3_009), SAMPLE_RATE_HZ).unwrap();
    assert!(cents(estimate.f0_hz, 50.0).abs() <= 5.0, "{estimate:?}");
}

// --- Octave errors ---------------------------------------------------------------------------

/// DESIGN.md § 6.2: an uncertain pitch is surfaced, not guessed away. A 220 Hz tone whose
/// fundamental is 12 dB below its second harmonic (0.2 and 0.8) has period 1/220 s; it is
/// reported at 440 Hz, an octave up, with confidence 0.885, above the threshold.
#[test]
fn adversary2_strong_second_harmonic_is_not_reported_an_octave_up() {
    let samples: Vec<f32> = (0..48_000)
        .map(|i| {
            let t = i as f64 / RATE;
            (0.2 * (2.0 * PI * 220.0 * t).sin() + 0.8 * (2.0 * PI * 440.0 * t).sin()) as f32
        })
        .collect();
    match estimate_f0(&samples, SAMPLE_RATE_HZ) {
        Ok(estimate) => assert!(
            cents(estimate.f0_hz, 220.0).abs() <= 5.0,
            "220 Hz with a -12 dB fundamental reported as {estimate:?}"
        ),
        Err(AnalysisError::PitchAmbiguous { .. }) => {}
        Err(other) => panic!("{other:?}"),
    }
}

/// A DC offset and a very quiet level change nothing. Green.
#[test]
fn adversary2_f0_ignores_dc_offset_and_level() {
    for f in [55.0, 440.0] {
        let offset: Vec<f32> = saw(f, 0.5, 48_000).iter().map(|x| x + 0.4).collect();
        let quiet: Vec<f32> = saw(f, 0.5, 48_000).iter().map(|x| x * 1e-30).collect();
        for samples in [offset, quiet] {
            let estimate = estimate_f0(&samples, SAMPLE_RATE_HZ).unwrap();
            assert!(
                cents(estimate.f0_hz, f).abs() <= 5.0,
                "{f} Hz: {estimate:?}"
            );
        }
    }
}

// --- The magnitude bound ---------------------------------------------------------------------

/// `analyze` checks [`MAX_STFT_VALUES`] before it allocates: exactly at the bound (1023 frames of
/// 32 769 bins) passes the check and the clip is refused later for its pitch; one frame past it
/// is `FrameCountOverBound`. Green; kills a `>=` mutant of the check in `analyze`.
#[test]
fn adversary2_analyze_admits_exactly_the_magnitude_bound() {
    let config = AnalysisConfig {
        stfts: vec![StftConfig {
            fft_length: 65_536,
            hop_length: 1,
        }],
    };
    const { assert!(1_023 * 32_769 <= MAX_STFT_VALUES && 1_024 * 32_769 > MAX_STFT_VALUES) };
    assert_eq!(
        analyze(&sine(440.0, 0.5, 1_024), SAMPLE_RATE_HZ, &config).map(drop),
        Err(AnalysisError::FrameCountOverBound {
            fft_length: 65_536,
            hop_length: 1,
            frame_count: 1_024
        })
    );
    assert_eq!(
        analyze(&sine(440.0, 0.5, 1_023), SAMPLE_RATE_HZ, &config).map(drop),
        Err(AnalysisError::ClipTooShortForPitch {
            frame_count: 1_023,
            required_frame_count: 2_074
        })
    );
}

// --- Band energies ---------------------------------------------------------------------------

/// `log_band_energies` reports "the power sum m_k^2 of the bins in each band ... in dB, floored
/// at LOG_FLOOR_DB". Dividing by the largest magnitude first underflows the square of a bin
/// 200 orders of magnitude below it: a band of magnitude 1e100 (2000 dB) is reported at the
/// floor, -120 dB. `analyze` never produces such a spectrum.
#[test]
fn adversary2_band_energy_survives_a_wide_dynamic_range() {
    let bands = log_band_energies(&[1e300, 0.0, 0.0, 0.0, 0.0, 1e100], 4_000.0).unwrap();
    assert_eq!(bands[0], 6_000.0);
    assert!(
        (bands[7] - 2_000.0).abs() < 1e-9,
        "band 7 holds one bin of magnitude 1e100: {bands:?}"
    );
}
