//! The DSP units of the fixed `subtractive-mono/1` voice (DESIGN.md § 5.3, § 12.1): golden
//! waveforms of the oscillator, the envelope and the filter, their limits, and finite output for
//! bounded input and controls.

use std::f64::consts::PI;

use patchback_core::parameter::Topology;
use patchback_dsp::{
    Adsr, AdsrStage, Drive, LIMITER_THRESHOLD, Limiter, MAX_CUTOFF_HZ, MAX_DRIVE_DB,
    MAX_OSCILLATOR_HZ, MAX_RESONANCE_Q, MIN_CUTOFF_HZ, MIN_DRIVE_DB, MIN_RESONANCE_Q, OutputStage,
    PolyBlepSaw, RenderError, SAMPLE_RATE_HZ, Smoother, SvfLowpass, midi_to_hz,
};

const FS: f64 = SAMPLE_RATE_HZ as f64;

/// A deterministic sequence in `[-1, 1]`.
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

// Oscillator.

#[test]
fn the_saw_at_750_hz_matches_its_golden_waveform() {
    // 750 / 48000 = 1/64 exactly, so the phase is k/64 exactly and the polyBLEP correction falls
    // on the wrap sample alone: there it gives the midpoint 0, elsewhere the naive ramp 2t - 1.
    let mut saw = PolyBlepSaw::new();
    saw.set_frequency(750.0);
    for k in 0..640u32 {
        let t = f64::from(k % 64) / 64.0;
        let expected = if k % 64 == 0 { 0.0 } else { 2.0 * t - 1.0 };
        assert_eq!(saw.next_sample(), expected, "sample {k}");
    }
}

#[test]
fn the_saw_phase_is_continuous_across_a_frequency_change() {
    let mut saw = PolyBlepSaw::new();
    saw.set_frequency(110.0);
    for _ in 0..1000 {
        saw.next_sample();
    }
    let before = saw.phase();
    saw.set_frequency(midi_to_hz(81));
    assert_eq!(
        saw.phase(),
        before,
        "a frequency change does not move the phase"
    );
    let increment = midi_to_hz(81) / FS;
    let mut phase = before;
    for _ in 0..1000 {
        saw.next_sample();
        phase += increment;
        if phase >= 1.0 {
            phase -= 1.0;
        }
        assert!((saw.phase() - phase).abs() < 1e-9);
    }
}

#[test]
fn midi_notes_map_to_equal_temperament_at_a440() {
    assert_eq!(midi_to_hz(69), 440.0);
    assert!((midi_to_hz(45) - 110.0).abs() < 1e-12);
    assert!((midi_to_hz(0) - 8.175_798_915_643_707).abs() < 1e-9);
    assert!((midi_to_hz(127) - 12_543.853_951_415_975).abs() < 1e-6);
}

#[test]
fn the_saw_frequency_is_clamped_to_its_limits_and_stays_finite() {
    let mut saw = PolyBlepSaw::new();
    for (requested, applied) in [
        (1e12, MAX_OSCILLATOR_HZ),
        (f64::INFINITY, 0.0),
        (f64::NAN, 0.0),
        (-5.0, 0.0),
        (0.0, 0.0),
        (midi_to_hz(127), midi_to_hz(127)),
    ] {
        saw.set_frequency(requested);
        assert_eq!(saw.frequency(), applied, "{requested}");
        for _ in 0..4800 {
            let y = saw.next_sample();
            assert!(y.is_finite() && y.abs() <= 1.0 + 1e-12, "{requested}: {y}");
        }
    }
    const { assert!(MAX_OSCILLATOR_HZ < FS / 2.0) };
    assert!(midi_to_hz(127) <= MAX_OSCILLATOR_HZ);
}

/// The energy of `signal` at each DFT bin `0..=n/2`, by a direct DFT.
fn power_spectrum(signal: &[f64]) -> Vec<f64> {
    let n = signal.len();
    let twiddle: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let angle = -2.0 * PI * i as f64 / n as f64;
            (angle.cos(), angle.sin())
        })
        .collect();
    (0..=n / 2)
        .map(|k| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, x) in signal.iter().enumerate() {
                let (c, s) = twiddle[(k * i) % n];
                re += x * c;
                im += x * s;
            }
            re * re + im * im
        })
        .collect()
}

/// Alias energy over harmonic energy, in dB, of a saw at `f0` sampled `n` times at 48 kHz, where
/// the bins are 48000 / n Hz wide and every harmonic and alias lands on one.
fn alias_ratio_db(signal: &[f64], f0: f64) -> f64 {
    let n = signal.len();
    let bin_hz = FS / n as f64;
    let spectrum = power_spectrum(signal);
    let harmonics: Vec<usize> = (1..)
        .map(|h| f0 * f64::from(h))
        .take_while(|f| *f < FS / 2.0)
        .map(|f| (f / bin_hz).round() as usize)
        .collect();
    let harmonic: f64 = harmonics.iter().map(|k| spectrum[*k]).sum();
    let alias: f64 = spectrum
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(k, _)| !harmonics.contains(k))
        .map(|(_, p)| p)
        .sum();
    10.0 * (alias / harmonic).log10()
}

#[test]
fn polyblep_reduces_high_note_aliasing_against_the_naive_saw() {
    // 7010 Hz: 701 whole periods in 4800 samples, so harmonics and aliases fall on 10 Hz bins.
    let (f0, n) = (7010.0, 4800);
    let mut saw = PolyBlepSaw::new();
    saw.set_frequency(f0);
    let blep: Vec<f64> = (0..n).map(|_| saw.next_sample()).collect();
    let increment = f0 / FS;
    let mut phase = 0.0;
    let naive: Vec<f64> = (0..n)
        .map(|_| {
            let y = 2.0 * phase - 1.0;
            phase += increment;
            if phase >= 1.0 {
                phase -= 1.0;
            }
            y
        })
        .collect();
    let blep_db = alias_ratio_db(&blep, f0);
    let naive_db = alias_ratio_db(&naive, f0);
    println!("alias/harmonic energy at {f0} Hz: polyBLEP {blep_db:.2} dB, naive {naive_db:.2} dB");
    // Measured on this build: polyBLEP -22.91 dB, naive -6.81 dB, 16.1 dB apart.
    assert!(
        naive_db - blep_db >= 12.0,
        "polyBLEP {blep_db} dB against naive {naive_db} dB"
    );
}

// Drive.

#[test]
fn the_drive_matches_its_golden_curve() {
    // tanh(g x) / tanh(g), g = 10^(dB / 20).
    let unity = Drive::new(0.0);
    assert!((unity.process(0.5) - 0.462_117_157_260_009_8 / 0.761_594_155_955_764_9).abs() < 1e-12);
    let hot = Drive::new(20.0);
    assert!((hot.process(0.05) - 0.462_117_157_260_009_8 / 0.999_999_995_877_692_8).abs() < 1e-9);
    for drive in [0.0, 6.0, 24.0] {
        let stage = Drive::new(drive);
        assert_eq!(stage.process(0.0), 0.0);
        assert!((stage.process(1.0) - 1.0).abs() < 1e-12, "{drive}");
        assert!((stage.process(-1.0) + 1.0).abs() < 1e-12, "{drive}");
    }
}

#[test]
fn the_drive_is_bounded_monotonic_and_finite() {
    for requested in [MIN_DRIVE_DB, 6.0, MAX_DRIVE_DB, 1e9, -1e9, f64::NAN] {
        let stage = Drive::new(requested);
        let mut previous = f64::NEG_INFINITY;
        for i in -2000..=2000 {
            let x = f64::from(i) / 1000.0;
            let y = stage.process(x);
            assert!(y.is_finite() && y >= previous, "{requested} {x}");
            if x.abs() <= 1.0 {
                assert!(y.abs() <= 1.0 + 1e-12, "{requested} {x} {y}");
            }
            previous = y;
        }
        for x in [
            f64::MAX,
            f64::MIN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            let y = stage.process(x);
            assert!(
                y.is_finite() && y.abs() <= 1.0 / 1e0_f64.tanh() + 1e-12,
                "{x}: {y}"
            );
        }
    }
}

// Filter.

#[test]
fn the_lowpass_impulse_response_matches_its_golden_first_samples() {
    let (cutoff, q) = (1200.0, 0.7);
    let mut filter = SvfLowpass::new(cutoff, q);
    let g = (PI * cutoff / FS).tan();
    let k = 1.0 / q;
    let a1 = 1.0 / (1.0 + g * (g + k));
    // The coefficients in the published order (Cytomic, "SvfLinearTrapOptimised2"): a3 = g a2.
    let a2 = g * a1;
    let a3 = g * a2;
    // The TPT state-variable recursion written out for an impulse.
    let (mut ic1, mut ic2) = (0.0_f64, 0.0_f64);
    for n in 0..64 {
        let x = if n == 0 { 1.0 } else { 0.0 };
        let v3 = x - ic2;
        let v1 = a1 * ic1 + a2 * v3;
        let v2 = ic2 + a2 * ic1 + a3 * v3;
        ic1 = 2.0 * v1 - ic1;
        ic2 = 2.0 * v2 - ic2;
        assert_eq!(filter.process(x), v2, "sample {n}");
    }
    // The first sample in closed form, to rounding.
    let first = SvfLowpass::new(cutoff, q).process(1.0);
    let closed = g * g / (1.0 + g * g + g / q);
    assert!(
        (first - closed).abs() <= 4.0 * f64::EPSILON * closed,
        "{first} {closed}"
    );
}

#[test]
fn the_lowpass_passes_dc_and_peaks_at_q_on_its_cutoff() {
    let mut filter = SvfLowpass::new(1200.0, 0.7);
    let mut y = 0.0;
    for _ in 0..48_000 {
        y = filter.process(1.0);
    }
    assert!((y - 1.0).abs() < 1e-9, "DC gain {y}");

    for (cutoff, q) in [(1200.0, 0.7), (1000.0, 4.0), (500.0, MAX_RESONANCE_Q)] {
        let amplitude = |frequency: f64| {
            let mut filter = SvfLowpass::new(cutoff, q);
            let mut peak: f64 = 0.0;
            for n in 0..96_000 {
                let y = filter.process((2.0 * PI * frequency * f64::from(n) / FS).sin());
                if n >= 48_000 {
                    peak = peak.max(y.abs());
                }
            }
            peak
        };
        let at_cutoff = amplitude(cutoff);
        assert!(
            (at_cutoff / q - 1.0).abs() < 0.01,
            "{cutoff} {q}: {at_cutoff}"
        );
        let octaves_up = amplitude(cutoff * 8.0);
        assert!(octaves_up < 1.0 / 32.0, "{cutoff} {q}: {octaves_up}");
    }
}

#[test]
fn the_lowpass_clamps_cutoff_below_nyquist_and_bounds_resonance() {
    const { assert!(MAX_CUTOFF_HZ < FS / 2.0) };
    const { assert!(MIN_CUTOFF_HZ > 0.0) };
    for (cutoff, q, applied_cutoff, applied_q) in [
        (1e12, 1e12, MAX_CUTOFF_HZ, MAX_RESONANCE_Q),
        (FS / 2.0, 1.0, MAX_CUTOFF_HZ, 1.0),
        (-1.0, -1.0, MIN_CUTOFF_HZ, MIN_RESONANCE_Q),
        (0.0, 0.0, MIN_CUTOFF_HZ, MIN_RESONANCE_Q),
        (f64::NAN, f64::NAN, MIN_CUTOFF_HZ, MIN_RESONANCE_Q),
        (f64::INFINITY, f64::INFINITY, MIN_CUTOFF_HZ, MIN_RESONANCE_Q),
    ] {
        let filter = SvfLowpass::new(cutoff, q);
        assert_eq!(filter.cutoff_hz(), applied_cutoff, "{cutoff}");
        assert_eq!(filter.resonance_q(), applied_q, "{q}");
    }
}

#[test]
fn the_resonance_bounds_are_the_contract_range() {
    let topology = Topology::subtractive_mono();
    let q = topology.contract("resonance_q").unwrap();
    assert_eq!(
        (q.physical_at_zero, q.physical_at_one),
        (MIN_RESONANCE_Q, MAX_RESONANCE_Q)
    );
    let drive = topology.contract("drive").unwrap();
    assert_eq!(
        (drive.physical_at_zero, drive.physical_at_one),
        (MIN_DRIVE_DB, MAX_DRIVE_DB)
    );
    let cutoff = topology.contract("filter_cutoff").unwrap();
    assert!(MIN_CUTOFF_HZ <= cutoff.physical_at_zero && cutoff.physical_at_one <= MAX_CUTOFF_HZ);
}

#[test]
fn the_lowpass_is_stable_at_its_cutoff_and_resonance_bounds() {
    for (cutoff, q) in [
        (MAX_CUTOFF_HZ, MAX_RESONANCE_Q),
        (MAX_CUTOFF_HZ, MIN_RESONANCE_Q),
        (MIN_CUTOFF_HZ, MAX_RESONANCE_Q),
        (MIN_CUTOFF_HZ, MIN_RESONANCE_Q),
        (40.0, MAX_RESONANCE_Q),
        (16_000.0, MAX_RESONANCE_Q),
    ] {
        // The impulse response decays.
        let mut filter = SvfLowpass::new(cutoff, q);
        let response: Vec<f64> = (0..480_000)
            .map(|n| filter.process(if n == 0 { 1.0 } else { 0.0 }))
            .collect();
        assert!(response.iter().all(|y| y.is_finite()), "{cutoff} {q}");
        let tail = response[470_000..]
            .iter()
            .fold(0.0_f64, |m, y| m.max(y.abs()));
        assert!(tail < 1e-6, "{cutoff} {q}: impulse tail {tail}");
        // Bounded input gives bounded output: the gain never exceeds the peak at the cutoff
        // by more than a factor of a few.
        let mut filter = SvfLowpass::new(cutoff, q);
        for x in noise(480_000, 7) {
            let y = filter.process(x);
            assert!(
                y.is_finite() && y.abs() < 4.0 * MAX_RESONANCE_Q,
                "{cutoff} {q}: {y}"
            );
        }
    }
}

#[test]
fn the_lowpass_replaces_a_non_finite_input_with_silence() {
    let mut filter = SvfLowpass::new(1200.0, 0.7);
    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(filter.process(x).is_finite());
    }
    for _ in 0..100 {
        assert!(filter.process(0.5).is_finite());
    }
}

// Envelope.

fn adsr_samples(adsr: &mut Adsr, count: usize) -> Vec<f64> {
    (0..count).map(|_| adsr.next_sample()).collect()
}

#[test]
fn the_adsr_matches_its_golden_segments() {
    // The scenario envelope: attack 10 ms (480 samples), decay 200 ms (9600) to 0.6, release
    // 300 ms (14400). Each segment is linear and ends exactly on its target.
    let mut adsr = Adsr::new(0.01, 0.2, 0.6, 0.3);
    assert_eq!(adsr.stage(), AdsrStage::Idle);
    assert_eq!(adsr.next_sample(), 0.0);
    adsr.note_on();
    let on = adsr_samples(&mut adsr, 20_000);
    for (k, level) in on.iter().enumerate() {
        let expected = if k < 480 {
            (k as f64 + 1.0) / 480.0
        } else if k < 480 + 9600 {
            1.0 - 0.4 * ((k - 480) as f64 + 1.0) / 9600.0
        } else {
            0.6
        };
        assert!(
            (level - expected).abs() < 1e-12,
            "sample {k}: {level} against {expected}"
        );
    }
    assert_eq!(on[479], 1.0);
    assert_eq!(on[480 + 9599], 0.6);
    assert_eq!(adsr.stage(), AdsrStage::Sustain);
    adsr.note_off();
    let off = adsr_samples(&mut adsr, 14_500);
    for (j, level) in off.iter().enumerate() {
        let expected = if j < 14_400 {
            0.6 * (1.0 - (j as f64 + 1.0) / 14_400.0)
        } else {
            0.0
        };
        assert!((level - expected).abs() < 1e-12, "release {j}: {level}");
    }
    assert_eq!(off[14_399], 0.0);
    assert_eq!(adsr.stage(), AdsrStage::Idle);
}

#[test]
fn the_adsr_handles_zero_and_near_zero_times() {
    for time in [
        0.0,
        1e-12,
        1e-9,
        1.0 / FS / 2.0,
        -1.0,
        f64::NAN,
        f64::INFINITY,
    ] {
        let mut adsr = Adsr::new(time, time, 0.5, time);
        adsr.note_on();
        let on = adsr_samples(&mut adsr, 8);
        assert!(
            on.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "{time}: {on:?}"
        );
        if !time.is_finite() || time < 1.0 / FS {
            assert_eq!(
                on[0], 1.0,
                "{time}: a zero attack reaches 1 on the note-on sample"
            );
            assert_eq!(
                on[1], 0.5,
                "{time}: a zero decay reaches sustain on the next"
            );
        }
        adsr.note_off();
        let off = adsr_samples(&mut adsr, 8);
        assert!(
            off.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "{time}"
        );
        if !time.is_finite() || time < 1.0 / FS {
            assert_eq!(
                off[0], 0.0,
                "{time}: a zero release reaches 0 on the note-off sample"
            );
        }
    }
}

#[test]
fn the_adsr_retriggers_from_its_current_level() {
    let mut adsr = Adsr::new(0.01, 0.2, 0.6, 0.3);
    adsr.note_on();
    adsr_samples(&mut adsr, 20_000);
    adsr.note_off();
    let released = adsr_samples(&mut adsr, 1000);
    let level = *released.last().unwrap();
    assert!(level > 0.0 && level < 0.6);
    adsr.note_on();
    assert_eq!(adsr.stage(), AdsrStage::Attack);
    let first = adsr.next_sample();
    assert!(
        (first - (level + (1.0 - level) / 480.0)).abs() < 1e-12,
        "retrigger continues from {level}, got {first}"
    );
    // A note-off during the attack releases from the level reached.
    let mut adsr = Adsr::new(0.01, 0.2, 0.6, 0.3);
    adsr.note_on();
    let attack = adsr_samples(&mut adsr, 240);
    adsr.note_off();
    assert_eq!(adsr.stage(), AdsrStage::Release);
    let next = adsr.next_sample();
    assert!(next < attack[239] && next > 0.0);
    // A note-off while idle does nothing.
    let mut idle = Adsr::new(0.01, 0.2, 0.6, 0.3);
    idle.note_off();
    assert_eq!(idle.stage(), AdsrStage::Idle);
    assert_eq!(idle.next_sample(), 0.0);
}

#[test]
fn the_adsr_clamps_sustain_to_the_unit_interval() {
    for (sustain, applied) in [(2.0, 1.0), (-1.0, 0.0), (f64::NAN, 0.0), (0.25, 0.25)] {
        let mut adsr = Adsr::new(0.0, 0.0, sustain, 0.0);
        adsr.note_on();
        let levels = adsr_samples(&mut adsr, 3);
        assert_eq!(levels[2], applied, "{sustain}");
    }
}

// Gain, smoothing and the limiter.

#[test]
fn the_smoother_approaches_its_target_without_a_step() {
    let mut smoother = Smoother::new(0.0, 0.005);
    smoother.set_target(1.0);
    let mut previous = 0.0;
    for n in 0..4800 {
        let v = smoother.next_sample();
        assert!(v > previous && v < 1.0, "sample {n}: {v}");
        assert!(v - previous < 0.01, "sample {n}: step {}", v - previous);
        previous = v;
    }
    assert!(previous > 0.999);
    let mut instant = Smoother::new(0.0, 0.0);
    instant.set_target(0.7);
    assert_eq!(instant.next_sample(), 0.7);
    let mut jumped = Smoother::new(0.0, 0.005);
    jumped.jump_to(0.3);
    assert_eq!(jumped.next_sample(), 0.3);
}

#[test]
fn the_limiter_clamps_to_its_threshold_and_counts_every_limited_sample() {
    let mut limiter = Limiter::new(LIMITER_THRESHOLD);
    let input = [0.0, 0.5, 1.0, 1.5, -1.0, -3.0, 0.999, 1.000_001];
    let output: Vec<f64> = input.iter().map(|x| limiter.process(*x)).collect();
    assert_eq!(output, [0.0, 0.5, 1.0, 1.0, -1.0, -1.0, 0.999, 1.0]);
    assert_eq!(limiter.limited_samples(), 3);
    assert_eq!(LIMITER_THRESHOLD, 1.0);
}

#[test]
fn the_output_stage_refuses_a_non_finite_sample() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut stage = OutputStage::new(-6.0);
        assert!(stage.process(0, 0.25).is_ok());
        assert_eq!(
            stage.process(1, bad),
            Err(RenderError::NonFiniteRender { frame: 1 }),
            "{bad}"
        );
    }
}

#[test]
fn the_output_stage_applies_its_gain_then_the_limiter() {
    let mut stage = OutputStage::new(-6.0);
    let gain = 10f64.powf(-6.0 / 20.0);
    assert_eq!(stage.process(0, 0.5), Ok((0.5 * gain) as f32));
    assert_eq!(stage.process(1, 4.0), Ok(1.0));
    assert_eq!(stage.process(2, -4.0), Ok(-1.0));
    assert_eq!(stage.limited_samples(), 2);
}
