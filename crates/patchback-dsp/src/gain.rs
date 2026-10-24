//! Gain, smoothing and the protective output limiter (DESIGN.md § 5.3: "parameter smoothing
//! where necessary to prevent clicks; protective output clipping/limiting reported, not silently
//! used to hide instability").

use crate::FS;
use crate::render::RenderError;

/// The protective limiter's threshold, as a linear amplitude: a sample whose magnitude exceeds
/// it is set to it and counted.
///
/// Provisional: DESIGN.md § 5.3 asks for a stated threshold and § 17 does not settle one. Full
/// scale keeps every written float32 sample in `[-1, 1]`.
pub const LIMITER_THRESHOLD: f64 = 1.0;

/// The time constant of the one-pole smoothing of the velocity and the output gain, in seconds.
///
/// Provisional: DESIGN.md § 5.3 asks for smoothing "where necessary to prevent clicks" and names
/// no time; § 17 leaves it open.
pub const SMOOTHING_SECONDS: f64 = 0.002;

/// The linear gain of a level in dB.
pub fn decibels_to_gain(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

/// One-pole smoothing towards a target: each sample moves the value by `c` of the remaining
/// distance, `c = 1 - exp(-1 / (time * 48000))`. A time under one sample (zero, negative or not
/// finite included) jumps to the target at once.
#[derive(Clone, Debug, PartialEq)]
pub struct Smoother {
    value: f64,
    target: f64,
    coefficient: f64,
}

impl Smoother {
    /// A smoother resting at `initial`.
    pub fn new(initial: f64, time_s: f64) -> Self {
        let coefficient = if time_s.is_finite() && time_s * FS >= 1.0 {
            1.0 - (-1.0 / (time_s * FS)).exp()
        } else {
            1.0
        };
        Self {
            value: initial,
            target: initial,
            coefficient,
        }
    }

    /// The value to approach from the next sample on.
    pub fn set_target(&mut self, target: f64) {
        self.target = target;
    }

    /// Sets value and target at once, for a change that cannot click (the envelope is at 0).
    pub fn jump_to(&mut self, value: f64) {
        self.value = value;
        self.target = value;
    }

    /// The next smoothed value.
    pub fn next_sample(&mut self) -> f64 {
        self.value += (self.target - self.value) * self.coefficient;
        self.value
    }
}

/// The protective output limiter: a sample whose magnitude exceeds the threshold is set to the
/// threshold with its sign, and counted. A sample at or below it passes unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct Limiter {
    threshold: f64,
    limited: u64,
}

impl Limiter {
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            limited: 0,
        }
    }

    /// The limited sample. NaN passes, for the caller to refuse.
    pub fn process(&mut self, x: f64) -> f64 {
        if x.abs() > self.threshold {
            self.limited += 1;
            self.threshold.copysign(x)
        } else {
            x
        }
    }

    /// How many samples the limiter has changed.
    pub fn limited_samples(&self) -> u64 {
        self.limited
    }
}

/// The end of the voice: the output gain (smoothed), the limiter at [`LIMITER_THRESHOLD`], and
/// the refusal of a non-finite sample, which no stage may hide (§ 2 invariant 4).
#[derive(Clone, Debug, PartialEq)]
pub struct OutputStage {
    gain: Smoother,
    limiter: Limiter,
}

impl OutputStage {
    /// An output stage at `output_gain_db` (a non-finite gain is silence: 0 linear).
    pub fn new(output_gain_db: f64) -> Self {
        let gain = decibels_to_gain(output_gain_db);
        let gain = if gain.is_finite() { gain } else { 0.0 };
        Self {
            gain: Smoother::new(gain, SMOOTHING_SECONDS),
            limiter: Limiter::new(LIMITER_THRESHOLD),
        }
    }

    /// The output sample at `frame` of `x`, or [`RenderError::NonFiniteRender`] when the gained
    /// sample is NaN or infinite. The check comes before the limiter, which would otherwise turn
    /// an infinite sample into a full-scale one and hide it.
    pub fn process(&mut self, frame: u64, x: f64) -> Result<f32, RenderError> {
        let gained = x * self.gain.next_sample();
        if !gained.is_finite() {
            return Err(RenderError::NonFiniteRender { frame });
        }
        Ok(self.limiter.process(gained) as f32)
    }

    /// How many samples the limiter has changed.
    pub fn limited_samples(&self) -> u64 {
        self.limiter.limited_samples()
    }
}
