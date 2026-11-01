//! Adversary pass 1 on story:note-estimation (approval-record:wave-c1 Dc1-3, Dc1-9).
//!
//! The reference harness (`estimate_reference.rs`) measures one seed of one generator. These
//! cases measure the same Dc1-9 bounds on phrase shapes the generator under-samples, and the
//! confidence contract ("no quantity outside its tolerance carries a confidence at or above its
//! threshold") on single constructed inputs.

mod voice;

use patchback_analysis::{
    EstimatedNote, EstimationError, GATE_CONFIDENCE_THRESHOLD, ONSET_CONFIDENCE_THRESHOLD,
    PITCH_CONFIDENCE_THRESHOLD, SILENCE_THRESHOLD_DBFS, TAIL_CONFIDENCE_THRESHOLD, estimate_notes,
};
use patchback_core::prng::Xoshiro256StarStar;
use voice::{Note, Voice, centred_rms, cents, render_notes};

const MS: f64 = 48.0;
const ONSET_TOLERANCE: u64 = 256;
const GATE_TOLERANCE: u64 = 1_024;
const TAIL_TOLERANCE: u64 = 2_400;
const PITCH_TOLERANCE_CENTS: f64 = 10.0;
const MATCH_WINDOW: u64 = 2_400;

struct Rng(Xoshiro256StarStar);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0.next_f64()
    }
    fn uniform(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }
    fn log_uniform(&mut self, low: f64, high: f64) -> f64 {
        (low.ln() + (high.ln() - low.ln()) * self.unit()).exp()
    }
    fn int(&mut self, low: u64, high: u64) -> u64 {
        low + (self.0.next_u64() % (high - low + 1))
    }
}

/// A phrase generator: the reference generator's draws, with ranges a case narrows.
#[derive(Clone, Copy, Debug)]
struct Shape {
    attack_ms: (f64, f64),
    release_ms: (f64, f64),
    sustain: (f64, f64),
    gate_ms: (f64, f64),
    velocity: (f64, f64),
    midi: (u64, u64),
    /// Probability that a note repeats the previous pitch.
    repeat: f64,
    /// A repeated note may follow with a zero gap (attack 20 ms or less, as Dc1-9 allows).
    repeat_zero_gap: bool,
}

/// The reference generator's ranges (`estimate_reference.rs` `draw_phrase`).
const REFERENCE: Shape = Shape {
    attack_ms: (1.0, 40.0),
    release_ms: (10.0, 300.0),
    sustain: (0.3, 1.0),
    gate_ms: (100.0, 400.0),
    velocity: (0.5, 1.0),
    midi: (36, 84),
    repeat: 0.2,
    repeat_zero_gap: false,
};

fn draw_phrase(rng: &mut Rng, shape: &Shape) -> (Voice, Vec<Note>) {
    let attack = rng.log_uniform(shape.attack_ms.0, shape.attack_ms.1) / 1_000.0;
    let voice = Voice {
        attack,
        decay: rng.log_uniform(20.0, 300.0) / 1_000.0,
        sustain: rng.uniform(shape.sustain.0, shape.sustain.1),
        release: rng.log_uniform(shape.release_ms.0, shape.release_ms.1) / 1_000.0,
        cutoff: rng.log_uniform(800.0, 8_000.0),
        q: rng.uniform(0.5, 1.5),
        drive: rng.uniform(0.0, 18.0),
        gain: rng.uniform(-12.0, -3.0),
    };
    let count = rng.int(2, 8) as usize;
    let mut notes: Vec<Note> = Vec::with_capacity(count);
    let mut start = (rng.uniform(50.0, 150.0) * MS) as u64;
    let mut previous: Option<u8> = None;
    for _ in 0..count {
        let repeat = previous.is_some() && rng.unit() < shape.repeat;
        let midi = match previous {
            Some(p) if repeat => p,
            _ => loop {
                let m = rng.int(shape.midi.0, shape.midi.1) as u8;
                if Some(m) != previous || shape.midi.0 == shape.midi.1 {
                    break m;
                }
            },
        };
        if let Some(&(_, end, _, _)) = notes.last() {
            let gap_ms = if repeat {
                if shape.repeat_zero_gap && attack <= 0.020 && rng.unit() < 0.5 {
                    0.0
                } else {
                    rng.uniform(40.0, 200.0)
                }
            } else if attack <= 0.020 && rng.unit() < 0.3 {
                0.0
            } else {
                rng.uniform(0.0, 200.0)
            };
            start = end + (gap_ms * MS) as u64;
        }
        let gate = (rng.uniform(shape.gate_ms.0, shape.gate_ms.1) * MS) as u64;
        let velocity = rng.uniform(shape.velocity.0, shape.velocity.1) as f32;
        notes.push((start, start + gate, midi, velocity));
        previous = Some(midi);
    }
    (voice, notes)
}

fn silence_crossing(samples: &[f32], from: usize) -> usize {
    let threshold = 10f64.powf(SILENCE_THRESHOLD_DBFS / 20.0);
    (from..samples.len())
        .find(|&t| centred_rms(samples, t) < threshold)
        .unwrap_or(samples.len())
}

/// The truth of a phrase's tails, as the reference harness defines it.
fn tails(voice: Voice, notes: &[Note]) -> Vec<u64> {
    (0..notes.len())
        .map(|i| {
            let prefix = render_notes(voice, &notes[..=i]);
            let crossing = silence_crossing(&prefix, notes[i].1 as usize) as u64;
            match notes.get(i + 1) {
                Some(next) => crossing.min(next.0),
                None => crossing,
            }
        })
        .collect()
}

/// What the estimator returned for a phrase: its notes (accepted or refused for low confidence),
/// or another refusal.
fn estimate(samples: &[f32]) -> Result<(bool, Vec<EstimatedNote>), String> {
    match estimate_notes(samples, 48_000) {
        Ok(e) => Ok((true, e.notes)),
        Err(EstimationError::LowConfidenceNotes { notes }) => Ok((false, notes)),
        Err(error) => Err(error.domain_error().to_owned()),
    }
}

#[derive(Default, Debug)]
struct Rates {
    truths: usize,
    found: usize,
    spurious: usize,
    onset_inside: usize,
    pitch_inside: usize,
    gate_inside: usize,
    tail_inside: usize,
    inside_total: usize,
    inside_confident: usize,
    refused: Vec<String>,
    violations: Vec<String>,
    /// Confident quantities outside tolerance in phrases the estimator accepted (no refusal):
    /// these reach `estimation.txt` and `notes.events`.
    accepted_violations: Vec<String>,
    accepted: usize,
}

impl Rates {
    fn rate(a: usize, b: usize) -> f64 {
        if b == 0 { 0.0 } else { a as f64 / b as f64 }
    }

    fn summary(&self, name: &str) -> String {
        format!(
            "{name}: notes {}, accepted phrases {}, detection {:.4}, spurious {:.4}, onset {:.4}, pitch {:.4}, gate {:.4}, tail {:.4}, confident-inside {:.4}, confident-outside {} (in accepted phrases {}), refused {:?}",
            self.truths,
            self.accepted,
            Self::rate(self.found, self.truths),
            Self::rate(self.spurious, self.found + self.spurious),
            Self::rate(self.onset_inside, self.found),
            Self::rate(self.pitch_inside, self.found),
            Self::rate(self.gate_inside, self.found),
            Self::rate(self.tail_inside, self.found),
            Self::rate(self.inside_confident, self.inside_total),
            self.violations.len(),
            self.accepted_violations.len(),
            self.refused,
        )
    }

    /// Every Dc1-9 bound this measurement misses.
    fn misses(&self) -> Vec<String> {
        let mut misses = Vec::new();
        let check = |misses: &mut Vec<String>, name: &str, value: f64, ok: bool| {
            if !ok {
                misses.push(format!("{name} {value:.4}"));
            }
        };
        let d = Self::rate(self.found, self.truths);
        check(&mut misses, "detection < 0.95", d, d >= 0.95);
        let s = Self::rate(self.spurious, self.found + self.spurious);
        check(&mut misses, "spurious > 0.05", s, s <= 0.05);
        let o = Self::rate(self.onset_inside, self.found);
        check(&mut misses, "onset < 0.95", o, o >= 0.95);
        let p = Self::rate(self.pitch_inside, self.found);
        check(&mut misses, "pitch < 0.98", p, p >= 0.98);
        let g = Self::rate(self.gate_inside, self.found);
        check(&mut misses, "gate < 0.90", g, g >= 0.90);
        let t = Self::rate(self.tail_inside, self.found);
        check(&mut misses, "tail < 0.90", t, t >= 0.90);
        let c = Self::rate(self.inside_confident, self.inside_total);
        check(&mut misses, "confident inside < 0.90", c, c >= 0.90);
        for v in &self.violations {
            misses.push(format!("confident outside tolerance: {v}"));
        }
        misses
    }
}

fn pitch_inside(truth: &Note, note: &EstimatedNote) -> bool {
    note.midi_note == truth.2 && cents(note.f0_hz, truth.2).abs() <= PITCH_TOLERANCE_CENTS
}

/// Scores one phrase into `rates`, as the reference harness matches and counts.
fn score(
    rates: &mut Rates,
    label: &str,
    notes: &[Note],
    tails: &[u64],
    estimate: &Result<(bool, Vec<EstimatedNote>), String>,
) {
    rates.truths += notes.len();
    let (accepted, estimated): (bool, &[EstimatedNote]) = match estimate {
        Ok((accepted, notes)) => (*accepted, notes),
        Err(error) => {
            rates.refused.push(format!("{label}: {error}"));
            (false, &[])
        }
    };
    rates.accepted += usize::from(accepted);
    let mut candidates: Vec<(u64, usize, usize)> = Vec::new();
    for (t, truth) in notes.iter().enumerate() {
        for (e, note) in estimated.iter().enumerate() {
            let distance = note.start_sample.abs_diff(truth.0);
            if distance <= MATCH_WINDOW {
                candidates.push((distance, t, e));
            }
        }
    }
    candidates.sort_unstable();
    let mut truth_taken = vec![None; notes.len()];
    let mut estimate_taken = vec![false; estimated.len()];
    for (_, t, e) in candidates {
        if truth_taken[t].is_none() && !estimate_taken[e] {
            truth_taken[t] = Some(e);
            estimate_taken[e] = true;
        }
    }
    for (t, truth) in notes.iter().enumerate() {
        let Some(e) = truth_taken[t] else { continue };
        let f = &estimated[e];
        rates.found += 1;
        for (quantity, inside, confidence, threshold, error) in [
            (
                "onset",
                f.start_sample.abs_diff(truth.0) <= ONSET_TOLERANCE,
                f.onset_confidence,
                ONSET_CONFIDENCE_THRESHOLD,
                format!("{}", f.start_sample as i64 - truth.0 as i64),
            ),
            (
                "pitch",
                pitch_inside(truth, f),
                f.pitch_confidence,
                PITCH_CONFIDENCE_THRESHOLD,
                format!("MIDI {} for {}", f.midi_note, truth.2),
            ),
            (
                "gate",
                f.end_sample.abs_diff(truth.1) <= GATE_TOLERANCE,
                f.gate_confidence,
                GATE_CONFIDENCE_THRESHOLD,
                format!("{}", f.end_sample as i64 - truth.1 as i64),
            ),
            (
                "tail",
                f.tail_end_sample.abs_diff(tails[t]) <= TAIL_TOLERANCE,
                f.tail_confidence,
                TAIL_CONFIDENCE_THRESHOLD,
                format!("{}", f.tail_end_sample as i64 - tails[t] as i64),
            ),
        ] {
            match quantity {
                "onset" => rates.onset_inside += usize::from(inside),
                "pitch" => rates.pitch_inside += usize::from(inside),
                "gate" => rates.gate_inside += usize::from(inside),
                _ => rates.tail_inside += usize::from(inside),
            }
            let confident = confidence >= threshold;
            if inside {
                rates.inside_total += 1;
                rates.inside_confident += usize::from(confident);
            } else if confident {
                let v = format!(
                    "{label} note {t} {truth:?}: {quantity} error {error}, confidence {confidence:.3}"
                );
                if accepted {
                    rates.accepted_violations.push(v.clone());
                }
                rates.violations.push(v);
            }
        }
    }
    for (e, note) in estimated.iter().enumerate() {
        if !estimate_taken[e] {
            rates.spurious += 1;
            if note.onset_confidence >= ONSET_CONFIDENCE_THRESHOLD {
                let v = format!(
                    "{label}: spurious note at {} with onset confidence {:.3}",
                    note.start_sample, note.onset_confidence
                );
                if accepted {
                    rates.accepted_violations.push(v.clone());
                }
                rates.violations.push(v);
            }
        }
    }
}

/// One phrase's truth notes, truth tails and estimation.
type Slot = (
    Vec<Note>,
    Vec<u64>,
    Result<(bool, Vec<EstimatedNote>), String>,
);

/// Draws `phrases` phrases of `shape` from `seed`, estimates each on its own thread, and scores
/// them.
fn measure(shape: Shape, seed: u64, phrases: usize) -> Rates {
    let mut rng = Rng(Xoshiro256StarStar::new(seed));
    let drawn: Vec<(Voice, Vec<Note>)> = (0..phrases)
        .map(|_| draw_phrase(&mut rng, &shape))
        .collect();
    let mut slots: Vec<Option<Slot>> = (0..phrases).map(|_| None).collect();
    std::thread::scope(|scope| {
        for (slot, (voice, notes)) in slots.iter_mut().zip(drawn) {
            scope.spawn(move || {
                let samples = render_notes(voice, &notes);
                let truth_tails = tails(voice, &notes);
                *slot = Some((notes, truth_tails, estimate(&samples)));
            });
        }
    });
    let mut rates = Rates::default();
    for (p, slot) in slots.into_iter().enumerate() {
        let (notes, truth_tails, estimate) = slot.unwrap();
        score(
            &mut rates,
            &format!("seed {seed:#x} phrase {p}"),
            &notes,
            &truth_tails,
            &estimate,
        );
    }
    rates
}

/// Exploration: `ADV_SHAPE` names a shape below, `ADV_SEEDS` a comma-separated seed list,
/// `ADV_PHRASES` the phrases per seed. Prints the rates; asserts nothing.
#[test]
#[ignore = "exploration probe; run with --ignored and ADV_* variables"]
fn probe_rates() {
    let shape_name = std::env::var("ADV_SHAPE").unwrap_or_else(|_| "reference".into());
    let shape = shape_named(&shape_name);
    let phrases: usize = std::env::var("ADV_PHRASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(32);
    let seeds: Vec<u64> = std::env::var("ADV_SEEDS")
        .unwrap_or_else(|_| "1".into())
        .split(',')
        .map(|s| {
            let s = s.trim();
            s.strip_prefix("0x").map_or_else(
                || s.parse().unwrap(),
                |h| u64::from_str_radix(h, 16).unwrap(),
            )
        })
        .collect();
    for seed in seeds {
        let rates = measure(shape, seed, phrases);
        println!("{}", rates.summary(&format!("{shape_name} seed {seed:#x}")));
        for v in &rates.violations {
            println!("  violation: {v}");
        }
        for m in rates.misses() {
            println!("  miss: {m}");
        }
    }
}

/// Exploration: prints one phrase's voice, truth and estimate (`ADV_SHAPE`, `ADV_SEEDS` with one
/// seed, `ADV_PHRASE`).
#[test]
#[ignore = "exploration probe; run with --ignored and ADV_* variables"]
fn probe_phrase() {
    let shape = shape_named(&std::env::var("ADV_SHAPE").unwrap_or_else(|_| "reference".into()));
    let seed: u64 = std::env::var("ADV_SEEDS").unwrap().parse().unwrap();
    let index: usize = std::env::var("ADV_PHRASE").unwrap().parse().unwrap();
    let (voice, notes) = phrase(shape, seed, index);
    println!("voice {voice:?}");
    println!("notes {notes:?}");
    println!("tails {:?}", tails(voice, &notes));
    match estimate(&render_notes(voice, &notes)) {
        Ok((accepted, est)) => {
            println!("accepted {accepted}");
            for n in est {
                println!("  {n:?}");
            }
        }
        Err(e) => println!("refused {e}"),
    }
    for (i, note) in notes.iter().enumerate() {
        let alone = render_notes(voice, &[*note]);
        let pair = notes
            .get(i + 1)
            .map(|next| render_notes(voice, &[*note, *next]));
        println!(
            "  note {i} alone: {:?}; with the next: {:?}",
            estimate(&alone).map(|(a, n)| (a, n.len())),
            pair.map(|p| estimate(&p).map(|(a, n)| (a, n.len())))
        );
    }
}

/// Phrase `index` of `shape` drawn from `seed`.
fn phrase(shape: Shape, seed: u64, index: usize) -> (Voice, Vec<Note>) {
    let mut rng = Rng(Xoshiro256StarStar::new(seed));
    (0..=index)
        .map(|_| draw_phrase(&mut rng, &shape))
        .last()
        .unwrap()
}

fn shape_named(name: &str) -> Shape {
    match name {
        "reference" => REFERENCE,
        "short" => Shape {
            gate_ms: (45.0, 100.0),
            ..REFERENCE
        },
        "legato-repeat" => Shape {
            attack_ms: (1.0, 20.0),
            repeat: 0.5,
            repeat_zero_gap: true,
            ..REFERENCE
        },
        "low" => Shape {
            midi: (36, 43),
            ..REFERENCE
        },
        "high" => Shape {
            midi: (77, 84),
            ..REFERENCE
        },
        "quiet" => Shape {
            velocity: (0.1, 1.0),
            ..REFERENCE
        },
        "long-release" => Shape {
            release_ms: (300.0, 1_500.0),
            ..REFERENCE
        },
        other => panic!("unknown shape {other}"),
    }
}

/// Scores the listed phrases of `shape`, each `(seed, phrase index)`, one thread a phrase.
fn measure_phrases(shape: Shape, list: &[(u64, usize)]) -> Rates {
    let mut slots: Vec<Option<Slot>> = list.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        for (slot, &(seed, index)) in slots.iter_mut().zip(list) {
            scope.spawn(move || {
                let (voice, notes) = phrase(shape, seed, index);
                let samples = render_notes(voice, &notes);
                let truth_tails = tails(voice, &notes);
                *slot = Some((notes, truth_tails, estimate(&samples)));
            });
        }
    });
    let mut rates = Rates::default();
    for (slot, &(seed, index)) in slots.into_iter().zip(list) {
        let (notes, truth_tails, estimate) = slot.unwrap();
        score(
            &mut rates,
            &format!("seed {seed} phrase {index}"),
            &notes,
            &truth_tails,
            &estimate,
        );
    }
    rates
}

/// Scores one constructed phrase.
fn measure_one(voice: Voice, notes: &[Note], samples: &[f32]) -> Rates {
    let mut rates = Rates::default();
    score(
        &mut rates,
        "constructed",
        notes,
        &tails(voice, notes),
        &estimate(samples),
    );
    rates
}

/// Phrases of the reference generator itself (`estimate_reference.rs` `draw_phrase`, its 32
/// phrases a seed) on seeds other than the pinned 0x5eed_0c19, where the probe found a confident
/// quantity outside its tolerance. On the pinned seed every Dc1-9 rate measures exactly 1.
const REFERENCE_PHRASES: [(u64, usize); 10] = [
    (1, 7),
    (3, 6),
    (4, 12),
    (6, 20),
    (6, 31),
    (7, 10),
    (7, 17),
    (9, 12),
    (11, 20),
    (12, 28),
];

/// Dc1-9 calibration: "no quantity outside its tolerance carries a confidence at or above its
/// threshold", measured as `estimate_reference.rs` measures it (refused-for-low-confidence notes
/// count), on the reference generator's own phrases from unpinned seeds.
#[test]
fn reference_generator_phrases_on_unpinned_seeds_carry_no_confident_quantity_outside_tolerance() {
    let rates = measure_phrases(REFERENCE, &REFERENCE_PHRASES);
    println!("{}", rates.summary("reference phrases"));
    assert!(
        rates.violations.is_empty(),
        "{} confident quantities outside tolerance: {:#?}",
        rates.violations.len(),
        rates.violations
    );
}

/// The same contract on the estimations the estimator *accepts*: these are what `patchback
/// estimate` writes to `estimation.txt` and `notes.events`.
#[test]
fn an_accepted_estimation_of_a_reference_phrase_carries_no_confident_quantity_outside_tolerance() {
    let rates = measure_phrases(REFERENCE, &REFERENCE_PHRASES);
    println!("{}", rates.summary("reference phrases"));
    assert!(
        rates.accepted_violations.is_empty(),
        "accepted with confident quantities outside tolerance: {:#?}",
        rates.accepted_violations
    );
}

/// Dc1-3: a monophonic phrase of the subtractive voice is never refused as polyphonic. Both
/// phrases are the reference generator's (seed 8 phrase 8, seed 9 phrase 30), each holding a
/// MIDI 36 note; refusing them costs the seed its detection rate (0.9452 and 0.9459 < 0.95).
#[test]
fn monophonic_reference_phrases_are_not_refused_as_polyphonic() {
    let mut refused = Vec::new();
    for (seed, index) in [(8u64, 8usize), (9, 30)] {
        let (voice, notes) = phrase(REFERENCE, seed, index);
        if let Err(EstimationError::PolyphonicInput) =
            estimate_notes(&render_notes(voice, &notes), 48_000)
        {
            refused.push(format!("seed {seed} phrase {index}: {notes:?}"));
        }
    }
    assert!(refused.is_empty(), "refused as polyphonic: {refused:#?}");
}

/// Dc1-3 on one note: a single saw at MIDI 36 to 40 with a gate of 105 ms (inside the reference
/// generator's 100 to 400 ms) is not polyphonic.
#[test]
fn a_single_low_saw_note_of_105_ms_is_not_refused_as_polyphonic() {
    let voice = Voice {
        attack: 0.005,
        decay: 0.08,
        sustain: 0.7,
        release: 0.05,
        cutoff: 2_000.0,
        q: 0.8,
        drive: 6.0,
        gain: -6.0,
    };
    let mut refused = Vec::new();
    for midi in 36u8..=40 {
        let samples = render_notes(voice, &[(4_800, 4_800 + 5_040, midi, 0.8)]);
        if let Err(EstimationError::PolyphonicInput) = estimate_notes(&samples, 48_000) {
            refused.push(midi);
        }
    }
    assert!(
        refused.is_empty(),
        "single notes refused as polyphonic at MIDI {refused:?}"
    );
}

/// Dc1-9 allows a zero gap with an attack of 20 ms or less, and includes repeated same-pitch
/// notes; the reference generator never combines them. A same-pitch retrigger from a sustain of
/// 0.5 is a 6 dB rise at the true onset.
#[test]
fn a_same_pitch_retrigger_with_no_gap_is_within_tolerance_or_not_confident() {
    let voice = Voice {
        attack: 0.005,
        decay: 0.06,
        sustain: 0.5,
        release: 0.03,
        cutoff: 3_000.0,
        q: 0.8,
        drive: 3.0,
        gain: -6.0,
    };
    let notes: [Note; 3] = [
        (4_800, 19_200, 57, 0.8),
        (19_200, 33_600, 57, 0.8),
        (33_600, 48_000, 57, 0.8),
    ];
    let rates = measure_one(voice, &notes, &render_notes(voice, &notes));
    println!("{}", rates.summary("same-pitch retrigger"));
    assert_eq!(rates.found, 3, "{}", rates.summary("same-pitch retrigger"));
    assert!(rates.violations.is_empty(), "{:#?}", rates.violations);
}

/// The same with the probe's legato-repeat shape (zero-gap same-pitch repeats allowed), phrases
/// where the probe found onsets 1,386 to 1,672 samples early at confidence 1.
#[test]
fn zero_gap_same_pitch_repeats_carry_no_confident_quantity_outside_tolerance() {
    let rates = measure_phrases(shape_named("legato-repeat"), &[(1, 9), (1, 18), (1, 28)]);
    println!("{}", rates.summary("legato-repeat phrases"));
    assert!(
        rates.violations.is_empty(),
        "{} confident quantities outside tolerance: {:#?}",
        rates.violations.len(),
        rates.violations
    );
}

/// Releases of 300 ms to 1.5 s (pads; Dc1-9 says "varied ADSR" and fixes no release range,
/// the reference generator draws 10 to 300 ms) overlapping the next onset.
#[test]
fn long_releases_carry_no_confident_quantity_outside_tolerance() {
    let rates = measure_phrases(shape_named("long-release"), &[(1, 14), (1, 19), (1, 30)]);
    println!("{}", rates.summary("long-release phrases"));
    assert!(
        rates.violations.is_empty(),
        "{} confident quantities outside tolerance: {:#?}",
        rates.violations.len(),
        rates.violations
    );
}

/// The one-sided change point: a legato octave down and an octave up (one period a whole
/// multiple of the other) with no gap.
#[test]
fn a_legato_octave_change_is_within_tolerance_or_not_confident() {
    let voice = Voice {
        attack: 0.002,
        decay: 0.03,
        sustain: 0.8,
        release: 0.02,
        cutoff: 1_500.0,
        q: 1.0,
        drive: 12.0,
        gain: -5.0,
    };
    let mut violations = Vec::new();
    for (a, b) in [(58u8, 46u8), (46, 58), (69, 57), (57, 69)] {
        let notes: [Note; 2] = [(5_939, 22_528, a, 0.7), (22_528, 39_485, b, 0.85)];
        let rates = measure_one(voice, &notes, &render_notes(voice, &notes));
        println!("{}", rates.summary(&format!("octave {a} -> {b}")));
        violations.extend(
            rates
                .violations
                .into_iter()
                .map(|v| format!("{a} -> {b}: {v}")),
        );
    }
    assert!(violations.is_empty(), "{violations:#?}");
}

/// Three notes of the plain voice (`tests/estimate.rs` `PHRASE`, which the estimator accepts)
/// under a white noise floor at -70 dBFS RMS: 10 dB below the silence threshold, as an external
/// recording carries.
#[test]
fn a_phrase_over_a_noise_floor_10_db_below_the_silence_threshold_is_estimated() {
    let notes: [Note; 3] = [
        (4_800, 19_200, 45, 0.9),
        (20_160, 34_560, 52, 0.7),
        (34_560, 52_800, 57, 0.8),
    ];
    let clean = render_notes(voice::PLAIN, &notes);
    assert!(
        estimate_notes(&clean, 48_000).is_ok(),
        "the clean phrase is accepted"
    );
    let mut rng = Xoshiro256StarStar::new(0x00ad_015e);
    let rms = 10f64.powf(-70.0 / 20.0);
    let noisy: Vec<f32> = clean
        .iter()
        .map(|s| {
            let u = rng.next_f64() * 2.0 - 1.0;
            s + (u * rms * 3f64.sqrt()) as f32
        })
        .collect();
    match estimate_notes(&noisy, 48_000) {
        Ok(e) => assert_eq!(e.notes.len(), 3, "{:#?}", e.notes),
        Err(error) => panic!("refused: {error}"),
    }
}

/// Dc1-3: two simultaneous tones a minor second or a fifth apart, the upper 12 dB below the
/// lower (an amplitude ratio of 1:4), are two tones and are refused as polyphonic.
#[test]
fn a_dyad_whose_second_tone_is_12_db_down_is_refused_as_polyphonic() {
    let voice = Voice {
        attack: 0.01,
        decay: 0.1,
        sustain: 0.8,
        release: 0.05,
        cutoff: 5_000.0,
        q: 0.7,
        drive: 0.0,
        gain: -3.0,
    };
    let mut accepted = Vec::new();
    for root in [40u8, 52, 64] {
        for interval in [1u8, 7] {
            let low = render_notes(voice, &[(4_800, 33_600, root, 0.8)]);
            let high = render_notes(voice, &[(4_800, 33_600, root + interval, 0.8)]);
            let mix: Vec<f32> = low
                .iter()
                .zip(&high)
                .map(|(a, b)| 0.6 * a + 0.15 * b)
                .collect();
            match estimate_notes(&mix, 48_000) {
                Err(EstimationError::PolyphonicInput) => {}
                Ok(e) => accepted.push(format!(
                    "MIDI {root} + {interval}: accepted as {:?}",
                    e.notes.iter().map(|n| n.midi_note).collect::<Vec<_>>()
                )),
                Err(other) => accepted.push(format!(
                    "MIDI {root} + {interval}: {}",
                    other.domain_error()
                )),
            }
        }
    }
    assert!(
        accepted.is_empty(),
        "not refused as polyphonic: {accepted:#?}"
    );
}

/// Dc1-3 lets an octave go undetected. What is reported then must not be a confident wrong
/// pitch: the sum of two saws an octave apart has the lower tone's period.
#[test]
fn an_undetected_octave_dyad_reports_the_lower_pitch_or_is_not_confident() {
    let voice = Voice {
        attack: 0.01,
        decay: 0.1,
        sustain: 0.8,
        release: 0.05,
        cutoff: 5_000.0,
        q: 0.7,
        drive: 0.0,
        gain: -3.0,
    };
    let mut wrong = Vec::new();
    for root in [40u8, 52, 64] {
        for (low_gain, high_gain) in [(0.5f32, 0.5f32), (0.2, 0.8)] {
            let low = render_notes(voice, &[(4_800, 33_600, root, 0.8)]);
            let high = render_notes(voice, &[(4_800, 33_600, root + 12, 0.8)]);
            let mix: Vec<f32> = low
                .iter()
                .zip(&high)
                .map(|(a, b)| low_gain * a + high_gain * b)
                .collect();
            let outcome = estimate(&mix);
            println!("octave on {root} at {low_gain}/{high_gain}: {outcome:?}");
            if let Ok((_, notes)) = outcome {
                for n in notes {
                    if n.midi_note != root && n.pitch_confidence >= PITCH_CONFIDENCE_THRESHOLD {
                        wrong.push(format!(
                            "octave on {root} at {low_gain}/{high_gain}: MIDI {} at confidence {:.3}",
                            n.midi_note, n.pitch_confidence
                        ));
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// The reference harness (`estimate_reference.rs`) run unchanged on seed 8 instead of the pinned
/// seed: Dc1-9 detection is at least 95 % of notes.
#[test]
fn the_reference_generator_on_seed_8_finds_at_least_95_percent_of_notes() {
    let rates = measure(REFERENCE, 8, 32);
    println!("{}", rates.summary("reference seed 8"));
    let detection = Rates::rate(rates.found, rates.truths);
    assert!(
        detection >= 0.95,
        "detection {detection:.4}; refused {:?}",
        rates.refused
    );
}
