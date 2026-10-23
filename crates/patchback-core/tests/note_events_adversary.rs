//! Adversary cases for story:note-events (wave a4, pass 1). Each case asserts what the module
//! documentation, the story or `spec/domains/score.yaml` promises.

use patchback_core::score::{
    EventsFileError, EventsFileReason, FORMAT_VERSION, HEADER, MAX_FILE_BYTES, MAX_NOTES,
    NoteEvent, NoteSequence, ReadEventsError, parse_events, read_events, write_events,
};

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

fn with_body(body: &str) -> String {
    format!("{HEADER} 1\nsample_rate_hz 48000\npatch_id {PATCH}\n{body}")
}

/// `write_events` documents: "Each formatted line is checked by the parser, so the writer never
/// emits a file the parser refuses". An identifier ending in `\n` passes the per-line check
/// (the newline is whitespace to the field splitter) and then puts a blank line in the file.
#[test]
fn the_writer_never_emits_a_file_the_parser_refuses_identifier_with_a_newline() {
    for (patch_id, estimation) in [
        (format!("{PATCH}\n"), None),
        (PATCH.to_owned(), Some(format!("{ESTIMATION}\n"))),
    ] {
        let mut written = sequence(vec![note(0, 1, 60, 0.5)]);
        written.patch_id = patch_id;
        written.note_estimation_id = estimation;
        if let Ok(text) = write_events(&written) {
            assert!(
                parse_events(&text).is_ok(),
                "write_events accepted {written:?} and emitted {text:?}, which the parser refuses: {:?}",
                parse_events(&text)
            );
        }
    }
}

/// Story numeric obligation "Write then parse is identity (§ 11)": the writer either refuses a
/// sequence or the parser gives it back. Whitespace around an identifier is dropped on the way.
#[test]
fn write_then_parse_is_identity_or_the_writer_refuses_identifier_whitespace() {
    for patch_id in [
        format!(" {PATCH}"),
        format!("{PATCH} "),
        format!("\t{PATCH}"),
        format!("{PATCH}\r"),
    ] {
        let mut written = sequence(vec![note(0, 1, 60, 0.5)]);
        written.patch_id = patch_id;
        if let Ok(text) = write_events(&written) {
            assert_eq!(
                parse_events(&text),
                Ok(written.canonical()),
                "write_events accepted {written:?} and emitted {text:?}"
            );
        }
    }
}

/// The domain declares `overlap` before `unbounded-length` (score.yaml, LoadNoteSequence), and
/// the module documents "a file with several faults is refused with the fault whose error
/// `LoadNoteSequence` declares first". Two notes past `MAX_NOTES` that overlap each other are
/// both faults; the overlap must answer.
#[test]
fn overlap_among_notes_past_the_note_bound_outranks_the_bound() {
    let mut body = String::new();
    for i in 0..MAX_NOTES {
        body.push_str(&format!("note {i} {} 60 1\n", i + 1));
    }
    let base = MAX_NOTES as u64 + 100;
    body.push_str(&format!("note {base} {} 60 1\n", base + 10));
    body.push_str(&format!("note {} {} 60 1\n", base + 5, base + 20));
    assert_eq!(
        parse_events(&with_body(&body)),
        Err(EventsFileError {
            line: Some(3 + MAX_NOTES + 2),
            reason: EventsFileReason::OverlappingNotes {
                start_sample: base + 5,
            },
        })
    );
}

/// A valid file of exactly `MAX_FILE_BYTES` loads; one byte more is `FileTooLarge` alone. Kills
/// the mutant `bytes.len() >= MAX_FILE_BYTES` in `parse_bytes`, which no case of the suite
/// reaches (the largest valid file it builds is well under the bound).
#[test]
fn a_file_of_exactly_max_file_bytes_loads_and_one_more_byte_is_refused() {
    let head = with_body("");
    let remaining = MAX_FILE_BYTES - head.len();
    let lines = remaining.div_ceil(250);
    let (width, wider) = (remaining / lines, remaining % lines);
    let mut text = head;
    for i in 0..lines {
        let target = width + usize::from(i < wider) - 1;
        let mut line = format!("note {i} {} 60 1", i + 1);
        while line.len() < target {
            line.push(' ');
        }
        line.push('\n');
        text.push_str(&line);
    }
    assert_eq!(text.len(), MAX_FILE_BYTES);
    let loaded = read_events(text.as_bytes()).expect("a valid file at the byte bound loads");
    assert_eq!(loaded.notes.len(), lines);
    assert_eq!(parse_events(&text).unwrap().notes.len(), lines);

    text.insert(text.len() - 1, ' ');
    assert_eq!(text.len(), MAX_FILE_BYTES + 1);
    match read_events(text.as_bytes()) {
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

/// A UUID is `8-4-4-4-12` hexadecimal digits, 36 characters. Kills the mutant that drops
/// `token.len() == 36` from `uuid`: no case of the suite gives a hex identifier of the wrong
/// length.
#[test]
fn an_identifier_one_character_short_or_long_is_refused() {
    for token in [&PATCH[..35], &format!("{PATCH}0")[..]] {
        assert_eq!(
            parse_events(&format!(
                "{HEADER} 1\nsample_rate_hz 48000\npatch_id {token}\n"
            )),
            Err(EventsFileError {
                line: Some(3),
                reason: EventsFileReason::MalformedIdentifier {
                    token: token.to_owned()
                },
            })
        );
    }
}
