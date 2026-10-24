//! The saw oscillator (DESIGN.md § 5.3): a phase accumulator anti-aliased by polyBLEP.

use crate::FS;

/// The highest oscillator frequency, in Hz. MIDI 127 (about 12.5 kHz) lies below it, and it keeps
/// the phase increment below 0.5, where the two-sample polyBLEP residual is defined.
pub const MAX_OSCILLATOR_HZ: f64 = 20_000.0;

/// The equal-tempered frequency of a MIDI note, A4 (note 69) at 440 Hz.
pub fn midi_to_hz(note: u8) -> f64 {
    440.0 * 2f64.powf((f64::from(note) - 69.0) / 12.0)
}

/// A band-limited saw from -1 to 1 rising, by the polyBLEP method: the naive ramp `2t - 1` of the
/// phase `t` in `[0, 1)`, minus a two-sample polynomial residual around each wrap. The phase is
/// never reset by a frequency change, so it is continuous across notes.
#[derive(Clone, Debug, PartialEq)]
pub struct PolyBlepSaw {
    phase: f64,
    increment: f64,
}

impl Default for PolyBlepSaw {
    fn default() -> Self {
        Self::new()
    }
}

impl PolyBlepSaw {
    /// Phase 0, frequency 0.
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            increment: 0.0,
        }
    }

    /// Sets the frequency, clamped to `[0, MAX_OSCILLATOR_HZ]`; a non-finite or negative
    /// frequency is 0. The phase is unchanged.
    pub fn set_frequency(&mut self, hz: f64) {
        let hz = if hz.is_finite() {
            hz.clamp(0.0, MAX_OSCILLATOR_HZ)
        } else {
            0.0
        };
        self.increment = hz / FS;
    }

    /// The frequency in use, in Hz.
    pub fn frequency(&self) -> f64 {
        self.increment * FS
    }

    /// The phase of the next sample, in `[0, 1)`.
    pub fn phase(&self) -> f64 {
        self.phase
    }

    /// The next sample, then the phase advances by one increment.
    pub fn next_sample(&mut self) -> f64 {
        let t = self.phase;
        let y = 2.0 * t - 1.0 - poly_blep(t, self.increment);
        self.phase += self.increment;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        y
    }
}

/// The polyBLEP residual of a unit downward step at phase 0, for phase `t` and increment `dt`.
fn poly_blep(t: f64, dt: f64) -> f64 {
    if dt <= 0.0 {
        0.0
    } else if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}
