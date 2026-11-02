//! The reference generator of approval-record:wave-c1 Dc1-9 and Dc1-12 and its scoring, shared by
//! `estimate_reference.rs` (the reference distribution) and `estimate_stress.rs` (Dc1-13).
//!
//! A phrase is 2 to 8 notes of the subtractive voice (`voice::render_notes`), MIDI 36 to 84,
//! with the ADSR, cutoff, resonance, drive and gain drawn per phrase. The gap between two notes
//! is 0 to 200 ms: a zero gap, with probability 0.3, only in a phrase whose attack is 20 ms or
//! less, whether or not the two notes share a pitch; otherwise uniform. A note repeats the
//! previous pitch with probability 0.2. A [`Shape`] moves one range for a stress shape (Dc1-13,
//! Dc1-20), or adds a white noise floor, uniform or Gaussian.
//!
//! The truth of a note is its event: the onset is its start, the gate end its end. Its tail end
//! (Dc1-21) is where its level falls below the silence threshold for good
//! ([`SILENCE_THRESHOLD_DBFS`], the RMS of the 1,024 samples centred on a sample): one past the
//! last sample at or above the threshold in a render of the phrase up to and including that note,
//! without noise, even when that is before the gate end; or the next note's start when that comes
//! first.
//!
//! An estimated note is matched to the true note whose onset is nearest, within
//! [`MATCH_WINDOW`] samples, each true note matched at most once, in order of distance. Rates
//! count the notes the estimator returns whether it accepts the phrase or refuses it as
//! `LowConfidenceNotes` (whose refusal carries every note). A phrase refused otherwise finds none
//! of its notes. A spurious note's onset is a quantity outside its tolerance.

#![allow(dead_code)]

use patchback_analysis::{
    EstimatedNote, EstimationError, GATE_CONFIDENCE_THRESHOLD, ONSET_CONFIDENCE_THRESHOLD,
    PITCH_CONFIDENCE_THRESHOLD, SILENCE_THRESHOLD_DBFS, TAIL_CONFIDENCE_THRESHOLD, estimate_notes,
};
use patchback_core::prng::Xoshiro256StarStar;

use crate::voice::{Note, Voice, centred_rms, cents, pool_map, render_notes};

pub const PHRASES: usize = 32;
pub const MS: f64 = 48.0;
pub const ONSET_TOLERANCE: u64 = 256;
pub const GATE_TOLERANCE: u64 = 1_024;
pub const TAIL_TOLERANCE: u64 = 2_400;
pub const PITCH_TOLERANCE_CENTS: f64 = 10.0;
pub const MATCH_WINDOW: u64 = 2_400;

pub struct Rng(pub Xoshiro256StarStar);

impl Rng {
    pub fn unit(&mut self) -> f64 {
        self.0.next_f64()
    }
    pub fn uniform(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }
    pub fn log_uniform(&mut self, low: f64, high: f64) -> f64 {
        (low.ln() + (high.ln() - low.ln()) * self.unit()).exp()
    }
    pub fn int(&mut self, low: u64, high: u64) -> u64 {
        low + (self.0.next_u64() % (high - low + 1))
    }
}

/// The generator's ranges.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub name: &'static str,
    pub attack_ms: (f64, f64),
    pub decay_ms: (f64, f64),
    pub sustain: (f64, f64),
    pub release_ms: (f64, f64),
    pub cutoff_hz: (f64, f64),
    pub drive_db: (f64, f64),
    pub gate_ms: (f64, f64),
    /// The gap when it is not a zero gap.
    pub gap_ms: (f64, f64),
    pub velocity: (f64, f64),
    pub midi: (u64, u64),
    /// The probability that a note repeats the previous pitch.
    pub repeat: f64,
    /// The probability of a zero gap where the attack allows one.
    pub zero_gap: f64,
    /// A uniform white noise floor at this RMS in dBFS, if any.
    pub noise_dbfs: Option<f64>,
    /// A Gaussian white noise floor at this RMS in dBFS, if any.
    pub gaussian_dbfs: Option<f64>,
}

/// The reference distribution (Dc1-9, Dc1-12).
pub const REFERENCE: Shape = Shape {
    name: "reference",
    attack_ms: (1.0, 40.0),
    decay_ms: (20.0, 300.0),
    sustain: (0.3, 1.0),
    release_ms: (10.0, 300.0),
    cutoff_hz: (800.0, 8_000.0),
    drive_db: (0.0, 18.0),
    gate_ms: (100.0, 400.0),
    gap_ms: (0.0, 200.0),
    velocity: (0.5, 1.0),
    midi: (36, 84),
    repeat: 0.2,
    zero_gap: 0.3,
    noise_dbfs: None,
    gaussian_dbfs: None,
};

/// The stress shapes of Dc1-13 and Dc1-20, by name.
pub const STRESS_SHAPES: [&str; 12] = [
    "long-release",
    "short-gate",
    "low",
    "quiet",
    "zero-gap-repeat",
    "noise-floor",
    "gaussian-noise-floor",
    "dark-cutoff",
    "low-sustain",
    "short-gap",
    "high-drive",
    "long-decay",
];

/// The stress shape `name` (one of [`STRESS_SHAPES`]).
pub fn stress_shape(name: &str) -> Shape {
    match name {
        // Dc1-13.
        "long-release" => Shape {
            name: "long-release",
            release_ms: (300.0, 1_500.0),
            ..REFERENCE
        },
        "short-gate" => Shape {
            name: "short-gate",
            gate_ms: (45.0, 100.0),
            ..REFERENCE
        },
        "low" => Shape {
            name: "low",
            midi: (36, 43),
            ..REFERENCE
        },
        "quiet" => Shape {
            name: "quiet",
            velocity: (0.1, 1.0),
            ..REFERENCE
        },
        "zero-gap-repeat" => Shape {
            name: "zero-gap-repeat",
            attack_ms: (1.0, 20.0),
            repeat: 0.5,
            zero_gap: 0.5,
            ..REFERENCE
        },
        "noise-floor" => Shape {
            name: "noise-floor",
            noise_dbfs: Some(-70.0),
            ..REFERENCE
        },
        // Dc1-20.
        "gaussian-noise-floor" => Shape {
            name: "gaussian-noise-floor",
            gaussian_dbfs: Some(-70.0),
            ..REFERENCE
        },
        "dark-cutoff" => Shape {
            name: "dark-cutoff",
            cutoff_hz: (100.0, 800.0),
            ..REFERENCE
        },
        "low-sustain" => Shape {
            name: "low-sustain",
            sustain: (0.0, 0.3),
            ..REFERENCE
        },
        "short-gap" => Shape {
            name: "short-gap",
            gap_ms: (0.5, 10.0),
            zero_gap: 0.0,
            ..REFERENCE
        },
        "high-drive" => Shape {
            name: "high-drive",
            drive_db: (18.0, 24.0),
            ..REFERENCE
        },
        "long-decay" => Shape {
            name: "long-decay",
            decay_ms: (300.0, 2_000.0),
            ..REFERENCE
        },
        other => panic!("unknown shape {other}"),
    }
}

/// One drawn phrase: its voice and notes.
pub fn draw_phrase(rng: &mut Rng, shape: &Shape) -> (Voice, Vec<Note>) {
    let attack = rng.log_uniform(shape.attack_ms.0, shape.attack_ms.1) / 1_000.0;
    let voice = Voice {
        attack,
        decay: rng.log_uniform(shape.decay_ms.0, shape.decay_ms.1) / 1_000.0,
        sustain: rng.uniform(shape.sustain.0, shape.sustain.1),
        release: rng.log_uniform(shape.release_ms.0, shape.release_ms.1) / 1_000.0,
        cutoff: rng.log_uniform(shape.cutoff_hz.0, shape.cutoff_hz.1),
        q: rng.uniform(0.5, 1.5),
        drive: rng.uniform(shape.drive_db.0, shape.drive_db.1),
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
                if Some(m) != previous {
                    break m;
                }
            },
        };
        if let Some(&(_, end, _, _)) = notes.last() {
            let gap_ms = if attack <= 0.020 && rng.unit() < shape.zero_gap {
                0.0
            } else {
                rng.uniform(shape.gap_ms.0, shape.gap_ms.1)
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

/// The sample at or after `from` where the centred RMS first falls below the silence threshold.
pub fn silence_crossing(samples: &[f32], from: usize) -> usize {
    let threshold = 10f64.powf(SILENCE_THRESHOLD_DBFS / 20.0);
    (from..samples.len())
        .find(|&t| centred_rms(samples, t) < threshold)
        .unwrap_or(samples.len())
}

/// One past the last sample from `from` whose centred RMS is at or above the silence threshold
/// (Dc1-21), or `from` when there is none.
pub fn last_above(samples: &[f32], from: usize) -> usize {
    let threshold = 10f64.powf(SILENCE_THRESHOLD_DBFS / 20.0);
    (from..samples.len())
        .rev()
        .find(|&t| centred_rms(samples, t) >= threshold)
        .map_or(from, |t| t + 1)
}

/// The true tail ends of a phrase's notes (Dc1-21): where the note's level falls below the
/// silence threshold for good, even before its gate end, or the next note's start when that
/// comes first.
pub fn tails(voice: Voice, notes: &[Note]) -> Vec<u64> {
    (0..notes.len())
        .map(|i| {
            let prefix = render_notes(voice, &notes[..=i]);
            let end = last_above(&prefix, notes[i].0 as usize) as u64;
            notes.get(i + 1).map_or(end, |next| end.min(next.0))
        })
        .collect()
}

/// The phrase's samples, with the shape's noise floor added (its own seeded noise): uniform, or
/// Gaussian (Box-Muller).
pub fn samples(voice: Voice, notes: &[Note], shape: &Shape, noise_seed: u64) -> Vec<f32> {
    let clean = render_notes(voice, notes);
    let mut rng = Xoshiro256StarStar::new(noise_seed);
    if let Some(dbfs) = shape.noise_dbfs {
        let amplitude = 10f64.powf(dbfs / 20.0) * 3f64.sqrt();
        return clean
            .iter()
            .map(|s| s + ((rng.next_f64() * 2.0 - 1.0) * amplitude) as f32)
            .collect();
    }
    if let Some(dbfs) = shape.gaussian_dbfs {
        let sigma = 10f64.powf(dbfs / 20.0);
        return clean
            .iter()
            .map(|s| {
                let u1 = rng.next_f64().max(1e-300);
                let u2 = rng.next_f64();
                let g = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
                s + (sigma * g) as f32
            })
            .collect();
    }
    clean
}

/// What the estimator returned for a phrase.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// Accepted (`true`) or refused as `LowConfidenceNotes` (`false`), with every note.
    Notes(bool, Vec<EstimatedNote>),
    /// Refused otherwise: the domain error.
    Refused(String),
}

pub fn estimate(samples: &[f32]) -> Outcome {
    match estimate_notes(samples, 48_000) {
        Ok(e) => Outcome::Notes(true, e.notes),
        Err(EstimationError::LowConfidenceNotes { notes }) => Outcome::Notes(false, notes),
        Err(error) => Outcome::Refused(error.domain_error().to_owned()),
    }
}

/// One measured phrase.
pub struct Measured {
    pub seed: u64,
    pub index: usize,
    pub voice: Voice,
    pub notes: Vec<Note>,
    pub tails: Vec<u64>,
    pub outcome: Outcome,
}

/// Draws `phrases` phrases of `shape` from each seed and estimates them, spread over the
/// process's worker pool ([`pool_map`]). In seed order, then phrase order.
pub fn measure(shape: Shape, seeds: &[u64], phrases: usize) -> Vec<Measured> {
    let mut jobs = Vec::new();
    for &seed in seeds {
        let mut rng = Rng(Xoshiro256StarStar::new(seed));
        for index in 0..phrases {
            let (voice, notes) = draw_phrase(&mut rng, &shape);
            jobs.push((seed, index, voice, notes));
        }
    }
    pool_map(&jobs, |(seed, index, voice, notes)| {
        let noise_seed = seed.wrapping_mul(1_000).wrapping_add(*index as u64);
        let outcome = estimate(&samples(*voice, notes, &shape, noise_seed));
        Measured {
            seed: *seed,
            index: *index,
            voice: *voice,
            notes: notes.clone(),
            tails: tails(*voice, notes),
            outcome,
        }
    })
}

pub fn pitch_inside(truth: &Note, note: &EstimatedNote) -> bool {
    note.midi_note == truth.2 && cents(note.f0_hz, truth.2).abs() <= PITCH_TOLERANCE_CENTS
}

/// Counts over a set of phrases.
#[derive(Default, Debug, Clone)]
pub struct Rates {
    pub phrases: usize,
    pub accepted: usize,
    pub truths: usize,
    pub found: usize,
    pub spurious: usize,
    pub onset_inside: usize,
    pub pitch_exact: usize,
    pub pitch_inside: usize,
    pub gate_inside: usize,
    pub tail_inside: usize,
    pub inside_total: usize,
    pub inside_confident: usize,
    /// In-tolerance quantities below their threshold: onset, pitch, gate, tail.
    pub unconfident_inside: [usize; 4],
    pub refused: Vec<String>,
    pub polyphonic: Vec<String>,
    /// Confident quantities outside tolerance (a confident spurious onset included).
    pub violations: Vec<String>,
    /// Every quantity outside tolerance, confident or not, for diagnosis.
    pub misses: Vec<String>,
}

pub fn rate(a: usize, b: usize) -> f64 {
    if b == 0 { 0.0 } else { a as f64 / b as f64 }
}

impl Rates {
    pub fn add(&mut self, other: &Rates) {
        self.phrases += other.phrases;
        self.accepted += other.accepted;
        self.truths += other.truths;
        self.found += other.found;
        self.spurious += other.spurious;
        self.onset_inside += other.onset_inside;
        self.pitch_exact += other.pitch_exact;
        self.pitch_inside += other.pitch_inside;
        self.gate_inside += other.gate_inside;
        self.tail_inside += other.tail_inside;
        self.inside_total += other.inside_total;
        self.inside_confident += other.inside_confident;
        for (a, b) in self
            .unconfident_inside
            .iter_mut()
            .zip(other.unconfident_inside)
        {
            *a += b;
        }
        self.refused.extend(other.refused.iter().cloned());
        self.polyphonic.extend(other.polyphonic.iter().cloned());
        self.violations.extend(other.violations.iter().cloned());
        self.misses.extend(other.misses.iter().cloned());
    }

    pub fn detection(&self) -> f64 {
        rate(self.found, self.truths)
    }
    pub fn spurious_rate(&self) -> f64 {
        rate(self.spurious, self.found + self.spurious)
    }
    pub fn onset(&self) -> f64 {
        rate(self.onset_inside, self.found)
    }
    pub fn pitch_exact_rate(&self) -> f64 {
        rate(self.pitch_exact, self.found)
    }
    pub fn pitch(&self) -> f64 {
        rate(self.pitch_inside, self.found)
    }
    pub fn gate(&self) -> f64 {
        rate(self.gate_inside, self.found)
    }
    pub fn tail(&self) -> f64 {
        rate(self.tail_inside, self.found)
    }
    pub fn confident_inside(&self) -> f64 {
        rate(self.inside_confident, self.inside_total)
    }

    /// One table row.
    pub fn row(&self, label: &str) -> String {
        format!(
            "{label:>14} | {:>3} {:>4} | {:>3} | {:.4} {:.4} | {:.4} {:.4} {:.4} {:.4} {:.4} | {:.4} | {:>2} | {}",
            self.phrases,
            self.truths,
            self.accepted,
            self.detection(),
            self.spurious_rate(),
            self.onset(),
            self.pitch_exact_rate(),
            self.pitch(),
            self.gate(),
            self.tail(),
            self.confident_inside(),
            self.violations.len(),
            self.refused.len(),
        )
    }

    pub const HEADER: &str = "          rows | phr notes | acc | detect spurio | onset  midi   pitch  gate   tail   | conf-in | viol | refused";

    /// Every Dc1-9 bound this set misses, as text.
    pub fn bound_misses(&self) -> Vec<String> {
        let mut misses = Vec::new();
        for (name, value, ok) in [
            (
                "detection < 0.95",
                self.detection(),
                self.detection() >= 0.95,
            ),
            (
                "spurious > 0.05",
                self.spurious_rate(),
                self.spurious_rate() <= 0.05,
            ),
            ("onset < 0.95", self.onset(), self.onset() >= 0.95),
            (
                "exact MIDI < 0.98",
                self.pitch_exact_rate(),
                self.pitch_exact_rate() >= 0.98,
            ),
            ("pitch < 0.98", self.pitch(), self.pitch() >= 0.98),
            ("gate < 0.90", self.gate(), self.gate() >= 0.90),
            ("tail < 0.90", self.tail(), self.tail() >= 0.90),
            (
                "confident inside < 0.90",
                self.confident_inside(),
                self.confident_inside() >= 0.90,
            ),
        ] {
            if !ok {
                misses.push(format!("{name}: {value:.4}"));
            }
        }
        misses
    }
}

/// Scores one measured phrase.
pub fn score(m: &Measured) -> Rates {
    let label = format!("seed {} phrase {}", m.seed, m.index);
    let mut rates = Rates {
        phrases: 1,
        truths: m.notes.len(),
        ..Rates::default()
    };
    let (accepted, estimated): (bool, &[EstimatedNote]) = match &m.outcome {
        Outcome::Notes(accepted, notes) => (*accepted, notes),
        Outcome::Refused(error) => {
            rates.refused.push(format!("{label}: {error}"));
            if error == "patchback.analysis.PolyphonicInput" {
                rates.polyphonic.push(label.clone());
            }
            (false, &[])
        }
    };
    rates.accepted = usize::from(accepted);
    let mut candidates: Vec<(u64, usize, usize)> = Vec::new();
    for (t, truth) in m.notes.iter().enumerate() {
        for (e, note) in estimated.iter().enumerate() {
            let distance = note.start_sample.abs_diff(truth.0);
            if distance <= MATCH_WINDOW {
                candidates.push((distance, t, e));
            }
        }
    }
    candidates.sort_unstable();
    let mut truth_taken = vec![None; m.notes.len()];
    let mut estimate_taken = vec![false; estimated.len()];
    for (_, t, e) in candidates {
        if truth_taken[t].is_none() && !estimate_taken[e] {
            truth_taken[t] = Some(e);
            estimate_taken[e] = true;
        }
    }
    for (t, truth) in m.notes.iter().enumerate() {
        let Some(e) = truth_taken[t] else {
            rates
                .misses
                .push(format!("{label} note {t} {truth:?}: missed"));
            continue;
        };
        let f = &estimated[e];
        rates.found += 1;
        rates.pitch_exact += usize::from(f.midi_note == truth.2);
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
                format!(
                    "MIDI {} for {} ({:.1} cents)",
                    f.midi_note,
                    truth.2,
                    cents(f.f0_hz, truth.2)
                ),
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
                f.tail_end_sample.abs_diff(m.tails[t]) <= TAIL_TOLERANCE,
                f.tail_confidence,
                TAIL_CONFIDENCE_THRESHOLD,
                format!("{}", f.tail_end_sample as i64 - m.tails[t] as i64),
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
                let slot = match quantity {
                    "onset" => 0,
                    "pitch" => 1,
                    "gate" => 2,
                    _ => 3,
                };
                rates.unconfident_inside[slot] += usize::from(!confident);
            } else {
                let text = format!(
                    "{label} note {t} {truth:?}: {quantity} error {error}, confidence {confidence:.3}"
                );
                if confident {
                    rates.violations.push(text.clone());
                }
                rates.misses.push(text);
            }
        }
    }
    for (e, note) in estimated.iter().enumerate() {
        if !estimate_taken[e] {
            rates.spurious += 1;
            let text = format!(
                "{label}: spurious note at {} with onset confidence {:.3}",
                note.start_sample, note.onset_confidence
            );
            if note.onset_confidence >= ONSET_CONFIDENCE_THRESHOLD {
                rates.violations.push(text.clone());
            }
            rates.misses.push(text);
        }
    }
    rates
}

/// The rates of each seed, in `seeds` order, and of all of them pooled.
pub fn rates_by_seed(measured: &[Measured], seeds: &[u64]) -> (Vec<(u64, Rates)>, Rates) {
    let mut by_seed: Vec<(u64, Rates)> = seeds.iter().map(|s| (*s, Rates::default())).collect();
    let mut pooled = Rates::default();
    for m in measured {
        let rates = score(m);
        pooled.add(&rates);
        if let Some((_, r)) = by_seed.iter_mut().find(|(s, _)| *s == m.seed) {
            r.add(&rates);
        }
    }
    (by_seed, pooled)
}

/// Prints the per-seed table and the pooled row.
pub fn print_table(title: &str, by_seed: &[(u64, Rates)], pooled: &Rates) {
    println!("{title}");
    println!("{}", Rates::HEADER);
    for (seed, rates) in by_seed {
        println!("{}", rates.row(&format!("seed {seed:#x}")));
    }
    println!("{}", pooled.row("pooled"));
}
