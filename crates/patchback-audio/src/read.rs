//! The RIFF/WAVE reader. It walks the chunk headers first, checking every declared size against
//! the input length with `u64` arithmetic. The only buffer it sizes from the input holds the
//! samples, and only once their count is within [`MAX_FRAMES`]; the `fmt ` body is bounded by
//! [`MAX_FMT_BYTES`] and read into a fixed 16 bytes, and unknown chunks are skipped unread.

use std::convert::Infallible;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use crate::clip::{
    AudioClip, MAX_FRAMES, SAMPLE_RATE_HZ, WavSampleFormat, check_frames, downmix_frame, widen,
};
use crate::error::{Corruption, ReadError, Truncation, WavError};

/// The allocation bound of the `fmt ` body. The longest standard body, `WAVE_FORMAT_EXTENSIBLE`,
/// is 40 bytes.
pub const MAX_FMT_BYTES: u32 = 1024;

/// How many bytes of the `data` chunk are read at a time; a multiple of every block align.
const BLOCK_BYTES: usize = 64 * 1024;

/// Why a read stopped: a refusal, or an I/O failure of the source.
enum Fail<E> {
    Wav(WavError),
    Source(E),
}

impl<E> From<WavError> for Fail<E> {
    fn from(error: WavError) -> Self {
        Self::Wav(error)
    }
}

/// Random-access input of a known length.
trait Source {
    type Error;
    fn len(&self) -> u64;
    /// Fills `buf` from `offset`; running out of input is [`Truncation::Read`].
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), Fail<Self::Error>>;
}

struct Slice<'a>(&'a [u8]);

impl Source for Slice<'_> {
    type Error = Infallible;

    fn len(&self) -> u64 {
        widen(self.0.len())
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), Fail<Infallible>> {
        let truncated = || Fail::Wav(WavError::Truncated(Truncation::Read { offset }));
        let start = usize::try_from(offset).map_err(|_| truncated())?;
        let end = start.checked_add(buf.len()).ok_or_else(truncated)?;
        let bytes = self.0.get(start..end).ok_or_else(truncated)?;
        buf.copy_from_slice(bytes);
        Ok(())
    }
}

struct FileSource {
    file: File,
    len: u64,
}

impl Source for FileSource {
    type Error = io::Error;

    fn len(&self) -> u64 {
        self.len
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), Fail<io::Error>> {
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(Fail::Source)?;
        self.file.read_exact(buf).map_err(|error| {
            if error.kind() == io::ErrorKind::UnexpectedEof {
                Fail::Wav(WavError::Truncated(Truncation::Read { offset }))
            } else {
                Fail::Source(error)
            }
        })
    }
}

/// Reads a RIFF/WAVE byte slice into a clip, or refuses it.
pub fn decode(bytes: &[u8]) -> Result<AudioClip, WavError> {
    read(&mut Slice(bytes)).map_err(|fail| match fail {
        Fail::Wav(error) => error,
        Fail::Source(never) => match never {},
    })
}

/// Reads a RIFF/WAVE file into a clip, or refuses it. The file is read chunk by chunk; it is
/// never loaded whole.
pub fn read_wav(path: &Path) -> Result<AudioClip, ReadError> {
    let file = File::open(path).map_err(ReadError::Io)?;
    let len = file.metadata().map_err(ReadError::Io)?.len();
    read(&mut FileSource { file, len }).map_err(|fail| match fail {
        Fail::Wav(error) => ReadError::Wav(error),
        Fail::Source(error) => ReadError::Io(error),
    })
}

/// The `fmt ` fields the reader uses.
struct Fmt {
    format_tag: u16,
    channel_count: u16,
    sample_rate_hz: u32,
    byte_rate: u32,
    block_align: u16,
    bits_per_sample: u16,
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn corrupt<E>(how: Corruption) -> Fail<E> {
    Fail::Wav(WavError::CorruptChunk(how))
}

fn truncated<E>(how: Truncation) -> Fail<E> {
    Fail::Wav(WavError::Truncated(how))
}

fn read<S: Source>(source: &mut S) -> Result<AudioClip, Fail<S::Error>> {
    let len = source.len();
    if len < 12 {
        return Err(truncated(Truncation::Header));
    }
    let mut header = [0u8; 12];
    source.read_at(0, &mut header)?;
    if &header[0..4] != b"RIFF" {
        return Err(corrupt(Corruption::NotRiff));
    }
    let riff_size = u32_at(&header, 4);
    if riff_size < 4 {
        return Err(corrupt(Corruption::RiffSizeTooSmall {
            declared: riff_size,
        }));
    }
    let riff_end = 8 + u64::from(riff_size);
    if riff_end > len {
        return Err(truncated(Truncation::RiffSize {
            declared: riff_size,
            available: len - 8,
        }));
    }
    if &header[8..12] != b"WAVE" {
        return Err(corrupt(Corruption::NotWave));
    }

    let mut fmt: Option<Fmt> = None;
    let mut data: Option<(u64, u32)> = None;
    let mut offset = 12u64;
    while offset < riff_end {
        if riff_end - offset < 8 {
            return Err(truncated(Truncation::ChunkHeader { offset }));
        }
        let mut chunk = [0u8; 8];
        source.read_at(offset, &mut chunk)?;
        let id = [chunk[0], chunk[1], chunk[2], chunk[3]];
        let size = u32_at(&chunk, 4);
        if !id.iter().all(|b| (0x20..=0x7e).contains(b)) {
            return Err(corrupt(Corruption::BadChunkId { offset }));
        }
        let body = offset + 8;
        let end = body + u64::from(size) + u64::from(size & 1);
        if end > len {
            return Err(truncated(Truncation::Chunk { id, offset, size }));
        }
        if end > riff_end {
            return Err(corrupt(Corruption::ChunkPastRiffEnd { id, offset }));
        }
        match &id {
            b"fmt " => {
                if fmt.is_some() {
                    return Err(corrupt(Corruption::DuplicateFmt));
                }
                if !(16..=MAX_FMT_BYTES).contains(&size) {
                    return Err(corrupt(Corruption::FmtSize { size }));
                }
                let mut bytes = [0u8; 16];
                source.read_at(body, &mut bytes)?;
                fmt = Some(Fmt {
                    format_tag: u16_at(&bytes, 0),
                    channel_count: u16_at(&bytes, 2),
                    sample_rate_hz: u32_at(&bytes, 4),
                    byte_rate: u32_at(&bytes, 8),
                    block_align: u16_at(&bytes, 12),
                    bits_per_sample: u16_at(&bytes, 14),
                });
            }
            b"data" => {
                if data.is_some() {
                    return Err(corrupt(Corruption::DuplicateData));
                }
                data = Some((body, size));
            }
            _ => {}
        }
        offset = end;
    }

    let fmt = fmt.ok_or(corrupt(Corruption::MissingFmt))?;
    let (data_offset, data_size) = data.ok_or(corrupt(Corruption::MissingData))?;
    let format = check_fmt(&fmt)?;
    if data_size % u32::from(fmt.block_align) != 0 {
        return Err(corrupt(Corruption::PartialFrame {
            data_size,
            block_align: fmt.block_align,
        }));
    }
    let frames = data_size / u32::from(fmt.block_align);
    let frames = usize::try_from(frames).map_err(|_| {
        Fail::Wav(WavError::InputTooLong {
            frame_count: u64::from(frames),
        })
    })?;
    check_frames(frames)?;
    let samples = read_samples(source, data_offset, frames, fmt.channel_count, format)?;
    Ok(AudioClip::read(samples, fmt.channel_count, format))
}

/// Refuses an unsupported or inconsistent `fmt `, in the order the domain lists the refusals:
/// sample rate, sample format, channel count, then block align and byte rate.
fn check_fmt<E>(fmt: &Fmt) -> Result<WavSampleFormat, Fail<E>> {
    if fmt.sample_rate_hz != SAMPLE_RATE_HZ {
        return Err(Fail::Wav(WavError::UnsupportedSampleRate {
            sample_rate_hz: fmt.sample_rate_hz,
        }));
    }
    let format = match (fmt.format_tag, fmt.bits_per_sample) {
        (1, 16) => WavSampleFormat::Pcm16,
        (3, 32) => WavSampleFormat::Float32,
        (format_tag, bits_per_sample) => {
            return Err(Fail::Wav(WavError::UnsupportedSampleFormat {
                format_tag,
                bits_per_sample,
            }));
        }
    };
    if !(1..=2).contains(&fmt.channel_count) {
        return Err(Fail::Wav(WavError::UnsupportedChannelCount {
            channel_count: fmt.channel_count,
        }));
    }
    let block_align = u32::from(fmt.channel_count) * u32::from(fmt.bits_per_sample) / 8;
    if u32::from(fmt.block_align) != block_align {
        return Err(corrupt(Corruption::BlockAlign {
            declared: fmt.block_align,
        }));
    }
    if u64::from(fmt.byte_rate) != u64::from(fmt.sample_rate_hz) * u64::from(block_align) {
        return Err(corrupt(Corruption::ByteRate {
            declared: fmt.byte_rate,
        }));
    }
    Ok(format)
}

/// Reads `frames` frames of `channels` interleaved samples from `offset`, refusing a non-finite
/// sample and downmixing stereo by [`downmix_frame`]. `frames` is at most [`MAX_FRAMES`].
fn read_samples<S: Source>(
    source: &mut S,
    offset: u64,
    frames: usize,
    channels: u16,
    format: WavSampleFormat,
) -> Result<Vec<f32>, Fail<S::Error>> {
    debug_assert!(frames <= MAX_FRAMES);
    let width = format.bytes();
    let frame_bytes = width * usize::from(channels);
    let frames_per_block = BLOCK_BYTES / frame_bytes;
    let mut samples = Vec::with_capacity(frames);
    let mut block = vec![0u8; frames_per_block.min(frames) * frame_bytes];
    let mut frame = 0usize;
    while frame < frames {
        let count = frames_per_block.min(frames - frame);
        let bytes = &mut block[..count * frame_bytes];
        source.read_at(offset + widen(frame) * widen(frame_bytes), bytes)?;
        for (index, raw) in bytes.chunks_exact(frame_bytes).enumerate() {
            let at = frame + index;
            let mut values = [0f32; 2];
            for (channel, sample) in raw.chunks_exact(width).enumerate() {
                let value = match format {
                    WavSampleFormat::Pcm16 => {
                        f32::from(i16::from_le_bytes([sample[0], sample[1]])) / 32_768.0
                    }
                    WavSampleFormat::Float32 => {
                        f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]])
                    }
                };
                if !value.is_finite() {
                    return Err(Fail::Wav(WavError::NonFiniteSample {
                        frame: widen(at),
                        channel: u16::try_from(channel).unwrap_or(u16::MAX),
                    }));
                }
                values[channel] = value;
            }
            samples.push(if channels == 2 {
                downmix_frame(values[0], values[1])
            } else {
                values[0]
            });
        }
        frame += count;
    }
    Ok(samples)
}
