//! Oscillators, envelopes, filters and effects: the native DSP of the renderer (DESIGN.md § 4.1,
//! § 5.3), and the offline renderer of the fixed `subtractive-mono/1` voice (§ 5.2).
//!
//! # The units
//!
//! | Unit | DESIGN.md § 5.3 rule |
//! |---|---|
//! | [`PolyBlepSaw`] | phase accumulator, anti-aliased by polyBLEP; frequency clamped to `[0, MAX_OSCILLATOR_HZ]` |
//! | [`Drive`] | `tanh(g x) / tanh(g)`, `g = 10^(dB / 20)`: deterministic, bounded, odd, finite for any input |
//! | [`SvfLowpass`] | topology-preserving transform state-variable lowpass; cutoff clamped below Nyquist, resonance to the contract range |
//! | [`Adsr`] | linear segments with explicit note-on, retrigger and note-off; zero and near-zero times take one sample |
//! | [`Smoother`] | one-pole smoothing of the velocity and the output gain |
//! | [`Limiter`] | the protective output limiter: a clip at [`LIMITER_THRESHOLD`] that counts every sample it changes |
//! | [`OutputStage`] | output gain, the limiter, and the refusal of a non-finite sample |
//!
//! Every unit computes in `f64` and keeps its own state; nothing is shared and nothing is random,
//! so the same input gives the same output bit for bit (§ 2 invariant 5). Non-finite controls are
//! replaced by a bound (each unit's documentation says which) and a non-finite signal input by
//! silence, so bounded controls give finite samples (§ 2 invariant 4); the renderer still checks
//! every output sample and refuses a render that holds a non-finite one
//! ([`RenderError::NonFiniteRender`]).
//!
//! # The renderer
//!
//! [`render`] walks a compiled [`ExecutionPlan`](patchback_core::patch::ExecutionPlan) with a
//! [`NoteSequence`](patchback_core::score::NoteSequence), block by block; the [`render`]
//! documentation states the semantics, the length and the refusals.
//!
//! # Provisional constants
//!
//! DESIGN.md § 17 leaves these open; each is a named constant documented as provisional:
//! [`RENDER_TAIL_FRAMES`], [`LIMITER_THRESHOLD`], [`SMOOTHING_SECONDS`].

mod drive;
mod envelope;
mod filter;
mod gain;
mod oscillator;
mod render;

pub use drive::{Drive, MAX_DRIVE_DB, MIN_DRIVE_DB};
pub use envelope::{Adsr, AdsrStage};
pub use filter::{MAX_CUTOFF_HZ, MAX_RESONANCE_Q, MIN_CUTOFF_HZ, MIN_RESONANCE_Q, SvfLowpass};
pub use gain::{
    LIMITER_THRESHOLD, Limiter, OutputStage, SMOOTHING_SECONDS, Smoother, decibels_to_gain,
};
pub use oscillator::{MAX_OSCILLATOR_HZ, PolyBlepSaw, midi_to_hz};
pub use render::{
    DEFAULT_BLOCK_FRAMES, RENDER_TAIL_FRAMES, RenderConfig, RenderError, RenderReport, Rendered,
    render, render_frame_count,
};

/// The one sample rate of the renderer (§ 5.3: 48 kHz, nothing converted).
pub const SAMPLE_RATE_HZ: u32 = patchback_audio::SAMPLE_RATE_HZ;

/// [`SAMPLE_RATE_HZ`] as the units compute with it.
pub(crate) const FS: f64 = SAMPLE_RATE_HZ as f64;
