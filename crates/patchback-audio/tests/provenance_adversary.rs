//! Adversary cases for story:external-audio-provenance (wave a5, unit 2): the two-fault files of
//! every adjacent pair in `IMPORT_WAV_PRECEDENCE`, and the classes they belong to.
//!
//! The order is the one ESS evaluates the outcomes of `ImportWav` (`spec/domains/audio.yaml`)
//! in, documented in the `ess` repository at `website/docs/guides/specify/guards-and-predicates.md`
//! § "Which branch answers" and `website/docs/reference/diagnostics.md` (ESS-AUTHOR-041): the
//! input refusals in declaration order (the two guards, then rate, format, channel count, empty),
//! then the `before_row` externals (truncated, corrupt), then the other externals (too long,
//! non-finite). Every case is read on each path: `decode`, a regular file, a pipe.

mod support;

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use patchback_audio::{
    AudioClip, AudioOrigin, MAX_FMT_BYTES, MAX_FRAMES, ReadError, WavError, WavImport, decode,
    read_wav,
};
use support::*;

const PROVENANCE: &str = "recorded by the project from a hardware monosynth, 2026-10-20";
const LICENCE: &str = "CC-BY-4.0";

const PROVENANCE_MISSING: &str = "patchback.audio.ProvenanceMissing";
const LICENCE_MISSING: &str = "patchback.audio.LicenceMissing";
const RATE: &str = "patchback.audio.UnsupportedSampleRate";
const FORMAT: &str = "patchback.audio.UnsupportedSampleFormat";
const CHANNELS: &str = "patchback.audio.UnsupportedChannelCount";
const EMPTY: &str = "patchback.audio.EmptyAudio";
const TRUNCATED: &str = "patchback.audio.TruncatedRiff";
const CORRUPT: &str = "patchback.audio.CorruptRiffChunk";
const TOO_LONG: &str = "patchback.audio.InputTooLong";

fn external(provenance: Option<&str>, licence: Option<&str>) -> WavImport {
    WavImport {
        origin: AudioOrigin::ExternalRecording,
        provenance: provenance.map(str::to_owned),
        licence: licence.map(str::to_owned),
        render_id: None,
    }
}

/// An external request that names both, so no guard refuses it.
fn both() -> WavImport {
    external(Some(PROVENANCE), Some(LICENCE))
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-audio-provenance-adversary");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Reads `bytes` with `import` through a pipe (the reader's front-to-back path).
fn import_through_a_pipe(import: &WavImport, bytes: Vec<u8>) -> Result<AudioClip, WavError> {
    through_a_pipe(
        Some(import),
        &File {
            head: bytes,
            zeros: 0,
            tail: Vec::new(),
        },
    )
}

fn import_file(import: &WavImport, path: &Path) -> Result<AudioClip, WavError> {
    read_with(Some(import), path)
}

/// A file as `head`, then `zeros` zero bytes, then `tail`; the zeros are never held in memory
/// except by `decode`, which takes a slice.
struct File {
    head: Vec<u8>,
    zeros: u64,
    tail: Vec<u8>,
}

impl File {
    fn whole(bytes: Vec<u8>) -> Self {
        Self {
            head: bytes,
            zeros: 0,
            tail: Vec::new(),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut bytes = self.head.clone();
        bytes.resize(self.head.len() + usize::try_from(self.zeros).unwrap(), 0);
        bytes.extend_from_slice(&self.tail);
        bytes
    }

    /// Writes the file sparse: the zeros are a hole.
    fn write(&self, path: &Path) {
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(&self.head).unwrap();
        let tail_at = u64::try_from(self.head.len()).unwrap() + self.zeros;
        file.set_len(tail_at).unwrap();
        file.seek(SeekFrom::Start(tail_at)).unwrap();
        file.write_all(&self.tail).unwrap();
    }
}

/// A RIFF/WAVE file of `chunks`, then a `data` chunk of `data_size` bytes that start with
/// `first` and are zero after it, then `tail`, with an exact RIFF size.
fn with_long_data(chunks: &[Vec<u8>], data_size: u32, first: &[u8], tail: &[u8]) -> File {
    let mut body = chunks.concat();
    body.extend(chunk_header(b"data", data_size));
    body.extend_from_slice(first);
    assert_eq!(data_size % 2, 0);
    let zeros = u64::from(data_size) - u64::try_from(first.len()).unwrap();
    let riff_size = u64::try_from(body.len() + tail.len() + 4).unwrap() + zeros;
    File {
        head: riff_declaring(u32::try_from(riff_size).unwrap(), &body),
        zeros,
        tail: tail.to_vec(),
    }
}

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

/// Reads `file` through a pipe, with `import` or, without one, with [`read_wav`].
fn through_a_pipe(import: Option<&WavImport>, file: &File) -> Result<AudioClip, WavError> {
    use std::os::fd::AsRawFd;
    let (reader, mut writer) = std::io::pipe().unwrap();
    let path = PathBuf::from(format!("/proc/self/fd/{}", reader.as_raw_fd()));
    let head = file.head.clone();
    let tail = file.tail.clone();
    let zeros = file.zeros;
    let feeder = std::thread::spawn(move || {
        if writer.write_all(&head).is_err() {
            return;
        }
        let block = vec![0u8; 64 * 1024];
        let mut left = zeros;
        while left > 0 {
            let count = usize::try_from(left.min(64 * 1024)).unwrap();
            if writer.write_all(&block[..count]).is_err() {
                return;
            }
            left -= u64::try_from(count).unwrap();
        }
        let _ = writer.write_all(&tail);
    });
    let result = read_with(import, &path);
    drop(reader);
    feeder.join().unwrap();
    result
}

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

/// Reads `file` on every path, with `import` or without a request, and records in `wrong`
/// every path whose answer is not the error `expected`.
fn check(
    wrong: &mut Vec<String>,
    what: &str,
    import: Option<&WavImport>,
    file: &File,
    expected: &str,
) {
    let path = scratch(&format!(
        "case-{}.wav",
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    file.write(&path);
    let answers = [
        ("decode", {
            let bytes = file.bytes();
            match import {
                Some(import) => import.decode(&bytes),
                None => decode(&bytes),
            }
        }),
        ("file", read_with(import, &path)),
        ("pipe", through_a_pipe(import, file)),
    ];
    std::fs::remove_file(&path).unwrap();
    let request = match import {
        None => "no request".to_owned(),
        Some(import) => format!(
            "{:?}, provenance {}, licence {}",
            import.origin,
            if import.provenance.is_some() {
                "given"
            } else {
                "missing"
            },
            if import.licence.is_some() {
                "given"
            } else {
                "missing"
            },
        ),
    };
    for (read_path, answer) in answers {
        let got = match &answer {
            Ok(_) => "a clip",
            Err(error) => error.domain_error(),
        };
        if got != expected {
            wrong.push(format!(
                "{what} ({request}, {read_path}): expected {expected}, got {answer:?}"
            ));
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

// The adversary's first two cases, rewritten to the ESS order: a guard answers before every
// predicate, and a predicate before a corrupt `fmt ` or `data`.

/// Files whose `fmt ` or `data` is corrupt (block align, byte rate, partial frame) and whose
/// sample rate is also unsupported.
fn corrupt_behind_an_unsupported_rate() -> Vec<(&'static str, Vec<u8>)> {
    let mut block_align = fmt_body(1, 1, 44_100, 16);
    block_align[12..14].copy_from_slice(&4u16.to_le_bytes());
    let mut byte_rate = fmt_body(1, 1, 44_100, 16);
    byte_rate[8..12].copy_from_slice(&1u32.to_le_bytes());
    vec![
        (
            "block align 4 on 44.1 kHz mono PCM16",
            riff(&[
                chunk(b"fmt ", &block_align),
                chunk(b"data", &pcm16_data(&[0, 0])),
            ]),
        ),
        (
            "byte rate 1 on 44.1 kHz mono PCM16",
            riff(&[
                chunk(b"fmt ", &byte_rate),
                chunk(b"data", &pcm16_data(&[0, 0])),
            ]),
        ),
        (
            "6-byte data on 44.1 kHz stereo PCM16 (block align 4)",
            riff(&[
                chunk(b"fmt ", &fmt_body(1, 2, 44_100, 16)),
                chunk(b"data", &pcm16_data(&[0, 0, 0])),
            ]),
        ),
    ]
}

/// The guards are input refusals, answered before the rate predicate and before the
/// `before_row` corrupt-chunk external; with both given the rate answers.
#[test]
fn a_corrupt_fmt_behind_an_unsupported_rate_is_refused_by_the_missing_guard() {
    let mut wrong = Vec::new();
    for (what, bytes) in corrupt_behind_an_unsupported_rate() {
        let file = File::whole(bytes);
        for (import, expected) in [
            (external(None, Some(LICENCE)), PROVENANCE_MISSING),
            (external(Some(PROVENANCE), None), LICENCE_MISSING),
            (both(), RATE),
        ] {
            check(&mut wrong, what, Some(&import), &file, expected);
        }
    }
    verdict(wrong);
}

/// The same files read without a request: the rate predicate is an input refusal, answered
/// before the `before_row` corrupt-chunk external.
#[test]
fn a_corrupt_fmt_behind_an_unsupported_rate_is_refused_for_its_rate_without_a_request() {
    let mut wrong = Vec::new();
    for (what, bytes) in corrupt_behind_an_unsupported_rate() {
        check(&mut wrong, what, None, &File::whole(bytes), RATE);
    }
    verdict(wrong);
}

// Every adjacent pair of IMPORT_WAV_PRECEDENCE, in one file each where a file can carry both.

/// 1/2: a request that names neither is refused for its provenance.
#[test]
fn provenance_missing_answers_before_licence_missing() {
    let mut wrong = Vec::new();
    for (what, bytes) in [
        ("a valid 48 kHz file", pcm16_wav(1, &[0, 1])),
        (
            "a 44.1 kHz file",
            riff(&[
                fmt_chunk(1, 1, 44_100, 16),
                chunk(b"data", &pcm16_data(&[0, 1])),
            ]),
        ),
    ] {
        let file = File::whole(bytes);
        check(
            &mut wrong,
            what,
            Some(&external(None, None)),
            &file,
            PROVENANCE_MISSING,
        );
    }
    verdict(wrong);
}

/// 2/3: an external file at 44.1 kHz named with its provenance and without a licence.
#[test]
fn licence_missing_answers_before_an_unsupported_rate() {
    let mut wrong = Vec::new();
    let file = File::whole(riff(&[
        fmt_chunk(1, 1, 44_100, 16),
        chunk(b"data", &pcm16_data(&[0, 1])),
    ]));
    check(
        &mut wrong,
        "a 44.1 kHz file",
        Some(&external(Some(PROVENANCE), None)),
        &file,
        LICENCE_MISSING,
    );
    verdict(wrong);
}

/// 3/4: 44.1 kHz and 24-bit PCM.
#[test]
fn an_unsupported_rate_answers_before_an_unsupported_format() {
    let mut wrong = Vec::new();
    let file = File::whole(riff(&[
        fmt_chunk(1, 1, 44_100, 24),
        chunk(b"data", &[0; 6]),
    ]));
    for import in [None, Some(&both())] {
        check(&mut wrong, "44.1 kHz 24-bit", import, &file, RATE);
    }
    verdict(wrong);
}

/// 4/5: 24-bit PCM in three channels.
#[test]
fn an_unsupported_format_answers_before_an_unsupported_channel_count() {
    let mut wrong = Vec::new();
    let file = File::whole(riff(&[
        fmt_chunk(1, 3, 48_000, 24),
        chunk(b"data", &[0; 18]),
    ]));
    for import in [None, Some(&both())] {
        check(
            &mut wrong,
            "24-bit in three channels",
            import,
            &file,
            FORMAT,
        );
    }
    verdict(wrong);
}

/// 5/6: three channels and an empty `data` chunk.
#[test]
fn an_unsupported_channel_count_answers_before_an_empty_file() {
    let mut wrong = Vec::new();
    let file = File::whole(riff(&[fmt_chunk(1, 3, 48_000, 16), chunk(b"data", &[])]));
    for import in [None, Some(&both())] {
        check(
            &mut wrong,
            "three channels, no frames",
            import,
            &file,
            CHANNELS,
        );
    }
    verdict(wrong);
}

/// 6/7: an empty `data` chunk in a file whose RIFF size, or a later chunk's size, runs past its
/// end. The `fmt ` and the `data` header are read, so the model's frame count is 0.
#[test]
fn an_empty_file_answers_before_a_truncation() {
    let mut wrong = Vec::new();
    let body = [fmt_chunk(1, 1, 48_000, 16), chunk(b"data", &[])].concat();
    let riff_past_the_end = riff_declaring(u32::try_from(body.len() + 4 + 10).unwrap(), &body);
    let chunk_past_the_end = riff(&[
        fmt_chunk(1, 1, 48_000, 16),
        chunk(b"data", &[]),
        chunk_header(b"LIST", 100),
    ]);
    for (what, bytes) in [
        (
            "an empty file whose RIFF size runs past its end",
            riff_past_the_end,
        ),
        (
            "an empty file whose last chunk runs past its end",
            chunk_past_the_end,
        ),
    ] {
        let file = File::whole(bytes);
        for import in [None, Some(&both())] {
            check(&mut wrong, what, import, &file, EMPTY);
        }
    }
    verdict(wrong);
}

/// 7/8: a file both truncated and corrupt, where the reader sees both: the RIFF size past the
/// end with a corrupt block align or a second `data` chunk inside the file, a `data` chunk past
/// the end behind a corrupt byte rate or an oversized `fmt `, and a chunk past the end after a
/// second `fmt `. The reader stops its walk at a chunk header it cannot trust (not printable,
/// past the RIFF size), so a truncation behind one is not seen and is no case here.
#[test]
fn a_truncation_answers_before_a_corruption() {
    let mut wrong = Vec::new();
    let mut block_align = fmt_body(1, 1, 48_000, 16);
    block_align[12..14].copy_from_slice(&4u16.to_le_bytes());
    let body = [
        chunk(b"fmt ", &block_align),
        chunk(b"data", &pcm16_data(&[0, 1])),
    ]
    .concat();
    let block_align_riff_past_the_end =
        riff_declaring(u32::try_from(body.len() + 4 + 10).unwrap(), &body);

    let body = [
        fmt_chunk(1, 1, 48_000, 16),
        chunk(b"data", &pcm16_data(&[0, 1])),
        chunk(b"data", &pcm16_data(&[0, 1])),
    ]
    .concat();
    let second_data_riff_past_the_end =
        riff_declaring(u32::try_from(body.len() + 4 + 10).unwrap(), &body);

    let mut byte_rate = fmt_body(1, 1, 48_000, 16);
    byte_rate[8..12].copy_from_slice(&1u32.to_le_bytes());
    let mut body = chunk(b"fmt ", &byte_rate);
    body.extend(chunk_header(b"data", 8));
    body.extend(pcm16_data(&[0, 1]));
    let byte_rate_data_past_the_end = riff_declaring(u32::try_from(body.len() + 4).unwrap(), &body);

    let second_fmt_then_chunk_past_the_end = riff(&[
        fmt_chunk(1, 1, 48_000, 16),
        fmt_chunk(1, 1, 48_000, 16),
        chunk(b"data", &pcm16_data(&[0, 1])),
        chunk_header(b"LIST", 100),
    ]);

    let mut long_fmt = fmt_body(1, 1, 48_000, 16);
    long_fmt.resize(usize::try_from(MAX_FMT_BYTES).unwrap() + 2, 0);
    let mut body = chunk(b"fmt ", &long_fmt);
    body.extend(chunk_header(b"data", 8));
    body.extend(pcm16_data(&[0, 1]));
    let long_fmt_data_past_the_end = riff_declaring(u32::try_from(body.len() + 4).unwrap(), &body);

    for (what, bytes) in [
        (
            "a second fmt chunk, then a chunk past the end",
            second_fmt_then_chunk_past_the_end,
        ),
        (
            "a fmt chunk over its bound, then the data chunk past the end",
            long_fmt_data_past_the_end,
        ),
        (
            "a corrupt block align, the RIFF size past the end",
            block_align_riff_past_the_end,
        ),
        (
            "a second data chunk, the RIFF size past the end",
            second_data_riff_past_the_end,
        ),
        (
            "a corrupt byte rate, the data chunk past the end",
            byte_rate_data_past_the_end,
        ),
    ] {
        let file = File::whole(bytes);
        for import in [None, Some(&both())] {
            check(&mut wrong, what, import, &file, TRUNCATED);
        }
    }
    verdict(wrong);
}

/// 8/9: one frame past [`MAX_FRAMES`] with a corrupt byte rate, or with an unprintable chunk id
/// after the `data` chunk.
#[test]
fn a_corruption_answers_before_the_frame_bound() {
    let mut wrong = Vec::new();
    let data_size = u32::try_from((MAX_FRAMES + 1) * 2).unwrap();
    let mut byte_rate = fmt_body(1, 1, 48_000, 16);
    byte_rate[8..12].copy_from_slice(&1u32.to_le_bytes());
    for (what, file) in [
        (
            "a corrupt byte rate, one frame past the bound",
            with_long_data(&[chunk(b"fmt ", &byte_rate)], data_size, &[], &[]),
        ),
        (
            "an unprintable chunk id after a data chunk one frame past the bound",
            with_long_data(
                &[fmt_chunk(1, 1, 48_000, 16)],
                data_size,
                &[],
                &chunk(&[0, 1, 2, 3], &[0; 4]),
            ),
        ),
    ] {
        for import in [None, Some(&both())] {
            check(&mut wrong, what, import, &file, CORRUPT);
        }
    }
    verdict(wrong);
}

/// 9/10: one float32 frame past [`MAX_FRAMES`] whose first sample is NaN. The bound is answered
/// from the `data` header, before any sample is read.
#[test]
fn the_frame_bound_answers_before_a_non_finite_sample() {
    let mut wrong = Vec::new();
    let data_size = u32::try_from((MAX_FRAMES + 1) * 4).unwrap();
    let file = with_long_data(
        &[fmt_chunk(3, 1, 48_000, 32)],
        data_size,
        &f32::NAN.to_le_bytes(),
        &[],
    );
    for import in [None, Some(&both())] {
        check(
            &mut wrong,
            "NaN in a float32 file one frame past the bound",
            import,
            &file,
            TOO_LONG,
        );
    }
    verdict(wrong);
}

// The classes the pairs belong to.

/// A byte-level fault of a file whose `fmt ` and `data` header the reader reads, applied to a
/// `fmt ` body and `data` bytes; `None` where a file with that `data` cannot carry the fault.
type Fault = fn(Vec<u8>, Vec<u8>) -> Option<Vec<u8>>;

/// Every byte-level fault the reader finds after it has read the `fmt ` chunk and the `data`
/// chunk header, with the error it is.
fn byte_level_faults() -> Vec<(&'static str, &'static str, Fault)> {
    vec![
        ("a block align that disagrees", CORRUPT, |mut fmt, data| {
            let block_align = u16::from_le_bytes([fmt[12], fmt[13]]);
            fmt[12..14].copy_from_slice(&(block_align + 1).to_le_bytes());
            Some(riff(&[chunk(b"fmt ", &fmt), chunk(b"data", &data)]))
        }),
        ("a byte rate that disagrees", CORRUPT, |mut fmt, data| {
            fmt[8..12].copy_from_slice(&1u32.to_le_bytes());
            Some(riff(&[chunk(b"fmt ", &fmt), chunk(b"data", &data)]))
        }),
        // An empty `data` chunk holds whole frames, so no empty file holds a partial one.
        ("a partial frame", CORRUPT, |fmt, data| {
            if data.is_empty() {
                return None;
            }
            let block_align = usize::from(u16::from_le_bytes([fmt[12], fmt[13]]));
            Some(riff(&[
                chunk(b"fmt ", &fmt),
                chunk(b"data", &vec![0; block_align * 2 + 1]),
            ]))
        }),
        (
            "a RIFF size past the end of the file",
            TRUNCATED,
            |fmt, data| {
                let body = [chunk(b"fmt ", &fmt), chunk(b"data", &data)].concat();
                Some(riff_declaring(
                    u32::try_from(body.len() + 4 + 10).unwrap(),
                    &body,
                ))
            },
        ),
        // An empty `data` chunk declares no bytes to run past the end; the next case puts
        // the truncation in a later chunk instead.
        (
            "a data chunk past the end of the file",
            TRUNCATED,
            |fmt, data| {
                if data.is_empty() {
                    return None;
                }
                let mut body = chunk(b"fmt ", &fmt);
                body.extend(chunk_header(
                    b"data",
                    u32::try_from(data.len() + 10).unwrap(),
                ));
                body.extend(&data);
                Some(riff_declaring(
                    u32::try_from(body.len() + 4).unwrap(),
                    &body,
                ))
            },
        ),
        (
            "a chunk after the data past the end of the file",
            TRUNCATED,
            |fmt, data| {
                Some(riff(&[
                    chunk(b"fmt ", &fmt),
                    chunk(b"data", &data),
                    chunk_header(b"LIST", 100),
                ]))
            },
        ),
        (
            "a partial chunk header after the data",
            TRUNCATED,
            |fmt, data| {
                let mut body = [chunk(b"fmt ", &fmt), chunk(b"data", &data)].concat();
                body.extend_from_slice(b"LIS");
                Some(riff_declaring(
                    u32::try_from(body.len() + 4).unwrap(),
                    &body,
                ))
            },
        ),
        (
            "a chunk after the data past the RIFF size inside the file",
            CORRUPT,
            |fmt, data| {
                let body = [
                    chunk(b"fmt ", &fmt),
                    chunk(b"data", &data),
                    chunk(b"LIST", &[0; 4]),
                ]
                .concat();
                Some(riff_declaring(
                    u32::try_from(body.len() + 4 - 2).unwrap(),
                    &body,
                ))
            },
        ),
        (
            "a fmt chunk over its allocation bound",
            CORRUPT,
            |mut fmt, data| {
                fmt.resize(usize::try_from(MAX_FMT_BYTES).unwrap() + 2, 0);
                Some(riff(&[chunk(b"fmt ", &fmt), chunk(b"data", &data)]))
            },
        ),
        (
            "a second fmt chunk before the data",
            CORRUPT,
            |fmt, data| {
                Some(riff(&[
                    chunk(b"fmt ", &fmt),
                    chunk(b"fmt ", &fmt),
                    chunk(b"data", &data),
                ]))
            },
        ),
        ("a second fmt chunk after the data", CORRUPT, |fmt, data| {
            Some(riff(&[
                chunk(b"fmt ", &fmt),
                chunk(b"data", &data),
                chunk(b"fmt ", &fmt),
            ]))
        }),
        ("a second data chunk", CORRUPT, |fmt, data| {
            Some(riff(&[
                chunk(b"fmt ", &fmt),
                chunk(b"data", &data),
                chunk(b"data", &data),
            ]))
        }),
        (
            "an unprintable chunk id after the data",
            CORRUPT,
            |fmt, data| {
                Some(riff(&[
                    chunk(b"fmt ", &fmt),
                    chunk(b"data", &data),
                    chunk(&[0, 1, 2, 3], &[0; 4]),
                ]))
            },
        ),
    ]
}

/// The `fmt ` body and `data` bytes of each input refusal of the reader (3 to 6), and of a file
/// the reader accepts.
fn predicates() -> Vec<(&'static str, Vec<u8>, Vec<u8>)> {
    vec![
        ("44.1 kHz", fmt_body(1, 1, 44_100, 16), vec![0; 4]),
        ("24-bit PCM", fmt_body(1, 1, 48_000, 24), vec![0; 6]),
        ("three channels", fmt_body(1, 3, 48_000, 16), vec![0; 12]),
        ("no frames", fmt_body(1, 1, 48_000, 16), Vec::new()),
    ]
}

fn predicate_error(what: &str) -> &'static str {
    match what {
        "44.1 kHz" => RATE,
        "24-bit PCM" => FORMAT,
        "three channels" => CHANNELS,
        _ => EMPTY,
    }
}

/// Steps 3 to 6 answer before every byte-level fault whose file still yields its header facts;
/// each fault alone, on a file the reader otherwise accepts, is the error listed for it.
#[test]
fn a_predicate_answers_before_every_byte_level_fault_whose_header_facts_are_readable() {
    let mut wrong = Vec::new();
    let mut carried = 0;
    for (fault, error, apply) in byte_level_faults() {
        let alone = apply(fmt_body(1, 1, 48_000, 16), pcm16_data(&[0, 1])).unwrap();
        check(&mut wrong, fault, None, &File::whole(alone), error);
        for (predicate, fmt, data) in predicates() {
            let Some(bytes) = apply(fmt, data) else {
                continue;
            };
            carried += 1;
            let file = File::whole(bytes);
            let what = format!("{predicate} with {fault}");
            for import in [None, Some(&both())] {
                check(&mut wrong, &what, import, &file, predicate_error(predicate));
            }
        }
    }
    assert_eq!(carried, 50, "two faults no empty file can carry");
    verdict(wrong);
}

/// Steps 1 and 2 answer before every fault a file can carry, on every path, with or without
/// readable header facts; with both given the file's own refusal answers.
#[test]
fn a_missing_guard_answers_before_every_fault_of_the_file() {
    let mut wrong = Vec::new();
    let mut files: Vec<(String, File)> = Vec::new();
    for (predicate, fmt, data) in predicates() {
        files.push((
            predicate.to_owned(),
            File::whole(riff(&[chunk(b"fmt ", &fmt), chunk(b"data", &data)])),
        ));
    }
    for (fault, _, apply) in byte_level_faults() {
        let bytes = apply(fmt_body(1, 1, 48_000, 16), pcm16_data(&[0, 1])).unwrap();
        files.push((fault.to_owned(), File::whole(bytes)));
    }
    for (what, bytes) in header_facts_unreadable() {
        files.push((what.to_owned(), File::whole(bytes)));
    }
    files.push((
        "one frame past the bound".to_owned(),
        with_long_data(
            &[fmt_chunk(1, 1, 48_000, 16)],
            u32::try_from((MAX_FRAMES + 1) * 2).unwrap(),
            &[],
            &[],
        ),
    ));
    files.push((
        "a NaN sample".to_owned(),
        File::whole(f32_wav(1, &[0.0, f32::NAN])),
    ));
    for (what, file) in &files {
        for (import, expected) in [
            (external(None, None), PROVENANCE_MISSING),
            (external(None, Some(LICENCE)), PROVENANCE_MISSING),
            (external(Some(PROVENANCE), None), LICENCE_MISSING),
        ] {
            check(&mut wrong, what, Some(&import), file, expected);
        }
    }
    verdict(wrong);
}

/// Files with an unsupported rate whose header facts the reader cannot read: the RIFF header
/// is cut, the `data` chunk is missing, a chunk header before `data` is corrupt or cut, the
/// `fmt ` body is too short or cut. The model has no header facts for them.
fn header_facts_unreadable() -> Vec<(&'static str, Vec<u8>)> {
    let fmt = fmt_chunk(1, 1, 44_100, 16);
    let data = chunk(b"data", &pcm16_data(&[0, 1]));
    let whole = riff(&[fmt.clone(), data.clone()]);
    let fmt_cut = {
        let mut body = chunk_header(b"fmt ", 16);
        body.extend_from_slice(&fmt_body(1, 1, 44_100, 16)[..10]);
        riff_declaring(u32::try_from(body.len() + 4).unwrap(), &body)
    };
    let data_then_fmt_past_the_end = {
        let mut body = data.clone();
        body.extend(chunk_header(b"fmt ", 16));
        body.extend_from_slice(&fmt_body(1, 1, 44_100, 16)[..10]);
        riff_declaring(u32::try_from(body.len() + 4 + 6).unwrap(), &body)
    };
    vec![
        ("a RIFF header cut at 10 bytes", whole[..10].to_vec()),
        (
            "a 44.1 kHz fmt and no data chunk",
            riff(std::slice::from_ref(&fmt)),
        ),
        (
            "an unprintable chunk id between fmt and data",
            riff(&[fmt.clone(), chunk(&[0, 1, 2, 3], &[0; 4]), data.clone()]),
        ),
        (
            "a fmt chunk of 14 bytes",
            riff(&[
                chunk(b"fmt ", &fmt_body(1, 1, 44_100, 16)[..14]),
                data.clone(),
            ]),
        ),
        ("a fmt body cut at 10 bytes", fmt_cut),
        (
            "a data chunk, then a fmt body cut at 10 bytes",
            data_then_fmt_past_the_end,
        ),
    ]
}

/// Where the header facts cannot be read, steps 3 to 6 have nothing to decide and the
/// truncation or corruption the reader finds answers, as it did before.
#[test]
fn where_the_header_facts_cannot_be_read_the_truncation_or_corruption_answers() {
    let mut wrong = Vec::new();
    let expected = [TRUNCATED, CORRUPT, CORRUPT, CORRUPT, TRUNCATED, TRUNCATED];
    for ((what, bytes), expected) in header_facts_unreadable().into_iter().zip(expected) {
        let file = File::whole(bytes);
        for import in [None, Some(&both())] {
            check(&mut wrong, what, import, &file, expected);
        }
    }
    verdict(wrong);
}

// Regression guards from the adversary's pass, unchanged.

/// The `imported-downmixed` branch: a stereo external file is refused by each guard on every
/// path, and with both given it is downmixed and keeps the request, from a file and a pipe.
#[test]
fn a_stereo_external_recording_is_guarded_and_keeps_its_provenance_when_downmixed() {
    let bytes = pcm16_wav(2, &[1_000, -3_000, i16::MAX, i16::MIN, 0, 2]);
    let path = scratch("stereo-external.wav");
    std::fs::write(&path, &bytes).unwrap();
    for (import, expected) in [
        (external(None, Some(LICENCE)), WavError::ProvenanceMissing),
        (external(None, None), WavError::ProvenanceMissing),
        (external(Some(PROVENANCE), None), WavError::LicenceMissing),
    ] {
        assert_eq!(import.decode(&bytes).map(|_| ()), Err(expected));
        assert_eq!(import_file(&import, &path).map(|_| ()), Err(expected));
        assert_eq!(
            import_through_a_pipe(&import, bytes.clone()).map(|_| ()),
            Err(expected)
        );
    }
    let import = external(Some(PROVENANCE), Some(LICENCE));
    let plain = decode(&bytes).unwrap();
    for clip in [
        import.decode(&bytes).unwrap(),
        import_file(&import, &path).unwrap(),
        import_through_a_pipe(&import, bytes.clone()).unwrap(),
    ] {
        assert!(clip.downmixed());
        assert_eq!(clip.source_channel_count(), 2);
        assert_eq!(clip.frame_count(), 3);
        assert_eq!(clip.origin(), Some(AudioOrigin::ExternalRecording));
        assert_eq!(clip.provenance(), Some(PROVENANCE));
        assert_eq!(clip.licence(), Some(LICENCE));
        let bits = |c: &AudioClip| c.samples().iter().map(|s| s.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&clip), bits(&plain));
    }
}

/// `too-long` is an external outcome after the guards. On a regular file the reader learns the
/// frame count from the `data` header, so a sparse file one frame past [`MAX_FRAMES`] reaches it
/// without a sample read; through a pipe the reader skips the body. Either way the guard is
/// answered first, and with both given the bound still refuses.
#[test]
fn a_file_one_frame_past_the_bound_is_refused_by_the_guard_first_on_both_read_paths() {
    let data_size = u32::try_from((MAX_FRAMES + 1) * 2).unwrap();
    let fmt = chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16));
    let mut body = fmt.clone();
    body.extend_from_slice(&chunk_header(b"data", data_size));
    let riff_size = u32::try_from(body.len() + 4).unwrap() + data_size;
    let header = riff_declaring(riff_size, &body);
    let total = u64::try_from(header.len()).unwrap() + u64::from(data_size);

    let path = scratch("one-frame-past-the-bound.wav");
    {
        let file = std::fs::File::create(&path).unwrap();
        std::io::Write::write_all(&mut &file, &header).unwrap();
        file.set_len(total).unwrap();
    }
    let too_long = WavError::InputTooLong {
        frame_count: u64::try_from(MAX_FRAMES + 1).unwrap(),
    };
    match read_wav(&path) {
        Err(ReadError::Wav(error)) => assert_eq!(error, too_long),
        other => panic!("{other:?}"),
    }
    let mut piped = header.clone();
    piped.resize(usize::try_from(total).unwrap(), 0);
    for (import, expected) in [
        (external(None, Some(LICENCE)), WavError::ProvenanceMissing),
        (external(Some(PROVENANCE), None), WavError::LicenceMissing),
        (external(Some(PROVENANCE), Some(LICENCE)), too_long),
        (WavImport::new(AudioOrigin::PatchbackSynthesized), too_long),
    ] {
        assert_eq!(import_file(&import, &path).map(|_| ()), Err(expected));
        assert_eq!(
            import_through_a_pipe(&import, piped.clone()).map(|_| ()),
            Err(expected)
        );
    }
    std::fs::remove_file(&path).unwrap();
}
