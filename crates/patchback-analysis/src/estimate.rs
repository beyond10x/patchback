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
//! 1. **Sounding regions.** A run of at least [`QUIET_RUN_FRAMES`] quiet samples is silence; each
//!    stretch between silences is a sounding region, and its first sample is an onset *from
//!    silence*. A sample is quiet when no louder than [`QUIET_AMPLITUDE`]; over a noise floor
//!    whose RMS times [`NOISE_FLOOR_FACTOR`] is louder than that, when the RMS of the
//!    [`QUIET_RUN_FRAMES`] samples centred on it is no louder than that multiple. The floor is the
//!    median RMS of the [`NOISE_FLOOR_WINDOWS`] quietest of the clip's windows of
//!    [`NOISE_FLOOR_WINDOW_FRAMES`] samples, when above zero and below
//!    [`crate::SILENCE_THRESHOLD_DBFS`]: the level of the quietest windows, whatever the noise's
//!    amplitude distribution (Dc1-20). A region whose level (the RMS of [`TAIL_WINDOW_FRAMES`]
//!    samples) never reaches the silence threshold is the inaudible end of a release and holds no
//!    note. A region can begin in such an end, before the next note's attack: its onset from
//!    silence is where the attack begins (`attack_start`): of the points read every
//!    [`ATTACK_HOP_FRAMES`] samples up to the first at which the level of the
//!    [`TAIL_WINDOW_FRAMES`] samples before it reaches the threshold, the last at which that level
//!    is no more than [`NOISE_FLOOR_FACTOR`] times its least (the tail ripples with its waveform).
//! 2. **Onset candidates.** Within a region, the normalized spectral flux
//!    `sum max(0, |X_k| - |X_{k-1}|) / sum |X_k|` of Hann-windowed frames of
//!    [`NOVELTY_FFT_LENGTH`] samples centred every [`NOVELTY_HOP_FRAMES`] samples, over the bins
//!    below [`NOVELTY_MAX_HZ`], is the novelty. A frame whose novelty is at least
//!    [`NOVELTY_THRESHOLD`] and the largest within [`NOVELTY_PEAK_HALF_WIDTH`] frames either side
//!    is a candidate, unless it lies within [`REGION_ONSET_GUARD_FRAMES`] of the region's start.
//!    The hidden onsets of step 6 are candidates too.
//! 3. **Candidates kept.** The stretch between two neighbouring onsets, [`COARSE_MARGIN_FRAMES`]
//!    in from each candidate, has its pitch estimated as a note's is (step 5). A candidate is an onset
//!    *by pitch* when both sides have a pitch and they differ by [`PITCH_CHANGE_CENTS`] or more,
//!    or when one side's pitch is ambiguous (not too short to measure) and the other's period
//!    predicts it no better than
//!    [`PERIOD_MISMATCH`] (the power of `x[n] - x[n - P]` over that of `x[n]`); an onset *by
//!    step* when a hidden step of step 6 lies near it and the level falls across it by [`STEP_DB`]
//!    or more; and an onset *by level* when the level, the RMS over whole periods, rises by
//!    [`LEVEL_RISE_DB`] or more across it ([`REBOUND_RISE_DB`] near a hidden rebound); otherwise
//!    the weakest is dropped and the two stretches are one. This repeats until none is dropped.
//! 4. **Onsets refined.** An onset from silence is the region's first sample. An onset by pitch
//!    is the change point `t` that minimizes the periodic prediction error
//!    `C(t) = (sum_{t-W <= n < t} (x[n] - x[n - P1])^2 + sum_{t <= n < t+W} (x[n] - x[n + P2])^2)
//!    / sum_{t-W <= n < t+W} x[n]^2`, `P1` and `P2` the periods before and after,
//!    `W` [`CHANGE_POINT_WINDOW_FRAMES`], fractional delays read by linear interpolation, searched
//!    [`ONSET_SEARCH_FRAMES`] either side of the candidate; when the cost is flat towards one end
//!    (one period predicts both notes), the other prediction's error alone. An onset by level is
//!    the knee where the level (at least [`LEVEL_WINDOW_MIN_FRAMES`] long, sampled every
//!    [`KNEE_STEP_FRAMES`] samples) starts to rise, read from a fit of three lines (before,
//!    rising, after); a rise no longer than [`KNEE_SHORT_RISE`] level kernels is moved
//!    [`KNEE_SHIFT`] of the kernel later. An onset by step is where the level falls most across a
//!    sample within [`STEP_REACH_FRAMES`] of the candidate.
//! 5. **Notes.** Each onset starts a note, which ends at the next onset or at the region's end.
//!    Its pitch is [`estimate_f0`] of the note's samples. Where that refuses it as ambiguous
//!    (an octave the subharmonic test leaves undecided, as for a driven saw), the spans of
//!    [`PITCH_SPAN_FRAMES`] samples every half span are asked, and the median of the pitches they
//!    give is the note's, with the least of their confidences, when they all lie within
//!    [`PITCH_CHANGE_CENTS`] of it. The fundamental of the note's steady part read from its
//!    spectrum (Polyphony, below) refines a pitch it agrees with to within [`PITCH_CHANGE_CENTS`]
//!    (the period's grid is coarse at high notes, and a dark filter's aliases bias it at low
//!    ones); one that names another pitch leaves the pitch with confidence 0.
//!    `midi_note` is `f0_hz` rounded to the nearest MIDI number.
//!    A note too short for [`estimate_f0`], or whose pitch it refuses, has pitch confidence 0 (or
//!    the ambiguous estimate's confidence), `f0_hz` 0 and `midi_note` 0, and the estimation is
//!    refused as [`EstimationError::LowConfidenceNotes`] (Dc1-10).
//! 6. **Gate end.** The level from the note's peak, sampled every [`ENVELOPE_STEP_FRAMES`]
//!    samples up to half the level kernel's spread (the window plus seven eighths of a period)
//!    before the earliest the next onset may lie, its sample less its interval's half-width
//!    (the level at a point reads that far after it, and the next attack's sound, which the
//!    level must not read, would hide a release under its rise) or, for a region's last note,
//!    through its release into the quiet after the region (at most [`TAIL_WINDOW_FRAMES`] past
//!    its end, never into the next region), is fitted by one to three straight lines (least
//!    squares, each extra line costing [`SEGMENT_PENALTY`] times the squared peak level whatever
//!    the note's length, a line at least [`MIN_SEGMENT_POINTS`] points long). When the first line
//!    after the highest point rises, the note's own attack from the level the note before it left,
//!    the peak is that line's top. A last line that falls by at least [`DESCENT_MIN_FRACTION`] of
//!    the peak level is the release, and the gate ends where it meets the line before it. A last
//!    line of a note cut by the next onset that rises by [`REBOUND_FRACTION`] of the peak level
//!    within two level windows after a line falling by [`DESCENT_MIN_FRACTION`] is that onset's
//!    attack read early and is dropped. A note cut by the next onset whose last line does not
//!    fall held its gate to that onset. A note whose only falling line starts at its peak, or
//!    whose falling line follows a rise, is ambiguous: cut by the next onset, a decay cut by a
//!    legato note or a release from the peak, and its gate is taken to end at that onset; ending
//!    its region, a release from the peak or a decay running into a release of the same slope,
//!    and its gate is taken to end at the knee.
//!
//!    The level also tells of onsets the novelty missed (*hidden* onsets, candidates of a second
//!    pass of steps 2 to 8): a rise of at least [`REBOUND_FRACTION`] of the peak above the lowest
//!    level since the highest point (before the peak, after a fall of [`DESCENT_MIN_FRACTION`]),
//!    a *rebound*; and two lines of the body, decay and sustain, that do not meet within a
//!    window of their knee by [`STEP_FRACTION`] of the peak, a *step* (a line too short to clear
//!    the gate's slack is read from the points clear of its knees, if they span a window).
//! 7. **Tail end.** One past the last sample after the gate end at which the RMS of the
//!    [`TAIL_WINDOW_FRAMES`] samples centred on it is at or above the silence threshold
//!    (Dc1-9, Dc1-21: a level that dips below it and comes back has not ended); or the next
//!    note's onset when the level holds to it, or the clip's end.
//! 8. **Velocity** (Dc1-2). The note's peak level over its gate relative to the loudest note's,
//!    linear, the loudest note 1: the inverse of the renderer's linear velocity scaling.
//! 9. **Polyphony** (Dc1-3), below.
//!
//! # Confidence (Dc1-1, DESIGN.md § 10)
//!
//! A confidence falls wherever another explanation of the same level or waveform competes.
//!
//! - **Onset.** From silence: the rise of the RMS from the [`ONSET_CONTRAST_FRAMES`] samples
//!   before the onset to as many after it, in dB, over [`ONSET_CONTRAST_FULL_DB`], clamped to
//!   `[0, 1]`; 0 for a region that starts at the clip's first sample (the note may have begun
//!   before the clip). By pitch or by level, the onset's localisation (Dc1-15, Dc1-18): the
//!   half-width `h` of its interval gives `1 - (h / 256)^p`, clamped, `p` [`LOCALISATION_POWER`],
//!   so 0 once the interval reaches the tolerance of 256 samples; and no more than the
//!   prominence of the best position of the onset's cost, one minus its cost over the least cost
//!   of a position more than the tolerance away. The interval is never narrower than the grid the
//!   evidence was read on: one sample by pitch, [`KNEE_STEP_FRAMES`] by level. By pitch the cost
//!   is the periodic prediction error `C(t)` over the change point's span, and its interval every
//!   split whose cost lies within [`LOCALISATION_CHI2`] noise variances of the least, the
//!   variance the least cost over the residual's independent observations: the residual's length
//!   at the best split over `1 + 2 Σ ρ_k`, `ρ_k` its autocorrelation at lag `k`, summed while
//!   positive. `C(t)` steps at the waveform's edges, a period apart, so inside that interval the
//!   onset is refined on the sample grid (`refine`) from the residuals of the old note's
//!   continuation `x[n - T_old]` and the new note's backward continuation `x[n + T_new]`, each
//!   read as its median over [`REFINE_WINDOW_FRAMES`] samples: the onset is where the new
//!   prediction's residual stops being above the old one's for good (the last crossing when the
//!   old period is the longer, the first crossing that holds for a window otherwise, because the
//!   shorter lag also continues the smoother note of the longer period), and its interval spans
//!   the first and the last crossing. A prediction that does not tell the notes apart (its
//!   residual on the other note alone less than [`LOCALISATION_CHI2`] times that on its own
//!   note) leaves the onset unrefined, with the cost's interval. The same residuals read over a
//!   period either side of the cost's onset place it as well: the interval holds that placement,
//!   and where the refinement failed every crossing of that wider search too. A change point
//!   within the longer period of either end of its search may lie beyond it: the search bounds its
//!   interval on that side, the evidence does not, and its confidence is 0. Where one period
//!   predicts both notes, the cost is the one-sided error ratio and its interval is read the
//!   same way, unrefined (one independent observation a period of the longer period). Where the
//!   RMS rises by
//!   [`TAIL_FULL_DB`] or more across the change point (the old note faded almost to silence and
//!   predicts nothing), no more than the level onset's confidence when that lies more than the
//!   tolerance away. By level the cost is the error of the three-line fit for each knee, its
//!   interval every knee whose cost lies within [`LOCALISATION_CHI2`] noise variances of the least
//!   (one independent observation a level window), and it also holds the best knee when the onset
//!   is moved off it (a short attack) and the first point where the level falls below the line
//!   before the knee by more than the noise (a note that began with a lower velocity), and where
//!   a fall below the two lines about the knee began: the first point it moves by more than the
//!   noise (the knee, when that point lies past it), plus half the level kernel, less the kernel
//!   times the noise over the fall's depth (a fall seen barely above the noise may have begun
//!   half a kernel before that point); its
//!   prominence is the best knee's against the other fits' knees (the shift and the departure
//!   widen the interval, they are no competing fits). No more than the rise in dB over
//!   [`LEVEL_RISE_FULL_DB`], than [`KNEE_ATTACK_FULL_FRAMES`] over the attack's length beyond
//!   the level kernel, than how little the level dips below its two lines from two kernels before
//!   the knee to a kernel after it (short of half a kernel before the rise's top, a corner the
//!   other way), or than the change of slope at the knee over the larger slope (a level already
//!   rising as steeply before it bends nowhere); and at most [`AMBIGUOUS_GATE_CONFIDENCE`] when
//!   the level half a kernel before the knee stands [`LEVEL_RISE_DB`] or more above its highest
//!   point from the previous onset to one and a half kernels before the knee (one attack the two
//!   onsets cut, which of them starts it the level does not tell). By step: the fall in dB over
//!   [`LEVEL_RISE_FULL_DB`], less the rise of the level out of the fall (a release into the next
//!   note's attack); at most [`AMBIGUOUS_GATE_CONFIDENCE`] when the fall lies more than a level
//!   window from the step the body's lines placed (a decay's fall), or when the level rises out
//!   of it by [`LEVEL_RISE_DB`] or more and the next onset follows within the reach that rise is
//!   read over (the fall may be the release before that onset); and no more than the
//!   localisation of a reach that is the distance between the two places that read the step, at
//!   least [`KNEE_SHIFT`] of the level window (the windows smear a fall over their length), plus
//!   the 8-sample grid the fall is read on.
//! - **Pitch.** `1 - d'` of [`estimate_f0`] (its confidence), 0 when it refuses for length or
//!   range; no more than `1 - (m - F) / F`, clamped, `F` [`FOREIGN_MISMATCH`] and `m` the largest
//!   period mismatch of the gate's windows of [`PITCH_COVER_FRAMES`] samples (at least two
//!   periods, at least a tenth of the loudest window's RMS): a stretch the note's period does not
//!   predict is another note in it. The same bound holds the gate and the tail.
//! - **Gate.** The visibility of the knee where the release begins: the change of slope between
//!   the release line and the line before it, `|s2 - s1| / max(|s1|, |s2|)`; no more than its
//!   sharpness, the change of slope over [`GATE_SLACK_FRAMES`] samples over
//!   [`KNEE_SHARP_FULL_FRACTION`] of the peak level, nor than the release's straightness (1 minus
//!   the largest deviation of the release from its line, beyond a window of the knee and of the
//!   end, over [`REBOUND_FRACTION`] of the peak, scaled by how nearly the points of the window
//!   after that reach hold the slope of the line before the knee: a release that already falls as
//!   steeply as its line there bends by the drive's curve, not at a later knee), all clamped. The
//!   end is the slack of the next onset for a note cut by it, and otherwise where the release
//!   line reaches silence, a corner the window rounds too. For a note cut by the next onset, at
//!   most [`AMBIGUOUS_GATE_CONFIDENCE`] when the release line falls less steeply than the line
//!   before it (a decay still running at the onset), when a short last line follows a line
//!   falling from a flatter one (a release begun at the earlier knee and the next note's attack
//!   falling to a lower velocity), or when the line before the knee runs no more than a window
//!   from the peak (the peak's own smoothed corner: a decay cut by the onset or a release from the
//!   peak). For a gate held to the next onset, the lesser of how steady the level still is where
//!   it last holds and how near the onset a release seen before it begins. Steady: 1 plus the
//!   slope of the points of the last two windows that a release begun within the gate tolerance
//!   leaves alone, times the gate tolerance, over [`HOLD_DROP_FULL_FRACTION`] of the peak
//!   level, clamped (a level still falling there is a slow release, or a decay still running,
//!   which the level does not tell from one; a decay that ran into its sustain within the line is
//!   held). Near: the last points, from the tolerance and three windows before the onset, are
//!   fitted by a line and by a line less a release ramp smoothed by the level's kernel
//!   (`release_departure`); when the ramp explains them better by more than
//!   [`LOCALISATION_CHI2`] noise variances, `1 - (d / 1024)^p`, clamped, `d` how far before the
//!   onset the earliest start within that margin of the best lies and `p`
//!   [`LOCALISATION_POWER`]; otherwise 1 minus the mean drop of the last
//!   [`MIN_SEGMENT_POINTS`] points below the last line, or of the last point below the line of
//!   the points before, over [`HOLD_DROP_FULL_FRACTION`] of the peak. Never more than that
//!   onset's confidence at the gate tolerance (a level held to a spurious or misplaced onset is
//!   not a gate end). [`AMBIGUOUS_GATE_CONFIDENCE`] for the ambiguous cases of step 6; 0 for a
//!   note too short to fit or whose level never falls. No more than `1 - u / REBOUND_FRACTION`,
//!   clamped, `u` the largest rebound of the level (before the peak, after any fall of
//!   [`REBOUND_FRACTION`]), step or residual of the body's two lines, over the peak: an onset the
//!   note does not know of. The body is always fitted with two lines when it has points for two,
//!   with no penalty for the second: a decay is a line apart from its sustain however small its
//!   drop and however near the gate it ends.
//! - **Tail.** At the last crossing of the silence threshold: how far the level stands above the
//!   threshold [`TAIL_CONTEXT_FRAMES`] samples before it, in dB, over [`TAIL_FULL_DB`], clamped
//!   (a release that falls steeply through the threshold is certain, one that hovers about it is
//!   not). A tail cut by the next onset inherits that onset's confidence; one cut by the next
//!   region is 1; a tail that reaches the clip's end without falling below the threshold has
//!   confidence 0. No more than `1 - b / REBOUND_FRACTION`, clamped, `b` how far the level from
//!   the gate end to the tail strays from one falling line over the level at the gate end (and,
//!   for a cut tail, how far that line misses the line before the gate): a level that bends or
//!   stays on the way is another note. The gate's rebound bound holds the tail too.
//!
//! The estimation is refused as [`EstimationError::LowConfidenceNotes`] when any note has any
//! confidence below its quantity's threshold ([`ONSET_CONFIDENCE_THRESHOLD`],
//! [`PITCH_CONFIDENCE_THRESHOLD`], [`GATE_CONFIDENCE_THRESHOLD`],
//! [`TAIL_CONFIDENCE_THRESHOLD`]; Dc1-4), with every note.
//!
//! # Polyphony (Dc1-3, Dc1-22)
//!
//! Polyphony is read from the signal alone, before any confidence is formed: no pitch, gate or
//! confidence of the notes enters it. Each note's *sound* runs from its onset to the next onset
//! or the end of its tail. Its *steady part* skips a quarter of the sound (at most 50 ms, the
//! attack) and is the window of the largest power of two up to [`POLYPHONY_MAX_FFT_LENGTH`]
//! samples centred in the rest; a sound with less than [`POLYPHONY_MIN_FFT_LENGTH`] samples left
//! is not read. A run of neighbouring notes whose pitches lie less than [`PITCH_CHANGE_CENTS`]
//! apart or that have none (a pitch span, which may be one sustained tone the onsets cut, or two
//! tones whose beats the onsets cut, each reading the pitch between the two, whichever MIDI
//! number it rounds to) is read as one sound too: its longer window resolves near tones that no
//! window of one of its notes resolves (a minor second at 110 Hz is 6.5 Hz wide, under the main
//! lobe of a 4,096-sample window).
//!
//! A steady part whose two halves' magnitude spectra have a cosine similarity below
//! [`POLYPHONY_STATIONARY`] holds a change of tone, not two tones, and is not read. Otherwise its
//! Blackman-windowed spectrum is read only when its largest peak stands [`POLYPHONY_TONAL_DB`]
//! above the median (noise has peaks that are no harmonics), and its peaks below
//! [`POLYPHONY_MAX_HZ`] within [`POLYPHONY_PEAK_FLOOR_DB`] of the largest are found. Every peak
//! within [`POLYPHONY_CANDIDATE_FLOOR_DB`] of the largest and at or above [`crate::F0_MIN_HZ`] is
//! a candidate fundamental `F`. A candidate explains the sound when the peaks from `F / 2` up that
//! are no harmonics of it hold at most [`POLYPHONY_INHARMONIC_SHARE`] of those peaks' power. A
//! peak is a harmonic when within [`POLYPHONY_HARMONIC_TOLERANCE`] of `F`, or
//! [`POLYPHONY_LOBE_BINS`] bins when that is wider, of a whole multiple of `F`. An inharmonic
//! peak beside a harmonic at least 6 dB stronger is a sideband of that harmonic, the envelope of
//! one tone changing within the window, and is left out, when its mirror image about the
//! harmonic holds within [`POLYPHONY_SIDEBAND_BINS`] bins another peak, or a bin off the
//! harmonic's main lobe with at least a quarter of its power: an envelope spreads a harmonic to
//! both sides alike, and on the side nearer a stronger neighbour the spread may be a shoulder,
//! not a peak, while a stable tone has nothing past its main lobe's first null. Peaks below `F / 2` are left out too: they are aliases of the driven oscillator's
//! harmonics past the Nyquist frequency (at `|k F - m fs|`), which a dark filter passes more than
//! the fundamental, and the second tone of a Dc1-3 interval is never that low.
//!
//! A sound no candidate explains is two tones, unless its two halves have clear pitches
//! [`PITCH_CHANGE_CENTS`] or more apart (a change of tone). One such sound refuses the clip as
//! [`EstimationError::PolyphonicInput`], before low confidence is considered: two tones a minor
//! second, a major third, a tritone or a fifth apart are refused as polyphonic, never as
//! low-confidence notes (Dc1-22).
//!
//! The candidate that explains the steady part best, refined from the harmonics within the main
//! lobe of their places weighted by power, is also the note's spectral pitch (step 5).
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

/// The start of a region's attack is looked for every this many samples. Provisional.
pub const ATTACK_HOP_FRAMES: usize = 8;

/// The noise floor is measured over windows of this many samples. Provisional.
pub const NOISE_FLOOR_WINDOW_FRAMES: usize = 256;

/// The noise floor is the median RMS of this many of the quietest windows. Provisional.
pub const NOISE_FLOOR_WINDOWS: usize = 8;

/// Over a noise floor, a sample is quiet when no louder than this many times the floor's RMS
/// (or [`QUIET_AMPLITUDE`], if that is louder). Provisional: 12 dB.
pub const NOISE_FLOOR_FACTOR: f64 = 4.0;

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

/// The window over which a note's pitch is checked against every part of its gate (step 5).
/// Provisional.
pub const PITCH_COVER_FRAMES: usize = 1_024;

/// The [`period_mismatch`] of a window of the gate above which a note's pitch is contested: at
/// this its confidences start to fall, at twice it they are 0 (step 5). Provisional.
pub const FOREIGN_MISMATCH: f64 = 1.0;

/// A rise of the level by at least this many dB across a candidate starts a note. Provisional.
pub const LEVEL_RISE_DB: f64 = 1.0;

/// An onset by level whose level rises by this many dB has onset confidence 1. Provisional.
pub const LEVEL_RISE_FULL_DB: f64 = 6.0;

/// The rise in dB that confirms an onset candidate at a rebound of the level inside a note (step 6).
pub const REBOUND_RISE_DB: f64 = 0.25;

/// How far before an estimated gate end the release may already have begun: the body test of
/// step 6 ignores it.
pub const GATE_SLACK_FRAMES: f64 = 1_024.0;

/// A knee between two lines is located within [`GATE_SLACK_FRAMES`] only as far as moving it that
/// far changes the level: a change of slope that moves the level by this share of the peak over
/// that span gives full confidence, a smaller one proportionally less. Provisional.
pub const KNEE_SHARP_FULL_FRACTION: f64 = 0.04;

/// The gap, as a fraction of the peak level, between a body's two lines at their knee that
/// marks a step of the level (step 6).
pub const STEP_FRACTION: f64 = 0.02;

/// The fall in dB across a step that confirms it as an onset (step 3).
pub const STEP_DB: f64 = 0.3;

/// How far from a step candidate its sample is searched (step 4).
pub const STEP_REACH_FRAMES: usize = 1_024;

/// The share of the level kernel's width a short rise's knee is moved later (step 4).
pub const KNEE_SHIFT: f64 = 0.25;

/// The attack length, beyond the level kernel, up to which a level knee can be fully confident;
/// a longer attack scales its confidence down (step 4).
pub const KNEE_ATTACK_FULL_FRAMES: f64 = 384.0;

/// The level window is a whole number of periods at least this long. Provisional.
pub const LEVEL_WINDOW_MIN_FRAMES: usize = 256;

/// The level is averaged over this many shifts of its window across one period ([`Signal::level`]).
pub const LEVEL_PHASES: usize = 8;

/// The spacing of the level's points in the knee fit of an onset by level, in samples.
/// Provisional.
pub const KNEE_STEP_FRAMES: usize = 16;

/// The knee fit of an onset by level reads the level this many samples past its search range, so
/// a slow attack and the decay after it are inside the fit. Provisional.
pub const KNEE_REACH_FRAMES: usize = 3_072;

/// A rise no longer than this many level windows is an attack shorter than the window
/// ([`level_onset`]). Provisional.
pub const KNEE_SHORT_RISE: f64 = 1.25;

/// The onset tolerance of Dc1-9 in samples: an onset whose localisation interval reaches this
/// half-width has confidence 0 (Dc1-15).
pub const ONSET_TOLERANCE_FRAMES: usize = 256;

/// The gate tolerance of Dc1-9 in samples: a gate held to the next onset is as confident as that
/// onset is of lying within it.
pub const GATE_TOLERANCE_FRAMES: usize = 1_024;

/// The tail tolerance of Dc1-9 in samples: a tail cut by the next onset is as confident as that
/// onset is of lying within it.
pub const TAIL_TOLERANCE_FRAMES: usize = 2_400;

/// The localisation margin of an onset (Dc1-15), in units of the cost's noise per independent
/// observation: the 95 % point of the chi-squared distribution with one degree of freedom. A
/// position whose cost lies within this many noise variances of the least cost is as likely an
/// onset as the best one. Provisional.
pub const LOCALISATION_CHI2: f64 = 3.84;

/// The onset confidence of a localisation interval of half-width `h` samples is
/// `1 - (h / ONSET_TOLERANCE_FRAMES)^LOCALISATION_POWER`, clamped (Dc1-15): 0 once the interval
/// reaches the tolerance. Provisional.
pub const LOCALISATION_POWER: f64 = 8.0;

/// The window over which a pitch-change onset's prediction residuals are read on the sample grid
/// (Dc1-18), in samples: their median, so a waveform edge of a few samples does not decide.
/// Provisional.
pub const REFINE_WINDOW_FRAMES: usize = 64;

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
pub const HOLD_DROP_FULL_FRACTION: f64 = 0.005;

/// A level that rises by this share of its peak after the peak holds an onset the note does not
/// know of (step 6): a candidate, and the note's gate and tail confidences fall to 0 as the rise
/// reaches it. Provisional.
pub const REBOUND_FRACTION: f64 = 0.01;

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
pub const POLYPHONY_MAX_FFT_LENGTH: usize = 16_384;

/// The polyphony check reads the peaks below this frequency, in Hz. Provisional.
pub const POLYPHONY_MAX_HZ: f64 = 5_000.0;

/// The polyphony check reads the peaks within this many dB of the largest: above the
/// aliasing and distortion products of a driven saw at high notes (about -40 dB), below the
/// second tone of an interval. Provisional.
pub const POLYPHONY_PEAK_FLOOR_DB: f64 = -30.0;

/// A peak within this share of a harmonic of the fundamental is that harmonic. Provisional.
pub const POLYPHONY_HARMONIC_TOLERANCE: f64 = 0.1;

/// A run of notes without a pitch is checked only when its largest peak stands this many dB
/// above the median of its spectrum: a tone, not noise. Provisional.
pub const POLYPHONY_TONAL_DB: f64 = 30.0;

/// A peak within this many bins of a harmonic is that harmonic, however near the fundamental
/// allows: the half-width of the Blackman window's main lobe. Provisional.
pub const POLYPHONY_LOBE_BINS: f64 = 3.0;

/// A spectrum is read only when the magnitude spectra of its two halves have at least this cosine
/// similarity: one sound throughout, not a change from one tone to another. Provisional.
pub const POLYPHONY_STATIONARY: f64 = 0.8;

/// A peak within this many dB of the largest is a candidate fundamental. Provisional.
pub const POLYPHONY_CANDIDATE_FLOOR_DB: f64 = -20.0;

/// An inharmonic peak whose mirror image about a stronger harmonic holds, within this many bins,
/// another peak or a quarter of its power off the harmonic's main lobe is a sideband of an
/// envelope that changes within the spectrum, and is left out. Provisional.
pub const POLYPHONY_SIDEBAND_BINS: f64 = 2.0;

/// A note whose inharmonic peaks hold more than this share of the peaks' power is polyphonic.
/// Provisional, set on the Dc1-3 cases.
pub const POLYPHONY_INHARMONIC_SHARE: f64 = 0.008;

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

/// The MIDI number nearest to `f0_hz` (A4 = 440 Hz = 69), clamped to 0 to 127; 0 for no pitch.
pub(crate) fn nearest_midi(f0_hz: f64) -> u8 {
    if f0_hz > 0.0 {
        (69.0 + 12.0 * (f0_hz / 440.0).log2())
            .round()
            .clamp(0.0, 127.0) as u8
    } else {
        0
    }
}

/// The estimator's configuration as `(name, value)`: [`ESTIMATOR`], then every constant that
/// decides its result, in a fixed order, each value as Rust prints it.
pub fn configuration() -> Vec<(String, String)> {
    let entries: [(&str, String); 65] = [
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
        ("attack_hop_frames", ATTACK_HOP_FRAMES.to_string()),
        (
            "noise_floor_window_frames",
            NOISE_FLOOR_WINDOW_FRAMES.to_string(),
        ),
        ("noise_floor_windows", NOISE_FLOOR_WINDOWS.to_string()),
        ("noise_floor_factor", NOISE_FLOOR_FACTOR.to_string()),
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
        ("pitch_cover_frames", PITCH_COVER_FRAMES.to_string()),
        ("foreign_mismatch", FOREIGN_MISMATCH.to_string()),
        ("period_mismatch", PERIOD_MISMATCH.to_string()),
        ("level_rise_db", LEVEL_RISE_DB.to_string()),
        ("level_rise_full_db", LEVEL_RISE_FULL_DB.to_string()),
        ("rebound_rise_db", REBOUND_RISE_DB.to_string()),
        ("gate_slack_frames", GATE_SLACK_FRAMES.to_string()),
        (
            "knee_sharp_full_fraction",
            KNEE_SHARP_FULL_FRACTION.to_string(),
        ),
        ("step_fraction", STEP_FRACTION.to_string()),
        ("step_db", STEP_DB.to_string()),
        ("step_reach_frames", STEP_REACH_FRAMES.to_string()),
        ("knee_shift", KNEE_SHIFT.to_string()),
        (
            "knee_attack_full_frames",
            KNEE_ATTACK_FULL_FRAMES.to_string(),
        ),
        (
            "level_window_min_frames",
            LEVEL_WINDOW_MIN_FRAMES.to_string(),
        ),
        ("level_phases", LEVEL_PHASES.to_string()),
        ("knee_step_frames", KNEE_STEP_FRAMES.to_string()),
        ("knee_reach_frames", KNEE_REACH_FRAMES.to_string()),
        ("knee_short_rise", KNEE_SHORT_RISE.to_string()),
        ("onset_tolerance_frames", ONSET_TOLERANCE_FRAMES.to_string()),
        ("gate_tolerance_frames", GATE_TOLERANCE_FRAMES.to_string()),
        ("tail_tolerance_frames", TAIL_TOLERANCE_FRAMES.to_string()),
        ("localisation_chi2", LOCALISATION_CHI2.to_string()),
        ("localisation_power", LOCALISATION_POWER.to_string()),
        ("refine_window_frames", REFINE_WINDOW_FRAMES.to_string()),
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
        ("rebound_fraction", REBOUND_FRACTION.to_string()),
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

/// The polyphony constants as one value:
/// `min_fft/max_fft/max_hz/floor_db/tolerance/tonal_db/lobe_bins/stationary/
/// candidate_floor_db/sideband_bins/share`.
fn polyphony_configuration() -> String {
    format!(
        "{}/{}/{}/{}/{}/{}/{}/{}/{}/{}/{}",
        POLYPHONY_MIN_FFT_LENGTH,
        POLYPHONY_MAX_FFT_LENGTH,
        POLYPHONY_MAX_HZ,
        POLYPHONY_PEAK_FLOOR_DB,
        POLYPHONY_HARMONIC_TOLERANCE,
        POLYPHONY_TONAL_DB,
        POLYPHONY_LOBE_BINS,
        POLYPHONY_STATIONARY,
        POLYPHONY_CANDIDATE_FLOOR_DB,
        POLYPHONY_SIDEBAND_BINS,
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
    // A region that never reaches the silence threshold is the inaudible end of a release.
    let threshold = 10f64.powf(SILENCE_THRESHOLD_DBFS / 20.0);
    let regions: Vec<(usize, usize)> = sounding_regions(&quiet_samples(&signal))
        .into_iter()
        .filter(|&(a, b)| {
            (a..b)
                .step_by(QUIET_RUN_FRAMES)
                .chain(std::iter::once(b - 1))
                .any(|t| signal.centred(t as isize, TAIL_WINDOW_FRAMES) >= threshold)
        })
        .collect();
    for (index, region) in regions.iter().enumerate() {
        let after_region = regions.get(index + 1).map(|r| r.0);
        let build = |extra: &[Hidden]| -> Vec<Built> {
            let onsets = region_onsets(&signal, samples, &novelty, *region, extra);
            onsets
                .iter()
                .enumerate()
                .map(|(i, onset)| {
                    let next = onsets.get(i + 1);
                    let end = next.map_or(region.1, |n| n.sample);
                    build_note(&signal, samples, onset, end, next, after_region)
                })
                .collect()
        };
        let mut built = build(&[]);
        // Step 6: a note whose level rebounds or steps hides an onset there; each is a candidate.
        let hidden: Vec<Hidden> = built
            .iter()
            .flat_map(|b| b.hidden.iter().copied())
            .collect();
        if !hidden.is_empty() {
            built = build(&hidden);
        }
        // Step 9: polyphony, on the steady part of each note's sound (to the next onset or the
        // end of its tail), and over each run of notes that may be one tone: the longer window
        // resolves near tones none of its notes resolves on its own.
        let sounds: Vec<(usize, usize)> = built
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let tail = b.note.tail_end_sample.max(b.note.end_sample);
                let end = built
                    .get(i + 1)
                    .map_or(tail, |next| tail.min(next.note.start_sample));
                (
                    b.note.start_sample as usize,
                    end.max(b.note.end_sample) as usize,
                )
            })
            .collect();
        polyphonic |= sounds
            .iter()
            .any(|&sound| polyphony(&signal, samples, sound) == Some(true));
        for (first, last) in pitch_spans(&built) {
            if first < last {
                let run = (sounds[first].0, sounds[last].1);
                polyphonic |= polyphony(&signal, samples, run) == Some(true);
            }
        }
        for b in built {
            levels.push(b.level);
            notes.push(b.note);
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
    /// `magnitude[i]` is the sum of `|x[j]|` for `j < i`.
    magnitude: Vec<f64>,
}

impl Signal {
    fn new(samples: &[f32]) -> Self {
        let x: Vec<f64> = samples.iter().map(|s| f64::from(*s)).collect();
        let mut energy = Vec::with_capacity(x.len() + 1);
        let mut magnitude = Vec::with_capacity(x.len() + 1);
        let (mut sum, mut abs) = (0.0, 0.0);
        energy.push(0.0);
        magnitude.push(0.0);
        for v in &x {
            sum += v * v;
            abs += v.abs();
            energy.push(sum);
            magnitude.push(abs);
        }
        Self {
            x,
            energy,
            magnitude,
        }
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

    /// The sum of `|x|` over `[start, end)`, samples outside the clip zero.
    fn sum_magnitudes(&self, start: isize, end: isize) -> f64 {
        let n = self.len() as isize;
        let a = start.clamp(0, n) as usize;
        let b = end.clamp(0, n) as usize;
        if b <= a {
            0.0
        } else {
            (self.magnitude[b] - self.magnitude[a]).max(0.0)
        }
    }

    /// The level at `t`: the mean magnitude of the `window` samples centred on `t` (a whole
    /// number of periods when `period` is known), averaged over [`LEVEL_PHASES`] shifts spread
    /// over one period. The mean magnitude of a tone is proportional to its amplitude, so an
    /// amplitude that changes linearly gives a level that does too (an RMS would bend); a
    /// whole-period window of a changing amplitude ripples with where its periods fall, and the
    /// average over the shifts cancels that ripple.
    fn level(&self, t: isize, window: usize, period: Option<f64>) -> f64 {
        let half = (window / 2) as isize;
        let Some(p) = period else {
            return self.sum_magnitudes(t - half, t - half + window as isize) / window as f64;
        };
        let mut sum = 0.0;
        for k in 0..LEVEL_PHASES {
            let shift = ((k as f64 + 0.5) / LEVEL_PHASES as f64 - 0.5) * p;
            let a = t - half + shift.round() as isize;
            sum += self.sum_magnitudes(a, a + window as isize);
        }
        sum / (LEVEL_PHASES * window) as f64
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

/// Which samples are quiet (step 1): with no noise floor louder than [`QUIET_AMPLITUDE`] over
/// [`NOISE_FLOOR_FACTOR`], a sample no louder than [`QUIET_AMPLITUDE`]; over such a floor, a
/// sample the RMS of the [`QUIET_RUN_FRAMES`] samples centred on which is no more than
/// [`NOISE_FLOOR_FACTOR`] times the floor's RMS. A short window's RMS of noise stays near the
/// noise's RMS whatever its amplitude distribution, where a single sample of Gaussian noise
/// exceeds any multiple of it now and then (Dc1-20).
fn quiet_samples(signal: &Signal) -> Vec<bool> {
    match noise_floor(signal) {
        Some(floor) if NOISE_FLOOR_FACTOR * floor > QUIET_AMPLITUDE => {
            let level = NOISE_FLOOR_FACTOR * floor;
            (0..signal.len())
                .map(|t| signal.centred(t as isize, QUIET_RUN_FRAMES) <= level)
                .collect()
        }
        _ => signal
            .x
            .iter()
            .map(|v| v.abs() <= QUIET_AMPLITUDE)
            .collect(),
    }
}

/// The noise floor's RMS, if the clip has one: the median RMS of the [`NOISE_FLOOR_WINDOWS`]
/// quietest of the clip's windows of [`NOISE_FLOOR_WINDOW_FRAMES`] samples, when above zero and
/// below [`crate::SILENCE_THRESHOLD_DBFS`]. Wherever a clip pauses (its lead-in, a gap, its end),
/// its quietest windows hold its noise alone; a level statistic of them assumes nothing of the
/// noise's amplitude distribution, and a clip of digital silence anywhere has a floor of zero.
fn noise_floor(signal: &Signal) -> Option<f64> {
    let ceiling = 10f64.powf(SILENCE_THRESHOLD_DBFS / 20.0);
    let w = NOISE_FLOOR_WINDOW_FRAMES;
    let mut rms: Vec<f64> = (0..signal.len() / w)
        .map(|k| signal.rms((k * w) as isize, ((k + 1) * w) as isize))
        .collect();
    if rms.is_empty() {
        return None;
    }
    rms.sort_by(f64::total_cmp);
    let floor = rms[(NOISE_FLOOR_WINDOWS / 2).min(rms.len() - 1)];
    (floor > 0.0 && floor < ceiling).then_some(floor)
}

/// The sounding regions `[start, end)` of the clip (step 1 of the method), `quiet` telling which
/// samples are quiet.
fn sounding_regions(quiet: &[bool]) -> Vec<(usize, usize)> {
    let n = quiet.len();
    let mut regions = Vec::new();
    let mut i = 0;
    while i < n {
        while i < n && quiet[i] {
            i += 1;
        }
        if i >= n {
            break;
        }
        let start = i;
        let mut run_start = None;
        let mut j = i;
        while j < n {
            if quiet[j] {
                let begun = *run_start.get_or_insert(j);
                if j + 1 - begun >= QUIET_RUN_FRAMES {
                    break;
                }
            } else {
                run_start = None;
            }
            j += 1;
        }
        let end = run_start.unwrap_or(n);
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
    /// A step of the level inside a note: the velocity changed without a rise (step 6).
    Step,
}

/// Where a built note's level shows another onset (step 6): a rebound, or a step (`step`).
#[derive(Clone, Copy, Debug)]
struct Hidden {
    at: usize,
    step: bool,
}

/// A refined onset with its confidence: `support`, the part of it that is not about the onset's
/// place, and `reach`, the half-width in samples of the interval its place lies in (0 when that
/// is not what bounds it).
#[derive(Clone, Copy, Debug)]
struct Onset {
    sample: usize,
    confidence: f64,
    support: f64,
    reach: f64,
}

impl Onset {
    /// An onset whose confidence is all support.
    fn plain(sample: usize, confidence: f64) -> Self {
        Self {
            sample,
            confidence,
            support: confidence,
            reach: 0.0,
        }
    }

    /// The confidence that the onset lies within `tolerance` samples of where it was placed, at
    /// least its confidence at the onset tolerance: what a gate or tail held to it inherits.
    fn within(&self, tolerance: usize) -> f64 {
        let placed = 1.0 - (self.reach / tolerance as f64).powf(LOCALISATION_POWER);
        self.confidence
            .max(self.support.min(placed.clamp(0.0, 1.0)))
    }
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
    extra: &[Hidden],
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
    // A rebound of the level inside a note (step 6) is a candidate too, unless one is near it.
    for e in extra.iter().map(|h| h.at) {
        if e >= start + REGION_ONSET_GUARD_FRAMES
            && e + 512 < end
            && candidates
                .iter()
                .all(|c| c.abs_diff(e) > 2 * NOVELTY_HOP_FRAMES)
        {
            candidates.push(e);
        }
    }
    candidates.sort_unstable();

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
                    let near = |step: bool| {
                        extra
                            .iter()
                            .any(|h| h.step == step && h.at.abs_diff(c) <= 2 * NOVELTY_HOP_FRAMES)
                    };
                    let rise = level_rise(signal, c, bounds[j], bounds[j + 2], period).0;
                    // A rebound of the level (step 6) is an onset the window has smeared: a smaller
                    // rise confirms it.
                    let needed = if near(false) {
                        REBOUND_RISE_DB
                    } else {
                        LEVEL_RISE_DB
                    };
                    if near(true)
                        && level_step(signal, c, (bounds[j], bounds[j + 2]), period).0 >= STEP_DB
                    {
                        Some(OnsetKind::Step)
                    } else if rise >= needed {
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
    let first = silence_onset(signal, (start, end));
    let mut onsets = vec![first];
    for (j, &c) in candidates.iter().enumerate() {
        // A candidate before the attack the region's start leads to is that attack.
        if c < first.sample + 512 {
            continue;
        }
        let previous = onsets.last().map_or(start, |o: &Onset| o.sample);
        let next = candidates.get(j + 1).copied().unwrap_or(end);
        let lo = c
            .saturating_sub(ONSET_SEARCH_FRAMES)
            .max(previous + 512)
            .min(c);
        let hi = (c + ONSET_SEARCH_FRAMES)
            .min(next.saturating_sub(512))
            .max(c);
        let bounds = (previous + 256, next);
        let onset = match kinds[j] {
            OnsetKind::Pitch => {
                let before = pitches[j].map(|p| f64::from(SAMPLE_RATE_HZ) / p.f0_hz);
                let after = pitches[j + 1].map(|p| f64::from(SAMPLE_RATE_HZ) / p.f0_hz);
                match (before, after) {
                    (Some(p1), Some(p2)) => {
                        let pitch = change_point(signal, lo, hi, p1, p2);
                        // An old note faded almost to silence predicts nothing: the onset is the new
                        // note's attack, and a change point away from it is not confident.
                        let t = pitch.sample as isize;
                        let w = ONSET_CONTRAST_FRAMES as isize;
                        let contrast = db(signal.rms(t, t + w)) - db(signal.rms(t - w, t));
                        if contrast >= TAIL_FULL_DB {
                            let level = level_onset(signal, (lo, hi), bounds, Some(p2));
                            if level.sample.abs_diff(pitch.sample) > ONSET_TOLERANCE_FRAMES {
                                Onset {
                                    confidence: pitch.confidence.min(level.confidence),
                                    support: pitch.support.min(level.support),
                                    reach: pitch
                                        .reach
                                        .max(level.sample.abs_diff(pitch.sample) as f64),
                                    ..pitch
                                }
                            } else {
                                pitch
                            }
                        } else {
                            pitch
                        }
                    }
                    (Some(p), None) | (None, Some(p)) => {
                        level_onset(signal, (lo, hi), bounds, Some(p))
                    }
                    (None, None) => level_onset(signal, (lo, hi), bounds, None),
                }
            }
            OnsetKind::Step => {
                let period = pitches[j + 1]
                    .or(pitches[j])
                    .map(|p| f64::from(SAMPLE_RATE_HZ) / p.f0_hz);
                let (drop, at) = level_step(signal, c, (previous, next), period);
                // A level that rises again out of the fall was a release into the next note's
                // attack, whose onset is that rise: the fall does not place it.
                let (again, _) = level_rise(signal, at + 512, previous, end, period);
                // The level's two lines placed the step; a fall a window away from there is a
                // decay's, and either may be the onset.
                let window = level_window(period, LEVEL_WINDOW_MIN_FRAMES, 1_024);
                let placed = extra
                    .iter()
                    .filter(|h| h.step)
                    .map(|h| h.at)
                    .min_by_key(|h| h.abs_diff(c))
                    .unwrap_or(at);
                let agree = if placed.abs_diff(at) > window {
                    AMBIGUOUS_GATE_CONFIDENCE
                } else {
                    1.0
                };
                // A fall the level rises out of by a level onset's least rise into the next onset,
                // within the reach that rise is read over, may be the release before that onset:
                // which of the two starts the note, the level does not tell.
                let released = if again >= LEVEL_RISE_DB && next <= at + 512 + 2_048 {
                    AMBIGUOUS_GATE_CONFIDENCE
                } else {
                    1.0
                };
                let support = (drop / LEVEL_RISE_FULL_DB)
                    .min(1.0 - again / LEVEL_RISE_FULL_DB)
                    .min(agree)
                    .min(released)
                    .clamp(0.0, 1.0);
                // The step lies between the two places that read it; and the windows smear a fall
                // over their length, which places it no closer than a quarter of a window, as a
                // short rise is placed (`level_onset`), and on the 8-sample grid `level_step` reads.
                let reach = (placed.abs_diff(at) as f64).max(window as f64 * KNEE_SHIFT) + 8.0;
                Onset {
                    sample: at,
                    confidence: support.min(interval_confidence(reach)),
                    support,
                    reach,
                }
            }
            OnsetKind::Level => {
                let period = pitches[j + 1]
                    .or(pitches[j])
                    .map(|p| f64::from(SAMPLE_RATE_HZ) / p.f0_hz);
                level_onset(signal, (lo, hi), bounds, period)
            }
        };
        if onset.sample > previous + 256 && onset.sample + 256 < end {
            onsets.push(onset);
        }
    }
    onsets
}

/// Where the attack of the region `[start, end)` begins. A region can begin in the inaudible end
/// of a release (its waveform's peaks above [`QUIET_AMPLITUDE`], its level below the silence
/// threshold) before the next note's attack. The level over the [`TAIL_WINDOW_FRAMES`] samples
/// before a point is read every [`ATTACK_HOP_FRAMES`] samples from the region's start to its
/// first point at which that level reaches the silence threshold; the attack begins at the last
/// of those points at which it is no more than [`NOISE_FLOOR_FACTOR`] times the least of them.
/// A region that begins with its attack begins there.
fn attack_start(signal: &Signal, (start, end): (usize, usize)) -> usize {
    let threshold = 10f64.powf(SILENCE_THRESHOLD_DBFS / 20.0);
    let before = |t: usize| signal.rms(t as isize - TAIL_WINDOW_FRAMES as isize, t as isize);
    let Some(audible) = (start..end)
        .step_by(ATTACK_HOP_FRAMES)
        .find(|&t| before(t) >= threshold)
    else {
        return start;
    };
    let points: Vec<(usize, f64)> = (start..=audible)
        .step_by(ATTACK_HOP_FRAMES)
        .map(|t| (t, before(t)))
        .collect();
    // The tail before the attack ripples with its waveform about its least level; the attack
    // begins where the level leaves that band for good, as a sample leaves the noise floor.
    let least = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    points
        .iter()
        .rev()
        .find(|p| p.1 <= NOISE_FLOOR_FACTOR * least)
        .map_or(start, |p| p.0)
}

/// The onset from silence of the region `[start, end)`, at the start of the attack its first
/// sample leads to ([`attack_start`]), with its contrast confidence.
fn silence_onset(signal: &Signal, (start, end): (usize, usize)) -> Onset {
    let start = attack_start(signal, (start, end));
    let t = start as isize;
    let w = ONSET_CONTRAST_FRAMES as isize;
    let confidence = if start == 0 {
        0.0
    } else {
        let before = db(signal.rms(t - w, t));
        let after = db(signal.rms(t, t + w));
        ((after - before) / ONSET_CONTRAST_FULL_DB).clamp(0.0, 1.0)
    };
    Onset::plain(start, confidence)
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
        let level = signal.level(t as isize, window, period);
        if level < dip.0 {
            dip = (level, t);
        }
        t += 16;
    }
    let mut peak = 0.0f64;
    let top = (dip.1.max(c) + 2_048).min(next.saturating_sub(half));
    let mut t = dip.1;
    while t <= top {
        peak = peak.max(signal.level(t as isize, window, period));
        t += 16;
    }
    (db(peak) - db(dip.0), dip.1)
}

/// The largest fall of the level, in dB, across a sample within [`STEP_REACH_FRAMES`] of `c`, and
/// that sample: the level of the window before it over the level of the window after it, both
/// whole periods and inside `(previous, next)`.
fn level_step(
    signal: &Signal,
    c: usize,
    (previous, next): (usize, usize),
    period: Option<f64>,
) -> (f64, usize) {
    let window = level_window(period, LEVEL_WINDOW_MIN_FRAMES, 1_024);
    let half = window / 2;
    let lo = c.saturating_sub(STEP_REACH_FRAMES).max(previous + window);
    let hi = (c + STEP_REACH_FRAMES).min(next.saturating_sub(window));
    let mut best = (f64::NEG_INFINITY, c);
    let mut t = lo;
    while t <= hi {
        let before = signal.level(t as isize - half as isize, window, period);
        let after = signal.level((t + half) as isize, window, period);
        let drop = db(before) - db(after);
        if drop > best.0 {
            best = (drop, t);
        }
        t += 8;
    }
    best
}

/// The onset by level within `[lo, hi]` (step 4 of the method): the knee where the level starts
/// to rise, read from the level over `[from, to)` (the neighbouring onsets' side of it excluded).
///
/// The level (the RMS over whole periods, at least [`LEVEL_WINDOW_MIN_FRAMES`] samples, sampled
/// every [`KNEE_STEP_FRAMES`]) is fitted by three lines, before, rising and after; the onset is
/// where the first two meet. The window smears an attack shorter than itself into a rise one
/// window long whose lines meet up to a window early; a rise no longer than
/// [`KNEE_SHORT_RISE`] windows is moved a quarter window later, half the possible error.
/// Its confidence is the prominence of the knee: one minus the fit's error over the least
/// error of a fit whose knee lies more than the onset tolerance away from it, and no more than the
/// level's rise allows ([`LEVEL_RISE_FULL_DB`]).
fn level_onset(
    signal: &Signal,
    (lo, hi): (usize, usize),
    (from, to): (usize, usize),
    period: Option<f64>,
) -> Onset {
    let window = level_window(period, LEVEL_WINDOW_MIN_FRAMES, 1_024);
    let half = window / 2;
    let first = lo.saturating_sub(window).max(from + half);
    let last = (hi + KNEE_REACH_FRAMES).min(to.saturating_sub(half));
    let mut points = Vec::new();
    let mut t = first;
    while t <= last {
        points.push((t as f64, signal.level(t as isize, window, period)));
        t += KNEE_STEP_FRAMES;
    }
    let m = MIN_SEGMENT_POINTS;
    let n = points.len();
    if n < 3 * m {
        return Onset::plain((lo + hi) / 2, 0.0);
    }
    let sums = Sums::new(&points);
    // (cost, knee, rise end, lines) of the best fit, and every fit's knee and cost for the
    // localisation.
    let mut best: Option<(f64, f64, f64, Line, Line)> = None;
    let mut fits: Vec<(f64, f64)> = Vec::new();
    for i in m..=n - 2 * m {
        let before = sums.line(0, i);
        for (j, point) in points.iter().enumerate().take(n - m + 1).skip(i + m) {
            let rise = sums.line(i, j);
            if rise.b <= before.b || rise.b <= 0.0 {
                continue;
            }
            let after = sums.line(j, n);
            let cost = before.error + rise.error + after.error;
            let Some(knee) = intersection(before, rise) else {
                continue;
            };
            if knee < lo as f64 || knee > hi as f64 {
                continue;
            }
            let end = intersection(rise, after).unwrap_or(point.0);
            fits.push((knee, cost));
            if best.is_none_or(|b| cost < b.0) {
                best = Some((cost, knee, end, before, rise));
            }
        }
    }
    let Some((cost, knee, end, before, rise_line)) = best else {
        return Onset::plain((lo + hi) / 2, 0.0);
    };
    // The phases spread the window over one more period: the level's kernel is that wide.
    let spread =
        window as f64 + period.unwrap_or(0.0) * (LEVEL_PHASES - 1) as f64 / LEVEL_PHASES as f64;
    let sample = if end - knee <= KNEE_SHORT_RISE * spread {
        knee + spread * KNEE_SHIFT
    } else {
        knee
    };
    // The onset lies among the knees of the fits as good as the best, within the noise of its
    // error (the points hold one independent observation a window), the best knee itself when
    // the onset is moved off it, and where the level first leaves the line before the knee by
    // more than that noise: a level that falls before it rises began its note there.
    let observations = (n * KNEE_STEP_FRAMES) as f64 / window as f64;
    let noise = LOCALISATION_CHI2.sqrt() * (cost.max(0.0) / n as f64).sqrt();
    let departure = points
        .iter()
        .filter(|p| p.0 >= knee - 2.0 * spread && p.0 <= knee)
        .find(|p| before.a + before.b * p.0 - p.1 > noise)
        .map_or(knee, |p| p.0);
    // How far the level lies below its lines about the knee (the line before it, then the
    // rising line, up to half a kernel before the rise's top): a velocity that fell as the note
    // began. The kernel reads a fall of depth `d` from half its spread before the fall, in
    // proportion to the part of the kernel past it, so the first point it moves by more than the
    // noise lies `spread (noise / d) - spread / 2` after the fall began: a fall seen barely above
    // the noise may have begun up to half a kernel before that point. A fall first seen past the
    // knee is read from the knee: the rise it hides in starts there.
    let below: Vec<(f64, f64)> = points
        .iter()
        .filter(|p| p.0 >= knee - 2.0 * spread && p.0 <= (knee + spread).min(end - spread / 2.0))
        .map(|p| {
            let line = if p.0 < knee { before } else { rise_line };
            (p.0, line.a + line.b * p.0 - p.1)
        })
        .collect();
    let deepest = below.iter().map(|p| p.1).fold(0.0f64, f64::max);
    let began = below.iter().find(|p| p.1 > noise).map_or(departure, |p| {
        p.0.min(knee) + spread / 2.0 - spread * (noise / deepest).min(1.0)
    });
    let (reach, _) = localisation_parts(
        fits.iter()
            .copied()
            .chain([(knee, cost), (departure, cost), (began, cost)]),
        (sample, cost),
        observations,
        KNEE_STEP_FRAMES as f64,
    );
    // The competing explanations are the other fits, each knee judged from the best fit's own:
    // the shift off the knee and the departure widen the interval, they are not other fits.
    let prominent = prominence(fits.iter().copied(), (knee, cost));
    let localised = interval_confidence(reach).min(prominent);
    // A smoothed corner lies above its two lines; a level below them at the knee dipped first,
    // a velocity that fell as the note began: the knee need not be its onset. The rise's top is
    // a corner the other way, which the smoothed level lies below for half a kernel before it,
    // and past it the level follows the line after: neither is read against the rising line.
    let top = signal
        .level(end.round() as isize, window, period)
        .max(1e-12);
    let dip = points
        .iter()
        .filter(|p| p.0 >= knee - 2.0 * spread && p.0 <= (knee + spread).min(end - spread / 2.0))
        .map(|p| {
            let line = if p.0 < knee { before } else { rise_line };
            (line.a + line.b * p.0 - p.1) / top
        })
        .fold(0.0, f64::max);
    let plain = (1.0 - dip / (4.0 * STEP_FRACTION)).clamp(0.0, 1.0);
    // A slow attack bends into its top over its own length: its knee is no sharper than that.
    let attack = (end - knee - spread).max(0.0);
    let slow = if attack > 0.0 {
        (KNEE_ATTACK_FULL_FRAMES / attack).min(1.0)
    } else {
        1.0
    };
    let level_at = |t: f64| signal.level(t.round() as isize, window, period);
    let rise = db(level_at(end)) - db(level_at(knee - half as f64));
    // A level already rising into the knee from the onset before it, by a level onset's least
    // rise, never peaked between them: one attack the two onsets cut, and which of them starts
    // it, the level does not tell. Read half a kernel before the knee, clear of its smear.
    let settled = level_at(knee - spread / 2.0);
    let mut earlier = level_at(from as f64);
    let mut t = from as f64;
    while t <= knee - 1.5 * spread {
        earlier = earlier.max(level_at(t));
        t += KNEE_STEP_FRAMES as f64;
    }
    let continues = if db(settled) - db(earlier) >= LEVEL_RISE_DB {
        AMBIGUOUS_GATE_CONFIDENCE
    } else {
        1.0
    };
    // A knee is as visible as the slope changes there (as a gate's knee is): a level that
    // already rose as steeply before it bends nowhere.
    let visible = ((rise_line.b - before.b) / rise_line.b.abs().max(before.b.abs()).max(1e-300))
        .clamp(0.0, 1.0);
    let support = prominent
        .min((rise / LEVEL_RISE_FULL_DB).clamp(0.0, 1.0))
        .min(slow)
        .min(plain)
        .min(continues)
        .min(visible);
    Onset {
        sample: sample.round().max(0.0) as usize,
        confidence: localised.min(support),
        support,
        reach,
    }
}

/// The onset by pitch: the change point of the periodic prediction within `[lo, hi]` (step 4).
/// The number of independent observations in a residual series: its length over
/// `1 + 2 Σ ρ_k`, `ρ_k` its autocorrelation at lag `k`, summed while positive.
fn independent_observations(residual: &[f64]) -> f64 {
    let n = residual.len();
    let energy: f64 = residual.iter().map(|v| v * v).sum();
    if energy <= 0.0 {
        return n.max(1) as f64;
    }
    let mut sum = 0.0;
    for k in 1..n / 2 {
        let r = residual[..n - k]
            .iter()
            .zip(&residual[k..])
            .map(|(a, b)| a * b)
            .sum::<f64>()
            / energy;
        if r <= 0.0 {
            break;
        }
        sum += r;
    }
    (n as f64 / (1.0 + 2.0 * sum)).max(1.0)
}

fn change_point(signal: &Signal, lo: usize, hi: usize, p1: f64, p2: f64) -> Onset {
    let w = CHANGE_POINT_WINDOW_FRAMES;
    let margin = 256;
    let longer = p1.max(p2);
    let first = lo.saturating_sub(w + margin);
    let last = (hi + w + margin).min(signal.len());
    let len = last - first;
    let mut back = vec![0.0; len + 1];
    let mut forward = vec![0.0; len + 1];
    let mut power = vec![0.0; len + 1];
    let mut signed = vec![(0.0, 0.0); len];
    for i in 0..len {
        let n = first + i;
        let v = signal.x[n];
        // Each prediction scaled by the level a period away: an attack or a release changes
        // the amplitude, not the period.
        let gain = |p: f64, shifted: f64| {
            let w = p.round().max(1.0) as isize;
            let level = |c: f64| {
                signal.sum_magnitudes(c.round() as isize - w / 2, c.round() as isize - w / 2 + w)
            };
            let there = level(shifted);
            if there > 0.0 {
                (level(n as f64) / there).clamp(0.25, 4.0)
            } else {
                1.0
            }
        };
        let b = v - gain(p1, n as f64 - p1) * signal.at(n as f64 - p1);
        let f = v - gain(p2, n as f64 + p2) * signal.at(n as f64 + p2);
        back[i + 1] = back[i] + b * b;
        forward[i + 1] = forward[i] + f * f;
        power[i + 1] = power[i] + v * v;
        signed[i] = (b, f);
    }
    // The whole span split at `t`: the old period predicts before it, the new one after it.
    let cost = |t: usize| -> f64 {
        let m = t.clamp(first, last) - first;
        let numerator = back[m] + (forward[len] - forward[m]);
        numerator / (power[len] + 1e-12)
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
        let one_sided: Box<dyn Fn(usize) -> f64> = if t <= lo + edge {
            // The new period predicts the old note: the old period's error rises at the onset.
            Box::new(|t| {
                let (before, after) = sums(&back, t);
                before / (after + floor)
            })
        } else {
            // The old period predicts the new note: the new period's error falls at the onset.
            Box::new(|t| {
                let (before, after) = sums(&forward, t);
                after / (before + floor)
            })
        };
        let mut best = (f64::INFINITY, lo);
        for t in lo..=hi {
            let c = one_sided(t);
            if c < best.0 {
                best = (c, t);
            }
        }
        let t = best.1;
        // The two windows hold one independent observation a period of the longer one.
        let (reach, prominent) = localisation_parts(
            (lo..=hi).map(|u| (u as f64, one_sided(u))),
            (t as f64, best.0),
            2.0 * w as f64 / longer,
            1.0,
        );
        // Within a period of the search's edge, the step may lie beyond it.
        let reach = if t < lo + longer.ceil() as usize || t + longer.ceil() as usize > hi {
            f64::INFINITY
        } else {
            reach
        };
        return Onset {
            sample: t,
            confidence: interval_confidence(reach).min(prominent),
            support: prominent,
            reach,
        };
    }
    // The span's cost steps at the waveform's edges, a period apart: it places the onset to a
    // period. The residuals themselves place it to the sample.
    let prominence = prominence((lo..=hi).map(|u| (u as f64, cost(u))), (t as f64, best.0));
    // The cost's interval: every split within LOCALISATION_CHI2 noise variances of the least,
    // the variance the least over the residual's independent observations (Dc1-18).
    let split = t.clamp(first, last) - first;
    let residual: Vec<f64> = signed
        .iter()
        .enumerate()
        .map(|(i, (b, f))| if i < split { *b } else { *f })
        .collect();
    let observations = independent_observations(&residual);
    let margin = LOCALISATION_CHI2 * best.0.max(0.0) / observations;
    let interval = (lo..=hi)
        .filter(|&u| cost(u) <= best.0 + margin)
        .fold((t, t), |(a, b), u| (a.min(u), b.max(u)));
    // Refined on the sample grid inside that interval; unrefined, the cost's own interval.
    let refined = refine(signal, interval, p1, p2);
    let (sample, reach, prominent) = match refined {
        Some((sample, reach)) => (sample, reach + 1.0, 1.0),
        None => {
            let (reach, prominent) = localisation_parts(
                (lo..=hi).map(|u| (u as f64, cost(u))),
                (t as f64, best.0),
                observations,
                1.0,
            );
            (t, reach, prominent)
        }
    };
    let localised = interval_confidence(reach).min(prominent);
    // The residuals over a period either side of the cost's onset place it too, and the
    // interval holds that placement; where the refinement inside the cost's interval failed,
    // it holds every crossing of that wider search as well.
    let wide = (
        t.saturating_sub(longer.ceil() as usize).max(lo),
        (t + longer.ceil() as usize).min(hi),
    );
    let apart = refine(signal, wide, p1, p2).map_or(0.0, |(at, wide_reach)| {
        let spread = if refined.is_some() { 0.0 } else { wide_reach };
        at.abs_diff(sample) as f64 + spread + 1.0
    });
    let agreement = if apart > 0.0 {
        interval_confidence(apart)
    } else {
        1.0
    };
    // A change point within a period of the edge of its search may lie beyond it: the search
    // bounds its interval on that side, the evidence does not.
    let edged = t < lo + longer.ceil() as usize || t + longer.ceil() as usize > hi;
    let reach = if edged {
        f64::INFINITY
    } else {
        reach.max(apart)
    };
    Onset {
        sample,
        confidence: localised
            .min(prominence)
            .min(agreement)
            .min(interval_confidence(reach)),
        support: prominent.min(prominence),
        reach,
    }
}

/// The confidence of an onset whose interval has half-width `h` samples (Dc1-15):
/// `1 - (h / ONSET_TOLERANCE_FRAMES)^LOCALISATION_POWER`, clamped.
fn interval_confidence(h: f64) -> f64 {
    interval_confidence_at(h, ONSET_TOLERANCE_FRAMES)
}

/// `1 - (h / tolerance)^LOCALISATION_POWER`, clamped: the confidence of a placement whose
/// interval reaches `h` samples, against `tolerance`.
fn interval_confidence_at(h: f64, tolerance: usize) -> f64 {
    (1.0 - (h.max(0.0) / tolerance as f64).powf(LOCALISATION_POWER)).clamp(0.0, 1.0)
}

/// The level's kernel ([`Signal::level`]): a box of `window` samples at [`LEVEL_PHASES`] shifts
/// spread over a period, or one box without a period.
struct LevelKernel {
    window: f64,
    /// Each box's centre relative to the point the level is read at.
    centres: Vec<f64>,
}

impl LevelKernel {
    fn new(window: usize, period: Option<f64>) -> Self {
        let half = (window / 2) as f64;
        let centre = (window as f64 - 1.0) / 2.0 - half;
        let centres = match period {
            Some(p) => (0..LEVEL_PHASES)
                .map(|k| {
                    let shift = ((k as f64 + 0.5) / LEVEL_PHASES as f64 - 0.5) * p;
                    shift.round() + centre
                })
                .collect(),
            None => vec![centre],
        };
        Self {
            window: window as f64,
            centres,
        }
    }

    /// The level a release ramp `max(0, t - g)` reads at `x = t - g` through this kernel.
    fn ramp(&self, x: f64) -> f64 {
        self.centres
            .iter()
            .map(|c| smoothed_ramp(x + c, self.window))
            .sum::<f64>()
            / self.centres.len() as f64
    }
}

/// The level a release ramp leaves after a box of `spread` samples has smoothed it: `max(0, x)`
/// averaged over the `spread` samples centred on `x`.
fn smoothed_ramp(x: f64, spread: f64) -> f64 {
    let half = spread / 2.0;
    if x <= -half {
        0.0
    } else if x >= half {
        x
    } else {
        (x + half) * (x + half) / (2.0 * spread)
    }
}

/// The earliest start of a release seen at the end of a held level (step 6), `None` when none is
/// seen. The points (the level every [`ENVELOPE_STEP_FRAMES`] samples, read through `kernel`,
/// `spread` samples wide) are fitted by a line, and by a line less a
/// release ramp smoothed by the kernel, `y = a + b t - c R(t - g)` with `c > 0`, for every `g`
/// on a grid of [`KNEE_STEP_FRAMES`] samples up to `end`. A release is seen when the line alone
/// costs more than [`LOCALISATION_CHI2`] noise variances above the best ramp (the variance the
/// best cost over the points' independent observations, one a window); its earliest start is the
/// least `g` whose cost lies within that margin of the best.
fn release_departure(
    points: &[(f64, f64)],
    kernel: &LevelKernel,
    spread: f64,
    end: f64,
) -> Option<f64> {
    let n = points.len();
    if n < 2 * MIN_SEGMENT_POINTS {
        return None;
    }
    let line = Sums::new(points).line(0, n).error;
    let first = points[0].0;
    let mut fits: Vec<(f64, f64)> = Vec::new();
    let mut g = first;
    while g < end {
        if let Some(cost) = ramp_cost(points, g, kernel, spread) {
            fits.push((g, cost));
        }
        g += KNEE_STEP_FRAMES as f64;
    }
    let best = fits.iter().map(|f| f.1).fold(f64::INFINITY, f64::min);
    if !best.is_finite() {
        return None;
    }
    let observations = (n * ENVELOPE_STEP_FRAMES) as f64 / kernel.window;
    let margin = LOCALISATION_CHI2 * best.max(0.0) / observations.max(1.0);
    if line <= best + margin {
        return None;
    }
    fits.iter()
        .filter(|f| f.1 <= best + margin)
        .map(|f| f.0)
        .reduce(f64::min)
}

/// The squared error of the least-squares fit `y = a + b t + c R(t - g)` to `points`, `R` the
/// kernel's ramp ([`LevelKernel::ramp`]); `None` when the ramp is not seen or the fit would
/// raise the level (`c >= 0`).
fn ramp_cost(points: &[(f64, f64)], g: f64, kernel: &LevelKernel, spread: f64) -> Option<f64> {
    // Centred times keep the normal equations well conditioned.
    let t0 = points[0].0;
    let mut m = [[0.0f64; 3]; 3];
    let mut v = [0.0f64; 3];
    let mut yy = 0.0;
    let mut seen = 0usize;
    for &(t, y) in points {
        let r = kernel.ramp(t - g);
        seen += usize::from(r > 0.0);
        let basis = [1.0, (t - t0) / spread, r / spread];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += basis[i] * basis[j];
            }
            v[i] += basis[i] * y;
        }
        yy += y * y;
    }
    if seen < MIN_SEGMENT_POINTS {
        return None;
    }
    let c = solve(m, v)?;
    if c[2] >= 0.0 {
        return None;
    }
    // The residual of a least-squares fit: |y|^2 - c . v.
    Some((yy - (c[0] * v[0] + c[1] * v[1] + c[2] * v[2])).max(0.0))
}

/// The solution of the `N` by `N` system `m x = v` by Gaussian elimination with partial
/// pivoting; `None` when `m` is singular.
fn solve<const N: usize>(mut m: [[f64; N]; N], mut v: [f64; N]) -> Option<[f64; N]> {
    let scale = m.iter().flatten().fold(0.0f64, |s, x| s.max(x.abs()));
    if scale <= 0.0 {
        return None;
    }
    for col in 0..N {
        let pivot = (col..N).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[pivot][col].abs() <= 1e-12 * scale {
            return None;
        }
        m.swap(col, pivot);
        v.swap(col, pivot);
        for row in col + 1..N {
            let f = m[row][col] / m[col][col];
            let pivot_row = m[col];
            for (cell, p) in m[row].iter_mut().zip(pivot_row).skip(col) {
                *cell -= f * p;
            }
            v[row] -= f * v[col];
        }
    }
    let mut x = [0.0; N];
    for row in (0..N).rev() {
        let mut s = v[row];
        for k in row + 1..N {
            s -= m[row][k] * x[k];
        }
        x[row] = s / m[row][row];
    }
    Some(x)
}

/// The onset by pitch on the sample grid (Dc1-18), searched over `[from, to]`, and the
/// half-width of its interval; `None` when a prediction does not tell the two notes apart or the
/// residuals do not cross for good inside the span.
///
/// Two predictions, each scaled by the level over the period on its own side only, so neither
/// reads across the onset: the old note's periodic continuation `x[n] ≈ g x[n - T_old]`, `g` the
/// level over the period before `n` over the level over the period before that, and the new
/// note's backward continuation `x[n] ≈ g x[n + T_new]`, `g` the level over the period from `n`
/// over the level over the period after that. Each residual `|x[n] - g x[n ∓ T]|` is read as its
/// median over the [`REFINE_WINDOW_FRAMES`] samples from `n` (a median, so the waveform's edges,
/// a few samples a period, do not decide).
///
/// Each prediction must tell the notes apart: relative to the signal's median magnitude, its
/// residual where the note it does not continue sounds alone (a period beyond the span) is at
/// least [`LOCALISATION_CHI2`] times its residual where its own note sounds alone. The residuals
/// cross where the new prediction's is no longer above the old one's for a whole window. The
/// shorter lag also continues the longer period's smoother note between its edges, so its
/// prediction can win early (a lower old note) or lose late (a lower new note): with the old
/// period the longer the onset is the start of the last crossing, from which the new prediction
/// holds to the end of the span; otherwise the first crossing that holds for a window. The
/// interval spans both crossings.
fn refine(signal: &Signal, (from, to): (usize, usize), p1: f64, p2: f64) -> Option<(usize, f64)> {
    let s = REFINE_WINDOW_FRAMES;
    let t1 = p1.round().max(1.0) as isize;
    let t2 = p2.round().max(1.0) as isize;
    let alone = (4 * s).max(p1.max(p2).ceil() as usize);
    let start = from.checked_sub(alone)?;
    let end = to + 1 + alone + s;
    if (start as isize) < 2 * t1 || end as isize + 2 * t2 > signal.len() as isize {
        return None;
    }
    let mags = |a: isize, b: isize| signal.sum_magnitudes(a, b);
    let gain = |now: f64, then: f64| {
        if then > 0.0 {
            (now / then).clamp(0.25, 4.0)
        } else {
            1.0
        }
    };
    // The residuals and magnitudes over [start, end).
    let samples: Vec<(f64, f64, f64)> = (start..end)
        .map(|n| {
            let i = n as isize;
            let v = signal.x[n];
            let old = gain(mags(i - t1, i), mags(i - 2 * t1, i - t1));
            let new = gain(mags(i, i + t2), mags(i + t2, i + 2 * t2));
            (
                (v - old * signal.x[(i - t1) as usize]).abs(),
                (v - new * signal.x[(i + t2) as usize]).abs(),
                v.abs(),
            )
        })
        .collect();
    let median = |mut values: Vec<f64>| -> f64 {
        values.sort_by(f64::total_cmp);
        values[values.len() / 2]
    };
    let floor = 1e-9;
    let first = from - start;
    let last = to - start;
    // The relative residuals of the windows where each note sounds alone.
    let relative = |k: usize| {
        let window = &samples[k..k + s];
        let level = median(window.iter().map(|r| r.2).collect()) + floor;
        (
            median(window.iter().map(|r| r.0).collect()) / level,
            median(window.iter().map(|r| r.1).collect()) / level,
        )
    };
    let before: Vec<(f64, f64)> = (0..first).map(relative).collect();
    let after: Vec<(f64, f64)> = (last + 1..last + 1 + alone).map(relative).collect();
    let tells = |pick: fn(&(f64, f64)) -> f64, fails_on_new: bool| {
        let on_old = median(before.iter().map(pick).collect()) + floor;
        let on_new = median(after.iter().map(pick).collect()) + floor;
        let (fail, hold) = if fails_on_new {
            (on_new, on_old)
        } else {
            (on_old, on_new)
        };
        fail >= LOCALISATION_CHI2 * hold
    };
    if !tells(|r| r.0, true) || !tells(|r| r.1, false) {
        return None;
    }
    // Whether the new prediction's window from each sample of [from, to + s] is no worse.
    let window = |k: usize, pick: fn(&(f64, f64, f64)) -> f64| {
        median(samples[k..k + s].iter().map(pick).collect())
    };
    let crossed: Vec<bool> = (first..=last + s)
        .map(|k| window(k, |r| r.1) <= window(k, |r| r.0))
        .collect();
    let early = (0..=last - first).find(|&k| crossed[k..=k + s].iter().all(|c| *c))?;
    let mut late = crossed.len();
    while late > 0 && crossed[late - 1] {
        late -= 1;
    }
    if late > last - first {
        return None;
    }
    let at = if p1 >= p2 { late } else { early };
    Some((from + at, (late - early) as f64))
}

/// The localisation of an onset (Dc1-15): `positions` are the candidate onsets with their
/// costs, `best` the least of them at `at`, `observations` the number of independent
/// observations the cost sums and `resolution` the evidence's own resolution in samples. The
/// interval is every position whose cost lies within [`LOCALISATION_CHI2`] noise variances
/// (`best / observations`) of the least; returns its half-width `h` about `at`, widened by
/// `resolution` (whose confidence is [`interval_confidence`]), and the prominence
/// `1 - best / c`, `c` the least cost of a position further than the tolerance from `at` (a
/// competing onset).
fn localisation_parts(
    positions: impl Iterator<Item = (f64, f64)>,
    (at, best): (f64, f64),
    observations: f64,
    resolution: f64,
) -> (f64, f64) {
    let margin = LOCALISATION_CHI2 * best.max(0.0) / observations.max(1.0);
    let positions: Vec<(f64, f64)> = positions.collect();
    let prominence = prominence(positions.iter().copied(), (at, best));
    let reach = positions
        .iter()
        .filter(|(_, cost)| *cost <= best + margin)
        .map(|(t, _)| (t - at).abs())
        .fold(0.0f64, f64::max);
    (reach + resolution, prominence)
}

/// The prominence of the best of `positions` (candidate onsets with their costs), `best` at `at`:
/// `1 - best / c`, clamped, `c` the least cost of a position more than the onset tolerance away
/// (a competing onset); 1 when there is none.
fn prominence(positions: impl Iterator<Item = (f64, f64)>, (at, best): (f64, f64)) -> f64 {
    let competing = positions
        .filter(|(t, _)| (t - at).abs() > ONSET_TOLERANCE_FRAMES as f64)
        .map(|(_, cost)| cost)
        .fold(f64::INFINITY, f64::min);
    if competing.is_finite() && competing > 0.0 {
        (1.0 - best / competing).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// One note as built, before velocities are known.
struct Built {
    note: EstimatedNote,
    level: f64,
    /// Where the level rebounds by at least [`REBOUND_FRACTION`] of its peak, or steps by
    /// [`STEP_FRACTION`]: hidden onsets.
    hidden: Vec<Hidden>,
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
    // The spectrum's fundamental refines a pitch it agrees with; one that names another pitch
    // leaves the period's pitch without support.
    let (f0_hz, pitch_confidence) = match spectral_f0(signal, (start, end)) {
        Some(f) if f0_hz > 0.0 && cents_between(f, f0_hz).abs() < PITCH_CHANGE_CENTS => {
            (f, pitch_confidence)
        }
        Some(_) if f0_hz > 0.0 => (f0_hz, 0.0),
        _ => (f0_hz, pitch_confidence),
    };
    let period = (f0_hz > 0.0).then(|| f64::from(SAMPLE_RATE_HZ) / f0_hz);
    let midi_note = nearest_midi(f0_hz);

    // A note cut by the next onset sounds alone only up to the earliest that onset may lie: its
    // sample less its interval's half-width.
    let reach = match next {
        Some(onset) => end - onset.reach.max(0.0).min((end - start) as f64) as usize,
        None => after_region
            .unwrap_or(signal.len())
            .min(end + TAIL_WINDOW_FRAMES),
    };
    let gate = gate_end(signal, (start, end), next.is_some(), reach, period);
    let gate_sample = gate.sample.clamp(start + 1, end);
    // A gate held to the next onset is only as certain as that onset.
    let gate_confidence = match next {
        Some(n) if gate_sample == end => gate.confidence.min(n.within(GATE_TOLERANCE_FRAMES)),
        _ => gate.confidence,
    };
    // A level that rises again after its peak holds an onset the note does not know of: its gate
    // and tail may belong to the note after it.
    let unexplained = (1.0 - gate.rebound.0 / REBOUND_FRACTION).clamp(0.0, 1.0);
    let gate_confidence = gate_confidence.min(unexplained);

    // Tail: the last crossing of the silence threshold, capped at the next onset (in this region
    // or the next).
    let cap = next
        .map(|n| n.sample)
        .or(after_region)
        .unwrap_or(signal.len())
        .max(gate_sample);
    // The next onset may be placed up to its tolerance late: the level is read no nearer to it
    // than that, so the next note's attack is not this note's sound.
    let edge = match next {
        Some(_) => cap.saturating_sub(ONSET_TOLERANCE_FRAMES).max(start + 1),
        None => cap,
    };
    let threshold = 10f64.powf(SILENCE_THRESHOLD_DBFS / 20.0);
    // The tail ends after the last sample above the threshold: a level that dips below it and
    // comes back has not ended (Dc1-9, Dc1-21). The window stops at the edge.
    let above = |t: usize| {
        let from = t as isize - (TAIL_WINDOW_FRAMES / 2) as isize;
        let to = (from + TAIL_WINDOW_FRAMES as isize).min(edge as isize);
        signal.rms(from, to) >= threshold
    };
    let crossing = match (start..edge).rev().find(|&t| above(t)) {
        Some(t) if t + 1 == edge => None,
        Some(t) => Some(t + 1),
        None => Some(start + 1),
    };
    // A sound that falls silent before the gate end found for it was not heard to end its gate:
    // the gate ends there, inaudibly, at an unknown later sample.
    let (end_sample, gate_confidence) = match crossing {
        Some(t) if t < gate_sample => (t.max(start + 1), 0.0),
        _ => (gate_sample, gate_confidence),
    };
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
            Some(n) => (cap, n.within(TAIL_TOLERANCE_FRAMES)),
            None if after_region.is_some() => (cap, 1.0),
            None => (cap, 0.0),
        },
    };
    // A release cut by the next onset is one falling line; a level that bends on the way, or
    // stays, is another note between them.
    let bend = match (crossing, next) {
        (None, Some(_)) => release_bend(signal, end_sample, cap, period, true),
        (None, None) if after_region.is_some() => {
            release_bend(signal, end_sample, cap, period, true)
        }
        (Some(t), _) => release_bend(signal, end_sample, t, period, false),
        (None, None) => 0.0,
    };
    let tail_confidence = tail_confidence.min((1.0 - bend / REBOUND_FRACTION).clamp(0.0, 1.0));
    let cover = period.map_or(0.0, |p| pitch_cover(signal, (start, end_sample), p));
    // A stretch of the gate the note's period does not predict is another note in it: the pitch,
    // gate and tail may be either's.
    let foreign = (1.0 - (cover - FOREIGN_MISMATCH) / FOREIGN_MISMATCH).clamp(0.0, 1.0);

    Built {
        note: EstimatedNote {
            start_sample: start as u64,
            end_sample: end_sample as u64,
            tail_end_sample: tail_end_sample.max(end_sample) as u64,
            midi_note,
            f0_hz,
            velocity: 0.0,
            onset_confidence: onset.confidence,
            pitch_confidence: pitch_confidence.min(foreign),
            gate_confidence: gate_confidence.min(foreign),
            tail_confidence: tail_confidence.min(unexplained).min(foreign),
        },
        level: gate.level,
        hidden: [
            (gate.rebound.2 >= REBOUND_FRACTION).then_some(Hidden {
                at: gate.rebound.1,
                step: false,
            }),
            (gate.step.0 >= STEP_FRACTION).then_some(Hidden {
                at: gate.step.1,
                step: true,
            }),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

/// How badly the note's period predicts its worst window (step 5): the largest
/// [`period_mismatch`] of the windows of [`PITCH_COVER_FRAMES`] samples (at least two periods)
/// every half window over `[start, gate)` whose RMS is at least a tenth of the loudest one's.
fn pitch_cover(signal: &Signal, (start, gate): (usize, usize), period: f64) -> f64 {
    let w = PITCH_COVER_FRAMES.max((2.0 * period).ceil() as usize + 1);
    let mut windows = Vec::new();
    let mut a = start;
    while a + w <= gate {
        windows.push((a, signal.rms(a as isize, (a + w) as isize)));
        a += w / 2;
    }
    let loudest = windows.iter().map(|x| x.1).fold(0.0, f64::max);
    windows
        .iter()
        .filter(|x| x.1 >= 0.1 * loudest && x.1 > 0.0)
        .map(|&(a, _)| period_mismatch(signal, (a, a + w), period))
        .fold(0.0, f64::max)
}

/// How far the level between a gate end and the next onset strays from one line, over the
/// level where the release starts: 0 for a release (a linear fall) or too short a stretch to
/// tell. The gate's slack and the window's reach of either end are left out.
fn release_bend(signal: &Signal, gate: usize, next: usize, period: Option<f64>, meet: bool) -> f64 {
    let window = level_window(period, ENVELOPE_WINDOW_MIN_FRAMES, 1_024);
    let half = window / 2;
    let from = gate + GATE_SLACK_FRAMES as usize + half;
    let to = next.saturating_sub(window + half);
    let mut points = Vec::new();
    let mut t = from;
    while t <= to {
        points.push((t as f64, signal.level(t as isize, window, period)));
        t += ENVELOPE_STEP_FRAMES;
    }
    if points.len() < MIN_SEGMENT_POINTS {
        return 0.0;
    }
    let reference = signal.level(gate as isize, window, period);
    if reference <= 0.0 {
        return 0.0;
    }
    let line = Sums::new(&points).line(0, points.len());
    let mut worst = points
        .iter()
        .map(|p| (p.1 - line.a - line.b * p.0).abs())
        .fold(0.0, f64::max);
    // The release starts where the level before the gate ends: the line before it and the
    // release line meet within the slack and the window's reach of the gate.
    let reach = GATE_SLACK_FRAMES as usize + window;
    let mut before = Vec::new();
    let mut t = gate.saturating_sub(reach + 3 * window);
    while t + reach <= gate {
        if t >= half {
            before.push((t as f64, signal.level(t as isize, window, period)));
        }
        t += ENVELOPE_STEP_FRAMES;
    }
    if meet && before.len() >= MIN_SEGMENT_POINTS {
        let held = Sums::new(&before).line(0, before.len());
        let gap = |t: f64| held.a + held.b * t - line.a - line.b * t;
        let (g1, g2) = (
            gap(gate.saturating_sub(reach) as f64),
            gap((gate + reach) as f64),
        );
        if g1.signum() == g2.signum() {
            worst = worst.max(g1.abs().min(g2.abs()));
        }
    }
    worst / reference
}

/// A gate end with its confidence and the note's peak level over its gate.
struct Gate {
    sample: usize,
    confidence: f64,
    level: f64,
    /// The largest rise of the level above its lowest point since the peak, over the peak level,
    /// and the sample of that lowest point.
    /// The share of it a single envelope leaves unexplained (the rebound or the body's
    /// residual), and the rebound alone.
    rebound: (f64, usize, f64),
    /// The largest gap between the body's two lines at their knee, over the peak level, and
    /// the knee.
    step: (f64, usize),
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
    let spread =
        window as f64 + period.unwrap_or(0.0) * (LEVEL_PHASES - 1) as f64 / LEVEL_PHASES as f64;
    // A note cut by the next onset is read only where the window does not reach that onset at the
    // earliest it may lie (`reach`; the level at a point reads up to half the spread after it):
    // beyond, the next attack, not this note, moves the level, and a release it overlaps hides
    // under that rise.
    let mut points = Vec::new();
    let mut t = start + half;
    while t + half <= reach && t <= end && (!cut || t as f64 + spread / 2.0 <= reach as f64) {
        points.push((t as f64, signal.level(t as isize, window, period)));
        t += ENVELOPE_STEP_FRAMES;
    }
    let fallback = |level: f64| Gate {
        sample: end,
        confidence: 0.0,
        level,
        rebound: (0.0, end, 0.0),
        step: (0.0, end),
    };
    if points.len() < 2 * MIN_SEGMENT_POINTS {
        let level = signal.centred(((start + end) / 2) as isize, (end - start).max(1));
        return fallback(level);
    }
    let highest = |from: usize, to: usize| {
        (from..to).fold((from, 0.0f64), |best, i| {
            if points[i].1 > best.1 {
                (i, points[i].1)
            } else {
                best
            }
        })
    };
    let (mut peak_index, mut peak) = highest(0, points.len());
    // A first line that rises after the highest point is the note's own attack from the level
    // the note before it left (a retrigger, or the onset's smear of it): its top is the peak.
    if points.len() - peak_index >= 2 * MIN_SEGMENT_POINTS
        && let [(_, i, first), (_, j, _), ..] = fit_segments(&points[peak_index..], peak)[..]
        && first.b > 0.0
    {
        (peak_index, peak) = highest(peak_index + i - 1, peak_index + j);
    }
    let descent = &points[peak_index..];
    // The largest rise of the level after its lowest point since the peak; before the peak (from
    // beyond the onset's tolerance, where the window may still hold the note before), a rise after
    // a fall of at least [`DESCENT_MIN_FRACTION`] of the peak.
    let settled = start as f64 + half as f64 + ONSET_TOLERANCE_FRAMES as f64;
    let rebound_over = |upto: f64| {
        // The highest level so far and the lowest since it.
        let mut highest = f64::NEG_INFINITY;
        let mut lowest = (f64::INFINITY, 0usize);
        let mut rebound = (0.0f64, 0usize);
        // Any rise after a fall before the peak, for the confidence alone: a retrigger at the
        // velocity the note sustains at moves the level too little to be an onset.
        let mut doubt = 0.0f64;
        for (k, p) in points
            .iter()
            .enumerate()
            .skip_while(|(_, p)| p.0 < settled)
            .take_while(|(_, p)| p.0 <= upto)
        {
            if k == peak_index {
                highest = p.1;
                lowest = (p.1, k);
            }
            let fell = k > peak_index || highest - lowest.0 >= DESCENT_MIN_FRACTION * peak;
            if fell && p.1 - lowest.0 > rebound.0 {
                rebound = (p.1 - lowest.0, lowest.1);
            }
            if highest - lowest.0 >= REBOUND_FRACTION * peak {
                doubt = doubt.max(p.1 - lowest.0);
            }
            if k < peak_index && p.1 >= highest {
                highest = p.1;
                lowest = (p.1, k);
            } else if p.1 < lowest.0 {
                lowest = (p.1, k);
            }
        }
        (
            rebound.0.max(doubt) / peak,
            points[rebound.1].0 as usize,
            rebound.0 / peak,
        )
    };
    let rebound = rebound_over(f64::INFINITY);
    if descent.len() < MIN_SEGMENT_POINTS {
        return Gate {
            sample: if cut {
                end
            } else {
                points[peak_index].0 as usize
            },
            confidence: if cut { 1.0 } else { 0.0 },
            level: signal.centred(points[peak_index].0 as isize, window),
            rebound,
            step: (0.0, end),
        };
    }
    let mut segments = fit_segments(descent, peak);
    if let [.., (bi, bj, before), (ri, rj, rising)] = segments[..]
        && cut
        && rising.b * (descent[rj - 1].0 - descent[ri].0) >= REBOUND_FRACTION * peak
        && descent[rj - 1].0 - descent[ri].0 <= 2.0 * window as f64
        && -before.b * (descent[bj - 1].0 - descent[bi].0) >= DESCENT_MIN_FRACTION * peak
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
            // Held to the next onset: as steady as the level still is where it last holds, and
            // as near the onset as a release begun before it can start.
            let tail = &descent[last.1 - MIN_SEGMENT_POINTS..last.1];
            let bend = tail
                .iter()
                .map(|p| last_line.a + last_line.b * p.0 - p.1)
                .sum::<f64>()
                / tail.len() as f64;
            // The points a release begun within the gate tolerance of the onset leaves alone
            // (the window reaches half its width before the release).
            let settled: Vec<(f64, f64)> = descent[last.0..last.1]
                .iter()
                .filter(|p| p.0 + GATE_SLACK_FRAMES + window as f64 <= end as f64)
                .copied()
                .collect();
            // The slope where the level last holds, over the last two windows of those points:
            // a decay that ran into its sustain within the line is held, a level still falling
            // there a slow release (or a decay still running, which the level does not tell
            // from one). Read as the fall it makes over the gate tolerance: a release begun
            // further from the onset has run at least that far by it, and a sustain's ripple
            // extrapolated over the whole line is no fall.
            let recent = (2 * window / ENVELOPE_STEP_FRAMES).max(MIN_SEGMENT_POINTS);
            let slope = if settled.len() >= MIN_SEGMENT_POINTS {
                let from = settled.len().saturating_sub(recent);
                Sums::new(&settled[from..]).line(0, settled.len() - from).b
            } else {
                last_line.b
            };
            let steady = (1.0
                + slope * GATE_TOLERANCE_FRAMES as f64 / (HOLD_DROP_FULL_FRACTION * peak))
                .clamp(0.0, 1.0);
            let first = descent[last.0..last.1]
                .iter()
                .position(|p| {
                    p.0 + GATE_TOLERANCE_FRAMES as f64 + 3.0 * window as f64 >= end as f64
                })
                .map_or(last.0, |i| last.0 + i);
            let departure = release_departure(
                &descent[first..last.1],
                &LevelKernel::new(window, period),
                spread,
                end as f64,
            );
            let near = match departure {
                // A release seen: the gate end is the onset's when the release cannot have begun
                // more than the tolerance before it.
                Some(earliest) => {
                    interval_confidence_at(end as f64 - earliest, GATE_TOLERANCE_FRAMES)
                }
                // None seen: the last points stay on the line before them.
                None => {
                    let mut drop = bend;
                    if settled.len() >= MIN_SEGMENT_POINTS {
                        let held = Sums::new(&settled).line(0, settled.len());
                        let final_point = descent[last.1 - 1];
                        drop = drop.max(held.a + held.b * final_point.0 - final_point.1);
                    }
                    (1.0 - drop / (HOLD_DROP_FULL_FRACTION * peak)).clamp(0.0, 1.0)
                }
            };
            (end, steady.min(near))
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
        if s1 > 0.0 {
            // The falling line follows a rise: a decay cut by the onset or ending in an unseen
            // release, or a release begun at the rise's top; which, the level does not tell.
            if cut {
                (end, AMBIGUOUS_GATE_CONFIDENCE)
            } else {
                (knee.round() as usize, AMBIGUOUS_GATE_CONFIDENCE)
            }
        } else {
            // A release is one line from its knee. A last line the points bend away from, beyond
            // the window's reach of the knee and of the end (cut by the next onset, the slack of
            // the end, where that onset's attack may be read; ending in silence, where the line
            // reaches it), holds a later knee: a sustain the line has run into its release.
            let release = Sums::new(&descent[last.0..last.1]).line(0, last.1 - last.0);
            // Ending in silence, the release line's end is a corner too, which the window rounds.
            let silent = if release.b < 0.0 {
                -release.a / release.b
            } else {
                f64::INFINITY
            };
            let bend = descent[last.0..last.1]
                .iter()
                .filter(|p| {
                    p.0 >= knee + window as f64
                        && if cut {
                            p.0 + window as f64 + GATE_SLACK_FRAMES <= end as f64
                        } else {
                            p.0 + window as f64 <= silent
                        }
                        && p.1 >= DESCENT_MIN_FRACTION * peak
                })
                .map(|p| (p.1 - release.a - release.b * p.0).abs())
                .fold(0.0f64, f64::max);
            // A held sustain is as flat as the line before the knee. Where the points after the
            // window's reach of the knee already fall as steeply as the release, the bend is the
            // release's own curve (the drive's), not a later knee.
            let reach: Vec<(f64, f64)> = descent[last.0..last.1]
                .iter()
                .filter(|p| p.0 >= knee + window as f64 && p.0 <= knee + 2.0 * window as f64)
                .copied()
                .collect();
            let held = if reach.len() >= MIN_SEGMENT_POINTS && s2 < s1 {
                let local = Sums::new(&reach).line(0, reach.len());
                ((local.b - s2) / (s1 - s2)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let bend = bend * held;
            let straight = (1.0 - bend / (REBOUND_FRACTION * peak)).clamp(0.0, 1.0);
            let sharp = ((s2 - s1).abs() * GATE_SLACK_FRAMES / (KNEE_SHARP_FULL_FRACTION * peak))
                .clamp(0.0, 1.0);
            // A cut last line falling less steeply than the line before it may be that line's
            // decay still running at the next onset, the gate held to it; which, the level does
            // not tell.
            let confidence = visibility.min(straight).min(sharp);
            // A short cut last line after a line that falls from a flatter one: the release may
            // have begun at the earlier knee, the last line the next note's attack falling to a
            // lower velocity before its onset is seen.
            let earlier = match segments[..] {
                [.., (_, _, first), (bi, bj, middle), (ri, rj, _)] => {
                    descent[rj - 1].0 - descent[ri].0 <= 2.0 * window as f64
                        && middle.b < first.b.min(0.0)
                        && -middle.b * (descent[bj - 1].0 - descent[bi].0)
                            >= DESCENT_MIN_FRACTION * peak
                }
                _ => false,
            };
            // Cut by the next onset, a line before the knee no longer than a window from the peak
            // is the peak's own smoothed corner, not a held level: the falling line starts at the
            // peak, a decay cut by the onset or a release from the peak; which, the level does not
            // tell (a release ending in silence is read whole).
            let (bi, bj, _) = segments[segments.len() - 2];
            let from_peak = bi == 0 && descent[bj - 1].0 - descent[0].0 <= window as f64;
            let confidence = if cut && (s2.abs() < s1.abs() || earlier || from_peak) {
                confidence.min(AMBIGUOUS_GATE_CONFIDENCE)
            } else {
                confidence
            };
            (knee.round() as usize, confidence)
        }
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
    // The body from the peak to the gate end is a decay and a sustain: two lines. What they leave
    // unexplained, away from their knee and the window's reach of either end, is another note's.
    let body: Vec<(f64, f64)> = descent
        .iter()
        .filter(|p| p.0 + half as f64 <= sample as f64)
        .copied()
        .collect();
    let mut residual = 0.0f64;
    let mut step = (0.0f64, end);
    if body.len() >= 2 * MIN_SEGMENT_POINTS {
        let fit = fit_two_lines(&body);
        let knees: Vec<f64> = fit.iter().skip(1).map(|s| body[s.0].0).collect();
        let reach = window as f64;
        let mut lines: Vec<Option<Line>> = Vec::with_capacity(fit.len());
        for &(i, j, _) in &fit {
            // Each line refitted to the points the window's smear of a knee does not reach.
            let inner: Vec<(f64, f64)> = body[i..j]
                .iter()
                .filter(|p| {
                    !(knees.iter().any(|k| (p.0 - k).abs() < reach)
                        || p.0 - body[0].0 < reach
                        || body[body.len() - 1].0 - p.0 < reach + GATE_SLACK_FRAMES)
                })
                .copied()
                .collect();
            if inner.len() < MIN_SEGMENT_POINTS {
                // A line too short to clear the gate's slack: the points clear of the knees alone
                // still tell a step, not a residual.
                let short: Vec<(f64, f64)> = body[i..j]
                    .iter()
                    .filter(|p| {
                        !(knees.iter().any(|k| (p.0 - k).abs() < reach) || p.0 - body[0].0 < reach)
                    })
                    .copied()
                    .collect();
                lines.push(
                    (short.len() >= MIN_SEGMENT_POINTS
                        && short[short.len() - 1].0 - short[0].0 >= reach)
                        .then(|| Sums::new(&short).line(0, short.len())),
                );
                continue;
            }
            let line = Sums::new(&inner).line(0, inner.len());
            for p in &inner {
                residual = residual.max((p.1 - line.a - line.b * p.0).abs());
            }
            lines.push(Some(line));
        }
        // A decay meets its sustain: two lines that do not meet within the window's reach of
        // their knee are a step of the level, a velocity the note did not start with.
        for (k, pair) in knees.iter().zip(lines.windows(2)) {
            if let [Some(a), Some(b)] = pair {
                let gap = |t: f64| a.a + a.b * t - b.a - b.b * t;
                let (g1, g2) = (gap(k - reach), gap(k + reach));
                if g1.signum() == g2.signum() {
                    let gap = g1.abs().min(g2.abs());
                    residual = residual.max(gap);
                    if gap / peak > step.0 {
                        step = (gap / peak, k.round() as usize);
                    }
                }
            }
        }
    }
    let rebound = rebound_over(sample as f64 - half as f64 - window as f64 - GATE_SLACK_FRAMES);
    let rebound = (rebound.0.max(residual / peak), rebound.1, rebound.0);
    // The velocity reads the RMS where the level peaks within the gate (Dc1-2).
    let loudest = points
        .iter()
        .filter(|p| p.0 <= sample as f64 + half as f64)
        .fold(points[0], |best, p| if p.1 > best.1 { *p } else { best });
    let level = signal.centred(loudest.0 as isize, window);
    Gate {
        sample,
        confidence,
        level,
        rebound,
        step,
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

/// The best fit of two lines to `points`, a decay and its sustain, with no penalty for the
/// second: a decay that ends a level step above its sustain, or close to the gate, is still two
/// lines, and one line through both leaves their corner as a residual no other note made. One
/// line when there are too few points for two.
fn fit_two_lines(points: &[(f64, f64)]) -> Vec<(usize, usize, Line)> {
    let sums = Sums::new(points);
    let n = points.len();
    let m = MIN_SEGMENT_POINTS;
    let mut best: (f64, Vec<(usize, usize, Line)>) = {
        let line = sums.line(0, n);
        (line.error, vec![(0, n, line)])
    };
    for i in m..=n.saturating_sub(m) {
        let a = sums.line(0, i);
        let b = sums.line(i, n);
        if a.error + b.error < best.0 {
            best = (a.error + b.error, vec![(0, i, a), (i, n, b)]);
        }
    }
    best.1
}

/// The runs `(first, last)` of `built` (one region's notes) that may be one tone: each note
/// starts where the one before it ends its tail, each note with a pitch lies less than
/// [`PITCH_CHANGE_CENTS`] from the last pitch before it in the run (no change of tone, whichever
/// MIDI numbers the two round to: two tones beating read the pitch between them, which may lie
/// on a MIDI number's boundary), and a note without one is ambiguous, not too short to measure.
fn pitch_spans(built: &[Built]) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut pitch = 0.0f64;
    for (i, b) in built.iter().enumerate() {
        let f0 = b.note.f0_hz;
        let joins = i > 0
            && b.note.start_sample <= built[i - 1].note.tail_end_sample
            && (pitch <= 0.0 || f0 <= 0.0 || cents_between(f0, pitch).abs() < PITCH_CHANGE_CENTS);
        match spans.last_mut() {
            Some(span) if joins => span.1 = i,
            _ => {
                spans.push((i, i));
                pitch = 0.0;
            }
        }
        if f0 > 0.0 {
            pitch = f0;
        }
    }
    spans
}

/// The steady part of `[start, end)`, one note's sound up to the next onset or a run of them:
/// past the attack (a quarter of it, at most 50 ms), as a window of the largest power of two up
/// to [`POLYPHONY_MAX_FFT_LENGTH`] centred in the rest. `None` when shorter than
/// [`POLYPHONY_MIN_FFT_LENGTH`].
fn steady_window((start, end): (usize, usize)) -> Option<(usize, usize)> {
    let span = end.saturating_sub(start);
    let skip = (span / 4).min(2_400);
    let available = span - skip;
    if available < POLYPHONY_MIN_FFT_LENGTH {
        return None;
    }
    let mut n = POLYPHONY_MAX_FFT_LENGTH;
    while n > available {
        n /= 2;
    }
    Some((start + skip + (available - n) / 2, n))
}

/// The spectral peaks of one steady window (module documentation, polyphony): frequency and
/// power of each peak within [`POLYPHONY_PEAK_FLOOR_DB`] of the largest, below
/// [`POLYPHONY_MAX_HZ`].
struct Peaks {
    peaks: Vec<(f64, f64)>,
    /// The power of every bin.
    power: Vec<f64>,
    bin_hz: f64,
    largest: f64,
}

impl Peaks {
    /// The peaks of the `n` samples from `first`; `None` for a spectrum that is not tonal (its
    /// largest peak less than [`POLYPHONY_TONAL_DB`] above its median: noise has peaks that are
    /// no harmonics).
    fn read(signal: &Signal, first: usize, n: usize) -> Option<Self> {
        let magnitude = spectrum(signal, first, n);
        let bin_hz = f64::from(SAMPLE_RATE_HZ) / n as f64;
        let top = magnitude.len() - 2;
        let largest = magnitude[1..=top].iter().copied().fold(0.0f64, f64::max);
        let mut sorted = magnitude[1..=top].to_vec();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        if largest <= 0.0 || largest < median * 10f64.powf(POLYPHONY_TONAL_DB / 20.0) {
            return None;
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
        Some(Self {
            peaks,
            power: magnitude.iter().map(|m| m * m).collect(),
            bin_hz,
            largest: largest * largest,
        })
    }

    /// The largest power within [`POLYPHONY_SIDEBAND_BINS`] of `mirror` and off the main lobe
    /// ([`POLYPHONY_LOBE_BINS`]) of `carrier`: a stable tone has none there, past its lobe's
    /// first null, so what there is comes from its envelope changing within the window.
    fn skirt(&self, mirror: f64, carrier: f64) -> f64 {
        let reach = POLYPHONY_SIDEBAND_BINS * self.bin_hz;
        let lobe = POLYPHONY_LOBE_BINS * self.bin_hz;
        let first = ((mirror - reach) / self.bin_hz).ceil().max(0.0) as usize;
        let last = ((mirror + reach) / self.bin_hz).floor().max(0.0) as usize;
        (first..=last.min(self.power.len().saturating_sub(1)))
            .filter(|&k| (k as f64 * self.bin_hz - carrier).abs() > lobe)
            .map(|k| self.power[k])
            .fold(0.0, f64::max)
    }

    /// The distance within which a peak is a harmonic of `fundamental`:
    /// [`POLYPHONY_HARMONIC_TOLERANCE`] of it, or the window's main lobe when that is wider.
    fn near(&self, fundamental: f64) -> f64 {
        (POLYPHONY_HARMONIC_TOLERANCE * fundamental).max(POLYPHONY_LOBE_BINS * self.bin_hz)
    }

    /// The share of the power of the peaks from an octave below `fundamental` up that is no
    /// harmonic of it, once modulation sidebands are left out: an inharmonic peak beside a
    /// stronger harmonic whose mirror image holds another peak, or a skirt ([`Peaks::skirt`]) of
    /// at least a quarter of its power, is the envelope of one tone changing within the
    /// spectrum. Peaks more than an octave below are aliases of the oscillator's harmonics past
    /// the Nyquist frequency, which a dark filter passes more than the fundamental; a second
    /// tone of a Dc1-3 interval is never that low.
    fn share(&self, fundamental: f64) -> f64 {
        let read: Vec<(f64, f64)> = self
            .peaks
            .iter()
            .copied()
            .filter(|&(f, _)| f >= 0.5 * fundamental)
            .collect();
        let near = self.near(fundamental);
        let harmonic = |f: f64| (f / fundamental).round().max(1.0) * fundamental;
        let tolerance = POLYPHONY_SIDEBAND_BINS * self.bin_hz;
        let sideband = |f: f64, p: f64| {
            let carrier = harmonic(f);
            let mirror = 2.0 * carrier - f;
            read.iter()
                .any(|&(g, q)| (g - carrier).abs() <= near && q >= 4.0 * p)
                && (read.iter().any(|&(g, _)| {
                    (g - carrier).abs() > tolerance && (g - mirror).abs() <= tolerance
                }) || self.skirt(mirror, carrier) >= 0.25 * p)
        };
        let total: f64 = read.iter().map(|p| p.1).sum();
        let unexplained: f64 = read
            .iter()
            .filter(|&&(f, p)| (f - harmonic(f)).abs() > near && !sideband(f, p))
            .map(|p| p.1)
            .sum();
        unexplained / total.max(1e-300)
    }

    /// The candidate fundamentals: the peaks within [`POLYPHONY_CANDIDATE_FLOOR_DB`] of the
    /// largest and at or above [`F0_MIN_HZ`].
    fn candidates(&self) -> impl Iterator<Item = f64> + '_ {
        let audible = self.largest * 10f64.powf(POLYPHONY_CANDIDATE_FLOOR_DB / 10.0);
        self.peaks
            .iter()
            .filter(move |&&(f, p)| p >= audible && f >= F0_MIN_HZ)
            .map(|p| p.0)
    }

    /// The fundamental of one tone that explains the peaks (share at most
    /// [`POLYPHONY_INHARMONIC_SHARE`]; the least share when several do), refined from the
    /// harmonics within the main lobe of their places, weighted by power.
    fn fundamental(&self) -> Option<f64> {
        let best = self
            .candidates()
            .map(|f| (f, self.share(f)))
            .filter(|&(_, s)| s <= POLYPHONY_INHARMONIC_SHARE)
            .min_by(|a, b| a.1.total_cmp(&b.1))?
            .0;
        let lobe = POLYPHONY_LOBE_BINS * self.bin_hz;
        let (mut num, mut den) = (0.0, 0.0);
        for &(f, p) in &self.peaks {
            let h = (f / best).round();
            if h >= 1.0 && (f - h * best).abs() <= lobe {
                num += p * h * f;
                den += p * h * h;
            }
        }
        Some(if den > 0.0 { num / den } else { best })
    }
}

/// Whether the steady part of `[start, end)` sounds two tones at once (module documentation):
/// `Some(true)` two tones, `Some(false)` one tone or too short to read, `None` a change from one
/// tone to another, to be read in shorter parts. Reads the signal only: no pitch or confidence of
/// the notes.
fn polyphony(signal: &Signal, samples: &[f32], (start, end): (usize, usize)) -> Option<bool> {
    let Some((first, n)) = steady_window((start, end)) else {
        return Some(false);
    };
    // A spectrum whose halves differ holds a change of tone, not two tones at once.
    let halves = (
        spectrum(signal, first, n / 2),
        spectrum(signal, first + n / 2, n / 2),
    );
    let dot: f64 = halves.0.iter().zip(&halves.1).map(|(a, b)| a * b).sum();
    let norms = halves.0.iter().map(|a| a * a).sum::<f64>().sqrt()
        * halves.1.iter().map(|b| b * b).sum::<f64>().sqrt();
    if norms <= 0.0 || dot < POLYPHONY_STATIONARY * norms {
        return None;
    }
    let Some(peaks) = Peaks::read(signal, first, n) else {
        return Some(false);
    };
    // One tone when some candidate, taken as the fundamental, explains the rest.
    let mut candidates = peaks.candidates().peekable();
    if candidates.peek().is_none()
        || candidates.any(|f| peaks.share(f) <= POLYPHONY_INHARMONIC_SHARE)
    {
        return Some(false);
    }
    // Two halves with two clear pitches are a change of tone.
    let half = |a: usize| note_pitch(samples, a, a + n / 2).ok().map(|p| p.f0_hz);
    match (half(first), half(first + n / 2)) {
        (Some(a), Some(b)) if cents_between(a, b).abs() >= PITCH_CHANGE_CENTS => None,
        _ => Some(true),
    }
}

/// The fundamental of the steady part of `[start, end)` read from its spectrum
/// ([`Peaks::fundamental`]); `None` when that part is too short, not tonal, or no candidate
/// explains its peaks, and when the window does not resolve the harmonics: a harmonic nearer
/// its neighbour than the first null of the main lobe ([`POLYPHONY_LOBE_BINS`] bins) sits on
/// the neighbour's lobe, which pulls its peak's place towards the neighbour.
fn spectral_f0(signal: &Signal, (start, end): (usize, usize)) -> Option<f64> {
    let (first, n) = steady_window((start, end))?;
    let f0 = Peaks::read(signal, first, n)?.fundamental()?;
    let lobe = POLYPHONY_LOBE_BINS * f64::from(SAMPLE_RATE_HZ) / n as f64;
    (f0 >= lobe).then_some(f0)
}

/// The Blackman-windowed magnitude spectrum of the `n` samples from `first`, bins 0 to one past
/// [`POLYPHONY_MAX_HZ`].
fn spectrum(signal: &Signal, first: usize, n: usize) -> Vec<f64> {
    let mut buffer: Vec<Complex> = (0..n)
        .map(|k| {
            let phase = 2.0 * std::f64::consts::PI * k as f64 / n as f64;
            let w = 0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos();
            Complex::new(signal.x.get(first + k).copied().unwrap_or(0.0) * w, 0.0)
        })
        .collect();
    if fft(&mut buffer).is_err() {
        return vec![0.0; 3];
    }
    let bin_hz = f64::from(SAMPLE_RATE_HZ) / n as f64;
    let top = ((POLYPHONY_MAX_HZ / bin_hz) as usize).min(n / 2 - 1);
    buffer[..=top + 1].iter().map(|z| z.norm()).collect()
}
