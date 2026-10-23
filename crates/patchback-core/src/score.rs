//! The score (DESIGN.md § 10): note events and the `notes.events` file (§ 11) that holds them,
//! its parser and its canonical writer, with std only. The names follow the ESS domain
//! `patchback.score` (`spec/domains/score.yaml`): [`NoteEvent`] and [`NoteSequence`].
//!
//! The file holds notes only. Control automation (§ 10 `ControlEvent`) has its own file,
//! `controls.events`, with its own header; nothing about controls is part of this format.
//!
//! # Grammar
//!
//! A file is UTF-8 text of lines, each ended by `\n` or `\r\n`; the terminator is not part of
//! the line, and the last line may have none. Fields are separated by ASCII whitespace (one space
//! in the canonical form). There are no comments and no blank lines: both are refused.
//!
//! ```text
//! patchback-notes 1                               the header, first line: the format version
//! sample_rate_hz 48000                            key, exactly once
//! patch_id <uuid>                                 key, exactly once: the patch it was written for
//! note_estimation_id <uuid>                       key, at most once: the estimation it came from
//! note <start_sample> <end_sample> <midi_note> <velocity>   one line per note
//! ```
//!
//! After the header the records may come in any order.
//!
//! - The version and the sample rate are integers fitting a `u32`; `<start_sample>`,
//!   `<end_sample>` and `<midi_note>` are integers fitting a `u64`. An integer is ASCII digits
//!   only, with no sign: the `.patch` grammar ([`crate::patch::file`]). So a negative sample
//!   position or MIDI note is a malformed number, and the domain's `start_sample < 0` and
//!   `midi_note < 0` cannot be written.
//! - `<velocity>` is a finite decimal of the `.patch` grammar (an optional sign, digits, an
//!   optional `.` and digits, an optional exponent), read as an `f32` (§ 10) and finite after
//!   reading: `1e39` overflows `f32` and is refused.
//! - `<uuid>` is a UUID in its lower-case hyphenated text form, `8-4-4-4-12` hexadecimal digits
//!   (the domain types both identifiers as `Uuid`).
//!
//! The sample rate is read as written. Whether it is the rate of the patch that renders the
//! sequence, whether that patch exists and whether the estimation exists are relations
//! (`LoadNoteSequence` `rate-mismatch`, `unknown-patch`, `unknown-estimation`); the caller that
//! binds the sequence to its patch answers them, not this parser.
//!
//! # Notes
//!
//! A note ends after it starts (`end_sample > start_sample`), its MIDI note is 0 to 127 and its
//! velocity lies in `[0, 1]` (normalized; DESIGN.md § 17 OQ-8 keeps the MIDI range as the open
//! alternative). The sequence is monophonic: no note starts while another sounds and no two notes
//! start on one sample; a note may start on the sample the previous one ends. The notes may be
//! listed in any order; a note that overlaps one on an earlier line is refused on its own line.
//!
//! # Bounds
//!
//! The three bounds are provisional (DESIGN.md § 17, OQ-6, open): [`MAX_LINE_BYTES`] (bytes of a
//! line without its terminator), [`MAX_FILE_BYTES`] and [`MAX_NOTES`]. A reader takes at most
//! `MAX_FILE_BYTES + 1` bytes and refuses the file if it got them; a line past the line bound is
//! neither decoded nor split; the note count is checked before a note is stored, and a note past
//! the bound is not stored in the sequence. Its timing, range and overlap are still checked, and
//! it is still kept for the overlap check of later notes, because those faults outrank the bound;
//! the byte bound caps that work. `MAX_FILE_BYTES` admits `MAX_NOTES` notes of the longest
//! canonical line.
//!
//! # Refusals
//!
//! Any refusal loads nothing: the result is an [`EventsFileError`] naming the line (none for a
//! condition of the whole file) and an [`EventsFileReason`], which names the error of the ESS
//! domain it is an instance of ([`EventsFileReason::spec_error`]).
//!
//! # Precedence
//!
//! ESS answers a command with the first outcome it declares whose guard holds. So a file with
//! several faults is refused with the fault whose error `LoadNoteSequence` declares first
//! ([`LOAD_NOTE_SEQUENCE_PRECEDENCE`], [`EventsFileReason::precedence`]); among faults of one
//! rank, the one on the earliest line, and a fault of the whole file (a missing key) after every
//! line. The parser scans every line to find them, with the bounds above still in force.
//!
//! Two faults end the scan instead, as in the `.patch` parser: [`EventsFileReason::FileTooLarge`]
//! is answered alone, before any line is read, and so is a fault on line 1, because without a
//! version-1 header no later line has a meaning to be faulty in.
//!
//! A fault is not counted twice: after a line that cannot be read (past the line bound or not
//! UTF-8) no key is reported missing; a key whose line was refused is not also reported missing;
//! a note refused for its own fields or an overlap is not kept, so no later note is reported
//! overlapping it (a note refused only for the note bound is kept). A line with several
//! faults gives the highest-ranked of them.
//!
//! # Canonical form
//!
//! [`write_events`] emits the header, `sample_rate_hz`, `patch_id`, `note_estimation_id` when
//! there is one, then the notes in start order ([`NoteSequence::canonical`]). A velocity is
//! written as the shortest text that reads back to the same `f32`. Write then parse gives the
//! canonical sequence bit for bit; parse then write of a canonical file gives the same bytes.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// The first field of the header line.
pub const HEADER: &str = "patchback-notes";

/// The `notes.events` format version this module reads and writes.
pub const FORMAT_VERSION: u32 = 1;

/// The longest line, in bytes without its `\n`. Provisional (OQ-6).
pub const MAX_LINE_BYTES: usize = 256;

/// The largest file, in bytes. Provisional (OQ-6).
pub const MAX_FILE_BYTES: usize = 1 << 23;

/// The most notes a file may hold (§ 12.3 note count). Provisional (OQ-6).
pub const MAX_NOTES: usize = 65_536;

/// The errors of `LoadNoteSequence` a parser refusal names, in the order
/// `spec/domains/score.yaml` declares their outcomes: the precedence of refusals (module
/// documentation). The relations the domain declares between them (unknown patch, unknown
/// estimation, rate mismatch) are not the parser's.
pub const LOAD_NOTE_SEQUENCE_PRECEDENCE: [&str; 9] = [
    "patchback.score.UnknownFormatVersion",
    "patchback.score.InvalidNoteTiming",
    "patchback.score.MidiNoteOutOfRange",
    "patchback.score.VelocityOutOfRange",
    "patchback.score.OverlappingNotes",
    "patchback.score.DuplicateKey",
    "patchback.score.MalformedRecord",
    "patchback.score.MalformedNumber",
    "patchback.score.LengthBoundExceeded",
];

const SAMPLE_RATE_KEY: &str = "sample_rate_hz";
const PATCH_KEY: &str = "patch_id";
const ESTIMATION_KEY: &str = "note_estimation_id";
const NOTE: &str = "note";

/// One note (§ 10), timed in samples at the sequence's sample rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteEvent {
    pub start_sample: u64,
    pub end_sample: u64,
    pub midi_note: u8,
    pub velocity: f32,
}

/// The note events of one `notes.events` file, written for one patch.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteSequence {
    pub format_version: u32,
    pub sample_rate_hz: u32,
    /// The patch the sequence was written for, a lower-case hyphenated UUID.
    pub patch_id: String,
    /// The note estimation the sequence was written from; none for a hand-written file.
    pub note_estimation_id: Option<String>,
    /// The notes in the order the file lists them.
    pub notes: Vec<NoteEvent>,
}

impl NoteSequence {
    /// The sequence with its notes in start order, the order [`write_events`] emits. Starts are
    /// distinct in a sequence the parser accepts, so the order is total.
    pub fn canonical(&self) -> Self {
        let mut canonical = self.clone();
        canonical.notes.sort_by_key(|note| note.start_sample);
        canonical
    }
}

/// Why a file is refused.
#[derive(Clone, Debug, PartialEq)]
pub enum EventsFileReason {
    /// The first line is not `patchback-notes <version>`.
    MissingFormatVersion,
    /// The header names a version this parser does not read.
    UnknownFormatVersion { format_version: u32 },
    /// The first field of a line names no record of the format.
    UnknownRecord { record: String },
    /// A line holds no field.
    BlankLine,
    /// A record has more or fewer fields than its grammar.
    WrongFieldCount {
        record: String,
        expected: usize,
        found: usize,
    },
    /// A required key is absent from the file.
    MissingKey { key: String },
    /// The header or a key appears twice.
    DuplicateKey { key: String },
    /// A number is not an integer or finite decimal of the grammar.
    MalformedNumber { token: String },
    /// An identifier is not a lower-case hyphenated UUID.
    MalformedIdentifier { token: String },
    /// A note does not end after it starts.
    InvalidNoteTiming { start_sample: u64, end_sample: u64 },
    /// A note's MIDI number is above 127.
    MidiNoteOutOfRange { midi_note: u64 },
    /// A note's velocity is outside `[0, 1]`.
    VelocityOutOfRange { velocity: f32 },
    /// A note starts while a note of an earlier line sounds, or on the same sample.
    OverlappingNotes { start_sample: u64 },
    /// A line is not UTF-8.
    NotUtf8,
    /// A line is longer than [`MAX_LINE_BYTES`].
    LineTooLong { bytes: usize },
    /// The file is larger than [`MAX_FILE_BYTES`].
    FileTooLarge,
    /// A note beyond [`MAX_NOTES`].
    TooManyNotes,
}

impl EventsFileReason {
    /// The error of the ESS domain `patchback.score` this refusal is an instance of
    /// (`LoadNoteSequence`'s outcomes).
    pub fn spec_error(&self) -> &'static str {
        match self {
            Self::UnknownFormatVersion { .. } => "patchback.score.UnknownFormatVersion",
            Self::InvalidNoteTiming { .. } => "patchback.score.InvalidNoteTiming",
            Self::MidiNoteOutOfRange { .. } => "patchback.score.MidiNoteOutOfRange",
            Self::VelocityOutOfRange { .. } => "patchback.score.VelocityOutOfRange",
            Self::OverlappingNotes { .. } => "patchback.score.OverlappingNotes",
            Self::DuplicateKey { .. } => "patchback.score.DuplicateKey",
            Self::MalformedNumber { .. } => "patchback.score.MalformedNumber",
            Self::LineTooLong { .. } | Self::FileTooLarge | Self::TooManyNotes => {
                "patchback.score.LengthBoundExceeded"
            }
            Self::MissingFormatVersion
            | Self::UnknownRecord { .. }
            | Self::BlankLine
            | Self::WrongFieldCount { .. }
            | Self::MissingKey { .. }
            | Self::MalformedIdentifier { .. }
            | Self::NotUtf8 => "patchback.score.MalformedRecord",
        }
    }

    /// The rank of this refusal: the index of its [`spec_error`](Self::spec_error) in
    /// [`LOAD_NOTE_SEQUENCE_PRECEDENCE`]; lower is answered first.
    pub fn precedence(&self) -> usize {
        let error = self.spec_error();
        LOAD_NOTE_SEQUENCE_PRECEDENCE
            .iter()
            .position(|declared| *declared == error)
            .unwrap_or(LOAD_NOTE_SEQUENCE_PRECEDENCE.len())
    }
}

impl fmt::Display for EventsFileReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFormatVersion => {
                write!(f, "the first line is not `{HEADER} <format version>`")
            }
            Self::UnknownFormatVersion { format_version } => write!(
                f,
                "format version {format_version} is not read; this parser reads version {FORMAT_VERSION}"
            ),
            Self::UnknownRecord { record } => write!(f, "`{record}` is not a record"),
            Self::BlankLine => f.write_str("blank line"),
            Self::WrongFieldCount {
                record,
                expected,
                found,
            } => write!(
                f,
                "a `{record}` record has {expected} fields, this one has {found}"
            ),
            Self::MissingKey { key } => write!(f, "the key `{key}` is missing"),
            Self::DuplicateKey { key } => write!(f, "`{key}` appears twice"),
            Self::MalformedNumber { token } => {
                write!(f, "`{token}` is not a number of the format")
            }
            Self::MalformedIdentifier { token } => {
                write!(f, "`{token}` is not a lower-case hyphenated UUID")
            }
            Self::InvalidNoteTiming {
                start_sample,
                end_sample,
            } => write!(
                f,
                "the note starts at sample {start_sample} and ends at {end_sample}, not after it"
            ),
            Self::MidiNoteOutOfRange { midi_note } => {
                write!(f, "MIDI note {midi_note} is outside 0 to 127")
            }
            Self::VelocityOutOfRange { velocity } => {
                write!(f, "velocity {velocity} is outside 0 to 1")
            }
            Self::OverlappingNotes { start_sample } => write!(
                f,
                "the note at sample {start_sample} overlaps a note of an earlier line; the sequence is monophonic"
            ),
            Self::NotUtf8 => f.write_str("the line is not UTF-8"),
            Self::LineTooLong { bytes } => write!(
                f,
                "the line has {bytes} bytes, more than the bound of {MAX_LINE_BYTES}"
            ),
            Self::FileTooLarge => write!(
                f,
                "the file is larger than the bound of {MAX_FILE_BYTES} bytes"
            ),
            Self::TooManyNotes => write!(f, "more than {MAX_NOTES} notes"),
        }
    }
}

/// A refused `notes.events` file: the 1-based line, or none for a condition of the whole file,
/// and the reason.
#[derive(Clone, Debug, PartialEq)]
pub struct EventsFileError {
    pub line: Option<usize>,
    pub reason: EventsFileReason,
}

impl fmt::Display for EventsFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.reason),
            None => self.reason.fmt(f),
        }
    }
}

impl std::error::Error for EventsFileError {}

/// A `notes.events` file that could not be read, or was read and refused.
#[derive(Debug)]
pub enum ReadEventsError {
    Io(io::Error),
    Format(EventsFileError),
}

impl fmt::Display for ReadEventsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "cannot read the events file: {error}"),
            Self::Format(error) => write!(f, "events file refused: {error}"),
        }
    }
}

impl std::error::Error for ReadEventsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Format(error) => Some(error),
        }
    }
}

impl From<io::Error> for ReadEventsError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Parses a `notes.events` file held in memory.
pub fn parse_events(text: &str) -> Result<NoteSequence, EventsFileError> {
    parse_bytes(text.as_bytes())
}

/// Reads and parses a `notes.events` file, taking at most `MAX_FILE_BYTES + 1` bytes from
/// `reader`.
pub fn read_events(reader: impl Read) -> Result<NoteSequence, ReadEventsError> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    parse_bytes(&bytes).map_err(ReadEventsError::Format)
}

/// Opens and parses a `notes.events` file ([`read_events`]).
pub fn read_events_file(path: impl AsRef<Path>) -> Result<NoteSequence, ReadEventsError> {
    read_events(File::open(path)?)
}

/// Writes the canonical form of `sequence`. Each identifier is checked against the grammar on its
/// raw value before it is formatted, so whitespace or a line break in it is refused rather than
/// dropped or split by the field splitter; each formatted line is then checked by the parser. So
/// the writer never emits a file the parser refuses, nor one that reads back as another sequence;
/// the refusal is returned instead, naming the line of the canonical form. The other fields are
/// integers and a velocity formatted by this module, which hold no whitespace.
pub fn write_events(sequence: &NoteSequence) -> Result<String, EventsFileError> {
    let sequence = sequence.canonical();
    let mut writer = Writer::default();
    writer.line(format_args!("{HEADER} {}", sequence.format_version))?;
    writer.line(format_args!(
        "{SAMPLE_RATE_KEY} {}",
        sequence.sample_rate_hz
    ))?;
    let patch = writer.field(uuid(&sequence.patch_id))?;
    writer.line(format_args!("{PATCH_KEY} {patch}"))?;
    if let Some(estimation) = &sequence.note_estimation_id {
        let estimation = writer.field(uuid(estimation))?;
        writer.line(format_args!("{ESTIMATION_KEY} {estimation}"))?;
    }
    for note in &sequence.notes {
        writer.line(format_args!(
            "{NOTE} {} {} {} {}",
            note.start_sample,
            note.end_sample,
            note.midi_note,
            format_velocity(note.velocity)
        ))?;
    }
    writer.finish()
}

/// The shortest decimal text that reads back to `value` bit for bit: the plain form, or the
/// exponent form when it is strictly shorter (as `patch::format_decimal`, for `f32`).
fn format_velocity(value: f32) -> String {
    let plain = format!("{value}");
    let exponent = format!("{value:e}");
    if exponent.len() < plain.len() {
        exponent
    } else {
        plain
    }
}

fn parse_bytes(bytes: &[u8]) -> Result<NoteSequence, EventsFileError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(EventsFileError {
            line: None,
            reason: EventsFileReason::FileTooLarge,
        });
    }
    let mut parser = Parser::default();
    let mut faults = Faults::default();
    for (index, chunk) in bytes.split_inclusive(|byte| *byte == b'\n').enumerate() {
        let line = index + 1;
        let raw = match chunk.strip_suffix(b"\n") {
            Some(content) => content.strip_suffix(b"\r").unwrap_or(content),
            None => chunk,
        };
        if let Err(reason) = parser.line(raw) {
            let error = EventsFileError {
                line: Some(line),
                reason,
            };
            if line == 1 {
                return Err(error);
            }
            faults.note(error);
        }
    }
    parser.finish(faults)
}

/// The fault answered so far: the first-declared, then the earliest.
#[derive(Default)]
struct Faults(Option<EventsFileError>);

impl Faults {
    fn note(&mut self, error: EventsFileError) {
        if self
            .0
            .as_ref()
            .is_none_or(|kept| error.reason.precedence() < kept.reason.precedence())
        {
            self.0 = Some(error);
        }
    }
}

/// The highest-ranked of the faults of one line, the first listed among equals.
fn first_declared(faults: Vec<EventsFileReason>) -> Result<(), EventsFileReason> {
    match faults.into_iter().min_by_key(EventsFileReason::precedence) {
        Some(reason) => Err(reason),
        None => Ok(()),
    }
}

/// The value of a field, or none with its fault kept among the line's faults.
fn keep<T>(faults: &mut Vec<EventsFileReason>, field: Result<T, EventsFileReason>) -> Option<T> {
    field.map_err(|reason| faults.push(reason)).ok()
}

/// The canonical writer: each line goes through the parser before it is kept.
#[derive(Default)]
struct Writer {
    text: String,
    lines: usize,
    parser: Parser,
}

impl Writer {
    /// A field checked against the grammar on its raw value, refused on the line it would be
    /// written to.
    fn field<T>(&self, checked: Result<T, EventsFileReason>) -> Result<T, EventsFileError> {
        checked.map_err(|reason| EventsFileError {
            line: Some(self.lines + 1),
            reason,
        })
    }

    fn line(&mut self, args: fmt::Arguments<'_>) -> Result<(), EventsFileError> {
        let start = self.text.len();
        // Writing to a String cannot fail.
        let _ = self.text.write_fmt(args);
        self.lines += 1;
        let line = self.lines;
        self.parser
            .line(&self.text.as_bytes()[start..])
            .map_err(|reason| EventsFileError {
                line: Some(line),
                reason,
            })?;
        self.text.push('\n');
        Ok(())
    }

    fn finish(self) -> Result<String, EventsFileError> {
        self.parser.finish(Faults::default())?;
        if self.text.len() > MAX_FILE_BYTES {
            return Err(EventsFileError {
                line: None,
                reason: EventsFileReason::FileTooLarge,
            });
        }
        Ok(self.text)
    }
}

#[derive(Default)]
struct Parser {
    format_version: Option<u32>,
    sample_rate_hz: Option<u32>,
    sample_rate_seen: bool,
    patch_id: Option<String>,
    patch_seen: bool,
    note_estimation_id: Option<String>,
    estimation_seen: bool,
    notes: Vec<NoteEvent>,
    /// The kept notes by start sample: their end samples. These are the stored notes and those
    /// refused only for the note bound; they never overlap.
    sounding: BTreeMap<u64, u64>,
    /// A line could not be read, so what it declared is unknown.
    unreadable_line: bool,
}

impl Parser {
    fn line(&mut self, raw: &[u8]) -> Result<(), EventsFileReason> {
        if raw.len() > MAX_LINE_BYTES {
            self.unreadable_line = true;
            return Err(EventsFileReason::LineTooLong { bytes: raw.len() });
        }
        let Ok(text) = std::str::from_utf8(raw) else {
            self.unreadable_line = true;
            return Err(EventsFileReason::NotUtf8);
        };
        let fields: Vec<&str> = text.split_ascii_whitespace().collect();
        if self.format_version.is_none() {
            return self.header(&fields);
        }
        let Some(&record) = fields.first() else {
            return Err(EventsFileReason::BlankLine);
        };
        match record {
            HEADER => Err(EventsFileReason::DuplicateKey {
                key: HEADER.to_owned(),
            }),
            SAMPLE_RATE_KEY => {
                let seen = std::mem::replace(&mut self.sample_rate_seen, true);
                let value = key_value(&fields, seen, SAMPLE_RATE_KEY, parse_u32)?;
                self.sample_rate_hz = Some(value);
                Ok(())
            }
            PATCH_KEY => {
                let seen = std::mem::replace(&mut self.patch_seen, true);
                self.patch_id = Some(key_value(&fields, seen, PATCH_KEY, uuid)?);
                Ok(())
            }
            ESTIMATION_KEY => {
                let seen = std::mem::replace(&mut self.estimation_seen, true);
                self.note_estimation_id = Some(key_value(&fields, seen, ESTIMATION_KEY, uuid)?);
                Ok(())
            }
            NOTE => self.note(&fields),
            other => Err(EventsFileReason::UnknownRecord {
                record: other.to_owned(),
            }),
        }
    }

    fn header(&mut self, fields: &[&str]) -> Result<(), EventsFileReason> {
        if fields.first() != Some(&HEADER) || fields.len() < 2 {
            return Err(EventsFileReason::MissingFormatVersion);
        }
        expect_fields(fields, 2)?;
        let format_version = parse_u32(fields[1])?;
        if format_version != FORMAT_VERSION {
            return Err(EventsFileReason::UnknownFormatVersion { format_version });
        }
        self.format_version = Some(format_version);
        Ok(())
    }

    fn note(&mut self, fields: &[&str]) -> Result<(), EventsFileReason> {
        expect_fields(fields, 5)?;
        let mut faults = Vec::new();
        let start = keep(&mut faults, parse_u64(fields[1]));
        let end = keep(&mut faults, parse_u64(fields[2]));
        let midi = keep(&mut faults, parse_u64(fields[3]));
        let velocity = keep(&mut faults, parse_velocity(fields[4]));
        if let (Some(start_sample), Some(end_sample)) = (start, end)
            && end_sample <= start_sample
        {
            faults.push(EventsFileReason::InvalidNoteTiming {
                start_sample,
                end_sample,
            });
        }
        if let Some(midi_note) = midi.filter(|midi| *midi > 127) {
            faults.push(EventsFileReason::MidiNoteOutOfRange { midi_note });
        }
        if let Some(velocity) = velocity.filter(|v| !(0.0..=1.0).contains(v)) {
            faults.push(EventsFileReason::VelocityOutOfRange { velocity });
        }
        let (Some(start_sample), Some(end_sample), Some(midi), Some(velocity), true) =
            (start, end, midi, velocity, faults.is_empty())
        else {
            return first_declared(faults);
        };
        if self.overlaps(start_sample, end_sample) {
            return Err(EventsFileReason::OverlappingNotes { start_sample });
        }
        // A note past the bound is not stored, but it still sounds: a later note overlapping it
        // is an overlap, which outranks the bound. MAX_FILE_BYTES caps the notes kept here.
        self.sounding.insert(start_sample, end_sample);
        if self.notes.len() >= MAX_NOTES {
            return Err(EventsFileReason::TooManyNotes);
        }
        self.notes.push(NoteEvent {
            start_sample,
            end_sample,
            // At most 127, checked above.
            midi_note: midi as u8,
            velocity,
        });
        Ok(())
    }

    /// Whether `[start, end)` meets a kept note: one starting on `start`, one sounding at
    /// `start`, or one starting inside `(start, end)`. Kept notes do not overlap each other, so
    /// the nearest kept start on each side decides.
    fn overlaps(&self, start: u64, end: u64) -> bool {
        let before = self
            .sounding
            .range(..=start)
            .next_back()
            .is_some_and(|(&other_start, &other_end)| other_start == start || start < other_end);
        let after = self
            .sounding
            .range(start..)
            .next()
            .is_some_and(|(&other_start, _)| other_start < end);
        before || after
    }

    fn finish(self, mut faults: Faults) -> Result<NoteSequence, EventsFileError> {
        let missing = |key: &str| EventsFileError {
            line: None,
            reason: EventsFileReason::MissingKey {
                key: key.to_owned(),
            },
        };
        let Some(format_version) = self.format_version else {
            return Err(EventsFileError {
                line: Some(1),
                reason: EventsFileReason::MissingFormatVersion,
            });
        };
        if !self.unreadable_line {
            if !self.sample_rate_seen {
                faults.note(missing(SAMPLE_RATE_KEY));
            }
            if !self.patch_seen {
                faults.note(missing(PATCH_KEY));
            }
        }
        if let Some(error) = faults.0 {
            return Err(error);
        }
        Ok(NoteSequence {
            format_version,
            sample_rate_hz: self
                .sample_rate_hz
                .ok_or_else(|| missing(SAMPLE_RATE_KEY))?,
            patch_id: self.patch_id.ok_or_else(|| missing(PATCH_KEY))?,
            note_estimation_id: self.note_estimation_id,
            notes: self.notes,
        })
    }
}

/// The value of a key line `<key> <value>`, refused when the key was seen before; the highest
/// ranked fault when there are several.
fn key_value<T>(
    fields: &[&str],
    seen: bool,
    key: &str,
    value: fn(&str) -> Result<T, EventsFileReason>,
) -> Result<T, EventsFileReason> {
    let parsed = expect_fields(fields, 2).and_then(|()| value(fields[1]));
    if !seen {
        return parsed;
    }
    let duplicate = EventsFileReason::DuplicateKey {
        key: key.to_owned(),
    };
    Err(match parsed {
        Err(reason) if reason.precedence() < duplicate.precedence() => reason,
        _ => duplicate,
    })
}

fn expect_fields(fields: &[&str], expected: usize) -> Result<(), EventsFileReason> {
    if fields.len() == expected {
        Ok(())
    } else {
        Err(EventsFileReason::WrongFieldCount {
            record: fields.first().copied().unwrap_or_default().to_owned(),
            expected,
            found: fields.len(),
        })
    }
}

fn malformed(token: &str) -> EventsFileReason {
    EventsFileReason::MalformedNumber {
        token: token.to_owned(),
    }
}

/// An integer of the `.patch` grammar: ASCII digits only, no sign, fitting a `u32`.
fn parse_u32(token: &str) -> Result<u32, EventsFileReason> {
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(malformed(token));
    }
    token.parse().map_err(|_| malformed(token))
}

/// An integer of the `.patch` grammar, fitting a `u64`.
fn parse_u64(token: &str) -> Result<u64, EventsFileReason> {
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(malformed(token));
    }
    token.parse().map_err(|_| malformed(token))
}

/// A finite decimal of the `.patch` grammar (copied from `patch::file::parse_decimal`), read as
/// an `f32`.
fn parse_velocity(token: &str) -> Result<f32, EventsFileReason> {
    let bytes = token.as_bytes();
    let mut at = 0;
    let digits = |at: &mut usize| {
        let start = *at;
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
        }
        *at > start
    };
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        at += 1;
    }
    let mut well_formed = digits(&mut at);
    if well_formed && bytes.get(at) == Some(&b'.') {
        at += 1;
        well_formed = digits(&mut at);
    }
    if well_formed && matches!(bytes.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        well_formed = digits(&mut at);
    }
    if !well_formed || at != bytes.len() {
        return Err(malformed(token));
    }
    token
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| malformed(token))
}

/// A UUID in its lower-case hyphenated text form: `8-4-4-4-12` hexadecimal digits.
fn uuid(token: &str) -> Result<String, EventsFileReason> {
    let well_formed = token.len() == 36
        && token.bytes().enumerate().all(|(at, byte)| match at {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte),
        });
    if well_formed {
        Ok(token.to_owned())
    } else {
        Err(EventsFileReason::MalformedIdentifier {
            token: token.to_owned(),
        })
    }
}
