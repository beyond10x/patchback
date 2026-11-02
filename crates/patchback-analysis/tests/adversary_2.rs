//! Adversary pass 2 on story:note-estimation (approval-record:wave-c1 Dc1-9, Dc1-12, Dc1-13,
//! Dc1-14, Dc1-18).
//!
//! The constants of correction round 1 were chosen while looking at seeds 1 to 16 of the
//! reference generator and seeds 1 to 4 of the stress shapes. These cases measure the same
//! bounds out of sample: the reference generator on seeds 17 to 32, every stress shape on seeds
//! 5 to 8, and phrase shapes inside Dc1-9's wording ("gaps of 0 to 200 ms (zero gaps only with an
//! attack of 20 ms or less)", "varied ADSR, cutoff and drive") the generator does not draw.

mod reference;
mod voice;

use std::sync::OnceLock;

use patchback_analysis::{EstimationError, estimate_notes, read_estimation, write_estimation};
use patchback_core::prng::Xoshiro256StarStar;
use reference::{
    MS, Measured, Outcome, REFERENCE, Rates, Rng, Shape, estimate, measure, print_table,
    rates_by_seed, stress_shape, tails,
};
use voice::{Note, Voice, render_notes};

/// Seeds the reference harness never saw.
const UNSEEN_REFERENCE: [u64; 16] = [
    17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
];

/// Stress seeds the stress harness never saw.
const UNSEEN_STRESS: [u64; 4] = [5, 6, 7, 8];

fn unseen_reference() -> &'static (Vec<(u64, Rates)>, Rates) {
    static MEASURED: OnceLock<(Vec<(u64, Rates)>, Rates)> = OnceLock::new();
    MEASURED.get_or_init(|| {
        let measured = measure(REFERENCE, &UNSEEN_REFERENCE, reference::PHRASES);
        let (by_seed, pooled) = rates_by_seed(&measured, &UNSEEN_REFERENCE);
        print_table(
            "adversary 2: reference distribution, seeds 17 to 32",
            &by_seed,
            &pooled,
        );
        println!(
            "unconfident inside: onset {} pitch {} gate {} tail {} of {}",
            pooled.unconfident_inside[0],
            pooled.unconfident_inside[1],
            pooled.unconfident_inside[2],
            pooled.unconfident_inside[3],
            pooled.inside_total
        );
        for violation in &pooled.violations {
            println!("  violation: {violation}");
        }
        for refused in &pooled.refused {
            println!("  refused: {refused}");
        }
        (by_seed, pooled)
    })
}

/// Dc1-9 and Dc1-12 on seeds 17 to 32: every bound pooled, the calibration rule on every seed.
#[test]
fn the_reference_distribution_on_unseen_seeds_17_to_32_meets_every_dc1_9_bound_and_calibration_per_seed()
 {
    let (by_seed, pooled) = unseen_reference();
    let mut failures = pooled.bound_misses();
    for (seed, rates) in by_seed {
        if !rates.violations.is_empty() {
            failures.push(format!(
                "seed {seed}: {} confident quantities outside tolerance: {:?}",
                rates.violations.len(),
                rates.violations
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Dc1-3 on seeds 17 to 32: every reference phrase is monophonic, none is refused as polyphonic.
#[test]
fn no_reference_phrase_on_unseen_seeds_17_to_32_is_refused_as_polyphonic() {
    let (_, pooled) = unseen_reference();
    assert!(
        pooled.polyphonic.is_empty(),
        "monophonic phrases refused as PolyphonicInput: {:#?}",
        pooled.polyphonic
    );
}

/// Prints the violations of the estimations the estimator accepted: the ones `estimation.txt`
/// and `notes.events` would hold. Diagnostic only.
fn print_accepted_violations(measured: &[Measured]) {
    for m in measured {
        if let Outcome::Notes(true, _) = m.outcome {
            for violation in reference::score(m).violations {
                println!("  accepted violation: {violation}");
            }
        }
    }
}

/// Dc1-13 on seeds 5 to 8: calibration and polyphony, as `estimate_stress.rs` asserts on 1 to 4.
fn stress_holds_on_unseen_seeds(shape: Shape) {
    let measured = measure(shape, &UNSEEN_STRESS, reference::PHRASES);
    let (by_seed, pooled) = rates_by_seed(&measured, &UNSEEN_STRESS);
    print_table(
        &format!("adversary 2: {}, seeds 5 to 8 (not bounded)", shape.name),
        &by_seed,
        &pooled,
    );
    for violation in &pooled.violations {
        println!("  violation: {violation}");
    }
    print_accepted_violations(&measured);
    let mut failures = Vec::new();
    if !pooled.violations.is_empty() {
        failures.push(format!(
            "{}: {} confident quantities outside tolerance: {:#?}",
            shape.name,
            pooled.violations.len(),
            pooled.violations
        ));
    }
    if !pooled.polyphonic.is_empty() {
        failures.push(format!(
            "{}: monophonic phrases refused as PolyphonicInput: {:#?}",
            shape.name, pooled.polyphonic
        ));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn long_releases_on_unseen_seeds_5_to_8_hold_calibration_and_polyphony() {
    stress_holds_on_unseen_seeds(stress_shape("long-release"));
}

#[test]
fn short_gates_on_unseen_seeds_5_to_8_hold_calibration_and_polyphony() {
    stress_holds_on_unseen_seeds(stress_shape("short-gate"));
}

#[test]
fn low_notes_on_unseen_seeds_5_to_8_hold_calibration_and_polyphony() {
    stress_holds_on_unseen_seeds(stress_shape("low"));
}

#[test]
fn quiet_notes_on_unseen_seeds_5_to_8_hold_calibration_and_polyphony() {
    stress_holds_on_unseen_seeds(stress_shape("quiet"));
}

#[test]
fn zero_gap_same_pitch_repeats_on_unseen_seeds_5_to_8_hold_calibration_and_polyphony() {
    stress_holds_on_unseen_seeds(stress_shape("zero-gap-repeat"));
}

#[test]
fn a_noise_floor_on_unseen_seeds_5_to_8_holds_calibration_and_polyphony() {
    stress_holds_on_unseen_seeds(stress_shape("noise-floor"));
}

/// The noise-relative silence threshold at another level than the one it was tuned on: white
/// noise 4 dB below the silence threshold instead of 10, seeds 1 to 4.
#[test]
fn a_noise_floor_4_db_below_the_silence_threshold_holds_calibration_and_polyphony() {
    let shape = Shape {
        name: "noise-floor-64",
        noise_dbfs: Some(-64.0),
        ..REFERENCE
    };
    let measured = measure(shape, &[1, 2, 3, 4], reference::PHRASES);
    let (by_seed, pooled) = rates_by_seed(&measured, &[1, 2, 3, 4]);
    print_table(
        "adversary 2: noise floor -64 dBFS (not bounded)",
        &by_seed,
        &pooled,
    );
    print_accepted_violations(&measured);
    assert!(
        pooled.violations.is_empty() && pooled.polyphonic.is_empty(),
        "violations {:#?}\npolyphonic {:#?}",
        pooled.violations,
        pooled.polyphonic
    );
}

// Phrase shapes inside Dc1-9's wording that the reference generator does not draw.

/// Every range of a drawn phrase.
#[derive(Clone, Copy, Debug)]
struct Draw {
    name: &'static str,
    attack_ms: (f64, f64),
    decay_ms: (f64, f64),
    sustain: (f64, f64),
    release_ms: (f64, f64),
    cutoff_hz: (f64, f64),
    drive_db: (f64, f64),
    gate_ms: (f64, f64),
    /// The gap when it is not a zero gap.
    gap_ms: (f64, f64),
    zero_gap: f64,
    repeat: f64,
    /// A Gaussian white noise floor at this RMS in dBFS, if any.
    gaussian_dbfs: Option<f64>,
}

/// The reference generator's ranges (`reference::draw_phrase`).
const DRAW_REFERENCE: Draw = Draw {
    name: "reference",
    attack_ms: (1.0, 40.0),
    decay_ms: (20.0, 300.0),
    sustain: (0.3, 1.0),
    release_ms: (10.0, 300.0),
    cutoff_hz: (800.0, 8_000.0),
    drive_db: (0.0, 18.0),
    gate_ms: (100.0, 400.0),
    gap_ms: (0.0, 200.0),
    zero_gap: 0.3,
    repeat: 0.2,
    gaussian_dbfs: None,
};

fn draw(rng: &mut Rng, d: &Draw) -> (Voice, Vec<Note>) {
    let attack = rng.log_uniform(d.attack_ms.0, d.attack_ms.1) / 1_000.0;
    let voice = Voice {
        attack,
        decay: rng.log_uniform(d.decay_ms.0, d.decay_ms.1) / 1_000.0,
        sustain: rng.uniform(d.sustain.0, d.sustain.1),
        release: rng.log_uniform(d.release_ms.0, d.release_ms.1) / 1_000.0,
        cutoff: rng.log_uniform(d.cutoff_hz.0, d.cutoff_hz.1),
        q: rng.uniform(0.5, 1.5),
        drive: rng.uniform(d.drive_db.0, d.drive_db.1),
        gain: rng.uniform(-12.0, -3.0),
    };
    let count = rng.int(2, 8) as usize;
    let mut notes: Vec<Note> = Vec::with_capacity(count);
    let mut start = (rng.uniform(50.0, 150.0) * MS) as u64;
    let mut previous: Option<u8> = None;
    for _ in 0..count {
        let repeat = previous.is_some() && rng.unit() < d.repeat;
        let midi = match previous {
            Some(p) if repeat => p,
            _ => loop {
                let m = rng.int(36, 84) as u8;
                if Some(m) != previous {
                    break m;
                }
            },
        };
        if let Some(&(_, end, _, _)) = notes.last() {
            let gap_ms = if attack <= 0.020 && rng.unit() < d.zero_gap {
                0.0
            } else {
                rng.uniform(d.gap_ms.0, d.gap_ms.1)
            };
            start = end + (gap_ms * MS) as u64;
        }
        let gate = (rng.uniform(d.gate_ms.0, d.gate_ms.1) * MS) as u64;
        let velocity = rng.uniform(0.5, 1.0) as f32;
        notes.push((start, start + gate, midi, velocity));
        previous = Some(midi);
    }
    (voice, notes)
}

/// Draws and estimates `phrases` phrases a seed, in parallel; the truth as `reference` defines it.
fn measure_draw(d: Draw, seeds: &[u64], phrases: usize) -> Vec<Measured> {
    let mut jobs = Vec::new();
    for &seed in seeds {
        let mut rng = Rng(Xoshiro256StarStar::new(seed));
        for index in 0..phrases {
            let (voice, notes) = draw(&mut rng, &d);
            jobs.push((seed, index, voice, notes));
        }
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = jobs.len().div_ceil(threads).max(1);
    let mut out: Vec<Measured> = std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|(seed, index, voice, notes)| Measured {
                            seed: *seed,
                            index: *index,
                            voice: *voice,
                            notes: notes.clone(),
                            tails: tails(*voice, notes),
                            outcome: estimate(&noisy(
                                &render_notes(*voice, notes),
                                d.gaussian_dbfs,
                                seed.wrapping_mul(1_000).wrapping_add(*index as u64),
                            )),
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    out.sort_by_key(|m| (m.seed, m.index));
    out
}

fn draw_holds_calibration_and_polyphony(d: Draw) {
    let seeds = [1, 2, 3, 4];
    let measured = measure_draw(d, &seeds, reference::PHRASES);
    let (by_seed, pooled) = rates_by_seed(&measured, &seeds);
    print_table(
        &format!("adversary 2: {} (not bounded)", d.name),
        &by_seed,
        &pooled,
    );
    for violation in &pooled.violations {
        println!("  violation: {violation}");
    }
    print_accepted_violations(&measured);
    let mut failures = Vec::new();
    if !pooled.violations.is_empty() {
        failures.push(format!(
            "{}: {} confident quantities outside tolerance: {:#?}",
            d.name,
            pooled.violations.len(),
            pooled.violations
        ));
    }
    if !pooled.polyphonic.is_empty() {
        failures.push(format!(
            "{}: monophonic phrases refused as PolyphonicInput: {:#?}",
            d.name, pooled.polyphonic
        ));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// `samples` with Gaussian white noise at `dbfs` RMS added (Box-Muller from a seeded PRNG).
fn noisy(samples: &[f32], dbfs: Option<f64>, seed: u64) -> Vec<f32> {
    let Some(dbfs) = dbfs else {
        return samples.to_vec();
    };
    let sigma = 10f64.powf(dbfs / 20.0);
    let mut rng = Xoshiro256StarStar::new(seed);
    samples
        .iter()
        .map(|s| {
            let u1 = rng.next_f64().max(1e-300);
            let u2 = rng.next_f64();
            let g = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            s + (sigma * g) as f32
        })
        .collect()
}

/// Dc1-13's noise floor, -70 dBFS RMS, Gaussian instead of the generator's uniform noise (whose
/// peaks never exceed 1.73 times its RMS, so a factor-of-4 quiet threshold is never crossed).
#[test]
fn a_gaussian_noise_floor_at_minus_70_dbfs_holds_calibration_and_polyphony() {
    draw_holds_calibration_and_polyphony(Draw {
        name: "gaussian-noise-floor",
        gaussian_dbfs: Some(-70.0),
        ..DRAW_REFERENCE
    });
}

/// Exploration: `PB2_DRAW` (`pluck`, `dark`, `hot`, `tiny-gap`, `long-decay`, `gaussian`),
/// `PB2_SEEDS` (comma list), `PB2_SHOW` (`seed:index` list). Asserts nothing.
#[test]
#[ignore = "exploration probe"]
fn probe_draw() {
    let d = match std::env::var("PB2_DRAW").as_deref() {
        Ok("pluck") => Draw {
            name: "pluck",
            sustain: (0.0, 0.3),
            ..DRAW_REFERENCE
        },
        Ok("dark") => Draw {
            name: "dark",
            cutoff_hz: (100.0, 800.0),
            ..DRAW_REFERENCE
        },
        Ok("hot") => Draw {
            name: "hot",
            drive_db: (18.0, 24.0),
            ..DRAW_REFERENCE
        },
        Ok("tiny-gap") => Draw {
            name: "tiny-gap",
            gap_ms: (0.5, 10.0),
            zero_gap: 0.0,
            ..DRAW_REFERENCE
        },
        Ok("long-decay") => Draw {
            name: "long-decay",
            decay_ms: (300.0, 2_000.0),
            ..DRAW_REFERENCE
        },
        Ok("gaussian") => Draw {
            name: "gaussian",
            gaussian_dbfs: Some(-70.0),
            ..DRAW_REFERENCE
        },
        _ => DRAW_REFERENCE,
    };
    let seeds: Vec<u64> = std::env::var("PB2_SEEDS")
        .unwrap_or_else(|_| "1,2,3,4".into())
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    let measured = measure_draw(d, &seeds, reference::PHRASES);
    let (by_seed, pooled) = rates_by_seed(&measured, &seeds);
    print_table(d.name, &by_seed, &pooled);
    print_accepted_violations(&measured);
    for r in &pooled.refused {
        println!("  refused: {r}");
    }
    if let Ok(show) = std::env::var("PB2_SHOW") {
        for item in show.split(',') {
            let (s, i) = item.split_once(':').unwrap();
            let (s, i): (u64, usize) = (s.parse().unwrap(), i.parse().unwrap());
            let Some(m) = measured.iter().find(|m| m.seed == s && m.index == i) else {
                continue;
            };
            println!("== seed {s} phrase {i}: {:?}", m.voice);
            for (k, (note, tail)) in m.notes.iter().zip(&m.tails).enumerate() {
                println!("  truth {k}: {note:?} tail {tail}");
            }
            match &m.outcome {
                Outcome::Notes(accepted, notes) => {
                    println!("  accepted {accepted}");
                    for n in notes {
                        println!(
                            "  est {} {} {} midi {} f0 {:.2} v {:.3} conf o {:.3} p {:.3} g {:.3} t {:.3}",
                            n.start_sample,
                            n.end_sample,
                            n.tail_end_sample,
                            n.midi_note,
                            n.f0_hz,
                            n.velocity,
                            n.onset_confidence,
                            n.pitch_confidence,
                            n.gate_confidence,
                            n.tail_confidence
                        );
                    }
                }
                Outcome::Refused(e) => println!("  refused {e}"),
            }
        }
    }
}

/// Gaps of 0.5 to 10 ms after attacks of up to 40 ms: inside "gaps of 0 to 200 ms" (only a
/// zero gap needs a short attack), drawn by the reference generator about 5 % of the time.
#[test]
fn gaps_of_half_a_millisecond_to_10_ms_hold_calibration_and_polyphony() {
    draw_holds_calibration_and_polyphony(Draw {
        name: "tiny-gap",
        gap_ms: (0.5, 10.0),
        zero_gap: 0.0,
        ..DRAW_REFERENCE
    });
}

/// Sustains of 0 to 0.3, the part of the voice's sustain range the generator leaves out: the
/// level of a note can reach silence before its gate ends.
#[test]
fn plucked_notes_with_a_sustain_below_0_3_hold_calibration_and_polyphony() {
    draw_holds_calibration_and_polyphony(Draw {
        name: "pluck",
        sustain: (0.0, 0.3),
        ..DRAW_REFERENCE
    });
}

/// Decays of 300 ms to 2 s, the rest of the voice's decay range.
#[test]
fn long_decays_hold_calibration_and_polyphony() {
    draw_holds_calibration_and_polyphony(Draw {
        name: "long-decay",
        decay_ms: (300.0, 2_000.0),
        ..DRAW_REFERENCE
    });
}

/// Cutoffs of 100 to 800 Hz, below the generator's range: a dark, nearly sinusoidal voice.
#[test]
fn dark_cutoffs_of_100_to_800_hz_hold_calibration_and_polyphony() {
    draw_holds_calibration_and_polyphony(Draw {
        name: "dark",
        cutoff_hz: (100.0, 800.0),
        ..DRAW_REFERENCE
    });
}

/// Drives of 18 to 24 dB, the top of the voice's drive range.
#[test]
fn drives_of_18_to_24_db_hold_calibration_and_polyphony() {
    draw_holds_calibration_and_polyphony(Draw {
        name: "hot",
        drive_db: (18.0, 24.0),
        ..DRAW_REFERENCE
    });
}

// Dc1-3 out of sample: drawn voices and roots instead of the four fixed roots of `estimate.rs`.

/// Two simultaneous tones a minor second, a major third, a tritone or a fifth apart, each from
/// a voice drawn from the reference ranges at a root drawn from MIDI 36 to 84, are refused as
/// `PolyphonicInput`, never accepted and never `LowConfidenceNotes` (polyphony is decided before
/// confidence).
#[test]
fn drawn_dyads_a_minor_second_major_third_tritone_or_fifth_apart_are_refused_as_polyphonic() {
    let mut rng = Rng(Xoshiro256StarStar::new(0xad_0002));
    let mut cases = Vec::new();
    for k in 0..64 {
        let interval = [1u8, 4, 6, 7][k % 4];
        let voice = Voice {
            attack: rng.log_uniform(1.0, 40.0) / 1_000.0,
            decay: rng.log_uniform(20.0, 300.0) / 1_000.0,
            sustain: rng.uniform(0.3, 1.0),
            release: rng.log_uniform(10.0, 300.0) / 1_000.0,
            cutoff: rng.log_uniform(800.0, 8_000.0),
            q: rng.uniform(0.5, 1.5),
            drive: rng.uniform(0.0, 18.0),
            gain: rng.uniform(-12.0, -3.0),
        };
        let root = rng.int(36, 84 - u64::from(interval)) as u8;
        let gate = (rng.uniform(200.0, 400.0) * MS) as u64;
        cases.push((voice, root, interval, gate));
    }
    let failures = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for part in cases.chunks(4) {
            let failures = &failures;
            scope.spawn(move || {
                for &(voice, root, interval, gate) in part {
                    let low = render_notes(voice, &[(4_800, 4_800 + gate, root, 0.8)]);
                    let high = render_notes(voice, &[(4_800, 4_800 + gate, root + interval, 0.8)]);
                    let mix: Vec<f32> = low.iter().zip(&high).map(|(a, b)| 0.5 * (a + b)).collect();
                    match estimate_notes(&mix, 48_000) {
                        Err(EstimationError::PolyphonicInput) => {}
                        other => failures.lock().unwrap().push(format!(
                            "MIDI {root} + {interval}, gate {gate}, {voice:?}: {}",
                            match other {
                                Ok(e) => format!("accepted {} notes", e.notes.len()),
                                Err(e) => e.domain_error().to_owned(),
                            }
                        )),
                    }
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    assert!(
        failures.is_empty(),
        "{} of 64 dyads not refused as PolyphonicInput: {failures:#?}",
        failures.len()
    );
}

/// One note from a voice drawn from the reference ranges, at a MIDI number drawn from 36 to 84,
/// is never refused as `PolyphonicInput` (Dc1-3).
#[test]
fn a_single_drawn_note_is_never_refused_as_polyphonic() {
    let mut rng = Rng(Xoshiro256StarStar::new(0xad_0003));
    let mut cases = Vec::new();
    for _ in 0..128 {
        let voice = Voice {
            attack: rng.log_uniform(1.0, 40.0) / 1_000.0,
            decay: rng.log_uniform(20.0, 300.0) / 1_000.0,
            sustain: rng.uniform(0.3, 1.0),
            release: rng.log_uniform(10.0, 300.0) / 1_000.0,
            cutoff: rng.log_uniform(800.0, 8_000.0),
            q: rng.uniform(0.5, 1.5),
            drive: rng.uniform(0.0, 18.0),
            gain: rng.uniform(-12.0, -3.0),
        };
        let midi = rng.int(36, 84) as u8;
        let gate = (rng.uniform(100.0, 400.0) * MS) as u64;
        cases.push((voice, midi, gate));
    }
    let failures = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for part in cases.chunks(8) {
            let failures = &failures;
            scope.spawn(move || {
                for &(voice, midi, gate) in part {
                    let samples = render_notes(voice, &[(4_800, 4_800 + gate, midi, 0.8)]);
                    if let Err(EstimationError::PolyphonicInput) = estimate_notes(&samples, 48_000)
                    {
                        failures
                            .lock()
                            .unwrap()
                            .push(format!("MIDI {midi}, gate {gate}, {voice:?}"));
                    }
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    assert!(failures.is_empty(), "{failures:#?}");
}

// Determinism (Dc1-7, O6).

/// One clip estimated on eight threads at once and twice in a row writes one text.
#[test]
fn concurrent_estimations_of_one_clip_write_identical_bytes() {
    let mut rng = Rng(Xoshiro256StarStar::new(17));
    let (voice, notes) = draw(&mut rng, &DRAW_REFERENCE);
    let samples = render_notes(voice, &notes);
    let text = |samples: &[f32]| match estimate_notes(samples, 48_000) {
        Ok(e) => write_estimation(&e),
        Err(EstimationError::LowConfidenceNotes { notes }) => format!("{notes:?}"),
        Err(e) => e.to_string(),
    };
    let first = text(&samples);
    let texts: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8).map(|_| scope.spawn(|| text(&samples))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for t in texts {
        assert_eq!(t, first);
    }
    assert_eq!(text(&samples), first);
}

// The self-checking file (Dc1-14).

/// The phrase of `estimate.rs`, accepted, as `estimation.txt` text.
fn accepted_text() -> String {
    let notes: [Note; 3] = [
        (4_800, 19_200, 45, 0.9),
        (20_160, 34_560, 52, 0.7),
        (34_560, 52_800, 57, 0.8),
    ];
    let samples = render_notes(voice::PLAIN, &notes);
    write_estimation(&estimate_notes(&samples, 48_000).expect("the shared phrase is accepted"))
}

/// The module documentation: "a file whose clip, configuration or id was changed reads as
/// refused". The frame count is the clip's, and nothing the reader checks covers it.
#[test]
fn an_estimation_whose_frame_count_was_changed_is_refused() {
    let text = accepted_text();
    let estimation = read_estimation(&text).unwrap();
    let edited = text.replacen(
        &format!("frame_count {}\n", estimation.frame_count),
        &format!("frame_count {}\n", estimation.frame_count * 2),
        1,
    );
    assert_ne!(edited, text);
    assert!(
        read_estimation(&edited).is_err(),
        "an estimation with frame_count {} for a clip of {} samples reads: {:?}",
        estimation.frame_count * 2,
        estimation.frame_count,
        read_estimation(&edited).map(|e| e.frame_count)
    );
}
