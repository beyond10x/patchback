//! The numeric obligations of the note estimator (approval-record:wave-c1 Dc1-9) on the reference
//! distribution of Dc1-12 and Dc1-19: 32 phrases from each of the seeds 1 to 64 and the seed 0x5eed_0c19,
//! drawn by `reference::draw_phrase` (its module documentation states the generator, the truth
//! and the matching).
//!
//! Every Dc1-9 rate holds on the pooled set, and the calibration rule ("no quantity outside its
//! tolerance carries a confidence at or above its threshold") holds on every seed. Pooled
//! confident-inside is at least [`CONFIDENT_INSIDE_MARGIN`] (Dc1-19), above the 0.90 of Dc1-9, so
//! that a fresh block of 16 seeds clears 0.90. The test
//! prints the per-seed rates (`cargo test -- --nocapture` shows them); a bound is never relaxed.

mod reference;
mod voice;

use reference::{MS, REFERENCE, measure, print_table, rates_by_seed};

/// Pooled confident-inside on the reference set (Dc1-19).
const CONFIDENT_INSIDE_MARGIN: f64 = 0.92;

/// Seeds 1 to 64, then the seed the set was first measured on.
fn seeds() -> Vec<u64> {
    (1..=64).chain([0x5eed_0c19]).collect()
}

#[test]
fn the_reference_distribution_meets_every_dc1_9_bound_pooled_and_calibration_on_every_seed() {
    let seeds = seeds();
    let measured = measure(REFERENCE, &seeds, reference::PHRASES);

    // The generator draws what Dc1-9 names.
    let mut repeats = 0;
    let mut zero_gaps = 0;
    let mut zero_gap_repeats = 0;
    for m in &measured {
        assert!((2..=8).contains(&m.notes.len()));
        for note in &m.notes {
            assert!((36..=84).contains(&note.2));
        }
        for pair in m.notes.windows(2) {
            let gap = pair[1].0 - pair[0].1;
            assert!(gap <= (200.0 * MS) as u64);
            let repeat = pair[0].2 == pair[1].2;
            repeats += usize::from(repeat);
            if gap == 0 {
                assert!(m.voice.attack <= 0.020);
                zero_gaps += 1;
                zero_gap_repeats += usize::from(repeat);
            }
        }
    }
    let notes: usize = measured.iter().map(|m| m.notes.len()).sum();
    println!(
        "reference distribution: {} phrases, {notes} notes, {repeats} same-pitch repeats, \
         {zero_gaps} zero gaps, {zero_gap_repeats} zero-gap same-pitch repeats",
        measured.len()
    );
    assert!(repeats > 0 && zero_gaps > 0 && zero_gap_repeats > 0);

    let (by_seed, pooled) = rates_by_seed(&measured, &seeds);
    print_table("Dc1-9 rates by seed", &by_seed, &pooled);
    for violation in &pooled.violations {
        println!("  violation: {violation}");
    }
    let mut failures = pooled.bound_misses();
    if pooled.confident_inside() < CONFIDENT_INSIDE_MARGIN {
        failures.push(format!(
            "confident inside < {CONFIDENT_INSIDE_MARGIN} (Dc1-19): {:.4}",
            pooled.confident_inside()
        ));
    }
    for (seed, rates) in &by_seed {
        if !rates.violations.is_empty() {
            failures.push(format!(
                "seed {seed:#x}: {} confident quantities outside tolerance",
                rates.violations.len()
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Exploration: `PB_SHAPE` (`reference` or a stress shape), `PB_SEEDS` (`a-b` or a comma list),
/// `PB_PHRASES`, `PB_VERBOSE` (print every quantity outside tolerance). Asserts nothing.
#[test]
#[ignore = "exploration probe; run with --ignored and PB_* variables"]
fn probe() {
    let shape = match std::env::var("PB_SHAPE").as_deref() {
        Ok("reference") | Err(_) => REFERENCE,
        Ok(name) => reference::stress_shape(name),
    };
    let seeds = parse_seeds(&std::env::var("PB_SEEDS").unwrap_or_else(|_| "1-16".into()));
    let phrases = std::env::var("PB_PHRASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(reference::PHRASES);
    let measured = measure(shape, &seeds, phrases);
    let (by_seed, pooled) = rates_by_seed(&measured, &seeds);
    print_table(shape.name, &by_seed, &pooled);
    for v in &pooled.violations {
        println!("  violation: {v}");
    }
    for r in &pooled.refused {
        println!("  refused: {r}");
    }
    if std::env::var_os("PB_VERBOSE").is_some() {
        for miss in &pooled.misses {
            println!("  miss: {miss}");
        }
    }
    for miss in pooled.bound_misses() {
        println!("  bound: {miss}");
    }
    // `PB_SHOW` lists `seed:index` phrases to print in full.
    if let Ok(show) = std::env::var("PB_SHOW") {
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
                reference::Outcome::Notes(accepted, notes) => {
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
                reference::Outcome::Refused(e) => println!("  refused {e}"),
            }
        }
    }
    println!(
        "unconfident inside: onset {} pitch {} gate {} tail {} of {}",
        pooled.unconfident_inside[0],
        pooled.unconfident_inside[1],
        pooled.unconfident_inside[2],
        pooled.unconfident_inside[3],
        pooled.inside_total
    );
}

fn parse_seeds(text: &str) -> Vec<u64> {
    let number = |s: &str| -> u64 {
        let s = s.trim();
        s.strip_prefix("0x").map_or_else(
            || s.parse().unwrap(),
            |h| u64::from_str_radix(h, 16).unwrap(),
        )
    };
    match text.split_once('-') {
        Some((a, b)) => (number(a)..=number(b)).collect(),
        None => text.split(',').map(number).collect(),
    }
}
