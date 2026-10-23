//! The numeric obligations of story:wav-io (DESIGN.md § 6.1, § 12.1, § 12.3) on hand-built
//! buffers: import, downmix, byte-exact roundtrip, and each refusal reason.

mod support;

use patchback_audio::{
    AudioClip, Corruption, MAX_FRAMES, SAMPLE_RATE_HZ, Truncation, WavError, WavSampleFormat,
    decode, downmix_frame, encode, read_wav, write_wav,
};
use support::*;

fn refused(bytes: &[u8]) -> WavError {
    match decode(bytes) {
        Ok(clip) => panic!("imported {} frames, expected a refusal", clip.frame_count()),
        Err(error) => error,
    }
}

fn bits(samples: &[f32]) -> Vec<u32> {
    samples.iter().map(|s| s.to_bits()).collect()
}

// Import.

#[test]
fn imports_mono_pcm16_as_a_clip_that_is_not_downmixed() {
    let clip = decode(&pcm16_wav(1, &[0, 16_384, -16_384, i16::MIN, i16::MAX])).unwrap();
    assert_eq!(clip.samples(), &[0.0, 0.5, -0.5, -1.0, 32_767.0 / 32_768.0]);
    assert_eq!(clip.frame_count(), 5);
    assert_eq!(clip.sample_rate_hz(), SAMPLE_RATE_HZ);
    assert_eq!(clip.source_channel_count(), 1);
    assert!(!clip.downmixed());
    assert_eq!(clip.source_format(), Some(WavSampleFormat::Pcm16));
}

#[test]
fn imports_mono_float32_unchanged() {
    let samples = [0.0, -0.0, 0.25, -1.5, 3.0e30, f32::MIN_POSITIVE / 4.0];
    let clip = decode(&f32_wav(1, &samples)).unwrap();
    assert_eq!(bits(clip.samples()), bits(&samples));
    assert_eq!(clip.source_channel_count(), 1);
    assert!(!clip.downmixed());
    assert_eq!(clip.source_format(), Some(WavSampleFormat::Float32));
}

#[test]
fn skips_unknown_chunks_and_their_pad_byte() {
    let bytes = riff(&[
        chunk(b"LIST", b"odd"),
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk(b"junk", &[7; 5]),
        chunk(b"data", &pcm16_data(&[1, 2, 3])),
        chunk(b"id3 ", &[1]),
    ]);
    let clip = decode(&bytes).unwrap();
    assert_eq!(clip.frame_count(), 3);
}

#[test]
fn accepts_a_fmt_chunk_with_an_extension_size() {
    let mut fmt = fmt_body(3, 1, 48_000, 32);
    fmt.extend_from_slice(&0u16.to_le_bytes());
    let bytes = riff(&[chunk(b"fmt ", &fmt), chunk(b"data", &f32_data(&[0.5]))]);
    assert_eq!(decode(&bytes).unwrap().samples(), &[0.5]);
}

// Downmix (§ 6.1): one formula, per frame.

#[test]
fn downmix_is_the_mean_of_left_and_right_per_frame() {
    assert_eq!(downmix_frame(1.0, 0.0), 0.5);
    assert_eq!(downmix_frame(0.25, -0.25), 0.0);
    assert_eq!(downmix_frame(-1.0, -1.0), -1.0);
    assert_eq!(downmix_frame(0.1, 0.3), ((0.1f64 + 0.3f64) / 2.0) as f32);
    assert_eq!(downmix_frame(f32::MAX, f32::MAX), f32::MAX);
    assert_eq!(downmix_frame(f32::MIN, f32::MIN), f32::MIN);
    assert_eq!(downmix_frame(f32::MAX, f32::MIN), 0.0);
}

#[test]
fn stereo_float32_imports_as_a_mono_clip_that_records_the_downmix() {
    let frames = [1.0, 0.0, 0.25, -0.25, -1.0, -0.5, f32::MAX, f32::MAX];
    let clip = decode(&f32_wav(2, &frames)).unwrap();
    assert_eq!(clip.samples(), &[0.5, 0.0, -0.75, f32::MAX]);
    assert_eq!(clip.frame_count(), 4);
    assert!(clip.downmixed());
    assert_eq!(clip.source_channel_count(), 2);
    assert_eq!(clip.source_format(), Some(WavSampleFormat::Float32));
}

#[test]
fn stereo_pcm16_downmixes_after_conversion() {
    let clip = decode(&pcm16_wav(2, &[16_384, 0, i16::MIN, i16::MIN, 1, 2])).unwrap();
    assert_eq!(
        clip.samples(),
        &[0.25, -1.0, downmix_frame(1.0 / 32_768.0, 2.0 / 32_768.0)]
    );
    assert!(clip.downmixed());
    assert_eq!(clip.source_channel_count(), 2);
}

// Roundtrip (§ 12.1): byte-exact on export and re-import.

#[test]
fn pcm16_roundtrip_is_byte_exact() {
    let every: Vec<i16> = (i16::MIN..=i16::MAX).collect();
    let original = pcm16_wav(1, &every);
    let clip = decode(&original).unwrap();
    let exported = encode(&clip, WavSampleFormat::Pcm16).unwrap();
    assert_eq!(exported, original);
    let again = encode(&decode(&exported).unwrap(), WavSampleFormat::Pcm16).unwrap();
    assert_eq!(again, exported);
}

#[test]
fn float32_roundtrip_is_byte_exact() {
    let samples = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.123_456_79,
        f32::MAX,
        f32::MIN,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        -f32::from_bits(0x007f_ffff),
    ];
    let clip = AudioClip::from_mono(samples.clone()).unwrap();
    let exported = encode(&clip, WavSampleFormat::Float32).unwrap();
    assert_eq!(exported, f32_wav(1, &samples));
    let reimported = decode(&exported).unwrap();
    assert_eq!(bits(reimported.samples()), bits(&samples));
    assert_eq!(
        encode(&reimported, WavSampleFormat::Float32).unwrap(),
        exported
    );
}

#[test]
fn pcm16_export_quantizes_to_the_nearest_step_and_one_to_the_top_step() {
    let clip = AudioClip::from_mono(vec![1.0, -1.0, 0.5, 0.75 / 32_768.0, -0.0]).unwrap();
    let bytes = encode(&clip, WavSampleFormat::Pcm16).unwrap();
    assert_eq!(bytes, pcm16_wav(1, &[i16::MAX, i16::MIN, 16_384, 1, 0]));
}

#[test]
fn files_roundtrip_through_read_wav_and_write_wav() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-audio-files");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("roundtrip.wav");
    let clip = AudioClip::from_mono(vec![0.5, -0.25, 0.0]).unwrap();
    write_wav(&path, &clip, WavSampleFormat::Float32).unwrap();
    let back = read_wav(&path).unwrap();
    assert_eq!(back.samples(), clip.samples());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        encode(&clip, WavSampleFormat::Float32).unwrap()
    );
}

// Export refusals.

#[test]
fn pcm16_export_refuses_a_sample_outside_unit_range_and_writes_nothing() {
    for (value, frame) in [(1.000_000_1f32, 1u64), (-1.5, 1), (2.0, 1)] {
        let clip = AudioClip::from_mono(vec![0.0, value, 0.0]).unwrap();
        assert_eq!(
            encode(&clip, WavSampleFormat::Pcm16),
            Err(WavError::SampleOutOfRange { frame, value })
        );
        assert!(encode(&clip, WavSampleFormat::Float32).is_ok());
    }
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-audio-files");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("refused.wav");
    let _ = std::fs::remove_file(&path);
    let clip = AudioClip::from_mono(vec![3.0]).unwrap();
    assert!(write_wav(&path, &clip, WavSampleFormat::Pcm16).is_err());
    assert!(!path.exists());
}

#[test]
fn a_clip_refuses_non_finite_empty_and_over_bound_samples() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            AudioClip::from_mono(vec![0.0, 0.0, value]),
            Err(WavError::NonFiniteSample {
                frame: 2,
                channel: 0
            })
        );
    }
    assert_eq!(AudioClip::from_mono(Vec::new()), Err(WavError::EmptyAudio));
    assert_eq!(
        AudioClip::from_mono(vec![0.0; MAX_FRAMES + 1]),
        Err(WavError::InputTooLong {
            frame_count: MAX_FRAMES as u64 + 1
        })
    );
}

// Header refusals (§ 5.3, § 6.1).

#[test]
fn refuses_a_sample_rate_other_than_48000() {
    for rate in [44_100, 96_000, 0] {
        let bytes = riff(&[
            chunk(b"fmt ", &fmt_body(1, 1, rate, 16)),
            chunk(b"data", &pcm16_data(&[0])),
        ]);
        assert_eq!(
            refused(&bytes),
            WavError::UnsupportedSampleRate {
                sample_rate_hz: rate
            }
        );
    }
}

#[test]
fn refuses_every_sample_format_but_pcm16_and_float32() {
    for (tag, bits) in [
        (1, 8),
        (1, 24),
        (1, 32),
        (3, 64),
        (3, 16),
        (2, 16),
        (0xFFFE, 16),
        (0xFFFE, 32),
    ] {
        let bytes = riff(&[
            chunk(b"fmt ", &fmt_body(tag, 1, 48_000, bits)),
            chunk(b"data", &[0; 8]),
        ]);
        assert_eq!(
            refused(&bytes),
            WavError::UnsupportedSampleFormat {
                format_tag: tag,
                bits_per_sample: bits
            },
            "tag {tag} bits {bits}"
        );
    }
}

#[test]
fn refuses_wave_format_extensible_even_around_pcm16() {
    let mut fmt = fmt_body(0xFFFE, 1, 48_000, 16);
    fmt.extend_from_slice(&22u16.to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());
    fmt.extend_from_slice(&0u32.to_le_bytes());
    fmt.extend_from_slice(&[
        1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71,
    ]);
    let bytes = riff(&[chunk(b"fmt ", &fmt), chunk(b"data", &pcm16_data(&[0]))]);
    assert_eq!(
        refused(&bytes),
        WavError::UnsupportedSampleFormat {
            format_tag: 0xFFFE,
            bits_per_sample: 16
        }
    );
}

#[test]
fn refuses_a_channel_count_other_than_one_or_two() {
    for channels in [0, 3, 6] {
        let bytes = riff(&[
            chunk(b"fmt ", &fmt_body(3, channels, 48_000, 32)),
            chunk(b"data", &[0; 24]),
        ]);
        assert_eq!(
            refused(&bytes),
            WavError::UnsupportedChannelCount {
                channel_count: channels
            }
        );
    }
}

#[test]
fn refuses_a_data_chunk_without_frames() {
    assert_eq!(refused(&pcm16_wav(1, &[])), WavError::EmptyAudio);
    assert_eq!(refused(&f32_wav(2, &[])), WavError::EmptyAudio);
}

// Truncation and corruption (§ 6.1, § 12.1).

#[test]
fn refuses_input_shorter_than_the_riff_header() {
    for len in 0..12 {
        let bytes = pcm16_wav(1, &[0]);
        assert_eq!(
            refused(&bytes[..len]),
            WavError::Truncated(Truncation::Header)
        );
    }
}

#[test]
fn refuses_a_riff_size_larger_than_the_input() {
    let body = [
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk(b"data", &pcm16_data(&[0])),
    ]
    .concat();
    let exact = (body.len() + 4) as u32;
    for declared in [exact + 1, exact + 2, u32::MAX] {
        assert_eq!(
            refused(&riff_declaring(declared, &body)),
            WavError::Truncated(Truncation::RiffSize {
                declared,
                available: (body.len() + 4) as u64
            })
        );
    }
}

#[test]
fn refuses_a_chunk_size_of_u32_max() {
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk_header(b"data", u32::MAX),
    ]);
    assert_eq!(
        refused(&bytes),
        WavError::Truncated(Truncation::Chunk {
            id: *b"data",
            offset: 36,
            size: u32::MAX
        })
    );
    let bytes = riff(&[chunk_header(b"LIST", u32::MAX)]);
    assert_eq!(
        refused(&bytes),
        WavError::Truncated(Truncation::Chunk {
            id: *b"LIST",
            offset: 12,
            size: u32::MAX
        })
    );
}

#[test]
fn refuses_a_data_chunk_that_runs_past_the_end() {
    let mut bytes = pcm16_wav(1, &[1, 2, 3, 4]);
    bytes.truncate(bytes.len() - 2);
    let riff_size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
    assert_eq!(
        refused(&bytes),
        WavError::Truncated(Truncation::Chunk {
            id: *b"data",
            offset: 36,
            size: 8
        })
    );
}

#[test]
fn refuses_a_missing_pad_byte_and_a_partial_chunk_header() {
    let mut body = chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16));
    body.extend(chunk(b"data", &pcm16_data(&[0])));
    body.extend(chunk_header(b"LIST", 3));
    body.extend_from_slice(b"odd");
    assert_eq!(
        refused(&riff_declaring((body.len() + 4) as u32, &body)),
        WavError::Truncated(Truncation::Chunk {
            id: *b"LIST",
            offset: 46,
            size: 3
        })
    );
    let mut body = chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16));
    body.extend(chunk(b"data", &pcm16_data(&[0])));
    body.extend_from_slice(b"LIS");
    assert_eq!(
        refused(&riff_declaring((body.len() + 4) as u32, &body)),
        WavError::Truncated(Truncation::ChunkHeader { offset: 46 })
    );
}

#[test]
fn refuses_input_that_is_not_riff_wave() {
    let mut bytes = pcm16_wav(1, &[0]);
    bytes[0..4].copy_from_slice(b"RIFX");
    assert_eq!(refused(&bytes), WavError::CorruptChunk(Corruption::NotRiff));
    let mut bytes = pcm16_wav(1, &[0]);
    bytes[8..12].copy_from_slice(b"AVI ");
    assert_eq!(refused(&bytes), WavError::CorruptChunk(Corruption::NotWave));
    for declared in [0, 3] {
        assert_eq!(
            refused(&riff_declaring(declared, &[])),
            WavError::CorruptChunk(Corruption::RiffSizeTooSmall { declared })
        );
    }
}

#[test]
fn refuses_a_chunk_id_that_is_not_printable() {
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk(&[0, 1, 2, 3], &[0; 4]),
        chunk(b"data", &pcm16_data(&[0])),
    ]);
    assert_eq!(
        refused(&bytes),
        WavError::CorruptChunk(Corruption::BadChunkId { offset: 36 })
    );
}

#[test]
fn refuses_a_chunk_that_runs_past_the_riff_size_inside_the_file() {
    let body = [
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk(b"data", &pcm16_data(&[0, 0])),
    ]
    .concat();
    let mut bytes = riff_declaring((body.len() + 4 - 2) as u32, &body);
    bytes.extend_from_slice(&[0, 0]);
    assert_eq!(
        refused(&bytes),
        WavError::CorruptChunk(Corruption::ChunkPastRiffEnd {
            id: *b"data",
            offset: 36
        })
    );
}

#[test]
fn refuses_a_missing_or_duplicate_fmt_or_data_chunk() {
    let fmt = chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16));
    let data = chunk(b"data", &pcm16_data(&[0]));
    for (chunks, reason) in [
        (vec![data.clone()], Corruption::MissingFmt),
        (vec![fmt.clone()], Corruption::MissingData),
        (Vec::<Vec<u8>>::new(), Corruption::MissingFmt),
        (
            vec![fmt.clone(), fmt.clone(), data.clone()],
            Corruption::DuplicateFmt,
        ),
        (
            vec![fmt.clone(), data.clone(), data.clone()],
            Corruption::DuplicateData,
        ),
    ] {
        assert_eq!(refused(&riff(&chunks)), WavError::CorruptChunk(reason));
    }
}

#[test]
fn refuses_a_fmt_chunk_too_short_or_over_its_allocation_bound() {
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)[..14]),
        chunk(b"data", &pcm16_data(&[0])),
    ]);
    assert_eq!(
        refused(&bytes),
        WavError::CorruptChunk(Corruption::FmtSize { size: 14 })
    );
    let mut long = fmt_body(1, 1, 48_000, 16);
    long.resize(4096, 0);
    let bytes = riff(&[chunk(b"fmt ", &long), chunk(b"data", &pcm16_data(&[0]))]);
    assert_eq!(
        refused(&bytes),
        WavError::CorruptChunk(Corruption::FmtSize { size: 4096 })
    );
}

#[test]
fn refuses_a_fmt_chunk_whose_block_align_or_byte_rate_disagree() {
    let mut fmt = fmt_body(1, 2, 48_000, 16);
    fmt[12..14].copy_from_slice(&2u16.to_le_bytes());
    let bytes = riff(&[chunk(b"fmt ", &fmt), chunk(b"data", &pcm16_data(&[0, 0]))]);
    assert_eq!(
        refused(&bytes),
        WavError::CorruptChunk(Corruption::BlockAlign { declared: 2 })
    );
    let mut fmt = fmt_body(1, 1, 48_000, 16);
    fmt[8..12].copy_from_slice(&1u32.to_le_bytes());
    let bytes = riff(&[chunk(b"fmt ", &fmt), chunk(b"data", &pcm16_data(&[0]))]);
    assert_eq!(
        refused(&bytes),
        WavError::CorruptChunk(Corruption::ByteRate { declared: 1 })
    );
}

#[test]
fn refuses_a_data_chunk_holding_a_partial_frame() {
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(1, 2, 48_000, 16)),
        chunk(b"data", &pcm16_data(&[0, 0, 0])),
    ]);
    assert_eq!(
        refused(&bytes),
        WavError::CorruptChunk(Corruption::PartialFrame {
            data_size: 6,
            block_align: 4
        })
    );
}

// Non-finite samples (§ 2 invariant 4).

#[test]
fn refuses_nan_and_infinite_float32_samples() {
    for value in [f32::NAN, -f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            refused(&f32_wav(1, &[0.0, 0.5, value])),
            WavError::NonFiniteSample {
                frame: 2,
                channel: 0
            }
        );
        assert_eq!(
            refused(&f32_wav(2, &[0.0, 0.0, 0.5, value])),
            WavError::NonFiniteSample {
                frame: 1,
                channel: 1
            }
        );
    }
}

// The frame bound (§ 6.1, § 12.3).

#[test]
fn refuses_a_file_one_frame_past_the_frame_bound() {
    let frames = MAX_FRAMES + 1;
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk(b"data", &vec![0; frames * 2]),
    ]);
    assert_eq!(
        refused(&bytes),
        WavError::InputTooLong {
            frame_count: frames as u64
        }
    );
}

#[test]
fn accepts_a_file_exactly_at_the_frame_bound() {
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk(b"data", &vec![0; MAX_FRAMES * 2]),
    ]);
    assert_eq!(decode(&bytes).unwrap().frame_count(), MAX_FRAMES);
}

#[test]
fn the_frame_bound_is_ten_minutes_at_48_khz() {
    assert_eq!(MAX_FRAMES, 10 * 60 * 48_000);
}
