//! `patchback render --patch <file> --notes <file> --out <wav> [--block-size <frames>]`
//! (DESIGN.md § 11, `RenderPatch` of `spec/domains/audio.yaml`): loads, validates and compiles
//! the `.patch` file through `patchback-core`, reads the `notes.events` file, renders the plan
//! with `patchback-dsp` and writes a mono 48 kHz float32 WAV through `patchback-audio`.
//!
//! All or nothing: the WAV is written only after the whole render succeeded, to a file that must
//! not exist yet (an existing `--out` is never overwritten), and a failed write removes what it
//! wrote. A refused render writes nothing.
//!
//! # Output
//!
//! UTF-8 text on stdout, one `key value` line per record, each ending in `\n`, in this order:
//!
//! | Key | Value |
//! |---|---|
//! | `out` | the WAV written, as given, quoted and escaped as a refusal quotes a path (`Path`'s `Debug`: `\\` for a backslash, `\n` for a newline, `\"` for a quote, `\xNN` for a byte that is not UTF-8), so two paths never print one record; for example `out "out.wav"` |
//! | `sample_rate_hz` | `48000` |
//! | `block_size_frames` | the block size the render ran with (default 256) |
//! | `frame_count` | the samples written: the last note's end, the release and the tail |
//! | `limited` | `true` when the protective output limiter changed a sample, else `false` |
//! | `limited_samples` | how many samples it changed |
//!
//! # Refusals
//!
//! One line on stderr, exit status 1, `patchback: render refused: <error>: <message>`, where
//! `<error>` is an error of `spec/domains/audio.yaml`. They are answered in the order
//! `RenderPatch` declares its outcomes:
//!
//! 1. `patchback.audio.UnsupportedSampleRate`: the patch refused for a rate other than 48 kHz,
//!    or a note sequence at another rate. The render would run at that rate; nothing is
//!    converted.
//! 2. `patchback.audio.InvalidBlockSize`: `--block-size` of zero or less.
//! 3. `patchback.audio.PatchNotFound`: a `.patch` file that cannot be read, or that `LoadPatch`
//!    refuses (its `patchback.patch` error follows in the message): no patch was loaded.
//! 4. `patchback.audio.NoteSequenceNotFound`: a `notes.events` file that cannot be read or that
//!    the parser refuses (its `patchback.score` error follows).
//! 5. `patchback.audio.PatchNotCompiled`: a patch that loaded and that `ValidatePatch` refuses;
//!    it is in state `Loaded` and is never executed (its `patchback.patch` error follows).
//! 6. `patchback.audio.InputTooLong`: a render longer than [`patchback_audio::MAX_FRAMES`]
//!    frames, refused before anything is rendered.
//! 7. `patchback.audio.NonFiniteRender`: the render produced a NaN or infinite sample.
//!
//! The sequence's recorded patch is not compared with `--patch` (`RenderPatch`: any patch at the
//! sequence's rate may render it).
//!
//! The output file is not part of the model: an existing `--out`, or one that cannot be written,
//! is refused as `patchback: render refused: cannot write "<path>": <message>`.
//!
//! # Test seam
//!
//! A bounded voice cannot produce a non-finite sample, so the last refusal cannot be reached
//! from a file. In a debug build only, the environment variable
//! `PATCHBACK_TEST_RENDER_NON_FINITE_FRAME` holding a
//! frame number replaces that rendered sample with NaN before the write path checks it; a
//! release build does not read it.

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use patchback_audio::{AudioClip, WavError, WavSampleFormat, encode};
use patchback_core::patch::{
    LoadPatchError, VALIDATE_PATCH_PRECEDENCE, ValidatedPatch, compile, load_patch_file,
};
use patchback_core::score::{NoteSequence, ReadEventsError, read_events_file};
use patchback_dsp::{
    RenderConfig, RenderError, RenderReport, SAMPLE_RATE_HZ, render as render_plan,
};

/// The environment variable of the test seam (module documentation).
#[cfg(debug_assertions)]
const NON_FINITE_SEAM: &str = "PATCHBACK_TEST_RENDER_NON_FINITE_FRAME";

/// Why `render` wrote nothing: the one line it prints on stderr.
pub struct Refusal(String);

impl Refusal {
    pub fn line(&self) -> &str {
        &self.0
    }

    fn domain(error: &str, message: impl std::fmt::Display) -> Self {
        Self::one_line(format!("patchback: render refused: {error}: {message}"))
    }

    fn io(path: &Path, error: impl std::fmt::Display) -> Self {
        Self::one_line(format!(
            "patchback: render refused: cannot write {path:?}: {error}"
        ))
    }

    /// A refusal is one line: any control character left in a message is escaped. Paths are
    /// quoted with `Path`'s `Debug`, which escapes them too.
    fn one_line(text: String) -> Self {
        Self(escape_controls(&text))
    }
}

/// `text` with every control character escaped (`\n` as the two characters `\` `n`), so it fits
/// on one line: the escaping of a refusal. A path, in a refusal and in the `out` record of a
/// report alike, is quoted with `Path`'s `Debug`, which leaves no control character to escape.
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

impl From<RenderError> for Refusal {
    fn from(error: RenderError) -> Self {
        Self::domain(error.domain_error(), error)
    }
}

/// A successful render: where it was written and what the renderer reported.
pub struct Written {
    pub report: RenderReport,
}

/// Whether no patch was loaded: the file could not be read, the parser refused it, or the graph
/// check refused it with an error `LoadPatch` declares. [`VALIDATE_PATCH_PRECEDENCE`] lists
/// `LoadPatch`'s errors first and `ValidatePatch`'s from `UnknownNode` on; a patch refused with
/// one of those loaded, is in state `Loaded`, and was not compiled.
fn refused_at_load(error: &LoadPatchError) -> bool {
    let first_validate = VALIDATE_PATCH_PRECEDENCE
        .iter()
        .position(|e| *e == "patchback.patch.UnknownNode")
        .unwrap_or(VALIDATE_PATCH_PRECEDENCE.len());
    match error {
        LoadPatchError::Io(_) | LoadPatchError::File(_) => true,
        LoadPatchError::Graph(graph) => graph.precedence() < first_validate,
    }
}

/// Runs `render`; on success the WAV is at `out`.
pub fn run(
    patch: &Path,
    notes: &Path,
    out: &Path,
    block_size_frames: Option<i64>,
) -> Result<Written, Refusal> {
    let loaded = load_patch_file(patch);
    let sequence = read_events_file(notes);

    // 1. The rate of the render: the patch's, then the sequence's.
    if let Err(error) = &loaded
        && error.spec_error() == Some("patchback.patch.UnsupportedSampleRate")
    {
        return Err(Refusal::domain(
            "patchback.audio.UnsupportedSampleRate",
            format_args!("{patch:?}: {error}"),
        ));
    }
    if let Ok(sequence) = &sequence
        && sequence.sample_rate_hz != SAMPLE_RATE_HZ
    {
        return Err(Refusal::domain(
            "patchback.audio.UnsupportedSampleRate",
            format_args!(
                "{notes:?}: sample rate {} Hz; the renderer runs at {SAMPLE_RATE_HZ} Hz only and converts nothing",
                sequence.sample_rate_hz
            ),
        ));
    }
    // 2. The block size.
    let config = RenderConfig {
        block_size_frames: block_size_frames.unwrap_or(RenderConfig::default().block_size_frames),
    };
    if config.block_size_frames <= 0 {
        return Err(RenderError::InvalidBlockSize {
            block_size_frames: config.block_size_frames,
        }
        .into());
    }
    // 3. The patch, 4. the sequence, 5. the patch's compilation.
    let validated = patch_lookup(patch, loaded)?;
    let sequence = sequence_lookup(notes, sequence)?;
    let validated = validated.map_err(|error| {
        Refusal::domain(
            "patchback.audio.PatchNotCompiled",
            format_args!(
                "{patch:?}: state Loaded: {}: {error}",
                error
                    .spec_error()
                    .unwrap_or("patchback.patch.ValidatePatch")
            ),
        )
    })?;

    if out.exists() {
        return Err(Refusal::io(out, "already exists; it is never overwritten"));
    }
    let plan = compile(&validated);
    let rendered = render_plan(&plan, &sequence, &config).map_err(|error| match error {
        RenderError::NonFiniteRender { frame } => non_finite(patch, frame),
        error => error.into(),
    })?;
    let mut samples = rendered.samples;
    inject_non_finite(&mut samples);
    let clip = AudioClip::from_mono(samples).map_err(|error| match error {
        WavError::NonFiniteSample { frame, .. } => non_finite(patch, frame),
        WavError::InputTooLong { frame_count } => RenderError::RenderTooLong { frame_count }.into(),
        error => Refusal::domain(error.domain_error(), error),
    })?;
    let bytes = encode(&clip, WavSampleFormat::Float32)
        .map_err(|error| Refusal::domain(error.domain_error(), error))?;
    write_new(out, &bytes).map_err(|error| Refusal::io(out, error))?;
    Ok(Written {
        report: rendered.report,
    })
}

/// A patch that was not loaded is not found; a loaded patch is returned with its validation.
fn patch_lookup(
    patch: &Path,
    loaded: Result<ValidatedPatch, LoadPatchError>,
) -> Result<Result<ValidatedPatch, LoadPatchError>, Refusal> {
    match loaded {
        Err(error) if refused_at_load(&error) => Err(Refusal::domain(
            "patchback.audio.PatchNotFound",
            match error.spec_error() {
                Some(name) => format!("{patch:?}: {name}: {error}"),
                None => format!("{patch:?}: {error}"),
            },
        )),
        loaded => Ok(loaded),
    }
}

fn sequence_lookup(
    notes: &Path,
    sequence: Result<NoteSequence, ReadEventsError>,
) -> Result<NoteSequence, Refusal> {
    sequence.map_err(|error| {
        Refusal::domain(
            "patchback.audio.NoteSequenceNotFound",
            match &error {
                ReadEventsError::Io(io) => format!("{notes:?}: {io}"),
                ReadEventsError::Format(format) => {
                    format!("{notes:?}: {}: {format}", format.reason.spec_error())
                }
            },
        )
    })
}

fn non_finite(patch: &Path, frame: u64) -> Refusal {
    Refusal::domain(
        "patchback.audio.NonFiniteRender",
        format_args!(
            "{patch:?}: the render produced a non-finite sample at frame {frame}; nothing was written"
        ),
    )
}

/// The test seam (module documentation): debug builds only.
#[cfg(debug_assertions)]
fn inject_non_finite(samples: &mut [f32]) {
    let frame = std::env::var(NON_FINITE_SEAM)
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    if let Some(sample) = frame.and_then(|frame| samples.get_mut(frame)) {
        *sample = f32::NAN;
    }
}

#[cfg(not(debug_assertions))]
fn inject_non_finite(_samples: &mut [f32]) {}

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

/// The stdout lines of a successful render (module documentation).
pub fn report(out: &Path, report: &RenderReport) -> String {
    format!(
        "out {out:?}\nsample_rate_hz {SAMPLE_RATE_HZ}\nblock_size_frames {}\nframe_count {}\nlimited {}\nlimited_samples {}\n",
        report.block_size_frames, report.frame_count, report.limited, report.limited_samples
    )
}
