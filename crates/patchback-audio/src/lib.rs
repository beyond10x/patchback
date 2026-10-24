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
//! # Provenance
//!
//! [`WavImport`] is the `ImportWav` request of `spec/domains/audio.yaml`: the bytes or the file,
//! and beside them an [`AudioOrigin`], an optional provenance, an optional licence and an
//! optional render id. [`WavImport::decode`] and [`WavImport::read`] read as [`decode`] and
//! [`read_wav`] do; the clip keeps the request ([`AudioClip::origin`],
//! [`AudioClip::provenance`], [`AudioClip::licence`], [`AudioClip::render_id`]). An
//! [`AudioOrigin::ExternalRecording`] without provenance is refused with
//! [`WavError::ProvenanceMissing`], one with provenance and without a licence with
//! [`WavError::LicenceMissing`], on every import, not only of test fixtures (DESIGN.md § 12.3,
//! the AGENTS.md fixture rule); an [`AudioOrigin::PatchbackSynthesized`] clip needs neither and
//! may name its render. A refusal leaves no clip. A clip read by [`decode`] or [`read_wav`],
//! which take no request, or built by [`AudioClip::from_mono`] records no origin: its
//! [`AudioClip::origin`] is `None`.
//!
//! ESS answers a command's input refusals first, in declaration order, then the outcomes marked
//! `before_row: true`, then the existence check, then the other external outcomes in declaration
//! order (the `ess` repository, `website/docs/guides/specify/guards-and-predicates.md` § "Which
//! branch answers"). So a request or file with several faults is refused with the one ranked
//! first in [`IMPORT_WAV_PRECEDENCE`] ([`WavError::precedence`]):
//!
//! 1. [`WavError::ProvenanceMissing`], then [`WavError::LicenceMissing`];
//! 2. the sample rate, the sample format, the channel count, then an empty `data` chunk;
//! 3. truncated input ([`WavError::Truncated`]), then a corrupt chunk
//!    ([`WavError::CorruptChunk`]), the two `before_row` outcomes;
//! 4. the frame bound ([`WavError::InputTooLong`]), then a non-finite sample.
//!
//! [`WavImport::decode`] and [`WavImport::read`] answer the guards before a byte is read or the
//! file is opened, so a missing, unreadable, truncated or corrupt file without provenance is
//! refused for its provenance. [`decode`] and [`read_wav`], with or without a request, take the
//! order from step 2 on wherever the file's header facts exist: once the `fmt ` chunk and the
//! `data` chunk header are read, the rate, format, channel count and empty checks answer before
//! any truncation or corruption the chunk walk found, a block align or byte rate that disagrees
//! and a partial frame included. A file whose header facts cannot be read (its RIFF header cut
//! short, no `fmt ` or no `data` chunk, a chunk header corrupt before both were found) gives the
//! model nothing for step 2 to decide and is refused as truncated or corrupt. Of a truncation
//! and a corruption the reader sees both of, the truncation answers: a RIFF size or a chunk
//! past the end of the file before a block align, byte rate or partial frame, and before a
//! second `fmt ` or `data` chunk or a `fmt ` over its bound, which the walk steps over. The
//! walk stops at a chunk it cannot trust (a chunk header not printable or past the RIFF size),
//! so a truncation behind such a chunk is not seen and the corruption answers. A file a request
//! both guards pass
//! that cannot be opened or read is [`ReadError::Io`]. The request is not checked against
//! renders: `RenderNotFound` is no refusal of this crate, which holds none.
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
mod import;
mod read;
mod write;

pub use clip::{AudioClip, MAX_FRAMES, SAMPLE_RATE_HZ, WavSampleFormat, downmix_frame};
pub use error::{Corruption, ReadError, Truncation, WavError, WriteError};
pub use import::{AudioOrigin, IMPORT_WAV_PRECEDENCE, WavImport};
pub use read::{MAX_FMT_BYTES, decode, read_wav};
pub use write::{encode, write_wav};
