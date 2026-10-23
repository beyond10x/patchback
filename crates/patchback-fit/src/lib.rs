//! Candidate search and the audio comparison loss (DESIGN.md § 4.1, § 9), in Rust `std` only
//! (DESIGN.md § 3), on the FFT, STFT and descriptors of `patchback-analysis`.
//!
//! # The comparison
//!
//! [`compare`] (and [`compare_signals`] on bare samples) compares a candidate rendering with a
//! reference recording, in this order (`spec/domains/fit.yaml`, `CompareAudio`):
//!
//! 1. The two sample rates must be equal ([`CompareError::SampleRateMismatch`]); clips are never
//!    converted. The common rate must be the 48 kHz analysis rate.
//! 2. Each clip must hold samples, at most `patchback_analysis::MAX_DURATION_FRAMES` of them, all
//!    finite.
//! 3. The lengths may differ by at most [`ALIGNMENT_TOLERANCE_FRAMES`]
//!    ([`CompareError::LengthOutsideAlignment`]).
//! 4. Time alignment (DESIGN.md § 9.1: before any term): the lag `d` in
//!    `-ALIGNMENT_TOLERANCE_FRAMES..=ALIGNMENT_TOLERANCE_FRAMES` that maximises the
//!    cross-correlation `sum_k e_r[k] e_c[k + d]` of the two energy envelopes is found
//!    (`patchback_analysis::cross_correlation`); among lags within `1e-12` of the maximum, relative
//!    to it, the one nearest 0 wins, then the negative one. The energy envelope is
//!    `(x / max |x|)^2` smoothed twice by a moving mean of [`ALIGNMENT_SMOOTHING_FRAMES`] samples,
//!    over every sample where it is not 0: it aligns onsets and level contours, a delayed copy
//!    aligns back to its exact delay, and two steady tones of different pitch align within a few
//!    samples of 0 rather than at a lag where their waveforms happen to agree. The aligned
//!    candidate is `candidate[n + d]` for each sample `n` of the reference, 0 outside the
//!    candidate, so it has the reference's length. A positive `d` means the candidate was late.
//! 5. The four components of [`LossComponents`] are computed on the reference and the aligned
//!    candidate, and the total is their weighted sum by [`LOSS_WEIGHTS`].
//!
//! Note matching (DESIGN.md § 9.1) is not done: a comparison is of one clip with one clip.
//!
//! # The components
//!
//! Every component is finite and non-negative for every input the comparison accepts, and 0 when
//! the two clips are identical (DESIGN.md § 2 invariant 4). Hops are the FFT length divided by
//! `patchback_analysis::HOP_DIVISOR`; spectra are the Hann-windowed STFTs of
//! `patchback_analysis::stft`.
//!
//! - `log_spectral`: for each FFT length `n` of [`LOG_SPECTRAL_FFT_LENGTHS`], the normalised
//!   difference, over every frame and bin, of the log magnitudes above the floor
//!   `ln(EPSILON + |X| / (n / 2)) - ln(EPSILON)`; the mean over the lengths. The absolute
//!   difference of two such values is the absolute difference of `log(epsilon + |STFT|)` of
//!   DESIGN.md § 9.1; the sum it is divided by is its normalisation. `n / 2` is the sum of the
//!   Hann window, so a sine of amplitude `a` peaks near `a / 2` at every length. The loss grows
//!   with the detuning of a sine from 110 to 1760 Hz over 0 to 800 cents in doubling steps;
//!   between finer steps it follows the window's leakage as well as the pitch (see
//!   [`EPSILON`]).
//! - `envelope`: the normalised difference of the short-time RMS curves. The level of a frame
//!   of the STFT at length `n` is `sqrt(sum_k |X_k|^2) / (n / 2)`, the Hann-windowed RMS in the
//!   unit of the log-spectral magnitudes; the curve is the levels at [`ENVELOPE_FFT_LENGTH`].
//! - `transient`: the normalised difference of the onset envelopes. The onset envelope is the
//!   rise `max(0, ln(EPSILON + l_t) - ln(EPSILON + l_{t-1}))` of the levels `l` at
//!   [`TRANSIENT_FFT_LENGTH`], the level before the first frame being 0. It is taken on the
//!   level rather than on the spectral flux of `patchback_analysis::spectral_flux`: the flux of
//!   a steady tone follows its phase from frame to frame, so for faded 440 Hz sines the flux
//!   version scored 0.096 to 0.101 at every detuning from 3.125 to 800 cents, a floor that does
//!   not grow with the difference; the level version scores about 4e-5.
//! - `band_energy`: the total variation distance `0.5 sum_b |p_b - q_b|` between the two
//!   distributions of energy over the eight bands of `patchback_analysis::log_band_energies`,
//!   summed over the frames of the STFT at [`BAND_ENERGY_FFT_LENGTH`] (a band at the analysis
//!   floor counts as no energy). 0 when neither clip has energy, 1 when exactly one has.
//!
//! The normalised difference of two non-negative curves `a` and `b` is
//! `sum |a_t - b_t| / (sum a_t + sum b_t)`, in `[0, 1]`, computed on the curves divided by their
//! largest value so no sum overflows; 0 when both are all zero.
//!
//! # Provisional values
//!
//! DESIGN.md § 9.1 gives the weights as hypotheses and names no alignment tolerance, epsilon or
//! component definition beyond the sentence of each (OQ-11, `spec/domains/fit.yaml`
//! `UNMAPPED:`). Every such value is a named constant here and provisional until the owner sets
//! it: [`ALIGNMENT_TOLERANCE_FRAMES`], [`ALIGNMENT_SMOOTHING_FRAMES`], [`EPSILON`],
//! [`ENVELOPE_FFT_LENGTH`], [`TRANSIENT_FFT_LENGTH`] and [`BAND_ENERGY_FFT_LENGTH`]. The weights
//! are the § 9.1 values; no caller supplies its own yet.

mod error;
mod loss;

pub use error::CompareError;
pub use loss::{Comparison, LossComponents, Signal, compare, compare_signals, loss_components};

/// The weight of the log-spectral component (DESIGN.md § 9.1).
pub const WEIGHT_LOG_SPECTRAL: f64 = 0.45;
/// The weight of the envelope component (DESIGN.md § 9.1).
pub const WEIGHT_ENVELOPE: f64 = 0.25;
/// The weight of the transient component (DESIGN.md § 9.1).
pub const WEIGHT_TRANSIENT: f64 = 0.15;
/// The weight of the band-energy component (DESIGN.md § 9.1).
pub const WEIGHT_BAND_ENERGY: f64 = 0.15;

/// The weights every comparison uses: the four values of DESIGN.md § 9.1, summing to 1.
pub const LOSS_WEIGHTS: LossWeights = LossWeights {
    log_spectral: WEIGHT_LOG_SPECTRAL,
    envelope: WEIGHT_ENVELOPE,
    transient: WEIGHT_TRANSIENT,
    band_energy: WEIGHT_BAND_ENERGY,
};

/// How far the lengths of the reference and the candidate may differ, and how far the candidate
/// may be shifted to align it, in samples.
///
/// Provisional: DESIGN.md § 9.1 names no tolerance (OQ-11). 4 800 samples, 100 ms at 48 kHz.
pub const ALIGNMENT_TOLERANCE_FRAMES: usize = 4_800;

/// The `epsilon` of `log(epsilon + |STFT|)` (DESIGN.md § 9.1), on magnitudes divided by the
/// window sum, where a full-scale sine peaks at 0.5: about 44 dB below that peak. Also the floor
/// of the onset envelope's logarithm.
///
/// Provisional: DESIGN.md § 9.1 names no value (OQ-11). Measured on 1 s sines from 110 to
/// 1760 Hz detuned by 0, 3.125, 6.25, ... 800 cents: at 3e-3 the loss rises at every step; at
/// 1e-3 it does not (1000 Hz: 6.25 cents 0.0266, below 3.125 cents 0.0306), nor at 1e-5 (440 Hz:
/// 50 cents 0.0447, below 25 cents 0.0492). A deeper floor exposes the Hann sidelobes, whose
/// level depends on where a tone falls between bins rather than on how far it is from the
/// reference; it also compresses less of a quiet partial. The trade-off is the owner's.
pub const EPSILON: f64 = 3e-3;

/// The FFT lengths of the multi-resolution log-spectral component: the analysis lengths of
/// DESIGN.md § 6.3.
pub const LOG_SPECTRAL_FFT_LENGTHS: [usize; 3] = patchback_analysis::ANALYSIS_FFT_LENGTHS;

/// The width of the moving mean that smooths the energy envelope alignment correlates, twice, in
/// samples. Provisional: about 85 ms at 48 kHz. Measured on faded 1 s sines from 110 to 1760 Hz
/// against their copies detuned by 3.125 to 800 cents: smoothed once by 1 024 samples, 110 Hz
/// detuned by 3.125 cents aligned at -36, the ripple of the squared tone outweighing the
/// overlap; smoothed once by 4 096, at -33; smoothed twice by 4 096, within 2 samples of 0 at
/// 110 Hz and at 0 everywhere else. The twice-smoothed envelope of a delayed copy is the same
/// envelope delayed, so shifted copies align back exactly. With the tolerance it sets the
/// correlation's reach, `ALIGNMENT_TOLERANCE_FRAMES + 2 (w - 1)`, whose FFT is at the 65 536
/// bound of `patchback_analysis::MAX_FFT_LENGTH` at this width.
pub const ALIGNMENT_SMOOTHING_FRAMES: usize = 4_096;

/// The FFT length of the frames of the envelope component's short-time RMS curve.
/// Provisional: about 21 ms at 48 kHz.
pub const ENVELOPE_FFT_LENGTH: usize = 1_024;

/// The FFT length of the frames of the transient component's onset envelope. Provisional: the
/// shortest analysis length, for the finest time resolution.
pub const TRANSIENT_FFT_LENGTH: usize = 512;

/// The FFT length of the band-energy component. Provisional: the middle analysis length, whose
/// 23.4 Hz bins put eight bins in the lowest band.
pub const BAND_ENERGY_FFT_LENGTH: usize = 2_048;

/// The weights of the four components (DESIGN.md § 9.1; `spec/domains/fit.yaml`
/// `LossWeights`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LossWeights {
    pub log_spectral: f64,
    pub envelope: f64,
    pub transient: f64,
    pub band_energy: f64,
}

impl LossWeights {
    /// The sum of the four weights.
    pub fn sum(&self) -> f64 {
        self.log_spectral + self.envelope + self.transient + self.band_energy
    }

    /// The weighted sum of `components`: the loss `L` of DESIGN.md § 9.1.
    pub fn total(&self, components: &LossComponents) -> f64 {
        self.log_spectral * components.log_spectral
            + self.envelope * components.envelope
            + self.transient * components.transient
            + self.band_energy * components.band_energy
    }
}
