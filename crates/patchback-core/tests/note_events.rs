//! The numeric and structural obligations of story:note-events (DESIGN.md § 10, § 11, § 12.3).
//! The ESS scenarios `spec/scenarios/note-events-*.yaml` name the command outcomes; these tests
//! hold the grammar, the bounds, the exact sample offsets and the roundtrip they cannot express.

use std::collections::BTreeSet;
use std::io::{self, Read};

use patchback_core::score::{
    EventsFileError, EventsFileReason, FORMAT_VERSION, HEADER, LOAD_NOTE_SEQUENCE_PRECEDENCE,
    MAX_FILE_BYTES, MAX_LINE_BYTES, MAX_NOTES, NoteEvent, NoteSequence, ReadEventsError,
    parse_events, read_events, read_events_file, write_events,
};

// ---------------------------------------------------------------------------------------------
// Fixtures

const PATCH: &str = "7f0c0000-0000-4000-8000-000000000151";
const ESTIMATION: &str = "7f0c0000-0000-4000-8000-0000000003a1";

fn note(start_sample: u64, end_sample: u64, midi_note: u8, velocity: f32) -> NoteEvent {
    NoteEvent {
        start_sample,
        end_sample,
        midi_note,
        velocity,
    }
}

fn sequence(notes: Vec<NoteEvent>) -> NoteSequence {
    NoteSequence {
        format_version: FORMAT_VERSION,
        sample_rate_hz: 48_000,
        patch_id: PATCH.to_owned(),
        note_estimation_id: None,
        notes,
    }
}

/// The header and both required keys, then `body`.
fn with_body(body: &str) -> String {
    format!("{HEADER} 1\nsample_rate_hz 48000\npatch_id {PATCH}\n{body}")
}

fn refusal(text: &str) -> EventsFileError {
    match parse_events(text) {
        Ok(sequence) => panic!("accepted {text:?} as {sequence:?}"),
        Err(error) => error,
    }
}

fn refused_with(text: &str, line: Option<usize>, reason: EventsFileReason) {
    assert_eq!(refusal(text), EventsFileError { line, reason }, "{text:?}");
}

/// splitmix64: a fixed, std-only generator for the property tests.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    /// A velocity in [0, 1]: an edge value, or any f32 bit pattern from 0.0 to 1.0.
    fn velocity(&mut self) -> f32 {
        match self.below(8) {
            0 => 0.0,
            1 => 1.0,
            2 => -0.0,
            3 => f32::from_bits(1),
            4 => f32::from_bits(1.0_f32.to_bits() - 1),
            _ => f32::from_bits(self.below(u64::from(1.0_f32.to_bits()) + 1) as u32),
        }
    }

    fn uuid(&mut self) -> String {
        let hex = format!("{:016x}{:016x}", self.next(), self.next());
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }

    /// A monophonic sequence in a shuffled order: notes abut or leave gaps, and one in four
    /// sequences starts within ten million samples of `u64::MAX`.
    fn sequence(&mut self) -> NoteSequence {
        let count = self.below(40) as usize;
        let mut cursor = if self.below(4) == 0 {
            u64::MAX - 10_000_000
        } else {
            self.below(1 << 40)
        };
        let mut notes = Vec::new();
        for _ in 0..count {
            let start = cursor
                + if self.below(3) == 0 {
                    0
                } else {
                    self.below(48_000)
                };
            let end = start + 1 + self.below(96_000);
            notes.push(note(start, end, self.below(128) as u8, self.velocity()));
            cursor = end;
        }
        for i in (1..notes.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            notes.swap(i, j);
        }
        NoteSequence {
            format_version: FORMAT_VERSION,
            sample_rate_hz: 48_000,
            patch_id: self.uuid(),
            note_estimation_id: (self.below(2) == 0).then(|| self.uuid()),
            notes,
        }
    }
}

fn velocity_bits(sequence: &NoteSequence) -> Vec<u32> {
    sequence
        .notes
        .iter()
        .map(|n| n.velocity.to_bits())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Loaded (spec/scenarios/note-events-loaded.yaml)

#[test]
fn loaded_two_abutting_notes_at_the_patch_rate() {
    let text = with_body("note 0 24000 45 0.8\nnote 24000 48000 52 0.6\n");
    let sequence = parse_events(&text).expect("a monophonic sequence loads");
    assert_eq!(
        sequence,
        NoteSequence {
            format_version: 1,
            sample_rate_hz: 48_000,
            patch_id: PATCH.to_owned(),
            note_estimation_id: None,
            notes: vec![note(0, 24_000, 45, 0.8), note(24_000, 48_000, 52, 0.6)],
        }
    );
}

#[test]
fn loaded_an_empty_sequence_and_one_naming_its_estimation() {
    assert_eq!(parse_events(&with_body("")).unwrap(), sequence(vec![]));
    let text = with_body(&format!("note_estimation_id {ESTIMATION}\nnote 5 6 0 1\n"));
    let loaded = parse_events(&text).unwrap();
    assert_eq!(loaded.note_estimation_id.as_deref(), Some(ESTIMATION));
    assert_eq!(loaded.notes, vec![note(5, 6, 0, 1.0)]);
}

#[test]
fn loaded_keys_and_notes_in_any_order_and_crlf_lines() {
    let text = format!(
        "{HEADER} 1\r\nnote 100 200 127 0\r\npatch_id {PATCH}\r\nnote 0 100 0 1\r\nsample_rate_hz 48000"
    );
    let loaded = parse_events(&text).unwrap();
    assert_eq!(
        loaded.notes,
        vec![note(100, 200, 127, 0.0), note(0, 100, 0, 1.0)],
        "the parser keeps the file's order"
    );
    assert_eq!(
        loaded.canonical().notes,
        vec![note(0, 100, 0, 1.0), note(100, 200, 127, 0.0)]
    );
}

#[test]
fn loaded_the_sample_rate_is_read_as_written() {
    // The rate is checked against the patch that renders the sequence, not by the parser
    // (score.yaml rate-mismatch is a relation).
    let text = format!("{HEADER} 1\nsample_rate_hz 44100\npatch_id {PATCH}\n");
    assert_eq!(parse_events(&text).unwrap().sample_rate_hz, 44_100);
}

#[test]
fn read_events_and_read_events_file_load_the_same_sequence() {
    let text = with_body("note 0 24000 45 0.8\n");
    let expected = parse_events(&text).unwrap();
    assert_eq!(read_events(text.as_bytes()).unwrap(), expected);
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("note_events_read.events");
    std::fs::write(&path, &text).unwrap();
    assert_eq!(read_events_file(&path).unwrap(), expected);
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(
        read_events_file(&path),
        Err(ReadEventsError::Io(_))
    ));
}

// ---------------------------------------------------------------------------------------------
// Numeric obligations: write then parse is identity; offsets are exact

#[test]
fn write_then_parse_is_identity_over_generated_sequences() {
    let mut rng = Rng(0x5eed_0000_0000_0001);
    for _ in 0..500 {
        let generated = rng.sequence();
        let canonical = generated.canonical();
        let text = write_events(&generated).expect("a valid sequence is written");
        let read = parse_events(&text).expect("the writer's output parses");
        assert_eq!(read, canonical, "{text}");
        assert_eq!(velocity_bits(&read), velocity_bits(&canonical), "{text}");
        assert_eq!(write_events(&read).unwrap(), text, "parse then write");
    }
}

#[test]
fn exact_sample_offsets_survive_the_round_trip() {
    // 2^53 + 1 has no f64; u64::MAX is the last sample a note may end on.
    let offsets = [
        (0, 1),
        (1, 2),
        (47_999, 48_000),
        ((1 << 53) + 1, (1 << 53) + 3),
        (u64::MAX - 1, u64::MAX),
    ];
    let notes: Vec<NoteEvent> = offsets
        .iter()
        .map(|&(start, end)| note(start, end, 60, 0.5))
        .collect();
    let written = write_events(&sequence(notes.clone())).unwrap();
    for (start, end) in offsets {
        assert!(
            written.contains(&format!("note {start} {end} 60 0.5\n")),
            "{written}"
        );
    }
    let read = parse_events(&written).unwrap();
    let read_offsets: Vec<(u64, u64)> = read
        .notes
        .iter()
        .map(|n| (n.start_sample, n.end_sample))
        .collect();
    assert_eq!(read_offsets, offsets);
}

#[test]
fn the_canonical_form_is_header_keys_then_notes_by_start() {
    let mut written = sequence(vec![
        note(48_000, 96_000, 52, 0.25),
        note(0, 48_000, 45, 1.0),
    ]);
    written.note_estimation_id = Some(ESTIMATION.to_owned());
    assert_eq!(
        write_events(&written).unwrap(),
        format!(
            "{HEADER} 1\nsample_rate_hz 48000\npatch_id {PATCH}\nnote_estimation_id {ESTIMATION}\nnote 0 48000 45 1\nnote 48000 96000 52 0.25\n"
        )
    );
}

#[test]
fn velocities_are_written_in_their_shortest_form() {
    let tiny = f32::from_bits(1);
    let written = write_events(&sequence(vec![note(0, 1, 60, tiny), note(1, 2, 60, 0.1)])).unwrap();
    assert!(written.contains("note 0 1 60 1e-45\n"), "{written}");
    assert!(written.contains("note 1 2 60 0.1\n"), "{written}");
    let read = parse_events(&written).unwrap();
    assert_eq!(read.notes[0].velocity.to_bits(), 1);
    assert_eq!(read.notes[1].velocity, 0.1_f32);
}

/// The writer checks an identifier's raw value, not the line it formats: whitespace the field
/// splitter would drop, or a line break, does not read back.
#[test]
fn the_writer_refuses_an_identifier_that_would_not_read_back() {
    let shapes = [
        ("newline", format!("{PATCH}\n")),
        ("leading space", format!(" {PATCH}")),
        ("trailing space", format!("{PATCH} ")),
        ("tab", format!("\t{PATCH}")),
        ("carriage return", format!("{PATCH}\r")),
    ];
    for (shape, raw) in &shapes {
        let mut written = sequence(vec![note(0, 1, 60, 0.5)]);
        written.patch_id = raw.clone();
        assert_eq!(
            write_events(&written),
            Err(EventsFileError {
                line: Some(3),
                reason: EventsFileReason::MalformedIdentifier { token: raw.clone() },
            }),
            "patch_id with a {shape}"
        );
        let mut written = sequence(vec![note(0, 1, 60, 0.5)]);
        written.note_estimation_id = Some(raw.clone());
        assert_eq!(
            write_events(&written),
            Err(EventsFileError {
                line: Some(4),
                reason: EventsFileReason::MalformedIdentifier { token: raw.clone() },
            }),
            "note_estimation_id with a {shape}"
        );
    }
}

#[test]
fn the_writer_refuses_what_the_parser_would_refuse() {
    let cases = [
        (
            vec![note(0, 0, 60, 0.5)],
            "patchback.score.InvalidNoteTiming",
        ),
        (
            vec![note(0, 10, 128, 0.5)],
            "patchback.score.MidiNoteOutOfRange",
        ),
        (
            vec![note(0, 10, 60, 1.5)],
            "patchback.score.VelocityOutOfRange",
        ),
        (
            vec![note(0, 10, 60, f32::NAN)],
            "patchback.score.MalformedNumber",
        ),
        (
            vec![note(0, 10, 60, 0.5), note(5, 20, 61, 0.5)],
            "patchback.score.OverlappingNotes",
        ),
    ];
    for (notes, expected) in cases {
        let error = write_events(&sequence(notes.clone())).expect_err("refused");
        assert_eq!(error.reason.spec_error(), expected, "{notes:?}");
        assert!(error.line.is_some(), "{error}");
    }
    let mut bad_id = sequence(vec![]);
    bad_id.patch_id = "patch.patch".to_owned();
    assert_eq!(
        write_events(&bad_id).unwrap_err().reason.spec_error(),
        "patchback.score.MalformedRecord"
    );
    let mut bad_version = sequence(vec![]);
    bad_version.format_version = 2;
    assert_eq!(
        write_events(&bad_version).unwrap_err().reason,
        EventsFileReason::UnknownFormatVersion { format_version: 2 }
    );
}

// ---------------------------------------------------------------------------------------------
// Invalid notes (spec/scenarios/note-events-invalid-notes.yaml)

#[test]
fn invalid_notes_timing_midi_and_velocity_are_refused_with_the_line() {
    refused_with(
        &with_body("note 4800 4800 45 0.8\n"),
        Some(4),
        EventsFileReason::InvalidNoteTiming {
            start_sample: 4800,
            end_sample: 4800,
        },
    );
    refused_with(
        &with_body("note 0 10 45 0.8\nnote 4800 100 45 0.8\n"),
        Some(5),
        EventsFileReason::InvalidNoteTiming {
            start_sample: 4800,
            end_sample: 100,
        },
    );
    refused_with(
        &with_body("note 0 4800 128 0.8\n"),
        Some(4),
        EventsFileReason::MidiNoteOutOfRange { midi_note: 128 },
    );
    refused_with(
        &with_body("note 0 4800 60 1.2\n"),
        Some(4),
        EventsFileReason::VelocityOutOfRange { velocity: 1.2 },
    );
    refused_with(
        &with_body("note 0 4800 60 -0.1\n"),
        Some(4),
        EventsFileReason::VelocityOutOfRange { velocity: -0.1 },
    );
}

#[test]
fn invalid_notes_the_range_edges_are_accepted() {
    let loaded = parse_events(&with_body("note 0 1 0 0\nnote 1 2 127 1\n")).unwrap();
    assert_eq!(loaded.notes, vec![note(0, 1, 0, 0.0), note(1, 2, 127, 1.0)]);
    let next_above_one = f32::from_bits(1.0_f32.to_bits() + 1);
    refused_with(
        &with_body(&format!("note 0 1 60 {next_above_one}\n")),
        Some(4),
        EventsFileReason::VelocityOutOfRange {
            velocity: next_above_one,
        },
    );
}

// ---------------------------------------------------------------------------------------------
// Overlap and version (spec/scenarios/note-events-overlap-and-version.yaml)

#[test]
fn overlap_a_note_starting_while_another_sounds_is_refused() {
    refused_with(
        &with_body("note 0 24000 45 0.8\nnote 12000 36000 52 0.8\n"),
        Some(5),
        EventsFileReason::OverlappingNotes {
            start_sample: 12000,
        },
    );
}

#[test]
fn overlap_two_notes_on_one_start_are_refused() {
    refused_with(
        &with_body("note 0 24000 45 0.8\nnote 0 12000 52 0.8\n"),
        Some(5),
        EventsFileReason::OverlappingNotes { start_sample: 0 },
    );
}

#[test]
fn overlap_is_found_whatever_the_file_order() {
    // The later line starts before the earlier one but ends after the earlier one starts.
    refused_with(
        &with_body("note 12000 36000 52 0.8\nnote 0 24000 45 0.8\n"),
        Some(5),
        EventsFileReason::OverlappingNotes { start_sample: 0 },
    );
    // A long note around a short one already read.
    refused_with(
        &with_body("note 100 200 52 0.8\nnote 300 400 52 0.8\nnote 0 1000 45 0.8\n"),
        Some(6),
        EventsFileReason::OverlappingNotes { start_sample: 0 },
    );
}

#[test]
fn overlap_a_note_may_start_on_the_sample_the_previous_one_ends() {
    assert!(parse_events(&with_body("note 24000 48000 52 0.6\nnote 0 24000 45 0.8\n")).is_ok());
}

#[test]
fn version_an_unknown_format_version_is_refused_alone() {
    refused_with(
        &format!("{HEADER} 3\nsample_rate_hz x\n"),
        Some(1),
        EventsFileReason::UnknownFormatVersion { format_version: 3 },
    );
    refused_with(
        "patchback-patch 1\n",
        Some(1),
        EventsFileReason::MissingFormatVersion,
    );
    refused_with("", Some(1), EventsFileReason::MissingFormatVersion);
    refused_with(
        &format!("{HEADER} 1x\n"),
        Some(1),
        EventsFileReason::MalformedNumber {
            token: "1x".to_owned(),
        },
    );
}

// ---------------------------------------------------------------------------------------------
// Parser refusals (spec/scenarios/note-events-parser-refusals.yaml)

#[test]
fn parser_refusals_malformed_numbers() {
    for (body, token) in [
        ("note x 10 60 0.5\n", "x"),
        ("note -1 10 60 0.5\n", "-1"),
        ("note +1 10 60 0.5\n", "+1"),
        ("note 1.0 10 60 0.5\n", "1.0"),
        (
            "note 0 18446744073709551616 60 0.5\n",
            "18446744073709551616",
        ),
        ("note 0 10 -1 0.5\n", "-1"),
        ("note 0 10 60 nan\n", "nan"),
        ("note 0 10 60 inf\n", "inf"),
        ("note 0 10 60 .5\n", ".5"),
        ("note 0 10 60 5.\n", "5."),
        ("note 0 10 60 0x1\n", "0x1"),
        ("note 0 10 60 1e39\n", "1e39"),
    ] {
        refused_with(
            &with_body(body),
            Some(4),
            EventsFileReason::MalformedNumber {
                token: token.to_owned(),
            },
        );
    }
    refused_with(
        &format!("{HEADER} 1\nsample_rate_hz 48k\npatch_id {PATCH}\n"),
        Some(2),
        EventsFileReason::MalformedNumber {
            token: "48k".to_owned(),
        },
    );
}

#[test]
fn parser_refusals_decimal_forms_the_grammar_reads() {
    let loaded = parse_events(&with_body(
        "note 0 1 60 5e-1\nnote 1 2 60 +0.25\nnote 2 3 60 1E0\n",
    ))
    .unwrap();
    let velocities: Vec<f32> = loaded.notes.iter().map(|n| n.velocity).collect();
    assert_eq!(velocities, vec![0.5, 0.25, 1.0]);
}

#[test]
fn parser_refusals_records_outside_the_grammar() {
    refused_with(
        &with_body("chord 0 10 60 0.5\n"),
        Some(4),
        EventsFileReason::UnknownRecord {
            record: "chord".to_owned(),
        },
    );
    refused_with(&with_body("\n"), Some(4), EventsFileReason::BlankLine);
    refused_with(
        &with_body("note 0 10 60\n"),
        Some(4),
        EventsFileReason::WrongFieldCount {
            record: "note".to_owned(),
            expected: 5,
            found: 4,
        },
    );
    refused_with(
        &format!("{HEADER} 1\nsample_rate_hz 48000\npatch_id patch.patch\n"),
        Some(3),
        EventsFileReason::MalformedIdentifier {
            token: "patch.patch".to_owned(),
        },
    );
    refused_with(
        &format!(
            "{HEADER} 1\nsample_rate_hz 48000\npatch_id {}\n",
            PATCH.to_uppercase()
        ),
        Some(3),
        EventsFileReason::MalformedIdentifier {
            token: PATCH.to_uppercase(),
        },
    );
    refused_with(
        &format!("{HEADER} 1\nsample_rate_hz 48000\n"),
        None,
        EventsFileReason::MissingKey {
            key: "patch_id".to_owned(),
        },
    );
    refused_with(
        &format!("{HEADER} 1\npatch_id {PATCH}\n"),
        None,
        EventsFileReason::MissingKey {
            key: "sample_rate_hz".to_owned(),
        },
    );
    let mut bytes = with_body("").into_bytes();
    bytes.extend_from_slice(b"note 0 1 60 \xff\n");
    match read_events(&bytes[..]) {
        Err(ReadEventsError::Format(error)) => assert_eq!(
            error,
            EventsFileError {
                line: Some(4),
                reason: EventsFileReason::NotUtf8
            }
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn parser_refusals_duplicate_keys() {
    refused_with(
        &with_body("sample_rate_hz 48000\n"),
        Some(4),
        EventsFileReason::DuplicateKey {
            key: "sample_rate_hz".to_owned(),
        },
    );
    refused_with(
        &with_body(&format!("patch_id {PATCH}\n")),
        Some(4),
        EventsFileReason::DuplicateKey {
            key: "patch_id".to_owned(),
        },
    );
    refused_with(
        &with_body(&format!(
            "note_estimation_id {ESTIMATION}\nnote_estimation_id {ESTIMATION}\n"
        )),
        Some(5),
        EventsFileReason::DuplicateKey {
            key: "note_estimation_id".to_owned(),
        },
    );
    refused_with(
        &with_body(&format!("{HEADER} 1\n")),
        Some(4),
        EventsFileReason::DuplicateKey {
            key: HEADER.to_owned(),
        },
    );
}

#[test]
fn parser_refusals_line_past_the_line_bound() {
    let long = format!("note 0 10 60 0.{}\n", "5".repeat(MAX_LINE_BYTES));
    let bytes = long.len() - 1;
    refused_with(
        &with_body(&long),
        Some(4),
        EventsFileReason::LineTooLong { bytes },
    );
    let at_bound = format!("note 0 10 60 0.{}\n", "5".repeat(MAX_LINE_BYTES - 15));
    assert_eq!(at_bound.len() - 1, MAX_LINE_BYTES);
    assert!(parse_events(&with_body(&at_bound)).is_ok());
}

#[test]
fn parser_refusals_file_past_the_byte_bound_is_answered_alone() {
    struct Endless;
    impl Read for Endless {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            buf.fill(b'x');
            Ok(buf.len())
        }
    }
    match read_events(Endless) {
        Err(ReadEventsError::Format(error)) => assert_eq!(
            error,
            EventsFileError {
                line: None,
                reason: EventsFileReason::FileTooLarge
            }
        ),
        other => panic!("{other:?}"),
    }
}

/// A file of `count` abutting one-sample notes.
fn notes_body(count: usize) -> String {
    let mut body = String::new();
    for i in 0..count {
        body.push_str(&format!("note {i} {} 60 1\n", i + 1));
    }
    body
}

#[test]
fn the_note_count_bound_holds_at_max_notes_and_refuses_one_more() {
    let at_bound = with_body(&notes_body(MAX_NOTES));
    assert!(at_bound.len() <= MAX_FILE_BYTES);
    assert_eq!(parse_events(&at_bound).unwrap().notes.len(), MAX_NOTES);
    refused_with(
        &with_body(&notes_body(MAX_NOTES + 1)),
        Some(3 + MAX_NOTES + 1),
        EventsFileReason::TooManyNotes,
    );
    let written: Vec<NoteEvent> = (0..=MAX_NOTES as u64)
        .map(|i| note(i, i + 1, 60, 1.0))
        .collect();
    assert_eq!(
        write_events(&sequence(written)).unwrap_err().reason,
        EventsFileReason::TooManyNotes
    );
}

#[test]
fn the_byte_bound_admits_max_notes_of_the_longest_canonical_line() {
    // The largest subnormal f32 has the longest shortest form in [0, 1]: 1.1754942e-38.
    let mut written = sequence(vec![note(
        u64::MAX - 1,
        u64::MAX,
        127,
        f32::from_bits(0x007f_ffff),
    )]);
    written.note_estimation_id = Some(ESTIMATION.to_owned());
    let text = write_events(&written).unwrap();
    let longest = format!("{}\n", text.lines().last().unwrap());
    assert_eq!(
        longest,
        format!("note {} {} 127 1.1754942e-38\n", u64::MAX - 1, u64::MAX)
    );
    assert!(longest.len() - 1 <= MAX_LINE_BYTES, "{longest}");
    let header = with_body(&format!("note_estimation_id {ESTIMATION}\n"));
    assert!(header.len() + MAX_NOTES * longest.len() <= MAX_FILE_BYTES);
}

// ---------------------------------------------------------------------------------------------
// Precedence

#[test]
fn several_faults_resolve_by_the_declared_order() {
    // Malformed number on line 4, invalid timing on line 5: timing is declared first.
    refused_with(
        &with_body("note x 10 60 0.5\nnote 10 5 60 0.5\n"),
        Some(5),
        EventsFileReason::InvalidNoteTiming {
            start_sample: 10,
            end_sample: 5,
        },
    );
    // Velocity on line 4, MIDI on line 5: MIDI is declared first.
    refused_with(
        &with_body("note 0 10 60 2\nnote 10 20 200 0.5\n"),
        Some(5),
        EventsFileReason::MidiNoteOutOfRange { midi_note: 200 },
    );
    // Overlap on line 5, a duplicate key on line 6, a bound on line 7: overlap first.
    let long = format!("note 0 10 60 0.{}\n", "5".repeat(MAX_LINE_BYTES));
    refused_with(
        &with_body(&format!(
            "note 0 10 60 0.5\nnote 5 20 60 0.5\nsample_rate_hz 48000\n{long}"
        )),
        Some(5),
        EventsFileReason::OverlappingNotes { start_sample: 5 },
    );
    // One rank on two lines: the earliest line.
    refused_with(
        &with_body("note 0 10 60 x\nnote 10 20 60 y\n"),
        Some(4),
        EventsFileReason::MalformedNumber {
            token: "x".to_owned(),
        },
    );
    // Several faults on one line: the highest-ranked of them.
    refused_with(
        &with_body("note 10 5 300 7\n"),
        Some(4),
        EventsFileReason::InvalidNoteTiming {
            start_sample: 10,
            end_sample: 5,
        },
    );
    // A duplicate key outranks a malformed record; a missing key ranks after every line.
    refused_with(
        &format!("{HEADER} 1\nsample_rate_hz 48000\nsample_rate_hz 48000\nchord\n"),
        Some(3),
        EventsFileReason::DuplicateKey {
            key: "sample_rate_hz".to_owned(),
        },
    );
    refused_with(
        &format!("{HEADER} 1\nsample_rate_hz 48000\nchord\n"),
        Some(3),
        EventsFileReason::UnknownRecord {
            record: "chord".to_owned(),
        },
    );
    // A missing key is a malformed record, so it outranks a malformed number on a line.
    refused_with(
        &format!("{HEADER} 1\nsample_rate_hz 48000\nnote 0 1 60 x\n"),
        None,
        EventsFileReason::MissingKey {
            key: "patch_id".to_owned(),
        },
    );
}

#[test]
fn a_line_that_cannot_be_read_reports_no_missing_key() {
    let long = format!("patch_id {}\n", "a".repeat(MAX_LINE_BYTES));
    refused_with(
        &format!("{HEADER} 1\nsample_rate_hz 48000\n{long}"),
        Some(3),
        EventsFileReason::LineTooLong {
            bytes: long.len() - 1,
        },
    );
}

#[test]
fn load_note_sequence_precedence_is_the_declaration_order_of_the_spec() {
    let domain = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/domains/score.yaml"
    ))
    .expect("spec/domains/score.yaml");
    let mut declared: Vec<&str> = Vec::new();
    for error in domain
        .lines()
        .skip_while(|l| l.trim() != "- name: patchback.score.LoadNoteSequence")
        .skip(1)
        .take_while(|l| !l.starts_with("  - name: "))
        .filter_map(|l| l.trim().strip_prefix("error: "))
    {
        if LOAD_NOTE_SEQUENCE_PRECEDENCE.contains(&error) && !declared.contains(&error) {
            declared.push(error);
        }
    }
    assert_eq!(declared, LOAD_NOTE_SEQUENCE_PRECEDENCE);
}

#[test]
fn every_refusal_names_an_error_of_the_spec_domain_and_its_rank() {
    let domain = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/domains/score.yaml"
    ))
    .expect("spec/domains/score.yaml");
    let declared: BTreeSet<&str> = domain
        .lines()
        .skip_while(|l| *l != "errors:")
        .take_while(|l| *l != "commands:")
        .filter_map(|l| l.trim().strip_prefix("- name: "))
        .collect();
    let reasons = [
        EventsFileReason::MissingFormatVersion,
        EventsFileReason::UnknownFormatVersion { format_version: 2 },
        EventsFileReason::UnknownRecord {
            record: "x".to_owned(),
        },
        EventsFileReason::BlankLine,
        EventsFileReason::WrongFieldCount {
            record: "note".to_owned(),
            expected: 5,
            found: 1,
        },
        EventsFileReason::MissingKey {
            key: "patch_id".to_owned(),
        },
        EventsFileReason::DuplicateKey {
            key: "patch_id".to_owned(),
        },
        EventsFileReason::MalformedNumber {
            token: "x".to_owned(),
        },
        EventsFileReason::MalformedIdentifier {
            token: "x".to_owned(),
        },
        EventsFileReason::InvalidNoteTiming {
            start_sample: 1,
            end_sample: 0,
        },
        EventsFileReason::MidiNoteOutOfRange { midi_note: 128 },
        EventsFileReason::VelocityOutOfRange { velocity: 2.0 },
        EventsFileReason::OverlappingNotes { start_sample: 0 },
        EventsFileReason::NotUtf8,
        EventsFileReason::LineTooLong { bytes: 300 },
        EventsFileReason::FileTooLarge,
        EventsFileReason::TooManyNotes,
    ];
    let mut ranks = BTreeSet::new();
    for reason in reasons {
        let name = reason.spec_error();
        assert!(
            declared.contains(name),
            "{reason:?} names {name}, which spec/domains/score.yaml does not declare"
        );
        assert_eq!(LOAD_NOTE_SEQUENCE_PRECEDENCE[reason.precedence()], name);
        ranks.insert(reason.precedence());
        let error = EventsFileError {
            line: Some(7),
            reason,
        };
        assert_eq!(error.to_string(), format!("line 7: {}", error.reason));
    }
    assert_eq!(
        ranks,
        (0..LOAD_NOTE_SEQUENCE_PRECEDENCE.len()).collect::<BTreeSet<_>>(),
        "every declared error is reachable"
    );
}
