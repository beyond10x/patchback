//! The ADSR envelope (DESIGN.md § 5.3): explicit note-on, retrigger and note-off; events take
//! effect on the sample they are given before; zero and near-zero times are safe.

use crate::FS;

/// Where the envelope is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdsrStage {
    /// At 0, no note.
    Idle,
    /// Rising to 1.
    Attack,
    /// Falling from 1 to the sustain level.
    Decay,
    /// Holding the sustain level until note-off.
    Sustain,
    /// Falling to 0 after note-off.
    Release,
}

/// A linear ADSR from 0 to 1.
///
/// Each segment is a straight line from the level it starts at to its target, taking
/// `max(1, round(time * 48000))` samples and landing exactly on the target on its last sample:
/// a time under half a sample (zero included) takes one sample. A time that is negative or not
/// finite is zero, and the sustain level is clamped to `[0, 1]` (NaN is 0).
///
/// - [`note_on`](Self::note_on) starts the attack from the current level, so a retrigger during
///   decay, sustain or release continues from where the envelope is, without a step.
/// - [`note_off`](Self::note_off) starts the release from the current level, in any stage but
///   idle; when idle it does nothing.
/// - The sample [`next_sample`](Self::next_sample) returns after an event is the first sample of
///   the new segment, so an event given before the sample at offset `n` takes effect at `n`.
#[derive(Clone, Debug, PartialEq)]
pub struct Adsr {
    attack: u64,
    decay: u64,
    sustain: f64,
    release: u64,
    stage: AdsrStage,
    level: f64,
    start: f64,
    position: u64,
}

impl Adsr {
    /// An idle envelope with these times in seconds and this sustain level.
    pub fn new(attack_s: f64, decay_s: f64, sustain: f64, release_s: f64) -> Self {
        Self {
            attack: samples(attack_s),
            decay: samples(decay_s),
            sustain: if sustain.is_nan() {
                0.0
            } else {
                sustain.clamp(0.0, 1.0)
            },
            release: samples(release_s),
            stage: AdsrStage::Idle,
            level: 0.0,
            start: 0.0,
            position: 0,
        }
    }

    /// The release time in samples, as the envelope runs it.
    pub fn release_samples(&self) -> u64 {
        self.release
    }

    pub fn stage(&self) -> AdsrStage {
        self.stage
    }

    /// The level of the last sample returned.
    pub fn level(&self) -> f64 {
        self.level
    }

    /// Note-on, or a retrigger: the attack starts from the current level.
    pub fn note_on(&mut self) {
        self.enter(AdsrStage::Attack);
    }

    /// Note-off: the release starts from the current level, unless the envelope is idle.
    pub fn note_off(&mut self) {
        if self.stage != AdsrStage::Idle {
            self.enter(AdsrStage::Release);
        }
    }

    /// The next level, in `[0, 1]`.
    pub fn next_sample(&mut self) -> f64 {
        let (length, target, next) = match self.stage {
            AdsrStage::Idle | AdsrStage::Sustain => return self.level,
            AdsrStage::Attack => (self.attack, 1.0, AdsrStage::Decay),
            AdsrStage::Decay => (self.decay, self.sustain, AdsrStage::Sustain),
            AdsrStage::Release => (self.release, 0.0, AdsrStage::Idle),
        };
        self.position += 1;
        if self.position >= length {
            self.level = target;
            self.enter(next);
        } else {
            let progress = self.position as f64 / length as f64;
            self.level = self.start + (target - self.start) * progress;
        }
        self.level
    }

    fn enter(&mut self, stage: AdsrStage) {
        self.stage = stage;
        self.start = self.level;
        self.position = 0;
    }
}

/// A segment time in samples: at least one; a negative or non-finite time is zero.
fn samples(seconds: f64) -> u64 {
    let seconds = if seconds.is_finite() {
        seconds.max(0.0)
    } else {
        0.0
    };
    // `as` saturates a value past u64::MAX.
    ((seconds * FS).round() as u64).max(1)
}
