//! The typed refusals of WAV import and export. Each [`WavError`] names one error of
//! `spec/domains/audio.yaml` ([`WavError::domain_error`]).

use std::fmt;
use std::io;

/// Where the input ended before what its sizes declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truncation {
    /// Fewer than the 12 bytes of `RIFF`, the RIFF size and `WAVE`.
    Header,
    /// The RIFF size declares more bytes than follow it.
    RiffSize { declared: u32, available: u64 },
    /// Fewer than the 8 bytes of a chunk header remain inside the RIFF body.
    ChunkHeader { offset: u64 },
    /// A chunk's size, with its pad byte when the size is odd, runs past the end of the input.
    Chunk { id: [u8; 4], offset: u64, size: u32 },
    /// The input ended while a checked region was being read (a file shrank while it was read).
    Read { offset: u64 },
}

/// How a RIFF/WAVE structure is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corruption {
    /// The input does not start with `RIFF`.
    NotRiff,
    /// The RIFF form type is not `WAVE`.
    NotWave,
    /// The RIFF size cannot even hold the form type.
    RiffSizeTooSmall {
        declared: u32,
    },
    /// A chunk id holds a byte outside printable ASCII.
    BadChunkId {
        offset: u64,
    },
    /// A chunk runs past the RIFF size while the input continues.
    ChunkPastRiffEnd {
        id: [u8; 4],
        offset: u64,
    },
    /// The `fmt ` body is shorter than 16 bytes or longer than [`crate::MAX_FMT_BYTES`].
    FmtSize {
        size: u32,
    },
    MissingFmt,
    MissingData,
    DuplicateFmt,
    DuplicateData,
    /// The block align disagrees with the channel count and bits per sample.
    BlockAlign {
        declared: u16,
    },
    /// The byte rate disagrees with the sample rate and block align.
    ByteRate {
        declared: u32,
    },
    /// The `data` size is not a whole number of frames.
    PartialFrame {
        data_size: u32,
        block_align: u16,
    },
}

/// Why a WAV file was not imported, or a clip not exported. A refusal imports nothing and writes
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WavError {
    /// `patchback.audio.TruncatedRiff`.
    Truncated(Truncation),
    /// `patchback.audio.CorruptRiffChunk`.
    CorruptChunk(Corruption),
    /// `patchback.audio.UnsupportedSampleRate`: not 48 000 Hz; never converted.
    UnsupportedSampleRate { sample_rate_hz: u32 },
    /// `patchback.audio.UnsupportedSampleFormat`: neither tag 1 with 16 bits nor tag 3 with 32.
    UnsupportedSampleFormat {
        format_tag: u16,
        bits_per_sample: u16,
    },
    /// `patchback.audio.UnsupportedChannelCount`: neither one nor two channels.
    UnsupportedChannelCount { channel_count: u16 },
    /// `patchback.audio.EmptyAudio`: no frames.
    EmptyAudio,
    /// `patchback.audio.InputTooLong`: more than [`crate::MAX_FRAMES`] frames.
    InputTooLong { frame_count: u64 },
    /// `patchback.audio.NonFiniteSamples`: a NaN or infinite sample at `frame`, `channel`.
    NonFiniteSample { frame: u64, channel: u16 },
    /// `patchback.audio.SampleOutOfRange`: PCM16 export of a sample outside `[-1, 1]`.
    SampleOutOfRange { frame: u64, value: f32 },
    /// `patchback.audio.ProvenanceMissing`: an [`AudioOrigin::ExternalRecording`] import named
    /// no provenance ([`WavImport`]). The domain's `path` is the one the caller passed in, as
    /// for [`WavError::EmptyAudio`].
    ///
    /// [`AudioOrigin::ExternalRecording`]: crate::AudioOrigin::ExternalRecording
    /// [`WavImport`]: crate::WavImport
    ProvenanceMissing,
    /// `patchback.audio.LicenceMissing`: an [`AudioOrigin::ExternalRecording`] import named its
    /// provenance and no licence ([`WavImport`]).
    ///
    /// [`AudioOrigin::ExternalRecording`]: crate::AudioOrigin::ExternalRecording
    /// [`WavImport`]: crate::WavImport
    LicenceMissing,
}

impl WavError {
    /// Every error of `spec/domains/audio.yaml` this crate can emit.
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
        "patchback.audio.TruncatedRiff",
        "patchback.audio.CorruptRiffChunk",
        "patchback.audio.UnsupportedSampleRate",
        "patchback.audio.UnsupportedSampleFormat",
        "patchback.audio.UnsupportedChannelCount",
        "patchback.audio.EmptyAudio",
        "patchback.audio.InputTooLong",
        "patchback.audio.NonFiniteSamples",
        "patchback.audio.SampleOutOfRange",
        "patchback.audio.ProvenanceMissing",
        "patchback.audio.LicenceMissing",
    ];

    /// The error of `spec/domains/audio.yaml` this refusal is.
    pub fn domain_error(&self) -> &'static str {
        match self {
            Self::Truncated(_) => "patchback.audio.TruncatedRiff",
            Self::CorruptChunk(_) => "patchback.audio.CorruptRiffChunk",
            Self::UnsupportedSampleRate { .. } => "patchback.audio.UnsupportedSampleRate",
            Self::UnsupportedSampleFormat { .. } => "patchback.audio.UnsupportedSampleFormat",
            Self::UnsupportedChannelCount { .. } => "patchback.audio.UnsupportedChannelCount",
            Self::EmptyAudio => "patchback.audio.EmptyAudio",
            Self::InputTooLong { .. } => "patchback.audio.InputTooLong",
            Self::NonFiniteSample { .. } => "patchback.audio.NonFiniteSamples",
            Self::SampleOutOfRange { .. } => "patchback.audio.SampleOutOfRange",
            Self::ProvenanceMissing => "patchback.audio.ProvenanceMissing",
            Self::LicenceMissing => "patchback.audio.LicenceMissing",
        }
    }

    /// The rank of this refusal on import: the index of its
    /// [`domain_error`](Self::domain_error) in [`IMPORT_WAV_PRECEDENCE`]; lower is answered
    /// first. An export refusal ([`WavError::SampleOutOfRange`]) is no refusal of `ImportWav` and
    /// ranks after all of them, at `IMPORT_WAV_PRECEDENCE.len()`.
    ///
    /// [`IMPORT_WAV_PRECEDENCE`]: crate::IMPORT_WAV_PRECEDENCE
    pub fn precedence(&self) -> usize {
        let error = self.domain_error();
        crate::IMPORT_WAV_PRECEDENCE
            .iter()
            .position(|declared| *declared == error)
            .unwrap_or(crate::IMPORT_WAV_PRECEDENCE.len())
    }

    /// One value of every variant. The `match` below has no wildcard, so a new variant does not
    /// compile until it is listed here, and the domain test then checks its name.
    pub fn examples() -> Vec<WavError> {
        let examples = vec![
            Self::Truncated(Truncation::Header),
            Self::CorruptChunk(Corruption::MissingFmt),
            Self::UnsupportedSampleRate {
                sample_rate_hz: 44_100,
            },
            Self::UnsupportedSampleFormat {
                format_tag: 1,
                bits_per_sample: 24,
            },
            Self::UnsupportedChannelCount { channel_count: 6 },
            Self::EmptyAudio,
            Self::InputTooLong { frame_count: 0 },
            Self::NonFiniteSample {
                frame: 0,
                channel: 0,
            },
            Self::SampleOutOfRange {
                frame: 0,
                value: 2.0,
            },
            Self::ProvenanceMissing,
            Self::LicenceMissing,
        ];
        for example in &examples {
            match example {
                Self::Truncated(_)
                | Self::CorruptChunk(_)
                | Self::UnsupportedSampleRate { .. }
                | Self::UnsupportedSampleFormat { .. }
                | Self::UnsupportedChannelCount { .. }
                | Self::EmptyAudio
                | Self::InputTooLong { .. }
                | Self::NonFiniteSample { .. }
                | Self::SampleOutOfRange { .. }
                | Self::ProvenanceMissing
                | Self::LicenceMissing => {}
            }
        }
        examples
    }
}

impl fmt::Display for WavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated(how) => write!(f, "truncated RIFF/WAVE input: {how:?}"),
            Self::CorruptChunk(how) => write!(f, "corrupt RIFF/WAVE chunk: {how:?}"),
            Self::UnsupportedSampleRate { sample_rate_hz } => {
                write!(
                    f,
                    "unsupported sample rate {sample_rate_hz} Hz; only 48000 Hz"
                )
            }
            Self::UnsupportedSampleFormat {
                format_tag,
                bits_per_sample,
            } => write!(
                f,
                "unsupported sample format tag {format_tag} with {bits_per_sample} bits; only PCM16 (tag 1) and float32 (tag 3)"
            ),
            Self::UnsupportedChannelCount { channel_count } => {
                write!(f, "unsupported channel count {channel_count}; only 1 or 2")
            }
            Self::EmptyAudio => write!(f, "the audio holds no frames"),
            Self::InputTooLong { frame_count } => write!(
                f,
                "{frame_count} frames exceed the bound of {} frames",
                crate::MAX_FRAMES
            ),
            Self::NonFiniteSample { frame, channel } => {
                write!(f, "non-finite sample at frame {frame}, channel {channel}")
            }
            Self::SampleOutOfRange { frame, value } => write!(
                f,
                "sample {value} at frame {frame} is outside [-1, 1] and cannot be written as PCM16"
            ),
            Self::ProvenanceMissing => write!(
                f,
                "an external recording is imported only with its provenance recorded"
            ),
            Self::LicenceMissing => write!(
                f,
                "an external recording is imported only with its licence recorded"
            ),
        }
    }
}

impl std::error::Error for WavError {}

/// Why [`crate::read_wav`] imported nothing.
#[derive(Debug)]
pub enum ReadError {
    /// The file could not be opened or read.
    Io(io::Error),
    /// The file was read and refused.
    Wav(WavError),
}

/// Why [`crate::write_wav`] did not write the clip.
#[derive(Debug)]
pub enum WriteError {
    /// The clip was refused; nothing was written.
    Wav(WavError),
    /// The file could not be written.
    Io(io::Error),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "cannot read the WAV file: {error}"),
            Self::Wav(error) => error.fmt(f),
        }
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wav(error) => error.fmt(f),
            Self::Io(error) => write!(f, "cannot write the WAV file: {error}"),
        }
    }
}

impl std::error::Error for ReadError {}
impl std::error::Error for WriteError {}

impl From<WavError> for ReadError {
    fn from(error: WavError) -> Self {
        Self::Wav(error)
    }
}

impl From<WavError> for WriteError {
    fn from(error: WavError) -> Self {
        Self::Wav(error)
    }
}
