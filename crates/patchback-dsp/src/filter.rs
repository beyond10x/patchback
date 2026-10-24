//! The resonant lowpass (DESIGN.md § 5.3): a topology-preserving transform (TPT) state-variable
//! filter, trapezoidally integrated with the cutoff prewarped, which is stable for every cutoff
//! below Nyquist and every positive damping.

use std::f64::consts::PI;

use crate::FS;

/// The lowest cutoff, in Hz: the bottom of the audible range, below the 40 Hz where the
/// `filter_cutoff` contract starts (§ 5.2). At full resonance the impulse response there decays
/// by `e` in about 0.2 s.
pub const MIN_CUTOFF_HZ: f64 = 20.0;

/// The highest cutoff, in Hz: 0.49 of the sample rate, below Nyquist (24 kHz), where the
/// prewarped `tan` stays finite. The `filter_cutoff` contract ends at 16 kHz.
pub const MAX_CUTOFF_HZ: f64 = 0.49 * FS;

/// The lowest resonance Q: the lower bound of the `resonance_q` contract (§ 5.2).
pub const MIN_RESONANCE_Q: f64 = 0.5;

/// The highest resonance Q: the upper bound of the `resonance_q` contract (§ 5.2).
pub const MAX_RESONANCE_Q: f64 = 12.0;

/// The lowpass output of a TPT state-variable filter: unity gain at DC, gain `Q` at the cutoff,
/// 12 dB per octave above it.
#[derive(Clone, Debug, PartialEq)]
pub struct SvfLowpass {
    cutoff_hz: f64,
    resonance_q: f64,
    a1: f64,
    a2: f64,
    a3: f64,
    ic1eq: f64,
    ic2eq: f64,
}

impl SvfLowpass {
    /// A filter at rest. The cutoff is clamped to `[MIN_CUTOFF_HZ, MAX_CUTOFF_HZ]` and the
    /// resonance to `[MIN_RESONANCE_Q, MAX_RESONANCE_Q]`; a non-finite value of either is its
    /// lower bound.
    pub fn new(cutoff_hz: f64, resonance_q: f64) -> Self {
        let cutoff_hz = bounded(cutoff_hz, MIN_CUTOFF_HZ, MAX_CUTOFF_HZ);
        let resonance_q = bounded(resonance_q, MIN_RESONANCE_Q, MAX_RESONANCE_Q);
        let g = (PI * cutoff_hz / FS).tan();
        let k = 1.0 / resonance_q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        Self {
            cutoff_hz,
            resonance_q,
            a1,
            a2,
            a3: g * a2,
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    /// The cutoff in use, in Hz.
    pub fn cutoff_hz(&self) -> f64 {
        self.cutoff_hz
    }

    /// The resonance Q in use.
    pub fn resonance_q(&self) -> f64 {
        self.resonance_q
    }

    /// The next lowpass sample. A non-finite input is silence, so the state stays finite.
    pub fn process(&mut self, x: f64) -> f64 {
        let x = if x.is_finite() { x } else { 0.0 };
        let v3 = x - self.ic2eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.a2 * self.ic1eq + self.a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        v2
    }
}

fn bounded(value: f64, low: f64, high: f64) -> f64 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        low
    }
}
