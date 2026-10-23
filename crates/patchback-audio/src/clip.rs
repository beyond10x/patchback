//! The audio clip: finite mono samples at 48 kHz and what they were read from.

use std::fmt;

use crate::error::WavError;

/// The only sample rate Patchback reads, writes and renders (DESIGN.md § 5.3). Any other rate is
/// refused, never converted.
pub const SAMPLE_RATE_HZ: u32 = 48_000;

/// The frame bound of DESIGN.md § 6.1 and § 12.3: the most frames a clip holds, so the most an
/// import allocates for samples.
///
/// Provisional: DESIGN.md names no value (story:wav-io, open question). Ten minutes at 48 kHz,
/// well above the 0.5–2 s analysis unit of § 6.1 and the phrases of § 10, and small enough that
/// the samples of a bounded clip take at most 115.2 MB.
pub const MAX_FRAMES: usize = 10 * 60 * SAMPLE_RATE_HZ as usize;

/// The sample formats Patchback reads and writes (DESIGN.md § 6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WavSampleFormat {
    /// `fmt ` format tag 1, 16 bits per sample.
    Pcm16,
    /// `fmt ` format tag 3, 32 bits per sample.
    Float32,
}

impl WavSampleFormat {
    /// The `fmt ` format tag.
    pub fn format_tag(self) -> u16 {
        match self {
            Self::Pcm16 => 1,
            Self::Float32 => 3,
        }
    }

    /// Bits per sample.
    pub fn bits_per_sample(self) -> u16 {
        match self {
            Self::Pcm16 => 16,
            Self::Float32 => 32,
        }
    }

    /// Bytes per mono sample.
    pub(crate) fn bytes(self) -> usize {
        match self {
            Self::Pcm16 => 2,
            Self::Float32 => 4,
        }
    }
}

/// Mono audio at [`SAMPLE_RATE_HZ`]: between 1 and [`MAX_FRAMES`] finite samples.
#[derive(Clone, PartialEq)]
pub struct AudioClip {
    samples: Vec<f32>,
    source_channel_count: u16,
    downmixed: bool,
    source_format: Option<WavSampleFormat>,
}

/// The mean of a stereo frame, the one downmix formula (DESIGN.md § 6.1): `(left + right) / 2`
/// evaluated in `f64` and rounded once to `f32`. For finite input the result is finite and lies
/// between `left` and `right`.
pub fn downmix_frame(left: f32, right: f32) -> f32 {
    ((f64::from(left) + f64::from(right)) / 2.0) as f32
}

/// A count or index as `u64`; `usize` is never wider on the targets Rust supports, and the
/// fallback only keeps the conversion free of `as`.
pub(crate) fn widen(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// Refuses a frame count of zero or past [`MAX_FRAMES`].
pub(crate) fn check_frames(frame_count: usize) -> Result<(), WavError> {
    if frame_count == 0 {
        return Err(WavError::EmptyAudio);
    }
    if frame_count > MAX_FRAMES {
        return Err(WavError::InputTooLong {
            frame_count: widen(frame_count),
        });
    }
    Ok(())
}

impl AudioClip {
    /// A clip of `samples`, which were not read from a WAV file (a render, a test).
    pub fn from_mono(samples: Vec<f32>) -> Result<Self, WavError> {
        check_frames(samples.len())?;
        if let Some(frame) = samples.iter().position(|s| !s.is_finite()) {
            return Err(WavError::NonFiniteSample {
                frame: widen(frame),
                channel: 0,
            });
        }
        Ok(Self {
            samples,
            source_channel_count: 1,
            downmixed: false,
            source_format: None,
        })
    }

    /// A clip the reader built; the reader has already checked the samples.
    pub(crate) fn read(
        samples: Vec<f32>,
        source_channel_count: u16,
        source_format: WavSampleFormat,
    ) -> Self {
        Self {
            samples,
            source_channel_count,
            downmixed: source_channel_count == 2,
            source_format: Some(source_format),
        }
    }

    /// The mono samples.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// The number of frames, which is the number of samples.
    pub fn frame_count(&self) -> usize {
        self.samples.len()
    }

    /// Always [`SAMPLE_RATE_HZ`].
    pub fn sample_rate_hz(&self) -> u32 {
        SAMPLE_RATE_HZ
    }

    /// How many channels the source had: 1, or 2 for a downmixed stereo file.
    pub fn source_channel_count(&self) -> u16 {
        self.source_channel_count
    }

    /// True when a stereo source was downmixed to mono by [`downmix_frame`].
    pub fn downmixed(&self) -> bool {
        self.downmixed
    }

    /// The sample format of the WAV file the clip was read from, if it was read from one.
    pub fn source_format(&self) -> Option<WavSampleFormat> {
        self.source_format
    }
}

impl fmt::Debug for AudioClip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioClip")
            .field("frame_count", &self.samples.len())
            .field("source_channel_count", &self.source_channel_count)
            .field("downmixed", &self.downmixed)
            .field("source_format", &self.source_format)
            .finish_non_exhaustive()
    }
}
