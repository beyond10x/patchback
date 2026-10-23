//! std-only WAV input and output, and later the optional live audio backend (DESIGN.md § 4.1,
//! § 3, § 6.1).
//!
//! # Import
//!
//! [`decode`] reads a RIFF/WAVE byte slice and [`read_wav`] a file into an [`AudioClip`]: mono
//! audio at [`SAMPLE_RATE_HZ`]. Accepted are `fmt ` format tag 1 with 16 bits (PCM16) and tag 3
//! with 32 bits (IEEE float32), one or two channels, 48 000 Hz (DESIGN.md § 5.3, § 6.1). Unknown
//! chunks are skipped together with their pad byte. Everything else is refused with a typed
//! [`WavError`] and nothing is imported; `WAVE_FORMAT_EXTENSIBLE` (tag 65534) is refused as an
//! unsupported format even when it wraps PCM16 or float32.
//!
//! Every RIFF and chunk size is compared with the remaining input before anything is read, and
//! the only buffer sized from the input holds the samples, bounded by [`MAX_FRAMES`]. A `fmt `
//! body longer than [`MAX_FMT_BYTES`] is refused and only its first 16 bytes are read; unknown
//! chunks are skipped, never copied.
//!
//! # Downmix
//!
//! A stereo frame becomes one mono sample by [`downmix_frame`]: `(left + right) / 2`, evaluated
//! in `f64` and rounded once to `f32`, so it never overflows. The clip records that it was
//! downmixed and from how many channels ([`AudioClip::downmixed`],
//! [`AudioClip::source_channel_count`]).
//!
//! # Export
//!
//! [`encode`] and [`write_wav`] write a clip as a mono 48 kHz WAV, PCM16 or float32, with a
//! 44-byte header (`RIFF`, a 16-byte `fmt `, `data`). PCM16 maps a sample `x` to
//! `round(x * 32768)`, rounding half away from zero, with `+1.0` (and anything that rounds to
//! 32768) written as 32767; PCM16 import maps `s` to `s / 32768`, so import then export is
//! byte-exact. A sample outside `[-1, 1]` is refused for PCM16 export rather than clipped
//! ([`WavError::SampleOutOfRange`]); float32 export writes every finite sample as it is.

mod clip;
mod error;
mod read;
mod write;

pub use clip::{AudioClip, MAX_FRAMES, SAMPLE_RATE_HZ, WavSampleFormat, downmix_frame};
pub use error::{Corruption, ReadError, Truncation, WavError, WriteError};
pub use read::{MAX_FMT_BYTES, decode, read_wav};
pub use write::{encode, write_wav};
