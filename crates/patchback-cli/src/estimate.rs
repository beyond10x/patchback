//! `patchback estimate --input <wav> --out <dir>` (DESIGN.md § 10, § 11, `EstimateNotes` of
//! `spec/domains/analysis.yaml`, approval-record:wave-c1 Dc1-6): reads the WAV through
//! `patchback-audio`, estimates its notes with `patchback_analysis::estimate_notes` (the
//! monophonic estimator; its module documentation states the method, the four confidences and
//! the polyphony rule) and writes two files into `<dir>`:
//!
//! | File | Contents |
//! |---|---|
//! | `estimation.txt` | `patchback-note-estimation 1`: the estimation id, the sample rate, the clip's frame count and SHA-256, the estimator's configuration, then one line per note (`patchback_analysis::write_estimation` states the records) |
//! | `notes.events` | the estimated notes as a note sequence that records the estimation id (`note_estimation_id`), the canonical text of `patchback_core::score::write_events`; its patch id is `patchback_analysis::ESTIMATED_SEQUENCE_PATCH_ID`, the sequence being written for no particular patch |
//!
//! Two runs over one clip write identical bytes: the estimation id is a version 8 UUID of the
//! clip's samples, the format version and the configuration (Dc1-7).
//!
//! All or nothing: both files are written only after the whole estimation succeeded, each to a
//! file that must not exist yet, and `<dir>` (with its missing parents) is created only then. A
//! write that fails removes what this run wrote and the directories it created.
//!
//! # Output
//!
//! UTF-8 text on stdout, one `key value` line per record, each ending in `\n`, in this order:
//!
//! | Key | Value |
//! |---|---|
//! | `out` | the directory written, as given, quoted and escaped as a refusal quotes a path (`Path`'s `Debug`), for example `out "estimation"` |
//! | `note_estimation_id` | the estimation's id |
//! | `notes` | how many notes were estimated |
//!
//! # Refusals
//!
//! One line on stderr, exit status 1, nothing written, `patchback: estimate refused: <error>:
//! <message>` for a domain refusal, `<error>` an error of `spec/domains/analysis.yaml`:
//!
//! 1. The output, before the input is read: an `--out` that exists and is not a directory, or the
//!    nearest existing ancestor of an absent `--out` that is not one: `cannot write "<that
//!    path>": is not a directory`; then an existing `<out>/estimation.txt` or
//!    `<out>/notes.events` (a symbolic link counts whether it resolves or not): `cannot write
//!    "<path>": already exists; it is never overwritten`.
//! 2. An `--input` that cannot be read: `cannot read "<path>": <message>`. A WAV the importer
//!    refuses keeps its `patchback.audio` error, except the refusals `AnalyzeAudio` names too,
//!    which are reported as the analysis error: no frames (`EmptyAudio`), a rate other than
//!    48 kHz (`UnsupportedSampleRate`), a non-finite sample (`NonFiniteSamples`) and a clip past
//!    the import bound (`DurationOverBound`).
//! 3. The estimation's refusals, in `EstimateNotes`'s order: the input guards
//!    (`patchback.analysis.EmptyAudio`, `UnsupportedSampleRate`, `DurationOverBound`,
//!    `NonFiniteSamples`, `SilentInput`), then `patchback.analysis.PolyphonicInput`, then
//!    `patchback.analysis.LowConfidenceNotes`, whose message lists each note with a confidence
//!    below its quantity's threshold, by index from 0, with those quantities and confidences:
//!    `<k> of <n> estimated notes are below their confidence thresholds: note <i> (<quantity>
//!    <confidence>, ...), ...; every note is kept with its confidences`.
//!
//! A file that cannot be written, or a directory that cannot be created, is refused `cannot
//! write "<path>": <message>`.
//!
//! # Binding a sequence to its estimation
//!
//! [`bind`] is the seam `render --estimation` and `fit --estimation` share (Dc1-8): a note
//! sequence that records a `note_estimation_id` is refused `patchback.score.NoteEstimationNotFound`
//! unless `--estimation` names the `estimation.txt` of that id; a given `--estimation` that cannot
//! be read, or that `patchback_analysis::read_estimation` refuses, is refused so too, whether or
//! not the sequence names an estimation.

use std::fs;
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use patchback_analysis::{
    ESTIMATION_FILE, ESTIMATION_MAX_FILE_BYTES, NoteEstimation, bind_note_sequence, estimate_notes,
    read_estimation_bytes, write_estimation,
};
use patchback_audio::{ReadError, WavError, read_wav};
use patchback_core::score::{NoteSequence, write_events};

/// The note sequence's file name inside `--out`.
pub const NOTES_FILE: &str = "notes.events";

/// Why `estimate` wrote nothing: the one line it prints on stderr.
pub struct Refusal(String);

impl Refusal {
    pub fn line(&self) -> &str {
        &self.0
    }

    fn domain(error: &str, message: impl std::fmt::Display) -> Self {
        Self::one_line(format!("patchback: estimate refused: {error}: {message}"))
    }

    /// The path is quoted with its control characters escaped (`Path` `Debug`), so a newline in
    /// a file name cannot split the refusal.
    fn io(what: &str, path: &Path, error: impl std::fmt::Display) -> Self {
        Self::one_line(format!(
            "patchback: estimate refused: {what} {path:?}: {error}"
        ))
    }

    /// A refusal is one line: any control character left in a message is escaped.
    fn one_line(text: String) -> Self {
        Self(escape_controls(&text))
    }
}

/// `text` with every control character escaped (`\n` as the two characters `\` `n`).
fn escape_controls(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            let escaped: Vec<char> = if c.is_control() {
                c.escape_default().collect()
            } else {
                vec![c]
            };
            escaped
        })
        .collect()
}

/// A WAV refusal that the analysis domain names too is reported with the analysis error, as
/// `analyze` reports it; every other refusal keeps its `patchback.audio` error.
fn wav_refusal(error: WavError) -> Refusal {
    let analysis = match error {
        WavError::EmptyAudio => Some("patchback.analysis.EmptyAudio"),
        WavError::UnsupportedSampleRate { .. } => Some("patchback.analysis.UnsupportedSampleRate"),
        WavError::NonFiniteSample { .. } => Some("patchback.analysis.NonFiniteSamples"),
        WavError::InputTooLong { .. } => Some("patchback.analysis.DurationOverBound"),
        _ => None,
    };
    Refusal::domain(analysis.unwrap_or(error.domain_error()), error)
}

/// A completed estimation and where it was written.
pub struct Written {
    pub estimation: NoteEstimation,
}

/// Runs `estimate`; on success both files are in `out`.
pub fn run(input: &Path, out: &Path) -> Result<Written, Refusal> {
    if let Some(blocking) = non_directory_on_the_way(out) {
        return Err(Refusal::io("cannot write", blocking, "is not a directory"));
    }
    let targets = [out.join(ESTIMATION_FILE), out.join(NOTES_FILE)];
    for target in &targets {
        if fs::symlink_metadata(target).is_ok() {
            return Err(Refusal::io(
                "cannot write",
                target,
                "already exists; it is never overwritten",
            ));
        }
    }
    let clip = read_wav(input).map_err(|error| match error {
        ReadError::Wav(error) => wav_refusal(error),
        ReadError::Io(error) => Refusal::io("cannot read", input, error),
    })?;
    let estimation = estimate_notes(clip.samples(), clip.sample_rate_hz())
        .map_err(|error| Refusal::domain(error.domain_error(), &error))?;
    let estimation_text = write_estimation(&estimation);
    let events_text = write_events(&estimation.note_sequence()).map_err(|error| {
        Refusal::one_line(format!(
            "patchback: estimate refused: the note sequence cannot be written: {error}"
        ))
    })?;
    write_all(
        out,
        &[
            (&targets[0], estimation_text.as_bytes()),
            (&targets[1], events_text.as_bytes()),
        ],
    )?;
    Ok(Written { estimation })
}

/// The stdout lines of a successful estimation (module documentation).
pub fn report(out: &Path, written: &Written) -> String {
    format!(
        "out {out:?}\nnote_estimation_id {}\nnotes {}\n",
        written.estimation.note_estimation_id,
        written.estimation.notes.len()
    )
}

/// The nearest of `out` and its ancestors that exists in any form, a symbolic link included,
/// when it is not a directory (a link counts as one only when it resolves to one).
fn non_directory_on_the_way(out: &Path) -> Option<&Path> {
    out.ancestors()
        .filter(|path| !path.as_os_str().is_empty())
        .find(|path| fs::symlink_metadata(path).is_ok())
        .filter(|path| !path.is_dir())
}

/// Creates `out` and its missing parents and writes each file, all or nothing: on a failure the
/// files written and the directories created are removed.
fn write_all(out: &Path, files: &[(&PathBuf, &[u8])]) -> Result<(), Refusal> {
    let missing: Vec<PathBuf> = out
        .ancestors()
        .filter(|path| !path.as_os_str().is_empty())
        .take_while(|path| fs::symlink_metadata(path).is_err())
        .map(Path::to_path_buf)
        .collect();
    let undo = |written: &[&PathBuf]| {
        for path in written {
            let _ = fs::remove_file(path);
        }
        for directory in &missing {
            let _ = fs::remove_dir(directory);
        }
    };
    if let Err(error) = fs::create_dir_all(out) {
        undo(&[]);
        return Err(Refusal::io("cannot write", out, error));
    }
    let mut written: Vec<&PathBuf> = Vec::new();
    for (path, bytes) in files {
        if let Err(error) = write_new(path, bytes) {
            undo(&written);
            return Err(Refusal::io("cannot write", path, error));
        }
        written.push(path);
    }
    Ok(())
}

/// Writes `bytes` to a file that must not exist yet; a failed write removes the partial file.
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if written.is_err() {
        let _ = fs::remove_file(path);
    }
    written
}

/// Binds `sequence`, read from `notes`, to the estimation it records (module documentation,
/// Dc1-8). On a refusal returns the message that follows the verb's `NoteSequenceNotFound`:
/// `"<notes>": patchback.score.NoteEstimationNotFound: <why>`.
pub fn bind(
    sequence: &NoteSequence,
    notes: &Path,
    estimation: Option<&Path>,
) -> Result<(), String> {
    let refused = |why: String| format!("{notes:?}: patchback.score.NoteEstimationNotFound: {why}");
    let estimations = match estimation {
        Some(path) => vec![read_estimation_file(path).map_err(refused)?],
        None => Vec::new(),
    };
    bind_note_sequence(sequence, &estimations)
        .map(|_| ())
        .map_err(|error| {
            refused(match estimation {
                Some(path) => format!(
                    "{error}; --estimation {path:?} is estimation {}",
                    estimations[0].note_estimation_id
                ),
                None => format!("{error}; give its estimation.txt with --estimation"),
            })
        })
}

/// The estimation in the file at `path`, read up to the format's byte bound.
fn read_estimation_file(path: &Path) -> Result<NoteEstimation, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| {
            file.take(ESTIMATION_MAX_FILE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|error| format!("the estimation {path:?} cannot be read: {error}"))?;
    read_estimation_bytes(&bytes)
        .map_err(|error| format!("the estimation {path:?} is refused: {error}"))
}
