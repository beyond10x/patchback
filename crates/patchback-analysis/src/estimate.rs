//! The monophonic note estimator (story:note-estimation; DESIGN.md § 6.1, § 6.2, § 10 V1):
//! from one clip, the notes of a monophonic phrase, each with its onset, gate end, tail end,
//! pitch and velocity and one confidence in `[0, 1]` per estimated quantity
//! (approval-record:wave-c1 Dc1-1).
//!
//! # Method
//!
//! [`estimate_notes`] applies the input guards of `AnalyzeAudio` itself (no samples, a rate
//! other than 48 kHz, more than [`crate::MAX_DURATION_FRAMES`] samples, a non-finite sample, a
//! clip below [`crate::SILENCE_THRESHOLD_DBFS`]), in that order, and then:
//!
//! 1. **Sounding regions.** A run of at least [`QUIET_RUN_FRAMES`] samples no louder than
//!    [`QUIET_AMPLITUDE`] is silence; each stretch between silences is a sounding region, and its
//!    first sample is an onset *from silence*.
//! 2. **Onset candidates.** Within a region, the normalized spectral flux
//!    `sum max(0, |X_k| - |X_{k-1}|) / sum |X_k|` of Hann-windowed frames of
//!    [`NOVELTY_FFT_LENGTH`] samples centred every [`NOVELTY_HOP_FRAMES`] samples, over the bins
//!    below [`NOVELTY_MAX_HZ`], is the novelty. A frame whose novelty is at least
//!    [`NOVELTY_THRESHOLD`] and the largest within [`NOVELTY_PEAK_HALF_WIDTH`] frames either side
//!    is a candidate, unless it lies within [`REGION_ONSET_GUARD_FRAMES`] of the region's start.
//! 3. **Candidates kept.** The stretch between two neighbouring onsets, [`COARSE_MARGIN_FRAMES`]
//!    in from each candidate, has its pitch estimated as a note's is (step 5). A candidate is an onset
//!    *by pitch* when both sides have a pitch and they differ by [`PITCH_CHANGE_CENTS`] or more,
//!    or when one side's pitch is ambiguous (not too short to measure) and the other's period
//!    predicts it no better than
//!    [`PERIOD_MISMATCH`] (the power of `x[n] - x[n - P]` over that of `x[n]`),
//!    and an onset *by level* when the level, the RMS over whole periods, rises by
//!    [`LEVEL_RISE_DB`] or more across it; otherwise it is dropped and the two stretches are one.
//!    This repeats until no candidate is dropped.
//! 4. **Onsets refined.** An onset from silence is the region's first sample. An onset by pitch
//!    is the change point `t` that minimizes the periodic prediction error
//!    `C(t) = (sum_{t-W <= n < t} (x[n] - x[n - P1])^2 + sum_{t <= n < t+W} (x[n] - x[n + P2])^2)
//!    / sum_{t-W <= n < t+W} x[n]^2`, `P1` and `P2` the periods before and after,
//!    `W` [`CHANGE_POINT_WINDOW_FRAMES`], fractional delays read by linear interpolation, searched
//!    [`ONSET_SEARCH_FRAMES`] either side of the candidate. An onset by level is the vertex of the
//!    dip in the level (the RMS over whole periods, at least [`LEVEL_WINDOW_MIN_FRAMES`] long):
//!    the minimum, moved towards the steeper side by half the window times
//!    `(b - a) / (a + b)`, `a` and `b` the level's slopes before and after it.
//! 5. **Notes.** Each onset starts a note, which ends at the next onset or at the region's end.
//!    Its pitch is [`estimate_f0`] of the note's samples. Where that refuses it as ambiguous
//!    (an octave the subharmonic test leaves undecided, as for a driven saw), the spans of
//!    [`PITCH_SPAN_FRAMES`] samples every half span are asked, and the median of the pitches they
//!    give is the note's, with the least of their confidences, when they all lie within
//!    [`PITCH_CHANGE_CENTS`] of it. `midi_note` is `f0_hz` rounded to the nearest MIDI number.
//!    A note too short for [`estimate_f0`], or whose pitch it refuses, has pitch confidence 0 (or
//!    the ambiguous estimate's confidence), `f0_hz` 0 and `midi_note` 0, and the estimation is
//!    refused as [`EstimationError::LowConfidenceNotes`] (Dc1-10).
//! 6. **Gate end.** The level from the note's peak, sampled every [`ENVELOPE_STEP_FRAMES`]
//!    samples up to the next onset (whose sound the level must not read) or, for a region's last
//!    note, through its release into the quiet after the region (at most [`TAIL_WINDOW_FRAMES`]
//!    past its end, never into the next region), is fitted by one to three straight lines (least
//!    squares, each extra line costing [`SEGMENT_PENALTY`] times the squared peak level whatever
//!    the note's length, a line at least [`MIN_SEGMENT_POINTS`] points long). A last line that
//!    falls by at least [`DESCENT_MIN_FRACTION`] of the peak level is the release, and the gate
//!    ends where it meets the line before it. A last line of a note cut by the next onset that
//!    rises by [`DESCENT_MIN_FRACTION`] of the peak level after a falling one is that onset's
//!    attack read early and is dropped. A note cut by the next onset whose last line does not
//!    fall held its gate to that onset. A note whose only falling line starts at its peak is
//!    ambiguous: cut by the next onset, a decay cut by a legato note or a release from the peak,
//!    and its gate is taken to end at that onset; ending its region, a release from the peak or a
//!    decay running into a release of the same slope, and its gate is taken to end at the peak.
//! 7. **Tail end.** The first sample from the gate end at which the RMS of the
//!    [`TAIL_WINDOW_FRAMES`] samples centred on it falls below the silence threshold, or the next
//!    note's onset when that comes first, or the clip's end.
//! 8. **Velocity** (Dc1-2). The note's peak level over its gate relative to the loudest note's,
//!    linear, the loudest note 1: the inverse of the renderer's linear velocity scaling.
//!
//! # Confidence (Dc1-1, DESIGN.md § 10)
//!
//! - **Onset.** From silence: the rise of the RMS from the [`ONSET_CONTRAST_FRAMES`] samples
//!   before the onset to as many after it, in dB, over [`ONSET_CONTRAST_FULL_DB`], clamped to
//!   `[0, 1]`; 0 for a region that starts at the clip's first sample (the note may have begun
//!   before the clip). By pitch: the prominence of the change point,
//!   `1 - C(t) / min(C(t - 256), C(t + 256))`, clamped: 1 when moving the onset by the tolerance
//!   of 256 samples clearly worsens the prediction, 0 when it does not. By level: the level's rise
//!   in dB over [`LEVEL_RISE_FULL_DB`], clamped.
//! - **Pitch.** `1 - d'` of [`estimate_f0`] (its confidence), 0 when it refuses for length or
//!   range.
//! - **Gate.** The visibility of the knee where the release begins: the change of slope between
//!   the release line and the line before it, `|s2 - s1| / max(|s1|, |s2|)`, clamped. For a gate
//!   held to the next onset, how steady the level stays to the end: 1 minus the larger of the
//!   last line's fall over its span and the mean drop of its last [`MIN_SEGMENT_POINTS`] points
//!   below it, over [`HOLD_DROP_FULL_FRACTION`] of the peak level, clamped (a slow release, or one
//!   begun just before the onset, bends the level down), and
//!   never more than that onset's confidence (a level held to a spurious or misplaced onset is
//!   not a gate end). [`AMBIGUOUS_GATE_CONFIDENCE`] for the ambiguous cases of step 6 (for one
//!   cut by the next onset, at most that onset's confidence); 0 for a note too short to fit or
//!   whose level never falls.
//! - **Tail.** At a crossing of the silence threshold: how far the level stands above the
//!   threshold [`TAIL_CONTEXT_FRAMES`] samples before it, in dB, over [`TAIL_FULL_DB`], clamped
//!   (a release that falls steeply through the threshold is certain, one that hovers about it is
//!   not). A tail cut by the next onset inherits that onset's confidence; a tail that reaches the
//!   clip's end without falling below the threshold has confidence 0.
//!
//! The estimation is refused as [`EstimationError::LowConfidenceNotes`] when any note has any
//! confidence below its quantity's threshold ([`ONSET_CONFIDENCE_THRESHOLD`],
//! [`PITCH_CONFIDENCE_THRESHOLD`], [`GATE_CONFIDENCE_THRESHOLD`],
//! [`TAIL_CONFIDENCE_THRESHOLD`]; Dc1-4), with every note.
//!
//! # Polyphony (Dc1-3)
//!
//! Every note with at least [`POLYPHONY_MIN_FFT_LENGTH`] samples of gate is checked on one
//! Blackman-windowed spectrum of the largest power of two up to [`POLYPHONY_MAX_FFT_LENGTH`]
//! samples that fits its gate after its attack. Its peaks below [`POLYPHONY_MAX_HZ`] within
//! [`POLYPHONY_PEAK_FLOOR_DB`] of the largest are found. The input is polyphonic when the lowest
//! peak is not the note's fundamental (more than [`POLYPHONY_HARMONIC_TOLERANCE`] of a harmonic
//! away from `f0_hz`), as for two tones whose common period lies below both, or when the peaks
//! that are not near a whole multiple of the fundamental hold more than
//! [`POLYPHONY_INHARMONIC_SHARE`] of the peaks' power, as for two tones a minor second, a major
//! third, a tritone or a fifth apart. A note whose pitch is unknown takes its lowest peak as its
//! fundamental for the second test. One polyphonic note refuses the clip as
//! [`EstimationError::PolyphonicInput`], before low confidence is considered.
//!
//! Two tones an octave apart are not detected: every partial of the upper tone is a harmonic of
//! the lower, so the sum is one periodic tone with a brighter spectrum, which a single sawtooth
//! through a resonant filter can also produce. Telling the two apart needs a timbre model, which
//! the MVP does not have (Dc1-3).

use patchback_core::digest::{Sha256, hex, sha256};
use patchback_core::score::{NoteEvent, NoteSequence};

use crate::{
    AnalysisError, Complex, EstimationError, F0_CONFIDENCE_THRESHOLD, F0_MAX_HZ, F0_MIN_HZ,
    F0_WINDOW, MAX_DURATION_FRAMES, PitchEstimate, SAMPLE_RATE_HZ, SILENCE_THRESHOLD_DBFS,
    check_finite, estimate_f0, fft, hann,
};

/// A sample no louder than this is quiet. Provisional (Dc1-4): -80 dBFS.
pub const QUIET_AMPLITUDE: f64 = 1e-4;

/// A run of this many quiet samples is silence between sounding regions. Provisional.
pub const QUIET_RUN_FRAMES: usize = 64;

/// The FFT length of the novelty frames. Provisional.
pub const NOVELTY_FFT_LENGTH: usize = 2_048;

/// The hop between novelty frames, in samples. Provisional.
pub const NOVELTY_HOP_FRAMES: usize = 256;

/// The novelty reads the bins below this frequency, in Hz. Provisional.
pub const NOVELTY_MAX_HZ: f64 = 8_000.0;

/// A novelty peak at least this high is an onset candidate. Provisional, set on the reference
/// set (Dc1-9).
pub const NOVELTY_THRESHOLD: f64 = 0.04;

/// A novelty peak is the largest within this many frames either side. Provisional.
pub const NOVELTY_PEAK_HALF_WIDTH: usize = 4;

/// No candidate within this many samples after a region's start, whose onset from silence is
/// already known. Provisional.
pub const REGION_ONSET_GUARD_FRAMES: usize = 1_536;

/// The pitch of the stretch between candidates is estimated this many samples in from each
/// candidate. Provisional.
pub const COARSE_MARGIN_FRAMES: usize = 768;

/// Two stretches whose pitches differ by at least this many cents are two notes. Provisional.
pub const PITCH_CHANGE_CENTS: f64 = 50.0;

/// A candidate one of whose sides has no pitch is an onset by pitch when the other side's period
/// predicts it this badly ([`period_mismatch`]'s measure). Provisional.
pub const PERIOD_MISMATCH: f64 = 0.5;

/// When [`estimate_f0`] leaves a note's octave undecided, it is asked again on spans of this many
/// samples, every half span. Provisional.
pub const PITCH_SPAN_FRAMES: usize = 4_096;

/// A rise of the level by at least this many dB across a candidate starts a note. Provisional.
pub const LEVEL_RISE_DB: f64 = 1.0;

/// An onset by level whose level rises by this many dB has onset confidence 1. Provisional.
pub const LEVEL_RISE_FULL_DB: f64 = 3.0;

/// The level window is a whole number of periods at least this long. Provisional.
pub const LEVEL_WINDOW_MIN_FRAMES: usize = 256;

/// The window either side of a change point, in samples. Provisional.
pub const CHANGE_POINT_WINDOW_FRAMES: usize = 1_024;

/// An onset is searched this many samples either side of its candidate. Provisional.
pub const ONSET_SEARCH_FRAMES: usize = 1_536;

/// The window before and after an onset from silence whose RMS levels are compared.
/// Provisional.
pub const ONSET_CONTRAST_FRAMES: usize = 1_024;

/// An onset from silence whose level rises by this many dB has onset confidence 1. Provisional.
pub const ONSET_CONTRAST_FULL_DB: f64 = 20.0;

/// The spacing of the level's points for the gate fit, in samples. Provisional.
pub const ENVELOPE_STEP_FRAMES: usize = 32;

/// The level window of the gate fit is a whole number of periods at least this long.
/// Provisional.
pub const ENVELOPE_WINDOW_MIN_FRAMES: usize = 512;

/// The cost of each line beyond the first in the gate fit, times the squared peak level.
/// Provisional, set on the reference set (Dc1-9).
pub const SEGMENT_PENALTY: f64 = 0.01;

/// The fewest points of a line in the gate fit. Provisional.
pub const MIN_SEGMENT_POINTS: usize = 3;

/// A line falling by at least this share of the peak level is falling. Provisional.
pub const DESCENT_MIN_FRACTION: f64 = 0.05;

/// The gate confidence of a note whose only falling line starts at its peak (step 6 of the
/// method). Provisional; below [`GATE_CONFIDENCE_THRESHOLD`].
pub const AMBIGUOUS_GATE_CONFIDENCE: f64 = 0.3;

/// A gate held to the next onset whose last points have fallen this share of the peak level
/// below its last line has confidence 0 (a release begun just before the onset). Provisional.
pub const HOLD_DROP_FULL_FRACTION: f64 = 0.02;

/// The tail end is read from the RMS of this many samples centred on each sample. Provisional.
pub const TAIL_WINDOW_FRAMES: usize = 1_024;

/// The tail confidence reads the level this many samples before the crossing. Provisional.
pub const TAIL_CONTEXT_FRAMES: usize = 1_200;

/// A level this many dB above the threshold before the crossing gives tail confidence 1.
/// Provisional.
pub const TAIL_FULL_DB: f64 = 12.0;

/// The smallest polyphony spectrum; a note with a shorter gate is not checked. Provisional.
pub const POLYPHONY_MIN_FFT_LENGTH: usize = 2_048;

/// The largest polyphony spectrum. Provisional.
pub const POLYPHONY_MAX_FFT_LENGTH: usize = 8_192;

/// The polyphony check reads the peaks below this frequency, in Hz. Provisional.
pub const POLYPHONY_MAX_HZ: f64 = 5_000.0;

/// The polyphony check reads the peaks within this many dB of the largest: above the
/// aliasing and distortion products of a driven saw at high notes (about -40 dB), below the
/// second tone of an interval. Provisional.
pub const POLYPHONY_PEAK_FLOOR_DB: f64 = -30.0;

/// A peak within this share of a harmonic of the fundamental is that harmonic. Provisional.
pub const POLYPHONY_HARMONIC_TOLERANCE: f64 = 0.1;

/// A note whose inharmonic peaks hold more than this share of the peaks' power is polyphonic.
/// Provisional, set on the Dc1-3 cases.
pub const POLYPHONY_INHARMONIC_SHARE: f64 = 0.02;

/// The onset confidence below which an estimation is refused (Dc1-4). Provisional, set by the
/// calibration on the reference set (Dc1-9).
pub const ONSET_CONFIDENCE_THRESHOLD: f64 = 0.5;

/// The pitch confidence below which an estimation is refused (Dc1-4): the threshold
/// [`estimate_f0`] refuses below, so every pitch it accepts is confident. Provisional.
pub const PITCH_CONFIDENCE_THRESHOLD: f64 = F0_CONFIDENCE_THRESHOLD;

/// The gate confidence below which an estimation is refused (Dc1-4). Provisional, set by the
/// calibration on the reference set (Dc1-9).
pub const GATE_CONFIDENCE_THRESHOLD: f64 = 0.5;

/// The tail confidence below which an estimation is refused (Dc1-4). Provisional, set by the
/// calibration on the reference set (Dc1-9).
pub const TAIL_CONFIDENCE_THRESHOLD: f64 = 0.5;

/// The patch id of a note sequence written from an estimation: the sequence is written from
/// audio, for no patch, and any patch at its rate may render it (`RenderPatch`).
pub const ESTIMATED_SEQUENCE_PATCH_ID: &str = "00000000-0000-4000-8000-000000000000";

/// The name and version of this estimator, the first entry of its configuration.
pub const ESTIMATOR: &str = "monophonic/1";

/// One estimated note (`patchback.analysis.EstimatedNote`), timed in samples of its clip.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EstimatedNote {
    /// The onset.
    pub start_sample: u64,
    /// The gate end; the gate length is `end_sample - start_sample`.
    pub end_sample: u64,
    /// Where the release falls below the silence threshold, or the next note's onset when that
    /// comes first.
    pub tail_end_sample: u64,
    /// `f0_hz` rounded to the nearest MIDI number; 0 when the pitch is unknown.
    pub midi_note: u8,
    /// The fundamental in Hz; 0 when the pitch is unknown.
    pub f0_hz: f64,
    /// The note's peak level over its gate relative to the loudest note's, in `[0, 1]`.
    pub velocity: f32,
    pub onset_confidence: f64,
    pub pitch_confidence: f64,
    pub gate_confidence: f64,
    pub tail_confidence: f64,
}

/// An estimated quantity of a note, each with its own confidence and threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantity {
    Onset,
    Pitch,
    Gate,
    Tail,
}

impl Quantity {
    /// The four quantities in the order a note lists their confidences.
    pub const ALL: [Quantity; 4] = [Self::Onset, Self::Pitch, Self::Gate, Self::Tail];

    /// The quantity's name: `onset`, `pitch`, `gate` or `tail`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Onset => "onset",
            Self::Pitch => "pitch",
            Self::Gate => "gate",
            Self::Tail => "tail",
        }
    }

    /// The confidence below which the quantity is low (Dc1-4).
    pub fn threshold(self) -> f64 {
        match self {
            Self::Onset => ONSET_CONFIDENCE_THRESHOLD,
            Self::Pitch => PITCH_CONFIDENCE_THRESHOLD,
            Self::Gate => GATE_CONFIDENCE_THRESHOLD,
            Self::Tail => TAIL_CONFIDENCE_THRESHOLD,
        }
    }
}

impl EstimatedNote {
    /// The confidence of `quantity`.
    pub fn confidence(&self, quantity: Quantity) -> f64 {
        match quantity {
            Quantity::Onset => self.onset_confidence,
            Quantity::Pitch => self.pitch_confidence,
            Quantity::Gate => self.gate_confidence,
            Quantity::Tail => self.tail_confidence,
        }
    }

    /// The quantities whose confidence is below their threshold, in [`Quantity::ALL`] order.
    pub fn low_quantities(&self) -> Vec<Quantity> {
        Quantity::ALL
            .into_iter()
            .filter(|q| self.confidence(*q) < q.threshold())
            .collect()
    }
}

/// The notes of one clip (`patchback.analysis.NoteEstimation`) and what identifies them.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteEstimation {
    /// The deterministic id (Dc1-7, [`note_estimation_id`]).
    pub note_estimation_id: String,
    pub sample_rate_hz: u32,
    pub frame_count: u64,
    /// The SHA-256 of the clip's samples ([`clip_sha256`]).
    pub clip_sha256: [u8; 32],
    /// The estimator and its constants as `(name, value)`, in order ([`configuration`]).
    pub configuration: Vec<(String, String)>,
    /// The notes in onset order.
    pub notes: Vec<EstimatedNote>,
}

impl NoteEstimation {
    /// The note sequence written from the estimation (Dc1-5): one [`NoteEvent`] per note, in the
    /// same order, with its onset, gate end, MIDI number and velocity, at the clip's rate, for
    /// [`ESTIMATED_SEQUENCE_PATCH_ID`], recording the estimation's id. Tail ends and
    /// confidences stay in the estimation.
    pub fn note_sequence(&self) -> NoteSequence {
        NoteSequence {
            format_version: patchback_core::score::FORMAT_VERSION,
            sample_rate_hz: self.sample_rate_hz,
            patch_id: ESTIMATED_SEQUENCE_PATCH_ID.to_owned(),
            note_estimation_id: Some(self.note_estimation_id.clone()),
            notes: self
                .notes
                .iter()
                .map(|note| NoteEvent {
                    start_sample: note.start_sample,
                    end_sample: note.end_sample,
                    midi_note: note.midi_note,
                    velocity: note.velocity,
                })
                .collect(),
        }
    }
}

/// The SHA-256 of `samples` as little-endian IEEE 754 single-precision bytes, in order.
pub fn clip_sha256(samples: &[f32]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for chunk in samples.chunks(4_096) {
        let bytes: Vec<u8> = chunk.iter().flat_map(|s| s.to_le_bytes()).collect();
        digest.update(&bytes);
    }
    digest.finalize()
}

/// The estimator's configuration as `(name, value)`: [`ESTIMATOR`], then every constant that
/// decides its result, in a fixed order, each value as Rust prints it.
pub fn configuration() -> Vec<(String, String)> {
    let entries: [(&str, String); 40] = [
        ("estimator", ESTIMATOR.to_owned()),
        ("sample_rate_hz", SAMPLE_RATE_HZ.to_string()),
        ("silence_threshold_dbfs", SILENCE_THRESHOLD_DBFS.to_string()),
        (
            "f0_confidence_threshold",
            F0_CONFIDENCE_THRESHOLD.to_string(),
        ),
        ("f0_min_hz", F0_MIN_HZ.to_string()),
        ("f0_max_hz", F0_MAX_HZ.to_string()),
        ("f0_window", F0_WINDOW.to_string()),
        ("quiet_amplitude", QUIET_AMPLITUDE.to_string()),
        ("quiet_run_frames", QUIET_RUN_FRAMES.to_string()),
        ("novelty_fft_length", NOVELTY_FFT_LENGTH.to_string()),
        ("novelty_hop_frames", NOVELTY_HOP_FRAMES.to_string()),
        ("novelty_max_hz", NOVELTY_MAX_HZ.to_string()),
        ("novelty_threshold", NOVELTY_THRESHOLD.to_string()),
        (
            "novelty_peak_half_width",
            NOVELTY_PEAK_HALF_WIDTH.to_string(),
        ),
        (
            "region_onset_guard_frames",
            REGION_ONSET_GUARD_FRAMES.to_string(),
        ),
        ("coarse_margin_frames", COARSE_MARGIN_FRAMES.to_string()),
        ("pitch_change_cents", PITCH_CHANGE_CENTS.to_string()),
        ("pitch_span_frames", PITCH_SPAN_FRAMES.to_string()),
        ("period_mismatch", PERIOD_MISMATCH.to_string()),
        ("level_rise_db", LEVEL_RISE_DB.to_string()),
        ("level_rise_full_db", LEVEL_RISE_FULL_DB.to_string()),
        (
            "level_window_min_frames",
            LEVEL_WINDOW_MIN_FRAMES.to_string(),
        ),
        (
            "change_point_window_frames",
            CHANGE_POINT_WINDOW_FRAMES.to_string(),
        ),
        ("onset_search_frames", ONSET_SEARCH_FRAMES.to_string()),
        ("onset_contrast_frames", ONSET_CONTRAST_FRAMES.to_string()),
        ("onset_contrast_full_db", ONSET_CONTRAST_FULL_DB.to_string()),
        ("envelope_step_frames", ENVELOPE_STEP_FRAMES.to_string()),
        (
            "envelope_window_min_frames",
            ENVELOPE_WINDOW_MIN_FRAMES.to_string(),
        ),
        ("segment_penalty", SEGMENT_PENALTY.to_string()),
        ("min_segment_points", MIN_SEGMENT_POINTS.to_string()),
        ("descent_min_fraction", DESCENT_MIN_FRACTION.to_string()),
        (
            "ambiguous_gate_confidence",
            AMBIGUOUS_GATE_CONFIDENCE.to_string(),
        ),
        (
            "hold_drop_full_fraction",
            HOLD_DROP_FULL_FRACTION.to_string(),
        ),
        ("tail_window_frames", TAIL_WINDOW_FRAMES.to_string()),
        ("tail_context_frames", TAIL_CONTEXT_FRAMES.to_string()),
        ("tail_full_db", TAIL_FULL_DB.to_string()),
        ("polyphony", polyphony_configuration()),
        (
            "onset_confidence_threshold",
            ONSET_CONFIDENCE_THRESHOLD.to_string(),
        ),
        (
            "gate_confidence_threshold",
            GATE_CONFIDENCE_THRESHOLD.to_string(),
        ),
        (
            "tail_confidence_threshold",
            TAIL_CONFIDENCE_THRESHOLD.to_string(),
        ),
    ];
    entries
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

/// The polyphony constants as one value: `min_fft/max_fft/max_hz/floor_db/tolerance/share`.
fn polyphony_configuration() -> String {
    format!(
        "{}/{}/{}/{}/{}/{}",
        POLYPHONY_MIN_FFT_LENGTH,
        POLYPHONY_MAX_FFT_LENGTH,
        POLYPHONY_MAX_HZ,
        POLYPHONY_PEAK_FLOOR_DB,
        POLYPHONY_HARMONIC_TOLERANCE,
        POLYPHONY_INHARMONIC_SHARE
    )
}

/// The estimation id (Dc1-7): a version-8 UUID (RFC 9562) whose 128 bits are the first 16 bytes
/// of the SHA-256 of the identity text, with the version and variant bits set. The identity text
/// is `patchback-note-estimation <version>\n`, `clip_sha256 <hex>\n` and one
/// `config <name> <value>\n` per configuration entry, in order.
pub fn note_estimation_id(clip_sha256: &[u8; 32], configuration: &[(String, String)]) -> String {
    let mut identity = format!(
        "{} {}\nclip_sha256 {}\n",
        crate::ESTIMATION_HEADER,
        crate::ESTIMATION_FORMAT_VERSION,
        hex(clip_sha256)
    );
    for (name, value) in configuration {
        identity.push_str("config ");
        identity.push_str(name);
        identity.push(' ');
        identity.push_str(value);
        identity.push('\n');
    }
    let mut digest = sha256(identity.as_bytes());
    digest[6] = (digest[6] & 0x0f) | 0x80;
    digest[8] = (digest[8] & 0x3f) | 0x80;
    let h = hex(&digest);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// Estimates the notes of the monophonic `samples` at `sample_rate_hz` (module documentation).
pub fn estimate_notes(
    samples: &[f32],
    sample_rate_hz: u32,
) -> Result<NoteEstimation, EstimationError> {
    if samples.is_empty() {
        return Err(AnalysisError::EmptyAudio.into());
    }
    if sample_rate_hz != SAMPLE_RATE_HZ {
        return Err(AnalysisError::UnsupportedSampleRate { sample_rate_hz }.into());
    }
    if samples.len() > MAX_DURATION_FRAMES {
        return Err(AnalysisError::DurationOverBound {
            frame_count: samples.len(),
        }
        .into());
    }
    check_finite(samples)?;
    let signal = Signal::new(samples);
    let rms_dbfs = db(signal.rms(0, samples.len() as isize));
    if rms_dbfs < SILENCE_THRESHOLD_DBFS {
        return Err(AnalysisError::SilentInput { rms_dbfs }.into());
    }

    let novelty = novelty(&signal.x)?;
    let mut notes = Vec::new();
    let mut levels = Vec::new();
    let mut polyphonic = false;
    let regions = sounding_regions(&signal.x);
    for (index, region) in regions.iter().enumerate() {
        let onsets = region_onsets(&signal, samples, &novelty, *region);
        for (i, onset) in onsets.iter().enumerate() {
            let next = onsets.get(i + 1);
            let end = next.map_or(region.1, |n| n.sample);
            let after_region = regions.get(index + 1).map(|r| r.0);
            let built = build_note(&signal, samples, onset, end, next, after_region);
            polyphonic |= built.polyphonic;
            levels.push(built.level);
            notes.push(built.note);
        }
    }
    if polyphonic {
        return Err(EstimationError::PolyphonicInput);
    }
    let loudest = levels.iter().copied().fold(0.0f64, f64::max);
    for (note, level) in notes.iter_mut().zip(&levels) {
        note.velocity = if loudest > 0.0 {
            (level / loudest).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
    }
    if notes.iter().any(|note| !note.low_quantities().is_empty()) {
        return Err(EstimationError::LowConfidenceNotes { notes });
    }
    let clip_sha256 = clip_sha256(samples);
    let configuration = configuration();
    Ok(NoteEstimation {
        note_estimation_id: note_estimation_id(&clip_sha256, &configuration),
        sample_rate_hz,
        frame_count: samples.len() as u64,
        clip_sha256,
        configuration,
        notes,
    })
}

/// The level in dBFS of an RMS value, floored at [`crate::LOG_FLOOR_DB`].
fn db(value: f64) -> f64 {
    if value > 0.0 {
        (20.0 * value.log10()).max(crate::LOG_FLOOR_DB)
    } else {
        crate::LOG_FLOOR_DB
    }
}

/// The samples as `f64` and the running sum of their squares.
struct Signal {
    x: Vec<f64>,
    /// `energy[i]` is the sum of `x[j]^2` for `j < i`.
    energy: Vec<f64>,
}

impl Signal {
    fn new(samples: &[f32]) -> Self {
        let x: Vec<f64> = samples.iter().map(|s| f64::from(*s)).collect();
        let mut energy = Vec::with_capacity(x.len() + 1);
        let mut sum = 0.0;
        energy.push(0.0);
        for v in &x {
            sum += v * v;
            energy.push(sum);
        }
        Self { x, energy }
    }

    fn len(&self) -> usize {
        self.x.len()
    }

    /// The sum of squares over `[start, end)`, samples outside the clip zero.
    fn sum_squares(&self, start: isize, end: isize) -> f64 {
        let n = self.len() as isize;
        let a = start.clamp(0, n) as usize;
        let b = end.clamp(0, n) as usize;
        if b <= a {
            0.0
        } else {
            (self.energy[b] - self.energy[a]).max(0.0)
        }
    }

    /// The RMS over `[start, end)` counting samples outside the clip as zero.
    fn rms(&self, start: isize, end: isize) -> f64 {
        if end <= start {
            return 0.0;
        }
        (self.sum_squares(start, end) / (end - start) as f64).sqrt()
    }

    /// The RMS of the `window` samples centred on `t`.
    fn centred(&self, t: isize, window: usize) -> f64 {
        let half = (window / 2) as isize;
        self.rms(t - half, t - half + window as isize)
    }

    /// `x` at a fractional position, by linear interpolation; zero outside the clip.
    fn at(&self, position: f64) -> f64 {
        if position < 0.0 {
            return 0.0;
        }
        let i = position.floor() as usize;
        let frac = position - i as f64;
        let a = self.x.get(i).copied().unwrap_or(0.0);
        let b = self.x.get(i + 1).copied().unwrap_or(0.0);
        a + (b - a) * frac
    }
}

/// The sounding regions `[start, end)` of `x` (step 1 of the method).
fn sounding_regions(x: &[f64]) -> Vec<(usize, usize)> {
    let quiet = |v: f64| v.abs() <= QUIET_AMPLITUDE;
    let mut regions = Vec::new();
    let mut i = 0;
    while i < x.len() {
        while i < x.len() && quiet(x[i]) {
            i += 1;
        }
        if i >= x.len() {
            break;
        }
        let start = i;
        let mut run_start = None;
        let mut j = i;
        while j < x.len() {
            if quiet(x[j]) {
                let begun = *run_start.get_or_insert(j);
                if j + 1 - begun >= QUIET_RUN_FRAMES {
                    break;
                }
            } else {
                run_start = None;
            }
            j += 1;
        }
        let end = run_start.unwrap_or(x.len());
        regions.push((start, end));
        i = j;
    }
    regions
}

/// The normalized spectral flux of frames centred every [`NOVELTY_HOP_FRAMES`] samples (step 2).
fn novelty(x: &[f64]) -> Result<Vec<f64>, AnalysisError> {
    let n = NOVELTY_FFT_LENGTH;
    let window = hann(n)?;
    let bins = ((NOVELTY_MAX_HZ * n as f64 / f64::from(SAMPLE_RATE_HZ)) as usize).min(n / 2);
    let frames = x.len() / NOVELTY_HOP_FRAMES + 1;
    let mut previous = vec![0.0; bins];
    let mut current = vec![0.0; bins];
    let mut buffer = vec![Complex::ZERO; n];
    let mut out = Vec::with_capacity(frames);
    for k in 0..frames {
        let centre = (k * NOVELTY_HOP_FRAMES) as isize;
        let first = centre - (n / 2) as isize;
        let mut any = false;
        for (j, slot) in buffer.iter_mut().enumerate() {
            let index = first + j as isize;
            let value = if index >= 0 {
                x.get(index as usize).copied().unwrap_or(0.0)
            } else {
                0.0
            };
            any |= value != 0.0;
            *slot = Complex::new(value * window[j], 0.0);
        }
        if any {
            fft(&mut buffer)?;
            for (bin, slot) in current.iter_mut().enumerate() {
                *slot = buffer[bin].norm();
            }
        } else {
            current.iter_mut().for_each(|v| *v = 0.0);
        }
        let total: f64 = current.iter().sum();
        let rise: f64 = current
            .iter()
            .zip(&previous)
            .map(|(c, p)| (c - p).max(0.0))
            .sum();
        out.push(if total > 1e-12 { rise / total } else { 0.0 });
        std::mem::swap(&mut previous, &mut current);
    }
    Ok(out)
}

/// How an onset was found.
#[derive(Clone, Copy, Debug, PartialEq)]
enum OnsetKind {
    Pitch,
    Level,
}

/// A refined onset with its confidence.
#[derive(Clone, Copy, Debug)]
struct Onset {
    sample: usize,
    confidence: f64,
}

/// The pitch of `samples[start..end)` (an empty or reversed span is too short).
fn pitch_of(samples: &[f32], start: usize, end: usize) -> Result<PitchEstimate, AnalysisError> {
    let end = end.min(samples.len());
    let start = start.min(end);
    estimate_f0(&samples[start..end], SAMPLE_RATE_HZ)
}

/// The pitch of the note `samples[start..end)` (step 5 of the method): [`estimate_f0`] of the
/// whole note. When that refuses it as ambiguous, the spans of [`PITCH_SPAN_FRAMES`] samples
/// every half span are asked; if any gives a pitch and all that do lie within
/// [`PITCH_CHANGE_CENTS`] of their median, the median is the note's pitch, with the least of
/// their confidences. Otherwise the whole note's refusal stands.
fn note_pitch(samples: &[f32], start: usize, end: usize) -> Result<PitchEstimate, AnalysisError> {
    let whole = pitch_of(samples, start, end);
    let Err(AnalysisError::PitchAmbiguous { .. }) = whole else {
        return whole;
    };
    let mut found = Vec::new();
    let mut a = start;
    while a + PITCH_SPAN_FRAMES <= end {
        if let Ok(p) = pitch_of(samples, a, a + PITCH_SPAN_FRAMES) {
            found.push(p);
        }
        a += PITCH_SPAN_FRAMES / 2;
    }
    if found.is_empty() {
        return whole;
    }
    found.sort_by(|x, y| x.f0_hz.total_cmp(&y.f0_hz));
    let median = found[(found.len() - 1) / 2].f0_hz;
    if found
        .iter()
        .any(|p| cents_between(p.f0_hz, median).abs() > PITCH_CHANGE_CENTS)
    {
        return whole;
    }
    let confidence = found.iter().map(|p| p.confidence).fold(1.0, f64::min);
    Ok(PitchEstimate {
        f0_hz: median,
        confidence,
    })
}

fn cents_between(a: f64, b: f64) -> f64 {
    1_200.0 * (a / b).log2()
}

/// The level window: a whole number of periods at least `min` samples long, or `fallback`
/// samples when the period is unknown.
fn level_window(period: Option<f64>, min: usize, fallback: usize) -> usize {
    match period {
        Some(p) if p > 1.0 => {
            let count = (min as f64 / p).ceil().max(1.0);
            (count * p).round() as usize
        }
        _ => fallback,
    }
}

/// The onsets of one sounding region (steps 2 to 4 of the method).
fn region_onsets(
    signal: &Signal,
    samples: &[f32],
    novelty: &[f64],
    (start, end): (usize, usize),
) -> Vec<Onset> {
    let mut candidates: Vec<usize> = Vec::new();
    for k in 0..novelty.len() {
        let centre = k * NOVELTY_HOP_FRAMES;
        if centre < start + REGION_ONSET_GUARD_FRAMES || centre + 512 >= end {
            continue;
        }
        let value = novelty[k];
        if value < NOVELTY_THRESHOLD {
            continue;
        }
        let low = k.saturating_sub(NOVELTY_PEAK_HALF_WIDTH);
        let high = (k + NOVELTY_PEAK_HALF_WIDTH).min(novelty.len() - 1);
        let peak =
            (low..=high).all(|j| j == k || novelty[j] < value || (novelty[j] == value && j > k));
        if peak {
            candidates.push(centre);
        }
    }

    // Step 3: keep candidates whose two sides differ in pitch or which the level rises across.
    let mut kinds: Vec<OnsetKind>;
    let mut pitches: Vec<Option<PitchEstimate>>;
    loop {
        let mut bounds = Vec::with_capacity(candidates.len() + 2);
        bounds.push(start);
        bounds.extend(&candidates);
        bounds.push(end);
        let stretches: Vec<(usize, usize)> = (0..bounds.len() - 1)
            .map(|i| {
                let a = if i == 0 {
                    bounds[i]
                } else {
                    bounds[i] + COARSE_MARGIN_FRAMES
                };
                let b = if i + 2 == bounds.len() {
                    bounds[i + 1]
                } else {
                    bounds[i + 1].saturating_sub(COARSE_MARGIN_FRAMES)
                };
                (a, b)
            })
            .collect();
        let results: Vec<Result<PitchEstimate, AnalysisError>> = stretches
            .iter()
            .map(|&(a, b)| note_pitch(samples, a, b))
            .collect();
        pitches = results.iter().map(|r| r.as_ref().ok().copied()).collect();
        // A side without a pitch for which `estimate_f0` found the period ambiguous (not one too
        // short to measure).
        let ambiguous = |i: usize| matches!(results[i], Err(AnalysisError::PitchAmbiguous { .. }));
        kinds = Vec::with_capacity(candidates.len());
        let mut weakest: Option<(usize, f64)> = None;
        for (j, &c) in candidates.iter().enumerate() {
            let before = pitches[j];
            let after = pitches[j + 1];
            let kind = match (before, after) {
                (Some(b), Some(a))
                    if cents_between(a.f0_hz, b.f0_hz).abs() >= PITCH_CHANGE_CENTS =>
                {
                    Some(OnsetKind::Pitch)
                }
                (Some(known), None) | (None, Some(known))
                    if ambiguous(if before.is_none() { j } else { j + 1 })
                        && period_mismatch(
                            signal,
                            stretches[if before.is_none() { j } else { j + 1 }],
                            f64::from(SAMPLE_RATE_HZ) / known.f0_hz,
                        ) >= PERIOD_MISMATCH =>
                {
                    Some(OnsetKind::Pitch)
                }
                _ => {
                    let period = match (before, after) {
                        (Some(b), _) | (None, Some(b)) => Some(f64::from(SAMPLE_RATE_HZ) / b.f0_hz),
                        (None, None) => None,
                    };
                    let rise = level_rise(signal, c, bounds[j], bounds[j + 2], period).0;
                    if rise >= LEVEL_RISE_DB {
                        Some(OnsetKind::Level)
                    } else {
                        if weakest.is_none_or(|(_, r)| rise < r) {
                            weakest = Some((j, rise));
                        }
                        None
                    }
                }
            };
            kinds.push(kind.unwrap_or(OnsetKind::Level));
        }
        match weakest {
            Some((j, _)) => {
                candidates.remove(j);
            }
            None => break,
        }
    }

    // Step 4: refine.
    let mut onsets = vec![silence_onset(signal, start)];
    for (j, &c) in candidates.iter().enumerate() {
        let previous = onsets.last().map_or(start, |o: &Onset| o.sample);
        let next = candidates.get(j + 1).copied().unwrap_or(end);
        let lo = c
            .saturating_sub(ONSET_SEARCH_FRAMES)
            .max(previous + 512)
            .min(c);
        let hi = (c + ONSET_SEARCH_FRAMES)
            .min(next.saturating_sub(512))
            .max(c);
        let onset = match kinds[j] {
            OnsetKind::Pitch => {
                let before = pitches[j].map(|p| f64::from(SAMPLE_RATE_HZ) / p.f0_hz);
                let after = pitches[j + 1].map(|p| f64::from(SAMPLE_RATE_HZ) / p.f0_hz);
                match (before, after) {
                    (Some(p1), Some(p2)) => change_point(signal, lo, hi, p1, p2),
                    (Some(p), None) | (None, Some(p)) => level_onset(signal, lo, hi, Some(p)),
                    (None, None) => level_onset(signal, lo, hi, None),
                }
            }
            _ => {
                let period = pitches[j + 1]
                    .or(pitches[j])
                    .map(|p| f64::from(SAMPLE_RATE_HZ) / p.f0_hz);
                level_onset(signal, lo, hi, period)
            }
        };
        if onset.sample > previous + 256 && onset.sample + 256 < end {
            onsets.push(onset);
        }
    }
    onsets
}

/// The onset from silence at a region's first sample, with its contrast confidence.
fn silence_onset(signal: &Signal, start: usize) -> Onset {
    let t = start as isize;
    let w = ONSET_CONTRAST_FRAMES as isize;
    let confidence = if start == 0 {
        0.0
    } else {
        let before = db(signal.rms(t - w, t));
        let after = db(signal.rms(t, t + w));
        ((after - before) / ONSET_CONTRAST_FULL_DB).clamp(0.0, 1.0)
    };
    Onset {
        sample: start,
        confidence,
    }
}

/// How badly the period `period` predicts the stretch `[a, b)`: the power of `x[n] - x[n - period]`
/// over the power of `x[n]`, fractional delays by linear interpolation; about 0 for a tone of that
/// period (or a whole fraction of it), about 2 for an unrelated one.
fn period_mismatch(signal: &Signal, (a, b): (usize, usize), period: f64) -> f64 {
    let first = a + period.ceil() as usize;
    let (mut error, mut power) = (0.0, 0.0);
    for n in first..b.min(signal.len()) {
        let v = signal.x[n];
        let d = v - signal.at(n as f64 - period);
        error += d * d;
        power += v * v;
    }
    if power > 0.0 { error / power } else { 0.0 }
}

/// The rise of the level across candidate `c`, in dB, and the sample of the dip: the minimum of
/// the level over `[c - ONSET_SEARCH_FRAMES, c + 512]` and the maximum from it to 2,048 samples
/// after `c`, both within `(previous, next)`.
fn level_rise(
    signal: &Signal,
    c: usize,
    previous: usize,
    next: usize,
    period: Option<f64>,
) -> (f64, usize) {
    let window = level_window(period, LEVEL_WINDOW_MIN_FRAMES.max(512), 2_048);
    let half = window / 2;
    let lo = c
        .saturating_sub(ONSET_SEARCH_FRAMES)
        .max(previous + half)
        .min(c);
    let hi = (c + 512).min(next.saturating_sub(half)).max(lo);
    let mut dip = (f64::INFINITY, lo);
    let mut t = lo;
    while t <= hi {
        let level = signal.centred(t as isize, window);
        if level < dip.0 {
            dip = (level, t);
        }
        t += 16;
    }
    let mut peak = 0.0f64;
    let top = (dip.1.max(c) + 2_048).min(next.saturating_sub(half));
    let mut t = dip.1;
    while t <= top {
        peak = peak.max(signal.centred(t as isize, window));
        t += 16;
    }
    (db(peak) - db(dip.0), dip.1)
}

/// The onset by level within `[lo, hi]` (step 4 of the method).
fn level_onset(signal: &Signal, lo: usize, hi: usize, period: Option<f64>) -> Onset {
    let window = level_window(period, LEVEL_WINDOW_MIN_FRAMES, 1_024);
    let half = window / 2;
    let mut dip = (f64::INFINITY, lo);
    let mut t = lo;
    while t <= hi {
        let level = signal.centred(t as isize, window);
        if level < dip.0 {
            dip = (level, t);
        }
        t += 4;
    }
    let m = dip.1 as isize;
    let w = window as isize;
    let left = (signal.centred(m - w, window) - dip.0).max(0.0) / w as f64;
    let right = (signal.centred(m + w, window) - dip.0).max(0.0) / w as f64;
    let shift = if left + right > 0.0 {
        half as f64 * (right - left) / (left + right)
    } else {
        0.0
    };
    let sample = (m as f64 + shift).round().max(0.0) as usize;
    let mut peak = 0.0f64;
    let mut t = dip.1;
    while t <= dip.1 + 2_048 {
        peak = peak.max(signal.centred(t as isize, window));
        t += 16;
    }
    let rise = db(peak) - db(dip.0);
    Onset {
        sample,
        confidence: (rise / LEVEL_RISE_FULL_DB).clamp(0.0, 1.0),
    }
}

/// The onset by pitch: the change point of the periodic prediction within `[lo, hi]` (step 4).
fn change_point(signal: &Signal, lo: usize, hi: usize, p1: f64, p2: f64) -> Onset {
    let w = CHANGE_POINT_WINDOW_FRAMES;
    let margin = 256;
    let first = lo.saturating_sub(w + margin);
    let last = (hi + w + margin).min(signal.len());
    let len = last - first;
    let mut back = vec![0.0; len + 1];
    let mut forward = vec![0.0; len + 1];
    let mut power = vec![0.0; len + 1];
    for i in 0..len {
        let n = first + i;
        let v = signal.x[n];
        let b = v - signal.at(n as f64 - p1);
        let f = v - signal.at(n as f64 + p2);
        back[i + 1] = back[i] + b * b;
        forward[i + 1] = forward[i] + f * f;
        power[i + 1] = power[i] + v * v;
    }
    let cost = |t: usize| -> f64 {
        let a = t.saturating_sub(w).max(first) - first;
        let m = t.clamp(first, last) - first;
        let b = (t + w).min(last) - first;
        let numerator = (back[m] - back[a]) + (forward[b] - forward[m]);
        let denominator = power[b] - power[a];
        numerator / (denominator + 1e-12)
    };
    let mut best = (f64::INFINITY, lo);
    for t in lo..=hi {
        let c = cost(t);
        if c < best.0 {
            best = (c, t);
        }
    }
    let t = best.1;
    let edge = 2 * ENVELOPE_STEP_FRAMES;
    if t <= lo + edge || t + edge >= hi {
        // The cost is flat towards one end of the search: one period predicts both notes (the
        // new period a whole multiple of the old, or the old of the new). Only the other
        // prediction tells the notes apart; its error steps at the onset.
        let sums = |prefix: &[f64], t: usize| -> (f64, f64) {
            let a = t.saturating_sub(w).max(first) - first;
            let m = t.clamp(first, last) - first;
            let b = (t + w).min(last) - first;
            (prefix[m] - prefix[a], prefix[b] - prefix[m])
        };
        let floor = 1e-12 * (power[len] + 1.0);
        let (one_sided, sharp): (Box<dyn Fn(usize) -> f64>, isize) = if t <= lo + edge {
            // The new period predicts the old note: the old period's error rises at the onset.
            (
                Box::new(|t| {
                    let (before, after) = sums(&back, t);
                    before / (after + floor)
                }),
                margin as isize,
            )
        } else {
            // The old period predicts the new note: the new period's error falls at the onset.
            (
                Box::new(|t| {
                    let (before, after) = sums(&forward, t);
                    after / (before + floor)
                }),
                -(margin as isize),
            )
        };
        let mut best = (f64::INFINITY, lo);
        for t in lo..=hi {
            let c = one_sided(t);
            if c < best.0 {
                best = (c, t);
            }
        }
        let t = best.1;
        let neighbour = (t as isize + sharp).clamp(first as isize, last as isize) as usize;
        let side = one_sided(neighbour);
        let confidence = if side > 0.0 {
            (1.0 - best.0 / side).clamp(0.0, 1.0)
        } else {
            0.0
        };
        return Onset {
            sample: t,
            confidence,
        };
    }
    let side = cost(t.saturating_sub(margin).max(first)).min(cost((t + margin).min(last)));
    let confidence = if side > 0.0 {
        (1.0 - best.0 / side).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Onset {
        sample: t,
        confidence,
    }
}

/// One note as built, before velocities are known.
struct Built {
    note: EstimatedNote,
    level: f64,
    polyphonic: bool,
}

/// The note starting at `onset` and ending at `end`, the next onset in its region (`next`) or
/// the region's end; `after_region` is the next region's start (steps 5 to 8).
fn build_note(
    signal: &Signal,
    samples: &[f32],
    onset: &Onset,
    end: usize,
    next: Option<&Onset>,
    after_region: Option<usize>,
) -> Built {
    let start = onset.sample;
    let end = end.max(start + 1);
    let pitch = note_pitch(samples, start, end);
    let (f0_hz, pitch_confidence) = match pitch {
        Ok(p) => (p.f0_hz, p.confidence),
        Err(AnalysisError::PitchAmbiguous { confidence }) => (0.0, confidence),
        Err(_) => (0.0, 0.0),
    };
    let period = (f0_hz > 0.0).then(|| f64::from(SAMPLE_RATE_HZ) / f0_hz);
    let midi_note = if f0_hz > 0.0 {
        (69.0 + 12.0 * (f0_hz / 440.0).log2())
            .round()
            .clamp(0.0, 127.0) as u8
    } else {
        0
    };

    let reach = match next {
        Some(_) => end,
        None => after_region
            .unwrap_or(signal.len())
            .min(end + TAIL_WINDOW_FRAMES),
    };
    let gate = gate_end(signal, (start, end), next.is_some(), reach, period);
    let end_sample = gate.sample.clamp(start + 1, end);
    // A gate held to the next onset is only as certain as that onset.
    let gate_confidence = match next {
        Some(n) if end_sample == end => gate.confidence.min(n.confidence),
        _ => gate.confidence,
    };

    // Tail: the first crossing of the silence threshold from the gate end, capped at the next
    // onset (in this region or the next).
    let cap = next
        .map(|n| n.sample)
        .or(after_region)
        .unwrap_or(signal.len())
        .max(end_sample);
    let threshold = 10f64.powf(SILENCE_THRESHOLD_DBFS / 20.0);
    let crossing =
        (end_sample..cap).find(|&t| signal.centred(t as isize, TAIL_WINDOW_FRAMES) < threshold);
    let (tail_end_sample, tail_confidence) = match crossing {
        Some(t) => {
            let before = signal.centred(
                t as isize - TAIL_CONTEXT_FRAMES as isize,
                TAIL_WINDOW_FRAMES,
            );
            let height = db(before) - SILENCE_THRESHOLD_DBFS;
            (t, (height / TAIL_FULL_DB).clamp(0.0, 1.0))
        }
        None => match next {
            Some(n) => (cap, n.confidence),
            None if after_region.is_some() => (cap, 1.0),
            None => (cap, 0.0),
        },
    };

    let polyphonic = polyphony(signal, start, end_sample, f0_hz);
    Built {
        note: EstimatedNote {
            start_sample: start as u64,
            end_sample: end_sample as u64,
            tail_end_sample: tail_end_sample.max(end_sample) as u64,
            midi_note,
            f0_hz,
            velocity: 0.0,
            onset_confidence: onset.confidence,
            pitch_confidence,
            gate_confidence,
            tail_confidence,
        },
        level: gate.level,
        polyphonic,
    }
}

/// A gate end with its confidence and the note's peak level over its gate.
struct Gate {
    sample: usize,
    confidence: f64,
    level: f64,
}

/// A least-squares line `y = a + b t` over points `[i, j)` and its squared error.
#[derive(Clone, Copy, Debug)]
struct Line {
    a: f64,
    b: f64,
    error: f64,
}

/// Running sums of `t`, `y`, `t^2`, `t y` and `y^2` for O(1) line fits.
struct Sums {
    t: Vec<f64>,
    y: Vec<f64>,
    tt: Vec<f64>,
    ty: Vec<f64>,
    yy: Vec<f64>,
}

impl Sums {
    fn new(points: &[(f64, f64)]) -> Self {
        let mut sums = Self {
            t: vec![0.0],
            y: vec![0.0],
            tt: vec![0.0],
            ty: vec![0.0],
            yy: vec![0.0],
        };
        for &(t, y) in points {
            sums.t.push(sums.t.last().unwrap() + t);
            sums.y.push(sums.y.last().unwrap() + y);
            sums.tt.push(sums.tt.last().unwrap() + t * t);
            sums.ty.push(sums.ty.last().unwrap() + t * y);
            sums.yy.push(sums.yy.last().unwrap() + y * y);
        }
        sums
    }

    fn line(&self, i: usize, j: usize) -> Line {
        let n = (j - i) as f64;
        let st = self.t[j] - self.t[i];
        let sy = self.y[j] - self.y[i];
        let stt = self.tt[j] - self.tt[i];
        let sty = self.ty[j] - self.ty[i];
        let syy = self.yy[j] - self.yy[i];
        let var = stt - st * st / n;
        let b = if var > 1e-9 {
            (sty - st * sy / n) / var
        } else {
            0.0
        };
        let a = (sy - b * st) / n;
        let error =
            (syy - 2.0 * a * sy - 2.0 * b * sty + n * a * a + 2.0 * a * b * st + b * b * stt)
                .max(0.0);
        Line { a, b, error }
    }
}

/// The gate end of the note `[start, end)` (step 6 of the method); `cut` when the next onset
/// ends it.
fn gate_end(
    signal: &Signal,
    (start, end): (usize, usize),
    cut: bool,
    reach: usize,
    period: Option<f64>,
) -> Gate {
    let window = level_window(period, ENVELOPE_WINDOW_MIN_FRAMES, 1_024);
    let half = window / 2;
    let mut points = Vec::new();
    let mut t = start + half;
    while t + half <= reach && t <= end {
        points.push((t as f64, signal.centred(t as isize, window)));
        t += ENVELOPE_STEP_FRAMES;
    }
    let fallback = |level: f64| Gate {
        sample: end,
        confidence: 0.0,
        level,
    };
    if points.len() < 2 * MIN_SEGMENT_POINTS {
        let level = signal.centred(((start + end) / 2) as isize, (end - start).max(1));
        return fallback(level);
    }
    let (peak_index, peak) =
        points.iter().enumerate().fold(
            (0, 0.0f64),
            |best, (i, p)| if p.1 > best.1 { (i, p.1) } else { best },
        );
    let descent = &points[peak_index..];
    if descent.len() < MIN_SEGMENT_POINTS {
        return Gate {
            sample: if cut {
                end
            } else {
                points[peak_index].0 as usize
            },
            confidence: if cut { 1.0 } else { 0.0 },
            level: peak,
        };
    }
    let mut segments = fit_segments(descent, peak);
    if let [.., (_, _, before), (i, j, rising)] = segments[..]
        && cut
        && before.b < 0.0
        && rising.b * (descent[j - 1].0 - descent[i].0) >= DESCENT_MIN_FRACTION * peak
    {
        // A last line of a cut note that rises after a fall is the next note's attack read early.
        segments.pop();
    }
    let last = segments.last().copied().unwrap();
    let last_line = last.2;
    let span = descent[last.1 - 1].0 - descent[last.0].0;
    let falls = -last_line.b * span >= DESCENT_MIN_FRACTION * peak;
    let has_rise = peak_index >= MIN_SEGMENT_POINTS;

    let (sample, confidence) = if !falls {
        if cut {
            // Held to the next onset: as steady as the last line is flat and its last points stay
            // on it.
            let tail = &descent[last.1 - MIN_SEGMENT_POINTS..last.1];
            let bend = tail
                .iter()
                .map(|p| last_line.a + last_line.b * p.0 - p.1)
                .sum::<f64>()
                / tail.len() as f64;
            let drop = bend.max(-last_line.b * span);
            (
                end,
                (1.0 - drop / (HOLD_DROP_FULL_FRACTION * peak)).clamp(0.0, 1.0),
            )
        } else {
            (descent[last.0].0 as usize, 0.0)
        }
    } else if segments.len() >= 2 {
        let before = segments[segments.len() - 2].2;
        let knee = intersection(before, last_line)
            .filter(|k| *k >= descent[0].0 - half as f64 && *k <= end as f64)
            .unwrap_or(descent[last.0].0);
        let s1 = before.b;
        let s2 = last_line.b;
        let scale = s1.abs().max(s2.abs());
        let visibility = if scale > 0.0 {
            ((s2 - s1).abs() / scale).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (knee.round() as usize, visibility)
    } else if cut {
        (end, AMBIGUOUS_GATE_CONFIDENCE)
    } else {
        // One falling line from the peak to silence: the release began at the peak, or a decay
        // ran into a release of the same slope; which, the level does not tell.
        let knee = if has_rise {
            let rise = Sums::new(&points[..=peak_index]).line(0, peak_index + 1);
            intersection(rise, last_line).unwrap_or(points[peak_index].0)
        } else {
            points[peak_index].0
        };
        (
            knee.round().max(start as f64) as usize,
            if has_rise {
                AMBIGUOUS_GATE_CONFIDENCE
            } else {
                0.0
            },
        )
    };
    let sample = sample.clamp(start + 1, end);
    let level = points
        .iter()
        .filter(|p| p.0 <= sample as f64 + half as f64)
        .map(|p| p.1)
        .fold(0.0f64, f64::max)
        .max(points[0].1);
    Gate {
        sample,
        confidence,
        level,
    }
}

/// Where two lines meet, if they are not parallel.
fn intersection(a: Line, b: Line) -> Option<f64> {
    let slope = a.b - b.b;
    (slope.abs() > 1e-15).then(|| (b.a - a.a) / slope)
}

/// The best fit of one to three lines to `points` (step 6): `(first, end, line)` per line.
fn fit_segments(points: &[(f64, f64)], peak: f64) -> Vec<(usize, usize, Line)> {
    let sums = Sums::new(points);
    let n = points.len();
    let m = MIN_SEGMENT_POINTS;
    let penalty = SEGMENT_PENALTY * peak * peak;
    let mut best: (f64, Vec<(usize, usize, Line)>) = {
        let line = sums.line(0, n);
        (line.error, vec![(0, n, line)])
    };
    // Two lines.
    for i in m..=n.saturating_sub(m) {
        let a = sums.line(0, i);
        let b = sums.line(i, n);
        let cost = a.error + b.error + penalty;
        if cost < best.0 {
            best = (cost, vec![(0, i, a), (i, n, b)]);
        }
    }
    // Three lines.
    if n >= 3 * m {
        for i in m..=n - 2 * m {
            let a = sums.line(0, i);
            for j in i + m..=n - m {
                let b = sums.line(i, j);
                let c = sums.line(j, n);
                let cost = a.error + b.error + c.error + 2.0 * penalty;
                if cost < best.0 {
                    best = (cost, vec![(0, i, a), (i, j, b), (j, n, c)]);
                }
            }
        }
    }
    best.1
}

/// Whether the note's gate `[start, gate_end)` sounds polyphonic (module documentation).
fn polyphony(signal: &Signal, start: usize, gate_end: usize, f0_hz: f64) -> bool {
    let span = gate_end.saturating_sub(start);
    let skip = (span / 4).min(2_400);
    let available = span - skip;
    if available < POLYPHONY_MIN_FFT_LENGTH {
        return false;
    }
    let mut n = POLYPHONY_MAX_FFT_LENGTH;
    while n > available {
        n /= 2;
    }
    let first = start + skip + (available - n) / 2;
    let mut buffer: Vec<Complex> = (0..n)
        .map(|k| {
            let phase = 2.0 * std::f64::consts::PI * k as f64 / n as f64;
            let w = 0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos();
            Complex::new(signal.x[first + k] * w, 0.0)
        })
        .collect();
    if fft(&mut buffer).is_err() {
        return false;
    }
    let bin_hz = f64::from(SAMPLE_RATE_HZ) / n as f64;
    let top = ((POLYPHONY_MAX_HZ / bin_hz) as usize).min(n / 2 - 1);
    let magnitude: Vec<f64> = buffer[..=top + 1].iter().map(|z| z.norm()).collect();
    let largest = magnitude[1..=top].iter().copied().fold(0.0f64, f64::max);
    if largest <= 0.0 {
        return false;
    }
    let floor = largest * 10f64.powf(POLYPHONY_PEAK_FLOOR_DB / 20.0);
    let mut peaks: Vec<(f64, f64)> = Vec::new();
    for k in 2..top {
        let m = magnitude[k];
        if m >= floor && m > magnitude[k - 1] && m >= magnitude[k + 1] {
            let (a, b, c) = (
                magnitude[k - 1].max(1e-300).ln(),
                m.ln(),
                magnitude[k + 1].max(1e-300).ln(),
            );
            let denominator = a - 2.0 * b + c;
            let offset = if denominator.abs() > 1e-12 {
                (0.5 * (a - c) / denominator).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            peaks.push(((k as f64 + offset) * bin_hz, m * m));
        }
    }
    let Some(&(lowest, _)) = peaks.first() else {
        return false;
    };
    let fundamental = if f0_hz > 0.0 {
        let h = lowest / f0_hz;
        if (h - 1.0).abs() > POLYPHONY_HARMONIC_TOLERANCE {
            return true;
        }
        f0_hz
    } else {
        lowest
    };
    let total: f64 = peaks.iter().map(|p| p.1).sum();
    let inharmonic: f64 = peaks
        .iter()
        .filter(|(f, _)| {
            let h = f / fundamental;
            (h - h.round()).abs() > POLYPHONY_HARMONIC_TOLERANCE || h.round() < 1.0
        })
        .map(|p| p.1)
        .sum();
    inharmonic > POLYPHONY_INHARMONIC_SHARE * total
}
