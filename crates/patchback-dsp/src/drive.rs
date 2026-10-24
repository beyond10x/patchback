//! The drive stage (DESIGN.md § 5.3): a deterministic bounded nonlinearity. Oversampling is
//! deferred (§ 5.3).

use crate::gain::decibels_to_gain;

/// The lowest drive, in dB: the lower bound of the `drive` contract (§ 5.2).
pub const MIN_DRIVE_DB: f64 = 0.0;

/// The highest drive, in dB: the upper bound of the `drive` contract (§ 5.2).
pub const MAX_DRIVE_DB: f64 = 24.0;

/// `y = tanh(g x) / tanh(g)` with `g = 10^(drive / 20)`. The division keeps a full-scale input at
/// full scale at every drive, so the stage shapes the waveform rather than its level. The curve
/// is odd and monotonic; `|y| <= 1` for `|x| <= 1`, and `|y| < 1 / tanh(1)` for any input.
#[derive(Clone, Debug, PartialEq)]
pub struct Drive {
    gain: f64,
    normalizer: f64,
}

impl Drive {
    /// A drive of `drive_db`, clamped to `[MIN_DRIVE_DB, MAX_DRIVE_DB]`; a non-finite drive is
    /// [`MIN_DRIVE_DB`].
    pub fn new(drive_db: f64) -> Self {
        let drive_db = if drive_db.is_finite() {
            drive_db.clamp(MIN_DRIVE_DB, MAX_DRIVE_DB)
        } else {
            MIN_DRIVE_DB
        };
        let gain = decibels_to_gain(drive_db);
        Self {
            gain,
            normalizer: 1.0 / gain.tanh(),
        }
    }

    /// The driven sample. A NaN input is silence; an infinite one saturates.
    pub fn process(&self, x: f64) -> f64 {
        if x.is_nan() {
            return 0.0;
        }
        (self.gain * x).tanh() * self.normalizer
    }
}
