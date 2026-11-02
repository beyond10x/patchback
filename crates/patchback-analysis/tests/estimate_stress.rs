//! Stress shapes of the note estimator (approval-record:wave-c1 Dc1-13, Dc1-20): the reference
//! generator of Dc1-12 with one range moved or a noise floor added, 32 phrases from each of the
//! seeds 1 to 8. On these shapes the rates are printed, not bounded; what holds is calibration and
//! polyphony: no quantity outside its tolerance carries a confidence at or above its threshold,
//! and no phrase, every one monophonic, is refused as `PolyphonicInput`. Refusing a hard note as
//! `LowConfidenceNotes` is allowed; a confident wrong answer is not.

mod reference;
mod voice;

use reference::{measure, print_table, rates_by_seed, stress_shape};

const SEEDS: [u64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

fn holds_calibration_and_polyphony(name: &str) {
    let shape = stress_shape(name);
    let measured = measure(shape, &SEEDS, reference::PHRASES);
    let (by_seed, pooled) = rates_by_seed(&measured, &SEEDS);
    print_table(
        &format!("{name}: rates by seed (not bounded)"),
        &by_seed,
        &pooled,
    );
    for violation in &pooled.violations {
        println!("  violation: {violation}");
    }
    assert!(
        pooled.violations.is_empty(),
        "{name}: {} confident quantities outside tolerance: {:#?}",
        pooled.violations.len(),
        pooled.violations
    );
    assert!(
        pooled.polyphonic.is_empty(),
        "{name}: monophonic phrases refused as polyphonic: {:#?}",
        pooled.polyphonic
    );
}

/// Releases of 300 ms to 1.5 s, overlapping the next onset.
#[test]
fn long_releases_hold_calibration_and_polyphony() {
    holds_calibration_and_polyphony("long-release");
}

/// Gates of 45 to 100 ms.
#[test]
fn short_gates_hold_calibration_and_polyphony() {
    holds_calibration_and_polyphony("short-gate");
}

/// Notes of MIDI 36 to 43.
#[test]
fn low_notes_hold_calibration_and_polyphony() {
    holds_calibration_and_polyphony("low");
}

/// Velocities of 0.1 to 1.
#[test]
fn quiet_notes_hold_calibration_and_polyphony() {
    holds_calibration_and_polyphony("quiet");
}

/// Attacks of 20 ms or less, half the notes repeating the previous pitch, half the gaps zero.
#[test]
fn zero_gap_same_pitch_repeats_hold_calibration_and_polyphony() {
    holds_calibration_and_polyphony("zero-gap-repeat");
}

/// A white noise floor at -70 dBFS RMS.
#[test]
fn a_noise_floor_holds_calibration_and_polyphony() {
    holds_calibration_and_polyphony("noise-floor");
}

/// A Gaussian white noise floor at -70 dBFS RMS (Dc1-20).
#[test]
fn a_gaussian_noise_floor_holds_calibration_and_polyphony() {
    holds_calibration_and_polyphony("gaussian-noise-floor");
}

/// Cutoffs of 100 to 800 Hz (Dc1-20).
#[test]
fn dark_cutoffs_hold_calibration_and_polyphony() {
    holds_calibration_and_polyphony("dark-cutoff");
}

/// Sustains of 0 to 0.3 (Dc1-20): a note's level can reach silence before its gate ends.
#[test]
fn low_sustains_hold_calibration_and_polyphony() {
    holds_calibration_and_polyphony("low-sustain");
}

/// Gaps of 0.5 to 10 ms, after attacks of up to 40 ms (Dc1-20).
#[test]
fn short_gaps_hold_calibration_and_polyphony() {
    holds_calibration_and_polyphony("short-gap");
}

/// Drives of 18 to 24 dB (Dc1-20).
#[test]
fn high_drives_hold_calibration_and_polyphony() {
    holds_calibration_and_polyphony("high-drive");
}

/// Decays of 300 ms to 2 s (Dc1-20).
#[test]
fn long_decays_hold_calibration_and_polyphony() {
    holds_calibration_and_polyphony("long-decay");
}
