//! The WAV writer: a mono 48 kHz file with a 44-byte header.

use std::fs;
use std::path::Path;

use crate::clip::{AudioClip, SAMPLE_RATE_HZ, WavSampleFormat, widen};
use crate::error::{WavError, WriteError};

/// The bytes of a mono 48 kHz WAV file holding `clip` as `format`, or the refusal.
pub fn encode(clip: &AudioClip, format: WavSampleFormat) -> Result<Vec<u8>, WavError> {
    let too_long = || WavError::InputTooLong {
        frame_count: widen(clip.frame_count()),
    };
    let width = format.bytes();
    let data_size = clip
        .frame_count()
        .checked_mul(width)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or_else(too_long)?;
    let riff_size = data_size.checked_add(36).ok_or_else(too_long)?;
    let block_align = u16::try_from(width).map_err(|_| too_long())?;
    let mut out = Vec::with_capacity(44 + clip.frame_count() * width);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&format.format_tag().to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE_HZ.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE_HZ * u32::from(block_align)).to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&format.bits_per_sample().to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    for (frame, &value) in clip.samples().iter().enumerate() {
        match format {
            WavSampleFormat::Pcm16 => {
                if !(-1.0..=1.0).contains(&value) {
                    return Err(WavError::SampleOutOfRange {
                        frame: widen(frame),
                        value,
                    });
                }
                out.extend_from_slice(&pcm16(value).to_le_bytes());
            }
            WavSampleFormat::Float32 => out.extend_from_slice(&value.to_le_bytes()),
        }
    }
    Ok(out)
}

/// `round(value * 32768)`, half away from zero, with 32768 written as 32767. `value` lies in
/// `[-1, 1]`, so the result lies in `[-32768, 32767]` before the conversion.
fn pcm16(value: f32) -> i16 {
    let step = (f64::from(value) * 32_768.0).round().min(32_767.0);
    step as i16
}

/// Writes `clip` to `path` as a mono 48 kHz WAV file. A refused clip writes nothing.
pub fn write_wav(path: &Path, clip: &AudioClip, format: WavSampleFormat) -> Result<(), WriteError> {
    let bytes = encode(clip, format)?;
    fs::write(path, bytes).map_err(WriteError::Io)
}
