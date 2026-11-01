//! The estimation file `estimation.txt`, format `patchback-note-estimation 1`
//! (approval-record:wave-c1 Dc1-6), and the binding of a note sequence to the estimation it was
//! written from (Dc1-8).
//!
//! # The file, version 1
//!
//! UTF-8 text, one record per line, every line ending in `\n`, fields separated by one space, at
//! most [`ESTIMATION_MAX_LINE_BYTES`] bytes a line and [`ESTIMATION_MAX_FILE_BYTES`] a file. In
//! this order, nothing else:
//!
//! | Line | Fields |
//! |---|---|
//! | `patchback-note-estimation 1` | the header and format version |
//! | `note_estimation_id <uuid>` | the estimation id, a lower-case hyphenated UUID |
//! | `sample_rate_hz 48000` | the clip's rate |
//! | `frame_count <n>` | the clip's samples |
//! | `clip_sha256 <hex>` | the SHA-256 of the clip's samples ([`crate::clip_sha256`]), 64 lower-case hex digits |
//! | `notes_sha256 <hex>` | the SHA-256 of the note lines, each with its `\n`, in file order (Dc1-14); of no bytes when there are no notes |
//! | `config <name> <value>` | one per configuration entry, in order ([`crate::configuration`]): a name of `a-z`, `0-9` and `_`, a value of printable ASCII without spaces |
//! | `note <start> <gate_end> <tail_end> <midi> <f0_hz> <velocity> <onset> <pitch> <gate> <tail>` | one per note in onset order: three sample positions, the MIDI number, f0 in Hz, the velocity and the four confidences |
//!
//! Integers are decimal without sign or leading zeros; reals are as Rust prints an `f64` (the
//! velocity an `f32`): the shortest text that reads back to the same value, never NaN or
//! infinite. A note satisfies `start < gate_end <= tail_end <= frame_count`, `f0_hz > 0`, `midi`
//! the nearest MIDI number to `f0_hz` and not 0, the velocity in `[0, 1]`, each confidence in
//! `[0, 1]` and at or above its threshold ([`crate::ONSET_CONFIDENCE_THRESHOLD`] and the other
//! three), and starts at or after the previous note's tail end: what an accepted estimation
//! holds (Dc1-14). The note lines must hash to `notes_sha256`, so a note edited without its
//! digest reads as refused. The id must be [`crate::note_estimation_id`] of the file's digest
//! and configuration, so a file whose clip, configuration or id was changed reads as refused.
//! [`write_estimation`] writes the one text [`read_estimation`] reads back to the same value.

use std::fmt;

use patchback_core::digest::{hex, sha256};
use patchback_core::score::NoteSequence;

use crate::estimate::nearest_midi;
use crate::{
    EstimatedNote, GATE_CONFIDENCE_THRESHOLD, NoteEstimation, ONSET_CONFIDENCE_THRESHOLD,
    PITCH_CONFIDENCE_THRESHOLD, SAMPLE_RATE_HZ, TAIL_CONFIDENCE_THRESHOLD, note_estimation_id,
};

/// The file's name inside `patchback estimate --out`.
pub const ESTIMATION_FILE: &str = "estimation.txt";

/// The first field of the header line.
pub const ESTIMATION_HEADER: &str = "patchback-note-estimation";

/// The format version this module reads and writes.
pub const ESTIMATION_FORMAT_VERSION: u32 = 1;

/// The longest line, in bytes without its `\n`. Provisional.
pub const ESTIMATION_MAX_LINE_BYTES: usize = 512;

/// The largest file, in bytes. Provisional: the same bound as a `notes.events` file.
pub const ESTIMATION_MAX_FILE_BYTES: usize = 1 << 23;

/// The most notes a file holds. Provisional: the same bound as a `notes.events` file.
pub const ESTIMATION_MAX_NOTES: usize = 65_536;

/// Why an estimation file was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum EstimationReason {
    /// The file is empty or its first line is not `patchback-note-estimation <version>`.
    MissingHeader,
    /// A format version other than 1.
    UnknownFormatVersion { version: String },
    /// A record that is not the one this line must hold.
    UnexpectedRecord { expected: &'static str },
    /// A record with the wrong number of fields.
    FieldCount { expected: usize },
    /// A field that is not a number of its kind.
    MalformedNumber,
    /// An id that is not a lower-case hyphenated UUID, or a configuration name or value outside
    /// its grammar.
    MalformedIdentifier,
    /// A digest that is not 64 lower-case hex digits.
    MalformedDigest,
    /// A rate other than 48 kHz.
    UnsupportedSampleRate { sample_rate_hz: u32 },
    /// A note outside the bounds of the module documentation.
    InvalidNote { why: &'static str },
    /// The id is not the one the digest and configuration give.
    IdentityMismatch,
    /// The note lines are not the ones `notes_sha256` was computed over.
    NotesDigestMismatch,
    /// A line longer than [`ESTIMATION_MAX_LINE_BYTES`].
    LineTooLong,
    /// A file longer than [`ESTIMATION_MAX_FILE_BYTES`].
    FileTooLarge,
    /// More than [`ESTIMATION_MAX_NOTES`] notes.
    TooManyNotes,
    /// Bytes that are not UTF-8.
    NotUtf8,
    /// A last line without its `\n`.
    MissingNewline,
}

impl fmt::Display for EstimationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader => write!(f, "not a {ESTIMATION_HEADER} file"),
            Self::UnknownFormatVersion { version } => write!(
                f,
                "format version {version:?} is not {ESTIMATION_FORMAT_VERSION}"
            ),
            Self::UnexpectedRecord { expected } => write!(f, "expected a {expected} record"),
            Self::FieldCount { expected } => write!(f, "expected {expected} fields"),
            Self::MalformedNumber => write!(f, "malformed number"),
            Self::MalformedIdentifier => write!(f, "malformed identifier"),
            Self::MalformedDigest => write!(f, "malformed SHA-256 digest"),
            Self::UnsupportedSampleRate { sample_rate_hz } => write!(
                f,
                "sample rate {sample_rate_hz} Hz; only {SAMPLE_RATE_HZ} Hz"
            ),
            Self::InvalidNote { why } => write!(f, "invalid note: {why}"),
            Self::IdentityMismatch => write!(
                f,
                "the estimation id is not the one its clip digest and configuration give"
            ),
            Self::NotesDigestMismatch => write!(
                f,
                "the note lines are not the ones notes_sha256 was computed over"
            ),
            Self::LineTooLong => write!(f, "line longer than {ESTIMATION_MAX_LINE_BYTES} bytes"),
            Self::FileTooLarge => write!(f, "file larger than {ESTIMATION_MAX_FILE_BYTES} bytes"),
            Self::TooManyNotes => write!(f, "more than {ESTIMATION_MAX_NOTES} notes"),
            Self::NotUtf8 => write!(f, "not UTF-8"),
            Self::MissingNewline => write!(f, "the last line does not end in a newline"),
        }
    }
}

/// An estimation file refused at `line` (1-based; 0 for the file as a whole).
#[derive(Clone, Debug, PartialEq)]
pub struct EstimationFileError {
    pub line: usize,
    pub reason: EstimationReason,
}

impl fmt::Display for EstimationFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.reason)
        } else {
            write!(f, "line {}: {}", self.line, self.reason)
        }
    }
}

impl std::error::Error for EstimationFileError {}

/// The text of `estimation` (module documentation).
pub fn write_estimation(estimation: &NoteEstimation) -> String {
    let notes: String = estimation
        .notes
        .iter()
        .map(|note| {
            format!(
                "note {} {} {} {} {} {} {} {} {} {}\n",
                note.start_sample,
                note.end_sample,
                note.tail_end_sample,
                note.midi_note,
                note.f0_hz,
                note.velocity,
                note.onset_confidence,
                note.pitch_confidence,
                note.gate_confidence,
                note.tail_confidence
            )
        })
        .collect();
    let mut text = format!(
        "{ESTIMATION_HEADER} {ESTIMATION_FORMAT_VERSION}\nnote_estimation_id {}\nsample_rate_hz {}\nframe_count {}\nclip_sha256 {}\nnotes_sha256 {}\n",
        estimation.note_estimation_id,
        estimation.sample_rate_hz,
        estimation.frame_count,
        hex(&estimation.clip_sha256),
        hex(&sha256(notes.as_bytes()))
    );
    for (name, value) in &estimation.configuration {
        text.push_str(&format!("config {name} {value}\n"));
    }
    text.push_str(&notes);
    text
}

/// Reads an estimation file from its bytes (module documentation).
pub fn read_estimation_bytes(bytes: &[u8]) -> Result<NoteEstimation, EstimationFileError> {
    if bytes.len() > ESTIMATION_MAX_FILE_BYTES {
        return Err(EstimationFileError {
            line: 0,
            reason: EstimationReason::FileTooLarge,
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|_| EstimationFileError {
        line: 0,
        reason: EstimationReason::NotUtf8,
    })?;
    read_estimation(text)
}

/// Reads an estimation file held in memory (module documentation).
pub fn read_estimation(text: &str) -> Result<NoteEstimation, EstimationFileError> {
    let at = |line: usize| move |reason: EstimationReason| EstimationFileError { line, reason };
    if text.len() > ESTIMATION_MAX_FILE_BYTES {
        return Err(at(0)(EstimationReason::FileTooLarge));
    }
    if text.is_empty() {
        return Err(at(1)(EstimationReason::MissingHeader));
    }
    let body = match text.strip_suffix('\n') {
        Some(body) => body,
        None => {
            let line = text.split('\n').count();
            return Err(at(line)(EstimationReason::MissingNewline));
        }
    };
    let lines: Vec<&str> = body.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        if line.len() > ESTIMATION_MAX_LINE_BYTES {
            return Err(at(i + 1)(EstimationReason::LineTooLong));
        }
    }
    let fields = |i: usize| -> Vec<&str> {
        lines
            .get(i)
            .map_or_else(Vec::new, |l| l.split(' ').collect())
    };

    // 1: the header.
    let header = fields(0);
    if header.len() != 2 || header[0] != ESTIMATION_HEADER {
        return Err(at(1)(EstimationReason::MissingHeader));
    }
    if header[1] != ESTIMATION_FORMAT_VERSION.to_string() {
        return Err(at(1)(EstimationReason::UnknownFormatVersion {
            version: header[1].to_owned(),
        }));
    }
    let record = |i: usize, key: &'static str| -> Result<&str, EstimationFileError> {
        let f = fields(i);
        if f.first() != Some(&key) {
            return Err(at(i + 1)(EstimationReason::UnexpectedRecord {
                expected: key,
            }));
        }
        if f.len() != 2 {
            return Err(at(i + 1)(EstimationReason::FieldCount { expected: 2 }));
        }
        Ok(lines[i].split(' ').nth(1).unwrap_or(""))
    };
    // 2: the id.
    let id = record(1, "note_estimation_id")?;
    if !is_uuid(id) {
        return Err(at(2)(EstimationReason::MalformedIdentifier));
    }
    // 3: the rate.
    let rate: u32 =
        integer(record(2, "sample_rate_hz")?).ok_or(at(3)(EstimationReason::MalformedNumber))?;
    if rate != SAMPLE_RATE_HZ {
        return Err(at(3)(EstimationReason::UnsupportedSampleRate {
            sample_rate_hz: rate,
        }));
    }
    // 4: the frame count.
    let frame_count: u64 =
        integer(record(3, "frame_count")?).ok_or(at(4)(EstimationReason::MalformedNumber))?;
    // 5: the digest.
    let digest = record(4, "clip_sha256")?;
    let clip_sha256 = parse_digest(digest).ok_or(at(5)(EstimationReason::MalformedDigest))?;
    // 6: the digest of the note lines.
    let notes_digest = record(5, "notes_sha256")?;
    let notes_sha256 =
        parse_digest(notes_digest).ok_or(at(6)(EstimationReason::MalformedDigest))?;

    // The configuration, then the notes.
    let mut i = 6;
    let mut configuration = Vec::new();
    while i < lines.len() && fields(i).first() == Some(&"config") {
        let f = fields(i);
        if f.len() != 3 {
            return Err(at(i + 1)(EstimationReason::FieldCount { expected: 3 }));
        }
        let name_ok = !f[1].is_empty()
            && f[1]
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        let value_ok = !f[2].is_empty() && f[2].bytes().all(|b| b.is_ascii_graphic());
        if !name_ok || !value_ok {
            return Err(at(i + 1)(EstimationReason::MalformedIdentifier));
        }
        configuration.push((f[1].to_owned(), f[2].to_owned()));
        i += 1;
    }
    let first_note = i;
    let mut notes: Vec<EstimatedNote> = Vec::new();
    while i < lines.len() {
        let line = i + 1;
        let f = fields(i);
        if f.first() != Some(&"note") {
            return Err(at(line)(EstimationReason::UnexpectedRecord {
                expected: "note",
            }));
        }
        if f.len() != 11 {
            return Err(at(line)(EstimationReason::FieldCount { expected: 11 }));
        }
        if notes.len() == ESTIMATION_MAX_NOTES {
            return Err(at(line)(EstimationReason::TooManyNotes));
        }
        let number = || at(line)(EstimationReason::MalformedNumber);
        let start: u64 = integer(f[1]).ok_or_else(number)?;
        let gate: u64 = integer(f[2]).ok_or_else(number)?;
        let tail: u64 = integer(f[3]).ok_or_else(number)?;
        let midi: u64 = integer(f[4]).ok_or_else(number)?;
        let f0_hz = real(f[5]).ok_or_else(number)?;
        let velocity: f32 = real32(f[6]).ok_or_else(number)?;
        let confidences = [
            real(f[7]).ok_or_else(number)?,
            real(f[8]).ok_or_else(number)?,
            real(f[9]).ok_or_else(number)?,
            real(f[10]).ok_or_else(number)?,
        ];
        let invalid = |why: &'static str| at(line)(EstimationReason::InvalidNote { why });
        if gate <= start {
            return Err(invalid("the gate does not end after the onset"));
        }
        if tail < gate {
            return Err(invalid("the tail ends before the gate"));
        }
        if tail > frame_count {
            return Err(invalid("the tail ends after the clip"));
        }
        if let Some(previous) = notes.last()
            && start < previous.tail_end_sample
        {
            return Err(invalid("the onset is before the previous note's tail end"));
        }
        let midi_note = u8::try_from(midi)
            .ok()
            .filter(|m| *m <= 127)
            .ok_or_else(|| invalid("the MIDI number is above 127"))?;
        if f0_hz <= 0.0 {
            return Err(invalid("f0 is not positive"));
        }
        if midi_note == 0 {
            return Err(invalid("the MIDI number is 0"));
        }
        if midi_note != nearest_midi(f0_hz) {
            return Err(invalid("the MIDI number is not the nearest to f0"));
        }
        if !(0.0..=1.0).contains(&velocity) {
            return Err(invalid("the velocity is outside 0 to 1"));
        }
        if confidences.iter().any(|c| !(0.0..=1.0).contains(c)) {
            return Err(invalid("a confidence is outside 0 to 1"));
        }
        let thresholds = [
            ONSET_CONFIDENCE_THRESHOLD,
            PITCH_CONFIDENCE_THRESHOLD,
            GATE_CONFIDENCE_THRESHOLD,
            TAIL_CONFIDENCE_THRESHOLD,
        ];
        if confidences.iter().zip(thresholds).any(|(c, t)| *c < t) {
            return Err(invalid("a confidence is below its threshold"));
        }
        notes.push(EstimatedNote {
            start_sample: start,
            end_sample: gate,
            tail_end_sample: tail,
            midi_note,
            f0_hz,
            velocity,
            onset_confidence: confidences[0],
            pitch_confidence: confidences[1],
            gate_confidence: confidences[2],
            tail_confidence: confidences[3],
        });
        i += 1;
    }
    let note_text: String = lines[first_note..]
        .iter()
        .map(|l| format!("{l}\n"))
        .collect();
    if sha256(note_text.as_bytes()) != notes_sha256 {
        return Err(at(6)(EstimationReason::NotesDigestMismatch));
    }
    if note_estimation_id(&clip_sha256, &configuration) != id {
        return Err(at(2)(EstimationReason::IdentityMismatch));
    }
    Ok(NoteEstimation {
        note_estimation_id: id.to_owned(),
        sample_rate_hz: rate,
        frame_count,
        clip_sha256,
        configuration,
        notes,
    })
}

/// A decimal integer without sign or leading zeros.
fn integer<T: std::str::FromStr>(text: &str) -> Option<T> {
    let digits = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    let canonical = text == "0" || !text.starts_with('0');
    if digits && canonical {
        text.parse().ok()
    } else {
        None
    }
}

/// A finite real in the text Rust prints for an `f64`.
fn real(text: &str) -> Option<f64> {
    let value: f64 = text.parse().ok()?;
    (value.is_finite() && value.to_string() == text).then_some(value)
}

/// A finite real in the text Rust prints for an `f32`.
fn real32(text: &str) -> Option<f32> {
    let value: f32 = text.parse().ok()?;
    (value.is_finite() && value.to_string() == text).then_some(value)
}

fn parse_digest(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let mut digest = [0u8; 32];
    for (i, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(digest)
}

/// A lower-case hyphenated UUID: hex groups of 8, 4, 4, 4 and 12 digits.
fn is_uuid(text: &str) -> bool {
    let groups: Vec<&str> = text.split('-').collect();
    groups.len() == 5
        && groups.iter().zip([8, 4, 4, 4, 12]).all(|(g, len)| {
            g.len() == len
                && g.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

/// `patchback.score.NoteEstimationNotFound`: the sequence names a note estimation that is not
/// among the estimations given; no sequence is loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteEstimationNotFound {
    /// The id the sequence names.
    pub note_estimation_id: String,
}

impl NoteEstimationNotFound {
    /// The error of `spec/domains/score.yaml` this refusal is.
    pub fn domain_error(&self) -> &'static str {
        "patchback.score.NoteEstimationNotFound"
    }
}

impl fmt::Display for NoteEstimationNotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the sequence was written from note estimation {}, which is not among the estimations given",
            self.note_estimation_id
        )
    }
}

impl std::error::Error for NoteEstimationNotFound {}

/// Binds `sequence` to the estimation it was written from (Dc1-8, `unknown-estimation` of
/// `LoadNoteSequence`): `None` for a sequence that names none, the estimation of `estimations`
/// whose id it names, or [`NoteEstimationNotFound`] when none of them has that id.
pub fn bind_note_sequence<'a>(
    sequence: &NoteSequence,
    estimations: &'a [NoteEstimation],
) -> Result<Option<&'a NoteEstimation>, NoteEstimationNotFound> {
    match &sequence.note_estimation_id {
        None => Ok(None),
        Some(id) => estimations
            .iter()
            .find(|e| &e.note_estimation_id == id)
            .map(Some)
            .ok_or_else(|| NoteEstimationNotFound {
                note_estimation_id: id.clone(),
            }),
    }
}

/// `patchback.score.EstimationClipMismatch`: the estimation a sequence is bound to was made from
/// another clip than the one it is fitted against (Dc1-14, `estimation-of-another-clip` of
/// `LoadNoteSequence`); no sequence is loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EstimationClipMismatch {
    /// The estimation the sequence is bound to.
    pub note_estimation_id: String,
    /// The SHA-256 of the clip the estimation records.
    pub estimated_clip_sha256: [u8; 32],
    /// The SHA-256 of the clip the sequence is fitted against ([`crate::clip_sha256`]).
    pub fitted_clip_sha256: [u8; 32],
}

impl EstimationClipMismatch {
    /// The error of `spec/domains/score.yaml` this refusal is.
    pub fn domain_error(&self) -> &'static str {
        "patchback.score.EstimationClipMismatch"
    }
}

impl fmt::Display for EstimationClipMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "note estimation {} was made from the clip of SHA-256 {}, not from the fitted clip of SHA-256 {}",
            self.note_estimation_id,
            hex(&self.estimated_clip_sha256),
            hex(&self.fitted_clip_sha256)
        )
    }
}

impl std::error::Error for EstimationClipMismatch {}

/// Checks that `estimation` was made from the clip whose samples hash to `fitted_clip_sha256`
/// ([`crate::clip_sha256`]; Dc1-14).
pub fn check_estimation_clip(
    estimation: &NoteEstimation,
    fitted_clip_sha256: &[u8; 32],
) -> Result<(), EstimationClipMismatch> {
    if &estimation.clip_sha256 == fitted_clip_sha256 {
        Ok(())
    } else {
        Err(EstimationClipMismatch {
            note_estimation_id: estimation.note_estimation_id.clone(),
            estimated_clip_sha256: estimation.clip_sha256,
            fitted_clip_sha256: *fitted_clip_sha256,
        })
    }
}
