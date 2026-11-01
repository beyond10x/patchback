//! The numeric obligations of the note estimator (approval-record:wave-c1 Dc1-9) on the seeded
//! reference set: 32 phrases of 2 to 8 notes synthesized by the subtractive voice, MIDI 36 to 84,
//! gaps of 0 to 200 ms (a zero gap only in a phrase whose attack is 20 ms or less, and only
//! between notes of different pitch), repeated same-pitch notes included, and the ADSR, cutoff,
//! resonance, drive and gain drawn per phrase.
//!
//! The truth of each note is its event: the onset is its start, the gate end its end. Its tail
//! end is where the release falls below the silence threshold
//! ([`SILENCE_THRESHOLD_DBFS`], the RMS of the 1,024 samples centred on a sample) in a render of
//! the phrase up to and including that note, or the next note's start when that comes first.
//!
//! An estimated note is matched to the true note whose onset is nearest, within 2,400 samples,
//! each true note matched at most once, in order of distance. Rates are counted over the notes
//! the estimator returns whether it accepts the phrase or refuses it as `LowConfidenceNotes`
//! (whose refusal carries every note). A phrase refused otherwise finds none of its notes.
//!
//! Each test prints its measured rates (`cargo test -- --nocapture` shows them) and fails when a
//! rate misses its bound; a bound is never relaxed.

mod voice;

use std::sync::OnceLock;

use patchback_analysis::{
    EstimatedNote, EstimationError, GATE_CONFIDENCE_THRESHOLD, ONSET_CONFIDENCE_THRESHOLD,
    PITCH_CONFIDENCE_THRESHOLD, SILENCE_THRESHOLD_DBFS, TAIL_CONFIDENCE_THRESHOLD, estimate_notes,
};
use patchback_core::prng::Xoshiro256StarStar;
use voice::{Note, Voice, centred_rms, cents, render_notes};

const SEED: u64 = 0x5eed_0c19;
const PHRASES: usize = 32;
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

/// One phrase of the reference set and the truth of each of its notes.
struct Phrase {
    voice: Voice,
    notes: Vec<Note>,
    tails: Vec<u64>,
    samples: Vec<f32>,
}

fn draw_phrase(rng: &mut Rng) -> (Voice, Vec<Note>) {
    let attack = rng.log_uniform(1.0, 40.0) / 1_000.0;
    let voice = Voice {
        attack,
        decay: rng.log_uniform(20.0, 300.0) / 1_000.0,
        sustain: rng.uniform(0.3, 1.0),
        release: rng.log_uniform(10.0, 300.0) / 1_000.0,
        cutoff: rng.log_uniform(800.0, 8_000.0),
        q: rng.uniform(0.5, 1.5),
        drive: rng.uniform(0.0, 18.0),
        gain: rng.uniform(-12.0, -3.0),
    };
    let count = rng.int(2, 8) as usize;
    let mut notes = Vec::with_capacity(count);
    let mut start = (rng.uniform(50.0, 150.0) * MS) as u64;
    let mut previous: Option<u8> = None;
    for _ in 0..count {
        let repeat = previous.is_some() && rng.unit() < 0.2;
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
            let gap_ms = if repeat {
                rng.uniform(40.0, 200.0)
            } else if attack <= 0.020 && rng.unit() < 0.3 {
                0.0
            } else {
                rng.uniform(0.0, 200.0)
            };
            start = end + (gap_ms * MS) as u64;
        }
        let gate = (rng.uniform(100.0, 400.0) * MS) as u64;
        let velocity = rng.uniform(0.5, 1.0) as f32;
        notes.push((start, start + gate, midi, velocity));
        previous = Some(midi);
    }
    (voice, notes)
}

/// The sample at or after `from` where the centred RMS first falls below the silence threshold.
fn silence_crossing(samples: &[f32], from: usize) -> usize {
    let threshold = 10f64.powf(SILENCE_THRESHOLD_DBFS / 20.0);
    (from..samples.len())
        .find(|&t| centred_rms(samples, t) < threshold)
        .unwrap_or(samples.len())
}

fn build(voice: Voice, notes: Vec<Note>) -> Phrase {
    let samples = render_notes(voice, &notes);
    let tails = (0..notes.len())
        .map(|i| {
            let prefix = render_notes(voice, &notes[..=i]);
            let crossing = silence_crossing(&prefix, notes[i].1 as usize) as u64;
            match notes.get(i + 1) {
                Some(next) => crossing.min(next.0),
                None => crossing,
            }
        })
        .collect();
    Phrase {
        voice,
        notes,
        tails,
        samples,
    }
}

/// The reference set's phrases with each one's estimated notes (none when refused otherwise).
/// A phrase's estimated notes, or why the estimator refused it.
type Estimate = Result<Vec<EstimatedNote>, String>;

struct Measured {
    phrases: Vec<Phrase>,
    estimates: Vec<Estimate>,
}

fn measured() -> &'static Measured {
    static SET: OnceLock<Measured> = OnceLock::new();
    SET.get_or_init(|| {
        let mut rng = Rng(Xoshiro256StarStar::new(SEED));
        let drawn: Vec<(Voice, Vec<Note>)> = (0..PHRASES).map(|_| draw_phrase(&mut rng)).collect();
        let mut slots: Vec<Option<(Phrase, Estimate)>> = (0..PHRASES).map(|_| None).collect();
        std::thread::scope(|scope| {
            for (slot, (voice, notes)) in slots.iter_mut().zip(drawn) {
                scope.spawn(move || {
                    let phrase = build(voice, notes);
                    let estimate = match estimate_notes(&phrase.samples, 48_000) {
                        Ok(estimation) => Ok(estimation.notes),
                        Err(EstimationError::LowConfidenceNotes { notes }) => Ok(notes),
                        Err(error) => Err(error.to_string()),
                    };
                    *slot = Some((phrase, estimate));
                });
            }
        });
        let (phrases, estimates) = slots.into_iter().map(Option::unwrap).unzip();
        Measured { phrases, estimates }
    })
}

/// One true note and the estimated note matched to it, if any.
struct Pair<'a> {
    phrase: usize,
    index: usize,
    truth: Note,
    tail: u64,
    found: Option<&'a EstimatedNote>,
}

struct Matching<'a> {
    pairs: Vec<Pair<'a>>,
    spurious: Vec<(usize, &'a EstimatedNote)>,
    refused: Vec<(usize, &'a str)>,
}

fn matching(set: &Measured) -> Matching<'_> {
    let mut pairs = Vec::new();
    let mut spurious = Vec::new();
    let mut refused = Vec::new();
    for (p, (phrase, estimate)) in set.phrases.iter().zip(&set.estimates).enumerate() {
        let estimated: &[EstimatedNote] = match estimate {
            Ok(notes) => notes,
            Err(error) => {
                refused.push((p, error.as_str()));
                &[]
            }
        };
        let mut candidates: Vec<(u64, usize, usize)> = Vec::new();
        for (t, truth) in phrase.notes.iter().enumerate() {
            for (e, note) in estimated.iter().enumerate() {
                let distance = note.start_sample.abs_diff(truth.0);
                if distance <= MATCH_WINDOW {
                    candidates.push((distance, t, e));
                }
            }
        }
        candidates.sort_unstable();
        let mut truth_taken = vec![None; phrase.notes.len()];
        let mut estimate_taken = vec![false; estimated.len()];
        for (_, t, e) in candidates {
            if truth_taken[t].is_none() && !estimate_taken[e] {
                truth_taken[t] = Some(e);
                estimate_taken[e] = true;
            }
        }
        for (t, truth) in phrase.notes.iter().enumerate() {
            pairs.push(Pair {
                phrase: p,
                index: t,
                truth: *truth,
                tail: phrase.tails[t],
                found: truth_taken[t].map(|e| &estimated[e]),
            });
        }
        for (e, note) in estimated.iter().enumerate() {
            if !estimate_taken[e] {
                spurious.push((p, note));
            }
        }
    }
    Matching {
        pairs,
        spurious,
        refused,
    }
}

fn rate(hits: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        hits as f64 / total as f64
    }
}

fn pitch_inside(truth: &Note, note: &EstimatedNote) -> bool {
    note.midi_note == truth.2 && cents(note.f0_hz, truth.2).abs() <= PITCH_TOLERANCE_CENTS
}

#[test]
fn the_reference_set_is_32_phrases_of_2_to_8_notes_within_its_ranges() {
    let set = measured();
    assert_eq!(set.phrases.len(), PHRASES);
    let mut repeats = 0;
    let mut zero_gaps = 0;
    for phrase in &set.phrases {
        assert!((2..=8).contains(&phrase.notes.len()));
        for note in &phrase.notes {
            assert!((36..=84).contains(&note.2));
        }
        for pair in phrase.notes.windows(2) {
            let gap = pair[1].0 - pair[0].1;
            assert!(gap <= (200.0 * MS) as u64);
            if gap == 0 {
                zero_gaps += 1;
                assert!(phrase.voice.attack <= 0.020);
                assert_ne!(pair[0].2, pair[1].2);
            }
            if pair[0].2 == pair[1].2 {
                repeats += 1;
            }
        }
    }
    let notes: usize = set.phrases.iter().map(|p| p.notes.len()).sum();
    println!(
        "reference set: {} phrases, {notes} notes, {repeats} same-pitch repeats, {zero_gaps} zero gaps",
        set.phrases.len()
    );
    assert!(repeats > 0, "the set holds repeated same-pitch notes");
    assert!(zero_gaps > 0, "the set holds zero gaps");
}

#[test]
fn at_least_95_percent_of_notes_are_found_and_at_most_5_percent_are_spurious() {
    let m = matching(measured());
    let found = m.pairs.iter().filter(|p| p.found.is_some()).count();
    let estimated = found + m.spurious.len();
    let detection = rate(found, m.pairs.len());
    let spurious = rate(m.spurious.len(), estimated);
    println!(
        "detection {found}/{} = {:.4}; spurious {}/{estimated} = {:.4}; refused phrases {:?}",
        m.pairs.len(),
        detection,
        m.spurious.len(),
        spurious,
        m.refused
    );
    for p in m.pairs.iter().filter(|p| p.found.is_none()) {
        println!(
            "  missed: phrase {} note {} {:?}",
            p.phrase, p.index, p.truth
        );
    }
    for (phrase, note) in &m.spurious {
        println!("  spurious: phrase {phrase} {note:?}");
    }
    assert!(detection >= 0.95, "detection {detection}");
    assert!(spurious <= 0.05, "spurious {spurious}");
}

#[test]
fn onsets_are_within_256_samples_for_at_least_95_percent_of_found_notes() {
    let m = matching(measured());
    let found: Vec<(&Pair, &EstimatedNote)> = m
        .pairs
        .iter()
        .filter_map(|p| p.found.map(|f| (p, f)))
        .collect();
    let inside = found
        .iter()
        .filter(|(p, f)| f.start_sample.abs_diff(p.truth.0) <= ONSET_TOLERANCE)
        .count();
    let measured_rate = rate(inside, found.len());
    println!(
        "onset within {ONSET_TOLERANCE}: {inside}/{} = {measured_rate:.4}",
        found.len()
    );
    for (p, f) in &found {
        let error = f.start_sample as i64 - p.truth.0 as i64;
        if error.unsigned_abs() > ONSET_TOLERANCE {
            println!(
                "  phrase {} note {}: onset error {error}",
                p.phrase, p.index
            );
        }
    }
    assert!(measured_rate >= 0.95, "onset rate {measured_rate}");
}

#[test]
fn pitch_is_the_exact_midi_number_within_10_cents_for_at_least_98_percent_of_found_notes() {
    let m = matching(measured());
    let found: Vec<(&Pair, &EstimatedNote)> = m
        .pairs
        .iter()
        .filter_map(|p| p.found.map(|f| (p, f)))
        .collect();
    let exact = found
        .iter()
        .filter(|(p, f)| f.midi_note == p.truth.2)
        .count();
    let inside = found
        .iter()
        .filter(|(p, f)| pitch_inside(&p.truth, f))
        .count();
    let exact_rate = rate(exact, found.len());
    let inside_rate = rate(inside, found.len());
    println!(
        "pitch exact MIDI {exact}/{} = {exact_rate:.4}; exact and within {PITCH_TOLERANCE_CENTS} cents {inside}/{} = {inside_rate:.4}",
        found.len(),
        found.len()
    );
    for (p, f) in &found {
        if !pitch_inside(&p.truth, f) {
            println!(
                "  phrase {} note {}: MIDI {} for {}, {:.2} cents",
                p.phrase,
                p.index,
                f.midi_note,
                p.truth.2,
                cents(f.f0_hz, p.truth.2)
            );
        }
    }
    assert!(exact_rate >= 0.98, "exact MIDI rate {exact_rate}");
    assert!(inside_rate >= 0.98, "MIDI and 10 cents rate {inside_rate}");
}

#[test]
fn gate_ends_are_within_1024_samples_for_at_least_90_percent_of_found_notes() {
    let m = matching(measured());
    let found: Vec<(&Pair, &EstimatedNote)> = m
        .pairs
        .iter()
        .filter_map(|p| p.found.map(|f| (p, f)))
        .collect();
    let inside = found
        .iter()
        .filter(|(p, f)| f.end_sample.abs_diff(p.truth.1) <= GATE_TOLERANCE)
        .count();
    let measured_rate = rate(inside, found.len());
    println!(
        "gate end within {GATE_TOLERANCE}: {inside}/{} = {measured_rate:.4}",
        found.len()
    );
    for (p, f) in &found {
        let error = f.end_sample as i64 - p.truth.1 as i64;
        if error.unsigned_abs() > GATE_TOLERANCE {
            println!("  phrase {} note {}: gate error {error}", p.phrase, p.index);
        }
    }
    assert!(measured_rate >= 0.90, "gate rate {measured_rate}");
}

#[test]
fn tail_ends_are_within_2400_samples_for_at_least_90_percent_of_found_notes() {
    let m = matching(measured());
    let found: Vec<(&Pair, &EstimatedNote)> = m
        .pairs
        .iter()
        .filter_map(|p| p.found.map(|f| (p, f)))
        .collect();
    let inside = found
        .iter()
        .filter(|(p, f)| f.tail_end_sample.abs_diff(p.tail) <= TAIL_TOLERANCE)
        .count();
    let measured_rate = rate(inside, found.len());
    println!(
        "tail end within {TAIL_TOLERANCE}: {inside}/{} = {measured_rate:.4}",
        found.len()
    );
    for (p, f) in &found {
        let error = f.tail_end_sample as i64 - p.tail as i64;
        if error.unsigned_abs() > TAIL_TOLERANCE {
            println!("  phrase {} note {}: tail error {error}", p.phrase, p.index);
        }
    }
    assert!(measured_rate >= 0.90, "tail rate {measured_rate}");
}

/// No quantity outside its tolerance carries a confidence at or above its threshold, and at least
/// 90 % of the quantities inside their tolerance do. A spurious note's onset is a quantity outside
/// its tolerance.
#[test]
fn confidences_are_calibrated_against_the_tolerances() {
    let m = matching(measured());
    let mut violations = Vec::new();
    let mut inside_total = 0;
    let mut inside_confident = 0;
    for p in &m.pairs {
        let Some(f) = p.found else { continue };
        for (quantity, inside, confidence, threshold) in [
            (
                "onset",
                f.start_sample.abs_diff(p.truth.0) <= ONSET_TOLERANCE,
                f.onset_confidence,
                ONSET_CONFIDENCE_THRESHOLD,
            ),
            (
                "pitch",
                pitch_inside(&p.truth, f),
                f.pitch_confidence,
                PITCH_CONFIDENCE_THRESHOLD,
            ),
            (
                "gate",
                f.end_sample.abs_diff(p.truth.1) <= GATE_TOLERANCE,
                f.gate_confidence,
                GATE_CONFIDENCE_THRESHOLD,
            ),
            (
                "tail",
                f.tail_end_sample.abs_diff(p.tail) <= TAIL_TOLERANCE,
                f.tail_confidence,
                TAIL_CONFIDENCE_THRESHOLD,
            ),
        ] {
            let confident = confidence >= threshold;
            if inside {
                inside_total += 1;
                inside_confident += usize::from(confident);
            } else if confident {
                violations.push(format!(
                    "phrase {} note {}: {quantity} outside its tolerance with confidence {confidence}",
                    p.phrase, p.index
                ));
            }
        }
    }
    for (phrase, note) in &m.spurious {
        if note.onset_confidence >= ONSET_CONFIDENCE_THRESHOLD {
            violations.push(format!(
                "phrase {phrase}: spurious note at {} with onset confidence {}",
                note.start_sample, note.onset_confidence
            ));
        }
    }
    let measured_rate = rate(inside_confident, inside_total);
    println!(
        "calibration: {} confident quantities outside tolerance; confident inside tolerance {inside_confident}/{inside_total} = {measured_rate:.4}",
        violations.len()
    );
    for violation in &violations {
        println!("  {violation}");
    }
    assert!(violations.is_empty(), "{violations:#?}");
    assert!(
        measured_rate >= 0.90,
        "confident inside tolerance {measured_rate}"
    );
}
