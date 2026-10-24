//! Adversary cases, second pass, for story:external-audio-provenance (wave a5, unit 2): the
//! correction's "note the fault and keep reading" walk (`first` in `src/read.rs`) and the guards
//! answered before the file is opened (`src/import.rs`).
//!
//! The order is `IMPORT_WAV_PRECEDENCE`. Every case is read on each path: `decode`, a regular
//! file, a pipe; with no request and with a request both guards pass.

mod support;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use patchback_audio::{
    AudioClip, AudioOrigin, Corruption, MAX_FMT_BYTES, ReadError, WavError, WavImport, decode,
    read_wav,
};
use support::*;

const PROVENANCE: &str = "recorded by the project from a hardware monosynth, 2026-10-20";
const LICENCE: &str = "CC-BY-4.0";

const RATE: &str = "patchback.audio.UnsupportedSampleRate";
const FORMAT: &str = "patchback.audio.UnsupportedSampleFormat";
const CHANNELS: &str = "patchback.audio.UnsupportedChannelCount";
const EMPTY: &str = "patchback.audio.EmptyAudio";
const TRUNCATED: &str = "patchback.audio.TruncatedRiff";
const CORRUPT: &str = "patchback.audio.CorruptRiffChunk";

fn external(provenance: Option<&str>, licence: Option<&str>) -> WavImport {
    WavImport {
        origin: AudioOrigin::ExternalRecording,
        provenance: provenance.map(str::to_owned),
        licence: licence.map(str::to_owned),
        render_id: None,
    }
}

fn both() -> WavImport {
    external(Some(PROVENANCE), Some(LICENCE))
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-audio-provenance-adversary2");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

fn read_with(import: Option<&WavImport>, path: &Path) -> Result<AudioClip, WavError> {
    let result = match import {
        Some(import) => import.read(path),
        None => read_wav(path),
    };
    match result {
        Ok(clip) => Ok(clip),
        Err(ReadError::Wav(error)) => Err(error),
        Err(ReadError::Io(error)) => panic!("{} could not be read: {error}", path.display()),
    }
}

fn through_a_pipe(import: Option<&WavImport>, bytes: &[u8]) -> Result<AudioClip, WavError> {
    use std::os::fd::AsRawFd;
    let (reader, mut writer) = std::io::pipe().unwrap();
    let path = PathBuf::from(format!("/proc/self/fd/{}", reader.as_raw_fd()));
    let bytes = bytes.to_vec();
    let feeder = std::thread::spawn(move || {
        let _ = writer.write_all(&bytes);
    });
    let result = read_with(import, &path);
    drop(reader);
    feeder.join().unwrap();
    result
}

/// Every path's answer to `bytes` with `import`, labelled.
fn answers(import: Option<&WavImport>, bytes: &[u8]) -> Vec<(&'static str, Result<(), WavError>)> {
    let path = scratch(&format!(
        "case-{}.wav",
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, bytes).unwrap();
    let decoded = match import {
        Some(import) => import.decode(bytes),
        None => decode(bytes),
    };
    let out = vec![
        ("decode", decoded.map(|_| ())),
        ("file", read_with(import, &path).map(|_| ())),
        ("pipe", through_a_pipe(import, bytes).map(|_| ())),
    ];
    std::fs::remove_file(&path).unwrap();
    out
}

/// Records every path whose answer is not the domain error `expected`.
fn check(wrong: &mut Vec<String>, what: &str, bytes: &[u8], expected: &str) {
    for import in [None, Some(&both())] {
        for (read_path, answer) in answers(import, bytes) {
            let got = match &answer {
                Ok(()) => "a clip",
                Err(error) => error.domain_error(),
            };
            if got != expected {
                wrong.push(format!(
                    "{what} ({}, {read_path}): expected {expected}, got {answer:?}",
                    if import.is_some() {
                        "both given"
                    } else {
                        "no request"
                    }
                ));
            }
        }
    }
}

/// Records every path whose answer is not exactly `expected`.
fn check_exact(wrong: &mut Vec<String>, what: &str, bytes: &[u8], expected: WavError) {
    for import in [None, Some(&both())] {
        for (read_path, answer) in answers(import, bytes) {
            if answer != Err(expected) {
                wrong.push(format!(
                    "{what} ({}, {read_path}): expected {expected:?}, got {answer:?}",
                    if import.is_some() {
                        "both given"
                    } else {
                        "no request"
                    }
                ));
            }
        }
    }
}

fn verdict(wrong: Vec<String>) {
    assert!(
        wrong.is_empty(),
        "{} answers out of order:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

fn fmt_chunk(tag: u16, channels: u16, rate: u32, bits: u16) -> Vec<u8> {
    chunk(b"fmt ", &fmt_body(tag, channels, rate, bits))
}

fn long_fmt(body: Vec<u8>) -> Vec<u8> {
    let mut body = body;
    body.resize(usize::try_from(MAX_FMT_BYTES).unwrap() + 2, 0);
    chunk(b"fmt ", &body)
}

const RATE_44100: WavError = WavError::UnsupportedSampleRate {
    sample_rate_hz: 44_100,
};

// 1. Which of two differing `fmt ` chunks supplies the header facts the predicates judge.

/// The first `fmt ` counts (the walk's documented rule): a valid one followed by a 44.1 kHz one
/// is the duplicate's corruption, a 44.1 kHz one followed by a valid one is the rate. Likewise
/// for a first `fmt ` over its bound. A reader that took the facts from the later `fmt ` would
/// answer the other way round in each pair.
#[test]
fn the_first_of_two_differing_fmt_chunks_supplies_the_header_facts() {
    let data = chunk(b"data", &pcm16_data(&[0, 1]));
    let mut wrong = Vec::new();
    check_exact(
        &mut wrong,
        "a valid fmt, then a 44.1 kHz fmt",
        &riff(&[
            fmt_chunk(1, 1, 48_000, 16),
            fmt_chunk(1, 1, 44_100, 16),
            data.clone(),
        ]),
        WavError::CorruptChunk(Corruption::DuplicateFmt),
    );
    check_exact(
        &mut wrong,
        "a 44.1 kHz fmt, then a valid fmt",
        &riff(&[
            fmt_chunk(1, 1, 44_100, 16),
            fmt_chunk(1, 1, 48_000, 16),
            data.clone(),
        ]),
        RATE_44100,
    );
    check_exact(
        &mut wrong,
        "a 44.1 kHz fmt over its bound, then a valid fmt",
        &riff(&[
            long_fmt(fmt_body(1, 1, 44_100, 16)),
            fmt_chunk(1, 1, 48_000, 16),
            data.clone(),
        ]),
        RATE_44100,
    );
    check_exact(
        &mut wrong,
        "a valid fmt over its bound, then a 44.1 kHz fmt",
        &riff(&[
            long_fmt(fmt_body(1, 1, 48_000, 16)),
            fmt_chunk(1, 1, 44_100, 16),
            data.clone(),
        ]),
        WavError::CorruptChunk(Corruption::FmtSize {
            size: MAX_FMT_BYTES + 2,
        }),
    );
    check_exact(
        &mut wrong,
        "a 44.1 kHz fmt after the data, then a valid fmt",
        &riff(&[
            data.clone(),
            fmt_chunk(1, 1, 44_100, 16),
            fmt_chunk(1, 1, 48_000, 16),
        ]),
        RATE_44100,
    );
    verdict(wrong);
}

// 2. Which of two differing `data` chunks decides `empty`.

/// The first `data` counts: two frames, then an empty `data`, is the duplicate's corruption; an
/// empty `data`, then two frames, is `EmptyAudio`. A reader that judged `empty` on the later
/// `data` chunk would answer the other way round.
#[test]
fn the_first_of_two_differing_data_chunks_decides_empty() {
    let fmt = fmt_chunk(1, 1, 48_000, 16);
    let two = chunk(b"data", &pcm16_data(&[0, 1]));
    let none = chunk(b"data", &[]);
    let mut wrong = Vec::new();
    check_exact(
        &mut wrong,
        "two frames, then an empty data chunk",
        &riff(&[fmt.clone(), two.clone(), none.clone()]),
        WavError::CorruptChunk(Corruption::DuplicateData),
    );
    check_exact(
        &mut wrong,
        "an empty data chunk, then two frames",
        &riff(&[fmt.clone(), none.clone(), two.clone()]),
        WavError::EmptyAudio,
    );
    check_exact(
        &mut wrong,
        "an empty data chunk before the fmt, then two frames",
        &riff(&[none, fmt, two]),
        WavError::EmptyAudio,
    );
    verdict(wrong);
}

// 3. Three faults in one file.

/// A second `fmt `, a second `data` and a third fault: a truncation answers before both
/// corruptions; an unprintable chunk id ties with them and the first met answers, as `first`
/// documents ("on a tie the recorded one"), on every path; a 44.1 kHz first `fmt ` answers
/// before all three.
#[test]
fn three_faults_answer_with_the_first_ranked_and_on_a_tie_the_first_met() {
    let data = chunk(b"data", &pcm16_data(&[0, 1]));
    let mut wrong = Vec::new();
    let with = |first: Vec<u8>, last: Vec<u8>| {
        riff(&[
            first,
            fmt_chunk(1, 1, 48_000, 16),
            data.clone(),
            data.clone(),
            last,
        ])
    };
    check(
        &mut wrong,
        "second fmt, second data, a chunk past the end",
        &with(fmt_chunk(1, 1, 48_000, 16), chunk_header(b"LIST", 100)),
        TRUNCATED,
    );
    check_exact(
        &mut wrong,
        "second fmt, second data, an unprintable chunk id",
        &with(fmt_chunk(1, 1, 48_000, 16), chunk(&[0, 1, 2, 3], &[0; 4])),
        WavError::CorruptChunk(Corruption::DuplicateFmt),
    );
    check_exact(
        &mut wrong,
        "a fmt over its bound, second fmt, second data, an unprintable chunk id",
        &with(
            long_fmt(fmt_body(1, 1, 48_000, 16)),
            chunk(&[0, 1, 2, 3], &[0; 4]),
        ),
        WavError::CorruptChunk(Corruption::FmtSize {
            size: MAX_FMT_BYTES + 2,
        }),
    );
    check_exact(
        &mut wrong,
        "a 44.1 kHz fmt, second fmt, second data, a chunk past the end",
        &with(fmt_chunk(1, 1, 44_100, 16), chunk_header(b"LIST", 100)),
        RATE_44100,
    );
    verdict(wrong);
}

// 4. Byte-level faults the correction's matrix leaves out: the `data` chunk before the `fmt `,
// the `data` chunk itself past the RIFF size, a `fmt ` cut after its 16 bytes.

/// The `fmt ` body and `data` bytes of each input refusal of the reader (3 to 6).
fn predicates() -> Vec<(&'static str, &'static str, Vec<u8>, Vec<u8>)> {
    vec![
        ("44.1 kHz", RATE, fmt_body(1, 1, 44_100, 16), vec![0; 4]),
        ("24-bit PCM", FORMAT, fmt_body(1, 1, 48_000, 24), vec![0; 6]),
        (
            "three channels",
            CHANNELS,
            fmt_body(1, 3, 48_000, 16),
            vec![0; 12],
        ),
        ("no frames", EMPTY, fmt_body(1, 1, 48_000, 16), Vec::new()),
    ]
}

type Fault = fn(Vec<u8>, Vec<u8>) -> Option<Vec<u8>>;

/// Files with the `data` chunk ahead of the `fmt `, or a fault on the `data` or `fmt ` chunk
/// itself, whose header facts are still read; with the error each is alone.
fn faults_beyond_the_matrix() -> Vec<(&'static str, &'static str, Fault)> {
    vec![
        (
            "data first, a block align that disagrees",
            CORRUPT,
            |mut fmt, data| {
                let block_align = u16::from_le_bytes([fmt[12], fmt[13]]);
                fmt[12..14].copy_from_slice(&(block_align + 1).to_le_bytes());
                Some(riff(&[chunk(b"data", &data), chunk(b"fmt ", &fmt)]))
            },
        ),
        (
            "data first, a byte rate that disagrees",
            CORRUPT,
            |mut fmt, data| {
                fmt[8..12].copy_from_slice(&1u32.to_le_bytes());
                Some(riff(&[chunk(b"data", &data), chunk(b"fmt ", &fmt)]))
            },
        ),
        ("data first, a partial frame", CORRUPT, |fmt, data| {
            if data.is_empty() {
                return None;
            }
            let block_align = usize::from(u16::from_le_bytes([fmt[12], fmt[13]]));
            Some(riff(&[
                chunk(b"data", &vec![0; block_align * 2 + 1]),
                chunk(b"fmt ", &fmt),
            ]))
        }),
        (
            "data first, the RIFF size past the end",
            TRUNCATED,
            |fmt, data| {
                let body = [chunk(b"data", &data), chunk(b"fmt ", &fmt)].concat();
                Some(riff_declaring(
                    u32::try_from(body.len() + 4 + 10).unwrap(),
                    &body,
                ))
            },
        ),
        (
            "data first, a chunk past the end",
            TRUNCATED,
            |fmt, data| {
                Some(riff(&[
                    chunk(b"data", &data),
                    chunk(b"fmt ", &fmt),
                    chunk_header(b"LIST", 100),
                ]))
            },
        ),
        ("data first, a fmt over its bound", CORRUPT, |fmt, data| {
            Some(riff(&[chunk(b"data", &data), long_fmt(fmt)]))
        }),
        ("data first, a second fmt", CORRUPT, |fmt, data| {
            Some(riff(&[
                chunk(b"data", &data),
                chunk(b"fmt ", &fmt),
                chunk(b"fmt ", &fmt),
            ]))
        }),
        ("data first, a second data", CORRUPT, |fmt, data| {
            Some(riff(&[
                chunk(b"data", &data),
                chunk(b"data", &data),
                chunk(b"fmt ", &fmt),
            ]))
        }),
        (
            "data first, an unprintable chunk id after the fmt",
            CORRUPT,
            |fmt, data| {
                Some(riff(&[
                    chunk(b"data", &data),
                    chunk(b"fmt ", &fmt),
                    chunk(&[0, 1, 2, 3], &[0; 4]),
                ]))
            },
        ),
        (
            "data first, a fmt of 18 bytes cut after its 16",
            TRUNCATED,
            |fmt, data| {
                let mut body = chunk(b"data", &data);
                body.extend(chunk_header(b"fmt ", 18));
                body.extend_from_slice(&fmt);
                Some(riff_declaring(
                    u32::try_from(body.len() + 4 + 2).unwrap(),
                    &body,
                ))
            },
        ),
        (
            "the data chunk itself past the RIFF size inside the file",
            CORRUPT,
            |fmt, data| {
                if data.is_empty() {
                    return None;
                }
                let body = [chunk(b"fmt ", &fmt), chunk(b"data", &data)].concat();
                Some(riff_declaring(
                    u32::try_from(body.len() + 4 - 2).unwrap(),
                    &body,
                ))
            },
        ),
        (
            "a fmt after the data past the RIFF size inside the file",
            CORRUPT,
            |fmt, data| {
                let body = [chunk(b"data", &data), chunk(b"fmt ", &fmt)].concat();
                Some(riff_declaring(
                    u32::try_from(body.len() + 4 - 2).unwrap(),
                    &body,
                ))
            },
        ),
    ]
}

/// Steps 3 to 6 answer before each of these faults, as before the faults of the correction's
/// matrix; each fault alone is the error listed for it.
#[test]
fn a_predicate_answers_before_the_byte_level_faults_the_matrix_leaves_out() {
    let mut wrong = Vec::new();
    for (fault, error, apply) in faults_beyond_the_matrix() {
        let alone = apply(fmt_body(1, 1, 48_000, 16), pcm16_data(&[0, 1])).unwrap();
        check(&mut wrong, fault, &alone, error);
        for (predicate, expected, fmt, data) in predicates() {
            let Some(bytes) = apply(fmt, data) else {
                continue;
            };
            check(
                &mut wrong,
                &format!("{predicate} with {fault}"),
                &bytes,
                expected,
            );
        }
    }
    verdict(wrong);
}

// 5. The guards before the file is opened.

/// A request a guard refuses reads nothing: the bytes written into a pipe are all still in it
/// after the import. With both given the import consumes them, which shows the probe can see a
/// read.
#[test]
fn a_pending_guard_reads_nothing_from_a_pipe() {
    use std::os::fd::AsRawFd;
    let bytes = pcm16_wav(1, &[0, 1, 2, 3]);
    let mut wrong = Vec::new();
    for (import, expected) in [
        (external(None, None), Err(WavError::ProvenanceMissing)),
        (
            external(None, Some(LICENCE)),
            Err(WavError::ProvenanceMissing),
        ),
        (
            external(Some(PROVENANCE), None),
            Err(WavError::LicenceMissing),
        ),
        (both(), Ok(())),
    ] {
        let (mut reader, mut writer) = std::io::pipe().unwrap();
        writer.write_all(&bytes).unwrap();
        drop(writer);
        let path = PathBuf::from(format!("/proc/self/fd/{}", reader.as_raw_fd()));
        let got = read_with(Some(&import), &path).map(|_| ());
        let mut left = Vec::new();
        reader.read_to_end(&mut left).unwrap();
        let expected_left = if expected.is_ok() { 0 } else { bytes.len() };
        if got != expected || left.len() != expected_left {
            wrong.push(format!(
                "{import:?}: expected {expected:?} with {expected_left} bytes left in the pipe, \
                 got {got:?} with {} left",
                left.len()
            ));
        }
    }
    verdict(wrong);
}

/// A file that exists but cannot be opened (mode 000) is refused for a pending guard, and is
/// `ReadError::Io` only for a request both guards pass. Skipped where the process can open it
/// anyway (root).
#[test]
fn an_unreadable_file_is_refused_by_a_pending_guard_before_it_is_opened() {
    use std::os::unix::fs::PermissionsExt;
    let path = scratch("unreadable.wav");
    std::fs::write(&path, pcm16_wav(1, &[0, 1])).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let opens = std::fs::File::open(&path).is_ok();
    let mut wrong = Vec::new();
    for (import, expected) in [
        (external(None, None), WavError::ProvenanceMissing),
        (external(Some(PROVENANCE), None), WavError::LicenceMissing),
    ] {
        match import.read(&path) {
            Err(ReadError::Wav(error)) if error == expected => {}
            other => wrong.push(format!("{import:?}: expected {expected:?}, got {other:?}")),
        }
    }
    if !opens && !matches!(both().read(&path), Err(ReadError::Io(_))) {
        wrong.push("both given: expected ReadError::Io".to_owned());
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::remove_file(&path).unwrap();
    verdict(wrong);
}
