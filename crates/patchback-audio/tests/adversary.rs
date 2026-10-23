//! Adversarial cases for story:wav-io (wave a2): frame-versus-sample counting at the bound on
//! stereo input, chunk order, the file reader against the slice reader, and how input that is
//! not a truncated RIFF is classified.

mod support;

use std::io::Write;
use std::path::Path;

use patchback_audio::{
    AudioClip, Corruption, MAX_FRAMES, ReadError, WavError, WavSampleFormat, decode, encode,
    read_wav,
};
use support::*;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-audio-adversary");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn stereo_exactly_at_the_frame_bound_counts_frames_not_samples() {
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(1, 2, 48_000, 16)),
        chunk(b"data", &vec![0; MAX_FRAMES * 4]),
    ]);
    let clip = decode(&bytes).unwrap();
    assert_eq!(clip.frame_count(), MAX_FRAMES);
    assert!(clip.downmixed());
    assert_eq!(clip.source_channel_count(), 2);
}

#[test]
fn stereo_one_frame_past_the_bound_reports_frames() {
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(3, 2, 48_000, 32)),
        chunk(b"data", &vec![0; (MAX_FRAMES + 1) * 8]),
    ]);
    assert_eq!(
        decode(&bytes),
        Err(WavError::InputTooLong {
            frame_count: MAX_FRAMES as u64 + 1
        })
    );
}

#[test]
fn a_data_chunk_before_the_fmt_chunk_imports() {
    let bytes = riff(&[
        chunk(b"data", &pcm16_data(&[16_384, -32_768])),
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
    ]);
    assert_eq!(decode(&bytes).unwrap().samples(), &[0.5, -1.0]);
}

#[test]
fn a_riff_size_of_exactly_four_holds_no_fmt() {
    let mut bytes = riff_declaring(4, &[]);
    bytes.extend(chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)));
    assert_eq!(
        decode(&bytes),
        Err(WavError::CorruptChunk(Corruption::MissingFmt))
    );
}

#[test]
fn stereo_nan_on_the_right_channel_names_channel_one() {
    let bytes = f32_wav(2, &[0.0, 0.0, 0.0, f32::NAN]);
    assert_eq!(
        decode(&bytes),
        Err(WavError::NonFiniteSample {
            frame: 1,
            channel: 1
        })
    );
}

#[test]
fn pcm16_export_of_negative_zero_and_subnormals_is_zero() {
    let clip = AudioClip::from_mono(vec![-0.0, f32::from_bits(1), -f32::from_bits(1)]).unwrap();
    assert_eq!(
        encode(&clip, WavSampleFormat::Pcm16).unwrap(),
        pcm16_wav(1, &[0, 0, 0])
    );
}

#[test]
fn the_file_reader_and_the_slice_reader_agree_on_every_truncation() {
    let whole = pcm16_wav(2, &[1, 2, 3, 4, 5, 6]);
    for cut in 0..whole.len() {
        let path = scratch("truncated.wav");
        std::fs::write(&path, &whole[..cut]).unwrap();
        let from_file = match read_wav(&path) {
            Ok(clip) => Ok(clip),
            Err(ReadError::Wav(error)) => Err(error),
            Err(ReadError::Io(error)) => panic!("cut {cut}: io error {error}"),
        };
        assert_eq!(from_file, decode(&whole[..cut]), "cut {cut}");
    }
}

/// A complete, valid WAV read through a pipe (`patchback analyze --input <(…)` in a shell) is
/// not truncated input; the reader takes the length from file metadata, which is 0 for a pipe.
#[test]
fn a_complete_wav_through_a_pipe_is_not_reported_as_truncated() {
    let bytes = pcm16_wav(1, &[0, 16_384, -16_384]);
    let (reader, mut writer) = std::io::pipe().unwrap();
    let fd = {
        use std::os::fd::AsRawFd;
        reader.as_raw_fd()
    };
    let feeder = std::thread::spawn(move || {
        writer.write_all(&bytes).unwrap();
    });
    let result = read_wav(Path::new(&format!("/proc/self/fd/{fd}")));
    feeder.join().unwrap();
    drop(reader);
    assert!(
        !matches!(result, Err(ReadError::Wav(WavError::Truncated(_)))),
        "a complete WAV from a pipe was refused as {result:?}"
    );
}

/// The domain's TruncatedRiff is "The RIFF size or a chunk size runs past the end of the file";
/// input that never starts with `RIFF` has no RIFF size to run past anything.
#[test]
fn short_input_that_is_not_riff_is_not_reported_as_truncated() {
    for input in [&b"OggS"[..], b"ID3\x04\x00\x00\x00\x00\x00\x00\x00"] {
        assert_eq!(
            decode(input),
            Err(WavError::CorruptChunk(Corruption::NotRiff)),
            "{input:?}"
        );
    }
}
