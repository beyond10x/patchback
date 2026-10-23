//! The RIFF/WAVE reader. Input of a known length (a byte slice, a regular file) is walked chunk
//! header by chunk header, every declared size checked against that length with `u64`
//! arithmetic; the only buffer sized from the input holds the samples, and only once their count
//! is within [`MAX_FRAMES`]. Input without one (a pipe) is read once, front to back, with the
//! same refusals; it holds the `data` bytes only while they are within [`MAX_FRAMES`] frames, in
//! a buffer that grows with the bytes read, never with a declared size. Either way the magic is
//! compared before any length, the `fmt ` body is bounded by [`MAX_FMT_BYTES`] and read into a
//! fixed 16 bytes, and unknown chunks are skipped, never held.

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

/// Reads a RIFF/WAVE file into a clip, or refuses it, with the refusals [`decode`] gives for
/// the same bytes. A regular file is read chunk by chunk at the offsets its headers declare,
/// its length taken by seeking to its end. Any other file (a pipe, a FIFO, `/dev/fd/N`) has no
/// length to look up and is read once, front to back. Neither is ever loaded whole.
pub fn read_wav(path: &Path) -> Result<AudioClip, ReadError> {
    let mut file = File::open(path).map_err(ReadError::Io)?;
    let regular = file.metadata().map_err(ReadError::Io)?.is_file();
    let result = if regular {
        let len = file.seek(SeekFrom::End(0)).map_err(ReadError::Io)?;
        read(&mut FileSource { file, len })
    } else {
        stream(&mut Stream {
            input: file,
            offset: 0,
        })
    };
    result.map_err(|fail| match fail {
        Fail::Wav(error) => ReadError::Wav(error),
        Fail::Source(error) => ReadError::Io(error),
    })
}

/// Input read once, front to back; its length is known only once its end is reached.
struct Stream<R> {
    input: R,
    /// How many bytes have been read.
    offset: u64,
}

impl<R: Read> Stream<R> {
    /// Fills as much of `buf` as the input holds; fewer than `buf.len()` bytes means its end.
    fn fill(&mut self, buf: &mut [u8]) -> Result<usize, Fail<io::Error>> {
        let mut filled = 0;
        while filled < buf.len() {
            match self.input.read(&mut buf[filled..]) {
                Ok(0) => break,
                Ok(count) => filled += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(Fail::Source(error)),
            }
        }
        self.offset += widen(filled);
        Ok(filled)
    }

    /// Reads and discards input up to offset `to`; false when the input ends first.
    fn skip_to(&mut self, to: u64) -> Result<bool, Fail<io::Error>> {
        let mut scratch = [0u8; 8 * 1024];
        while self.offset < to {
            let want = usize::try_from(to - self.offset)
                .unwrap_or(usize::MAX)
                .min(scratch.len());
            if self.fill(&mut scratch[..want])? < want {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Appends the next `count` bytes to `bytes`, which grows with the bytes read, never with
    /// `count`; false when the input ends first.
    fn take(&mut self, count: u32, bytes: &mut Vec<u8>) -> Result<bool, Fail<io::Error>> {
        let read = (&mut self.input)
            .take(u64::from(count))
            .read_to_end(bytes)
            .map_err(Fail::Source)?;
        self.offset += widen(read);
        Ok(widen(read) == u64::from(count))
    }
}

/// The refusal of a RIFF size that runs past input which ended at `len` bytes.
fn riff_past_input<E>(declared: u32, len: u64) -> Fail<E> {
    truncated(Truncation::RiffSize {
        declared,
        available: len - 8,
    })
}

/// [`read`] over a [`Stream`]. [`read`] refuses a RIFF size past the input before it looks at
/// any chunk; a stream learns its length only at its end, so a refusal found inside the RIFF
/// body stands only once the input is known to reach the RIFF end, and the input ending first
/// is that RIFF-size refusal instead. A `data` chunk is held as raw bytes until the walk ends,
/// since its `fmt ` may follow it, and only while it is within [`MAX_FRAMES`] frames.
fn stream<R: Read>(input: &mut Stream<R>) -> Result<AudioClip, Fail<io::Error>> {
    let mut header = [0u8; 12];
    let got = input.fill(&mut header)?;
    check_magic(&header[..got])?;
    if got < 12 {
        return Err(truncated(Truncation::Header));
    }
    let riff_size = u32_at(&header, 4);
    if riff_size < 4 {
        return Err(corrupt(Corruption::RiffSizeTooSmall {
            declared: riff_size,
        }));
    }
    let riff_end = 8 + u64::from(riff_size);
    let past = |input: &Stream<R>| riff_past_input(riff_size, input.offset);
    let settle = |input: &mut Stream<R>, error: WavError| -> Fail<io::Error> {
        match input.skip_to(riff_end) {
            Ok(true) => Fail::Wav(error),
            Ok(false) => riff_past_input(riff_size, input.offset),
            Err(fail) => fail,
        }
    };

    let mut fmt: Option<Fmt> = None;
    let mut data: Option<(u32, Option<Vec<u8>>)> = None;
    while input.offset < riff_end {
        let offset = input.offset;
        if riff_end - offset < 8 {
            return Err(settle(
                input,
                WavError::Truncated(Truncation::ChunkHeader { offset }),
            ));
        }
        let mut chunk = [0u8; 8];
        if input.fill(&mut chunk)? < 8 {
            return Err(past(input));
        }
        let id = [chunk[0], chunk[1], chunk[2], chunk[3]];
        let size = u32_at(&chunk, 4);
        if !id.iter().all(|b| (0x20..=0x7e).contains(b)) {
            return Err(settle(
                input,
                WavError::CorruptChunk(Corruption::BadChunkId { offset }),
            ));
        }
        let end = offset + 8 + u64::from(size) + u64::from(size & 1);
        if end > riff_end {
            if !input.skip_to(riff_end)? {
                return Err(past(input));
            }
            return Err(if input.skip_to(end)? {
                corrupt(Corruption::ChunkPastRiffEnd { id, offset })
            } else {
                truncated(Truncation::Chunk { id, offset, size })
            });
        }
        match &id {
            b"fmt " => {
                if fmt.is_some() {
                    return Err(settle(
                        input,
                        WavError::CorruptChunk(Corruption::DuplicateFmt),
                    ));
                }
                if !(16..=MAX_FMT_BYTES).contains(&size) {
                    return Err(settle(
                        input,
                        WavError::CorruptChunk(Corruption::FmtSize { size }),
                    ));
                }
                let mut bytes = [0u8; 16];
                if input.fill(&mut bytes)? < 16 {
                    return Err(past(input));
                }
                fmt = Some(Fmt::parse(&bytes));
            }
            b"data" => {
                if data.is_some() {
                    return Err(settle(
                        input,
                        WavError::CorruptChunk(Corruption::DuplicateData),
                    ));
                }
                let frame_bytes = fmt.as_ref().map_or(8, |fmt| u64::from(fmt.block_align));
                let bound = widen(MAX_FRAMES).saturating_mul(frame_bytes.min(8));
                let bytes = if u64::from(size) <= bound {
                    let mut bytes = Vec::new();
                    if !input.take(size, &mut bytes)? {
                        return Err(past(input));
                    }
                    Some(bytes)
                } else {
                    None
                };
                data = Some((size, bytes));
            }
            _ => {}
        }
        if !input.skip_to(end)? {
            return Err(past(input));
        }
    }

    let fmt = fmt.ok_or(corrupt(Corruption::MissingFmt))?;
    let (data_size, bytes) = data.ok_or(corrupt(Corruption::MissingData))?;
    let (format, frames) = check_data(&fmt, data_size)?;
    // A data chunk past the held bound holds more than MAX_FRAMES frames, which check_data
    // refuses; this refusal is not reached.
    let bytes = bytes.ok_or(Fail::Wav(WavError::InputTooLong {
        frame_count: widen(frames),
    }))?;
    let samples =
        read_samples(&mut Slice(&bytes), 0, frames, fmt.channel_count, format).map_err(|fail| {
            match fail {
                Fail::Wav(error) => Fail::Wav(error),
                Fail::Source(never) => match never {},
            }
        })?;
    Ok(AudioClip::read(samples, fmt.channel_count, format))
}

/// Compares the `RIFF` and `WAVE` magic on whichever of the header's 12 bytes exist, so short
/// input that is not RIFF/WAVE is refused as such rather than as truncated.
fn check_magic<E>(header: &[u8]) -> Result<(), Fail<E>> {
    let riff = &header[..header.len().min(4)];
    if riff != &b"RIFF"[..riff.len()] {
        return Err(corrupt(Corruption::NotRiff));
    }
    let wave = header.get(8..header.len().min(12)).unwrap_or(&[]);
    if wave != &b"WAVE"[..wave.len()] {
        return Err(corrupt(Corruption::NotWave));
    }
    Ok(())
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

impl Fmt {
    /// The first 16 bytes of a `fmt ` body.
    fn parse(bytes: &[u8; 16]) -> Self {
        Self {
            format_tag: u16_at(bytes, 0),
            channel_count: u16_at(bytes, 2),
            sample_rate_hz: u32_at(bytes, 4),
            byte_rate: u32_at(bytes, 8),
            block_align: u16_at(bytes, 12),
            bits_per_sample: u16_at(bytes, 14),
        }
    }
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
    let mut header = [0u8; 12];
    let got = header.len().min(usize::try_from(len).unwrap_or(usize::MAX));
    source.read_at(0, &mut header[..got])?;
    check_magic(&header[..got])?;
    if got < 12 {
        return Err(truncated(Truncation::Header));
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
                fmt = Some(Fmt::parse(&bytes));
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
    let (format, frames) = check_data(&fmt, data_size)?;
    let samples = read_samples(source, data_offset, frames, fmt.channel_count, format)?;
    Ok(AudioClip::read(samples, fmt.channel_count, format))
}

/// Refuses an unsupported `fmt ` ([`check_fmt`]), a `data` size that is not whole frames, and a
/// frame count that is zero or past [`MAX_FRAMES`]; otherwise the sample format and frame count.
fn check_data<E>(fmt: &Fmt, data_size: u32) -> Result<(WavSampleFormat, usize), Fail<E>> {
    let format = check_fmt(fmt)?;
    if !data_size.is_multiple_of(u32::from(fmt.block_align)) {
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
    Ok((format, frames))
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
