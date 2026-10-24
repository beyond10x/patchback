//! The import request: what `ImportWav` (`spec/domains/audio.yaml`) takes beyond the bytes, and
//! the two guards that keep every external recording licensed and traced (DESIGN.md § 12.3, the
//! AGENTS.md fixture rule).

use std::path::Path;

use crate::clip::AudioClip;
use crate::error::{ReadError, WavError};
use crate::read::{decode, read_wav};

/// The refusals of `ImportWav` this crate emits, in the order ESS answers them. That order is
/// documented in the `ess` repository at `website/docs/guides/specify/guards-and-predicates.md`
/// § "Which branch answers" and `website/docs/reference/diagnostics.md` (ESS-AUTHOR-041): the
/// input refusals (a `when:` with an `error:`) first, in declaration order (the two guards, then
/// the sample rate, format, channel count and empty predicates); then the outcomes marked
/// `before_row: true` (`truncated`, `corrupt-chunk`), taken before the row is read, so before the
/// existence check; then the existence check; then the other external outcomes (`too-long`,
/// `non-finite`) in declaration order. The precedence of refusals (crate documentation,
/// "Provenance").
///
/// `patchback.audio.RenderNotFound` (`no-such-render`, the existence check, between
/// `CorruptRiffChunk` and `InputTooLong`) is not here: this crate holds no renders, so it cannot
/// tell whether a named one exists, and records the render id as given.
pub const IMPORT_WAV_PRECEDENCE: [&str; 10] = [
    "patchback.audio.ProvenanceMissing",
    "patchback.audio.LicenceMissing",
    "patchback.audio.UnsupportedSampleRate",
    "patchback.audio.UnsupportedSampleFormat",
    "patchback.audio.UnsupportedChannelCount",
    "patchback.audio.EmptyAudio",
    "patchback.audio.TruncatedRiff",
    "patchback.audio.CorruptRiffChunk",
    "patchback.audio.InputTooLong",
    "patchback.audio.NonFiniteSamples",
];

/// Where audio came from (`patchback.audio.AudioOrigin`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioOrigin {
    /// Rendered by Patchback; needs neither provenance nor licence.
    PatchbackSynthesized,
    /// Recorded or obtained outside Patchback; imported only with both.
    ExternalRecording,
}

/// An `ImportWav` request: the origin, provenance, licence and render id the imported clip
/// records ([`AudioClip::origin`], [`AudioClip::provenance`], [`AudioClip::licence`],
/// [`AudioClip::render_id`]).
///
/// An [`AudioOrigin::ExternalRecording`] without provenance is refused with
/// [`WavError::ProvenanceMissing`], one with provenance and without a licence with
/// [`WavError::LicenceMissing`]. A provenance or licence counts as given when it is `Some`, as the
/// domain's `defined(…)` does; its text is recorded as given. Neither is asked of an
/// [`AudioOrigin::PatchbackSynthesized`] clip, and no origin has to name a render: whatever is
/// given is kept.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WavImport {
    pub origin: AudioOrigin,
    pub provenance: Option<String>,
    pub licence: Option<String>,
    /// The render whose WAV is imported, as the caller names it. Its existence is not checked
    /// here ([`IMPORT_WAV_PRECEDENCE`]).
    pub render_id: Option<String>,
}

impl WavImport {
    /// A request of `origin` that names no provenance, licence or render.
    pub fn new(origin: AudioOrigin) -> Self {
        Self {
            origin,
            provenance: None,
            licence: None,
            render_id: None,
        }
    }

    /// This request with `provenance` recorded.
    pub fn with_provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    /// This request with `licence` recorded.
    pub fn with_licence(mut self, licence: impl Into<String>) -> Self {
        self.licence = Some(licence.into());
        self
    }

    /// This request naming the render `render_id`.
    pub fn with_render_id(mut self, render_id: impl Into<String>) -> Self {
        self.render_id = Some(render_id.into());
        self
    }

    /// The two guards alone, without any bytes: the refusal this request carries whatever the
    /// file holds, since both rank before every refusal of the file ([`IMPORT_WAV_PRECEDENCE`]).
    pub fn check(&self) -> Result<(), WavError> {
        if self.origin == AudioOrigin::ExternalRecording {
            if self.provenance.is_none() {
                return Err(WavError::ProvenanceMissing);
            }
            if self.licence.is_none() {
                return Err(WavError::LicenceMissing);
            }
        }
        Ok(())
    }

    /// Answers this request's guards ([`WavImport::check`]) before a byte is read, then reads a
    /// RIFF/WAVE byte slice as [`crate::decode`] does; the clip records this request.
    pub fn decode(&self, bytes: &[u8]) -> Result<AudioClip, WavError> {
        self.check()?;
        decode(bytes).map(|clip| clip.imported(self.clone()))
    }

    /// Answers this request's guards ([`WavImport::check`]) before the file is opened, then
    /// reads a RIFF/WAVE file as [`crate::read_wav`] does; the clip records this request. An
    /// external recording without provenance is [`WavError::ProvenanceMissing`] even when the
    /// file is missing, unreadable, truncated or corrupt; a request both guards pass gets
    /// [`ReadError::Io`] for a file that cannot be opened or read.
    pub fn read(&self, path: &Path) -> Result<AudioClip, ReadError> {
        self.check()?;
        read_wav(path).map(|clip| clip.imported(self.clone()))
    }
}
