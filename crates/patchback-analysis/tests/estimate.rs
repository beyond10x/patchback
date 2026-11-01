//! The monophonic note estimator (story:note-estimation): the acceptance scenarios
//! `spec/scenarios/note-estimation-estimated.yaml`, `note-estimation-low-confidence.yaml`,
//! `note-estimation-polyphonic.yaml` and `note-events-from-estimation.yaml` driven through
//! [`estimate_notes`] and [`bind_note_sequence`], the polyphony detector's obligations
//! (approval-record:wave-c1 Dc1-3), the sequence written from an estimation (Dc1-5), the
//! deterministic estimation id (Dc1-7) and the `estimation.txt` file (Dc1-6).

mod voice;

use patchback_analysis::{
    AnalysisError, ESTIMATION_FILE, ESTIMATION_FORMAT_VERSION, ESTIMATION_HEADER, EstimatedNote,
    EstimationError, EstimationReason, GATE_CONFIDENCE_THRESHOLD, NoteEstimation,
    ONSET_CONFIDENCE_THRESHOLD, PITCH_CONFIDENCE_THRESHOLD, Quantity, TAIL_CONFIDENCE_THRESHOLD,
    bind_note_sequence, clip_sha256, estimate_notes, read_estimation, write_estimation,
};
use patchback_core::digest::{hex, sha256};
use voice::{Note, PLAIN, Voice, cents, render_notes, sequence, square};

const RATE: u32 = 48_000;

fn estimated(samples: &[f32]) -> NoteEstimation {
    match estimate_notes(samples, RATE) {
        Ok(estimation) => estimation,
        Err(error) => panic!("estimation refused: {error}"),
    }
}

fn is_uuid_v8(id: &str) -> bool {
    let groups: Vec<&str> = id.split('-').collect();
    groups.iter().map(|g| g.len()).collect::<Vec<_>>() == [8, 4, 4, 4, 12]
        && id
            .chars()
            .all(|c| c == '-' || c.is_ascii_digit() || ('a'..='f').contains(&c))
        && groups[2].starts_with('8')
        && groups[3].starts_with(['8', '9', 'a', 'b'])
}

fn assert_confident(note: &EstimatedNote) {
    for (quantity, confidence, threshold) in [
        ("onset", note.onset_confidence, ONSET_CONFIDENCE_THRESHOLD),
        ("pitch", note.pitch_confidence, PITCH_CONFIDENCE_THRESHOLD),
        ("gate", note.gate_confidence, GATE_CONFIDENCE_THRESHOLD),
        ("tail", note.tail_confidence, TAIL_CONFIDENCE_THRESHOLD),
    ] {
        assert!(
            (0.0..=1.0).contains(&confidence),
            "{quantity} confidence {confidence} of {note:?}"
        );
        assert!(
            confidence >= threshold,
            "{quantity} confidence {confidence} below {threshold}: {note:?}"
        );
    }
}

/// Three notes of the plain voice: a gap of 20 ms, then a legato change of pitch.
const PHRASE: [Note; 3] = [
    (4_800, 19_200, 45, 0.9),
    (20_160, 34_560, 52, 0.7),
    (34_560, 52_800, 57, 0.8),
];

// note-estimation-estimated

#[test]
fn a_monophonic_phrase_is_estimated_into_notes_each_with_four_confidences() {
    let samples = render_notes(PLAIN, &PHRASE);
    let estimation = estimated(&samples);
    assert!(is_uuid_v8(&estimation.note_estimation_id), "{estimation:?}");
    assert_eq!(estimation.sample_rate_hz, RATE);
    assert_eq!(estimation.frame_count, samples.len() as u64);
    assert_eq!(estimation.notes.len(), 3, "{:#?}", estimation.notes);
    for (note, &(start, end, midi, _)) in estimation.notes.iter().zip(&PHRASE) {
        assert_eq!(note.midi_note, midi, "{note:?}");
        assert!(cents(note.f0_hz, midi).abs() <= 10.0, "{note:?}");
        assert!(note.start_sample.abs_diff(start) <= 256, "{note:?}");
        assert!(note.end_sample.abs_diff(end) <= 1_024, "{note:?}");
        assert!(note.start_sample < note.end_sample, "{note:?}");
        assert!(note.end_sample <= note.tail_end_sample, "{note:?}");
        assert!(note.velocity > 0.0 && note.velocity <= 1.0, "{note:?}");
        assert_confident(note);
    }
    let loudest = estimation
        .notes
        .iter()
        .map(|n| n.velocity)
        .fold(0.0f32, f32::max);
    assert_eq!(loudest, 1.0);
    for pair in estimation.notes.windows(2) {
        assert!(pair[0].tail_end_sample <= pair[1].start_sample, "{pair:?}");
    }
}

// note-estimation-low-confidence

/// A 10 ms note between two long ones: its sounding span, gate and release, is shorter than
/// `estimate_f0` needs (2,074 samples), so its pitch confidence is 0 (Dc1-10).
const SHORT_MIDDLE: [Note; 3] = [
    (4_800, 24_000, 45, 0.8),
    (33_600, 34_080, 57, 0.8),
    (43_200, 62_400, 50, 0.8),
];

const SHORT_RELEASE: Voice = Voice {
    attack: 0.002,
    release: 0.01,
    ..PLAIN
};

#[test]
fn a_note_too_short_for_pitch_is_refused_with_every_note_and_its_confidences() {
    let samples = render_notes(SHORT_RELEASE, &SHORT_MIDDLE);
    let error = estimate_notes(&samples, RATE).unwrap_err();
    assert_eq!(
        error.domain_error(),
        "patchback.analysis.LowConfidenceNotes"
    );
    let EstimationError::LowConfidenceNotes { notes } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(notes.len(), 3, "no note is dropped: {notes:#?}");
    assert_eq!(notes[1].pitch_confidence, 0.0, "{:?}", notes[1]);
    assert!(notes[1].low_quantities().contains(&Quantity::Pitch));
    assert!(notes[0].low_quantities().is_empty(), "{:?}", notes[0]);
    assert!(notes[2].low_quantities().is_empty(), "{:?}", notes[2]);
    assert_eq!(notes[0].midi_note, 45);
    assert_eq!(notes[2].midi_note, 50);
    let message = error.to_string();
    assert!(message.contains("note 1"), "{message}");
    assert!(message.contains("pitch"), "{message}");
    assert!(!message.contains("note 0"), "{message}");
}

// note-estimation-polyphonic and Dc1-3

/// Two tones of the voice sounding together from 100 ms for 600 ms, `interval` semitones apart.
fn dyad(root: u8, interval: u8, voice: Voice) -> Vec<f32> {
    let low = render_notes(voice, &[(4_800, 33_600, root, 0.8)]);
    let high = render_notes(voice, &[(4_800, 33_600, root + interval, 0.8)]);
    low.iter().zip(&high).map(|(a, b)| 0.5 * (a + b)).collect()
}

const DYAD_VOICE: Voice = Voice {
    attack: 0.01,
    decay: 0.1,
    sustain: 0.8,
    release: 0.05,
    cutoff: 5_000.0,
    q: 0.7,
    drive: 0.0,
    gain: -3.0,
};

#[test]
fn simultaneous_tones_a_minor_second_major_third_fifth_or_tritone_apart_are_refused() {
    let mut failures = Vec::new();
    for root in [40u8, 52, 64, 76] {
        for (name, interval) in [
            ("minor second", 1u8),
            ("major third", 4),
            ("tritone", 6),
            ("fifth", 7),
        ] {
            let samples = dyad(root, interval, DYAD_VOICE);
            match estimate_notes(&samples, RATE) {
                Err(EstimationError::PolyphonicInput) => {}
                other => failures.push(format!("{name} on MIDI {root}: {other:?}")),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "not refused as polyphonic: {failures:#?}"
    );
    let error = estimate_notes(&dyad(52, 7, DYAD_VOICE), RATE).unwrap_err();
    assert_eq!(error.domain_error(), "patchback.analysis.PolyphonicInput");
}

/// A single saw, a filtered and driven saw, and a square, each one note at every MIDI number
/// from 36 to 84, are never refused as polyphonic (Dc1-3) and are estimated as that one note.
#[test]
fn single_saw_filtered_saw_and_square_tones_from_midi_36_to_84_are_not_polyphonic() {
    let saw = Voice {
        attack: 0.005,
        decay: 0.05,
        sustain: 0.8,
        release: 0.05,
        cutoff: 16_000.0,
        q: 0.7,
        drive: 0.0,
        gain: -6.0,
    };
    let failures = std::sync::Mutex::new(Vec::new());
    let midis: Vec<u8> = (36u8..=84).collect();
    std::thread::scope(|scope| {
        for chunk in midis.chunks(5) {
            let failures = &failures;
            scope.spawn(move || {
                for &midi in chunk {
                    let filtered = Voice {
                        cutoff: (2.0 * voice::midi_hz(midi)).max(300.0),
                        q: 1.2,
                        drive: 12.0,
                        ..saw
                    };
                    for (name, samples) in [
                        ("saw", render_notes(saw, &[(4_800, 24_000, midi, 0.8)])),
                        (
                            "filtered saw",
                            render_notes(filtered, &[(4_800, 24_000, midi, 0.8)]),
                        ),
                        ("square", square(midi, 0.4, 0.3, 4_800)),
                    ] {
                        match estimate_notes(&samples, RATE) {
                            Ok(e) if e.notes.len() == 1 && e.notes[0].midi_note == midi => {}
                            other => failures.lock().unwrap().push(format!(
                                "{name} at MIDI {midi}: {}",
                                match other {
                                    Ok(e) => format!("{:?}", e.notes),
                                    Err(error) => error.to_string(),
                                }
                            )),
                        }
                    }
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    assert!(failures.is_empty(), "{failures:#?}");
}

// note-events-from-estimation and Dc1-5

#[test]
fn a_note_sequence_written_from_an_estimation_equals_its_notes_and_records_it() {
    let estimation = estimated(&render_notes(PLAIN, &PHRASE));
    let sequence = estimation.note_sequence();
    assert_eq!(sequence.format_version, 1);
    assert_eq!(sequence.sample_rate_hz, RATE);
    assert_eq!(
        sequence.note_estimation_id.as_deref(),
        Some(estimation.note_estimation_id.as_str())
    );
    assert_eq!(sequence.notes.len(), estimation.notes.len());
    for (event, note) in sequence.notes.iter().zip(&estimation.notes) {
        assert_eq!(event.start_sample, note.start_sample);
        assert_eq!(event.end_sample, note.end_sample);
        assert_eq!(event.midi_note, note.midi_note);
        assert_eq!(event.velocity.to_bits(), note.velocity.to_bits());
    }
    // The file the score domain loads reads back to the same sequence.
    let text = patchback_core::score::write_events(&sequence).unwrap();
    assert_eq!(
        patchback_core::score::parse_events(&text).unwrap(),
        sequence
    );
}

#[test]
fn a_sequence_naming_an_estimation_binds_to_it_and_one_naming_an_unknown_estimation_is_refused() {
    let estimation = estimated(&render_notes(PLAIN, &PHRASE));
    let estimations = [estimation.clone()];
    let from_estimation = estimation.note_sequence();
    let bound = bind_note_sequence(&from_estimation, &estimations).unwrap();
    assert_eq!(bound, Some(&estimations[0]));

    let hand_written = sequence(&[(0, 24_000, 45, 0.8)]);
    assert_eq!(
        bind_note_sequence(&hand_written, &estimations).unwrap(),
        None
    );
    assert_eq!(bind_note_sequence(&hand_written, &[]).unwrap(), None);

    let mut unknown = sequence(&[(0, 24_000, 45, 0.8)]);
    unknown.note_estimation_id = Some("7f0c0000-0000-4000-8000-000000000969".to_owned());
    let refusal = bind_note_sequence(&unknown, &estimations).unwrap_err();
    assert_eq!(
        refusal.domain_error(),
        "patchback.score.NoteEstimationNotFound"
    );
    assert_eq!(
        refusal.note_estimation_id,
        "7f0c0000-0000-4000-8000-000000000969"
    );
    assert!(
        refusal
            .to_string()
            .contains("7f0c0000-0000-4000-8000-000000000969")
    );
    let refusal = bind_note_sequence(&from_estimation, &[]).unwrap_err();
    assert_eq!(refusal.note_estimation_id, estimation.note_estimation_id);
}

// Dc1-7

#[test]
fn the_estimation_id_is_a_version_8_uuid_of_the_clip_the_format_and_the_configuration() {
    let samples = render_notes(PLAIN, &PHRASE);
    let first = estimated(&samples);
    let second = estimated(&samples);
    assert_eq!(first, second);
    assert_eq!(write_estimation(&first), write_estimation(&second));

    let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    assert_eq!(first.clip_sha256, sha256(&bytes));
    assert_eq!(clip_sha256(&samples), sha256(&bytes));

    let mut identity = format!(
        "{ESTIMATION_HEADER} {ESTIMATION_FORMAT_VERSION}\nclip_sha256 {}\n",
        hex(&first.clip_sha256)
    );
    for (name, value) in &first.configuration {
        identity.push_str(&format!("config {name} {value}\n"));
    }
    let mut digest = sha256(identity.as_bytes());
    digest[6] = (digest[6] & 0x0f) | 0x80;
    digest[8] = (digest[8] & 0x3f) | 0x80;
    let h = hex(&digest);
    let expected = format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    );
    assert_eq!(first.note_estimation_id, expected);
    assert!(is_uuid_v8(&expected));
    assert!(!first.configuration.is_empty());

    let mut other = samples.clone();
    other[10_000] += 1e-3;
    assert_ne!(
        estimated(&other).note_estimation_id,
        first.note_estimation_id
    );
}

// Dc1-6: estimation.txt

#[test]
fn the_estimation_file_reads_back_to_the_estimation() {
    assert_eq!(ESTIMATION_FILE, "estimation.txt");
    assert_eq!(ESTIMATION_HEADER, "patchback-note-estimation");
    assert_eq!(ESTIMATION_FORMAT_VERSION, 1);
    let estimation = estimated(&render_notes(PLAIN, &PHRASE));
    let text = write_estimation(&estimation);
    assert!(text.starts_with("patchback-note-estimation 1\n"), "{text}");
    assert!(text.ends_with('\n'));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[1],
        format!("note_estimation_id {}", estimation.note_estimation_id)
    );
    assert_eq!(lines[2], "sample_rate_hz 48000");
    assert_eq!(lines[3], format!("frame_count {}", estimation.frame_count));
    assert_eq!(
        lines[4],
        format!("clip_sha256 {}", hex(&estimation.clip_sha256))
    );
    // Dc1-14: the SHA-256 of the note lines as written, each with its newline.
    let note_text: String = text
        .lines()
        .filter(|l| l.starts_with("note "))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(
        lines[5],
        format!("notes_sha256 {}", hex(&sha256(note_text.as_bytes())))
    );
    assert!(lines[6].starts_with("config "), "{}", lines[6]);
    assert_eq!(
        lines.iter().filter(|l| l.starts_with("note ")).count(),
        estimation.notes.len()
    );
    for line in lines.iter().filter(|l| l.starts_with("note ")) {
        assert_eq!(line.split(' ').count(), 11, "{line}");
    }
    assert_eq!(read_estimation(&text).unwrap(), estimation);
}

fn reason(text: &str) -> EstimationReason {
    read_estimation(text).unwrap_err().reason
}

#[test]
fn the_estimation_file_reader_refuses_what_the_writer_never_writes() {
    let estimation = estimated(&render_notes(PLAIN, &PHRASE));
    let text = write_estimation(&estimation);
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let with = |index: usize, line: &str| {
        let mut changed = lines.clone();
        changed[index] = line.to_owned();
        changed.join("\n") + "\n"
    };
    let note_line = lines.iter().position(|l| l.starts_with("note ")).unwrap();
    let note_fields: Vec<&str> = lines[note_line].split(' ').collect();
    let note_with = |field: usize, value: &str| {
        let mut fields = note_fields.clone();
        fields[field] = value;
        with(note_line, &fields.join(" "))
    };

    assert!(matches!(reason(""), EstimationReason::MissingHeader));
    assert!(matches!(
        reason(&with(0, "patchback-notes 1")),
        EstimationReason::MissingHeader
    ));
    assert!(matches!(
        reason(&with(0, "patchback-note-estimation 2")),
        EstimationReason::UnknownFormatVersion { .. }
    ));
    assert!(matches!(
        reason(&with(
            1,
            "note_estimation_id 7f0c0000-0000-4000-8000-000000000969"
        )),
        EstimationReason::IdentityMismatch
    ));
    assert!(matches!(
        reason(&with(1, "note_estimation_id not-a-uuid")),
        EstimationReason::MalformedIdentifier
    ));
    assert!(matches!(
        reason(&with(2, "sample_rate_hz 44100")),
        EstimationReason::UnsupportedSampleRate {
            sample_rate_hz: 44_100
        }
    ));
    assert!(matches!(
        reason(&with(4, "clip_sha256 00")),
        EstimationReason::MalformedDigest
    ));
    assert!(matches!(
        reason(&with(4, &format!("clip_sha256 {}", "0".repeat(64)))),
        EstimationReason::IdentityMismatch
    ));
    assert!(matches!(
        reason(&with(6, "config onset_hop")),
        EstimationReason::FieldCount { .. }
    ));
    assert!(matches!(
        reason(&with(note_line, "note 1 2 3")),
        EstimationReason::FieldCount { .. }
    ));
    assert!(matches!(
        reason(&note_with(1, "x")),
        EstimationReason::MalformedNumber
    ));
    assert!(matches!(
        reason(&note_with(5, "NaN")),
        EstimationReason::MalformedNumber
    ));
    assert!(matches!(
        reason(&note_with(5, "inf")),
        EstimationReason::MalformedNumber
    ));
    assert!(matches!(
        reason(&note_with(4, "128")),
        EstimationReason::InvalidNote { .. }
    ));
    assert!(matches!(
        reason(&note_with(7, "1.5")),
        EstimationReason::InvalidNote { .. }
    ));
    assert!(matches!(
        reason(&note_with(6, "-0.1")),
        EstimationReason::InvalidNote { .. }
    ));
    assert!(matches!(
        reason(&note_with(2, note_fields[1])),
        EstimationReason::InvalidNote { .. }
    ));
    let mut doubled = lines.clone();
    doubled.insert(note_line + 1, lines[note_line].clone());
    assert!(matches!(
        reason(&(doubled.join("\n") + "\n")),
        EstimationReason::InvalidNote { .. }
    ));
    assert!(matches!(
        reason(&format!("{text}unknown record\n")),
        EstimationReason::UnexpectedRecord { .. }
    ));
    assert!(matches!(
        reason(text.trim_end_matches('\n')),
        EstimationReason::MissingNewline
    ));
    assert!(matches!(
        reason(&format!("{text}{}\n", "x".repeat(600))),
        EstimationReason::LineTooLong
    ));
    let error = read_estimation(&with(2, "sample_rate_hz 44100")).unwrap_err();
    assert_eq!(error.line, 3);
    assert!(error.to_string().contains("line 3"), "{error}");
}

/// The file's lines with `notes_sha256` recomputed over its note lines, so that a refusal can
/// only come from what a note says.
fn resealed(lines: &[String]) -> String {
    let note_text: String = lines
        .iter()
        .filter(|l| l.starts_with("note "))
        .map(|l| format!("{l}\n"))
        .collect();
    let digest = format!("notes_sha256 {}", hex(&sha256(note_text.as_bytes())));
    lines
        .iter()
        .map(|l| {
            if l.starts_with("notes_sha256 ") {
                digest.clone()
            } else {
                l.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// Dc1-14: the file vouches for its own note lines and holds only notes an accepted estimation
/// can hold.
#[test]
fn the_estimation_file_refuses_note_lines_it_does_not_vouch_for() {
    let estimation = estimated(&render_notes(PLAIN, &PHRASE));
    let text = write_estimation(&estimation);
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let note_line = lines.iter().position(|l| l.starts_with("note ")).unwrap();
    let note_fields: Vec<String> = lines[note_line].split(' ').map(str::to_owned).collect();
    let note_with = |changes: &[(usize, &str)]| {
        let mut fields = note_fields.clone();
        for (field, value) in changes {
            fields[*field] = (*value).to_owned();
        }
        let mut changed = lines.clone();
        changed[note_line] = fields.join(" ");
        changed
    };
    assert_eq!(resealed(&lines), text, "resealing changes nothing");

    // A note line changed without its digest.
    let mut tampered = note_with(&[(6, "0.5")]);
    let tampered_text = tampered.join("\n") + "\n";
    assert!(
        matches!(
            reason(&tampered_text),
            EstimationReason::NotesDigestMismatch
        ),
        "{:?}",
        reason(&tampered_text)
    );
    let error = read_estimation(&tampered_text).unwrap_err();
    assert_eq!(error.line, 6, "{error}");
    // The same note with its digest recomputed is read.
    assert!(read_estimation(&resealed(&tampered)).is_ok());
    // A malformed or missing digest.
    tampered[5] = "notes_sha256 00".to_owned();
    assert!(matches!(
        reason(&(tampered.join("\n") + "\n")),
        EstimationReason::MalformedDigest
    ));
    let mut missing = lines.clone();
    missing.remove(5);
    assert!(matches!(
        reason(&(missing.join("\n") + "\n")),
        EstimationReason::UnexpectedRecord { .. }
    ));

    // What no accepted estimation holds, even sealed: no pitch, a MIDI number that is not the
    // nearest to f0, a confidence below its threshold.
    let invalid = |changed: Vec<String>| match reason(&resealed(&changed)) {
        EstimationReason::InvalidNote { why } => why,
        other => panic!("expected an invalid note, got {other:?}"),
    };
    invalid(note_with(&[(4, "0"), (5, "0")]));
    invalid(note_with(&[(5, "0")]));
    invalid(note_with(&[(4, "0")]));
    let f0: f64 = note_fields[5].parse().unwrap();
    let midi: u8 = note_fields[4].parse().unwrap();
    invalid(note_with(&[(4, &(midi + 1).to_string())]));
    invalid(note_with(&[(5, &(f0 * 2.0).to_string())]));
    for (field, threshold) in [
        (7, ONSET_CONFIDENCE_THRESHOLD),
        (8, PITCH_CONFIDENCE_THRESHOLD),
        (9, GATE_CONFIDENCE_THRESHOLD),
        (10, TAIL_CONFIDENCE_THRESHOLD),
    ] {
        let below = (threshold - 0.01).to_string();
        invalid(note_with(&[(field, &below)]));
        let at = threshold.to_string();
        assert!(
            read_estimation(&resealed(&note_with(&[(field, &at)]))).is_ok(),
            "a confidence at its threshold is read"
        );
    }
}

// The input guards AnalyzeAudio declares, which EstimateNotes passes on.

#[test]
fn empty_resampled_non_finite_and_silent_clips_are_refused_with_the_analysis_error() {
    assert!(matches!(
        estimate_notes(&[], RATE),
        Err(EstimationError::Analysis(AnalysisError::EmptyAudio))
    ));
    let samples = render_notes(PLAIN, &PHRASE);
    assert!(matches!(
        estimate_notes(&samples, 44_100),
        Err(EstimationError::Analysis(
            AnalysisError::UnsupportedSampleRate {
                sample_rate_hz: 44_100
            }
        ))
    ));
    let mut broken = samples.clone();
    broken[7] = f32::NAN;
    assert!(matches!(
        estimate_notes(&broken, RATE),
        Err(EstimationError::Analysis(AnalysisError::NonFiniteSamples {
            index: 7
        }))
    ));
    let quiet: Vec<f32> = samples.iter().map(|s| s * 1e-4).collect();
    let error = estimate_notes(&quiet, RATE).unwrap_err();
    assert_eq!(error.domain_error(), "patchback.analysis.SilentInput");
}
