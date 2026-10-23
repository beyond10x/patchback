//! FFT and STFT, envelopes, spectral descriptors and monophonic pitch (DESIGN.md § 4.1, § 6.2),
//! in Rust `std` only (DESIGN.md § 3).
//!
//! # Pieces
//!
//! - [`fft`] and [`ifft`]: an iterative radix-2 complex FFT over [`Complex`], for power-of-two
//!   lengths up to [`MAX_FFT_LENGTH`]; [`ifft`] scales by `1 / n`, so `ifft(fft(x)) == x` within
//!   rounding.
//! - [`hann`]: the periodic Hann window `w[k] = 0.5 - 0.5 cos(2 pi k / n)`, whose energy
//!   `sum w[k]^2` is `3n / 8` for `n >= 3`.
//! - [`stft`]: Hann-windowed frames of one FFT length and hop, their magnitude spectra
//!   (`n / 2 + 1` bins, unnormalised `|X_k|`) and [`log_magnitude`] in dB.
//! - [`rms`] and [`peak`] over a frame of samples; [`spectral_centroid`], [`spectral_rolloff`],
//!   [`spectral_flatness`], [`log_band_energies`] and [`spectral_flux`] over magnitude spectra.
//! - [`estimate_f0`]: a YIN-style cumulative mean normalised difference function with a
//!   confidence, refused as [`AnalysisError::PitchAmbiguous`] below [`F0_CONFIDENCE_THRESHOLD`], as
//!   [`AnalysisError::PitchOutOfRange`] outside [`F0_MIN_HZ`] to [`F0_MAX_HZ`] and as
//!   [`AnalysisError::ClipTooShortForPitch`] in a clip too short to search.
//! - [`cross_correlation`]: `sum_n a[n] b[n + d]` over a bounded lag range, by blocks of [`fft`],
//!   for the time alignment of the comparison loss (DESIGN.md § 9.1).
//! - [`analyze`]: the whole of `patchback analyze` over one clip.
//!
//! Every function refuses a non-finite input value ([`AnalysisError::NonFiniteSamples`]) and
//! returns finite values or a typed [`AnalysisError`] (DESIGN.md § 2 invariant 4).
//!
//! # Provisional values
//!
//! DESIGN.md § 12.3 asks for bounds and § 6.2 for thresholds without naming values (OQ-6, OQ-15,
//! OQ-16, `spec/domains/analysis.yaml` `UNMAPPED:`). Every such value is a named constant here
//! and provisional until the owner sets it: [`MAX_FFT_LENGTH`], [`MAX_DURATION_FRAMES`],
//! [`MAX_STFT_VALUES`], [`HOP_DIVISOR`], [`SILENCE_THRESHOLD_DBFS`],
//! [`F0_CONFIDENCE_THRESHOLD`], [`F0_MIN_HZ`], [`F0_MAX_HZ`], [`F0_WINDOW`],
//! [`F0_RANGE_TOLERANCE_CENTS`], [`ROLLOFF_FRACTION`], [`BAND_EDGES_HZ`] and [`LOG_FLOOR_DB`].

mod analyze;
mod complex;
mod correlation;
mod descriptors;
mod error;
mod fft;
mod pitch;
mod stft;
mod window;

pub use analyze::{
    Analysis, AnalysisConfig, FrameDescriptors, StftAnalysis, StftConfig, analyze, analyze_clip,
};
pub use complex::Complex;
pub use correlation::{cross_correlation, cross_correlation_fft_length};
pub use descriptors::{
    BAND_COUNT, log_band_energies, peak, rms, spectral_centroid, spectral_flatness, spectral_flux,
    spectral_rolloff,
};
pub use error::AnalysisError;
pub use fft::{fft, ifft};
pub use pitch::{F0_RANGE_TOLERANCE_CENTS, PitchEstimate, estimate_f0};
pub use stft::{Stft, log_magnitude, stft};
pub use window::hann;

/// The only sample rate analysis accepts (DESIGN.md § 5.3, § 6.1); any other is refused.
pub const SAMPLE_RATE_HZ: u32 = patchback_audio::SAMPLE_RATE_HZ;

/// The analysis FFT lengths at 48 kHz (DESIGN.md § 6.3).
pub const ANALYSIS_FFT_LENGTHS: [usize; 3] = [512, 2048, 8192];

/// The longest FFT, and so the longest window, analysis computes (DESIGN.md § 12.3).
///
/// Provisional: DESIGN.md names no value (OQ-6). 65 536 samples, eight times the longest
/// analysis length of § 6.3.
pub const MAX_FFT_LENGTH: usize = 65_536;

/// The most samples analysis accepts (DESIGN.md § 12.3).
///
/// Provisional: DESIGN.md names no value (OQ-6). Sixty seconds at 48 kHz, far above the
/// 0.5–2 s analysis unit of § 6.1, and below the ten-minute import bound of `patchback-audio`.
pub const MAX_DURATION_FRAMES: usize = 60 * SAMPLE_RATE_HZ as usize;

/// The most magnitudes the STFTs of one analysis may hold together, each STFT holding its frame
/// count `ceil(len / hop)` times `fft_length / 2 + 1` bins. [`analyze`] checks the sum over the
/// whole [`AnalysisConfig`] and [`stft`] its own count, before anything is allocated, so a tiny
/// hop or a long list of STFTs cannot exhaust memory (DESIGN.md § 12.3).
///
/// Provisional: DESIGN.md names no value (OQ-6). 2^25 magnitudes, 256 MiB of `f64`; the
/// default configuration over [`MAX_DURATION_FRAMES`] needs under 9 million in all.
pub const MAX_STFT_VALUES: usize = 1 << 25;

/// The default hop is the FFT length divided by this: 50 % overlap.
///
/// Provisional: DESIGN.md § 6.3 asks for "consistent overlap/window conventions" and names no
/// hop (OQ-15).
pub const HOP_DIVISOR: usize = 2;

/// A clip whose RMS level is below this many dBFS is silent and refused (DESIGN.md § 6.2).
///
/// Provisional: DESIGN.md names no threshold (OQ-17).
pub const SILENCE_THRESHOLD_DBFS: f64 = -60.0;

/// An f0 estimate whose confidence is below this is refused as ambiguous (DESIGN.md § 6.2).
///
/// Provisional: DESIGN.md names no threshold (OQ-16). Confidence is `1 - d'(tau)`, so this is
/// the YIN absolute threshold of 0.15.
pub const F0_CONFIDENCE_THRESHOLD: f64 = 0.85;

/// The lowest f0 the estimator searches, in Hz. Provisional.
pub const F0_MIN_HZ: f64 = 50.0;

/// The highest f0 the estimator searches, in Hz. Provisional.
pub const F0_MAX_HZ: f64 = 2_000.0;

/// The integration window of the f0 difference function, in samples. Provisional.
pub const F0_WINDOW: usize = 2_048;

/// The spectral rolloff is the frequency below which this fraction of the power lies.
/// Provisional.
pub const ROLLOFF_FRACTION: f64 = 0.85;

/// The edges of the eight logarithmic (octave) bands of [`log_band_energies`], in Hz at 48 kHz.
/// Band `b` holds the bins with `BAND_EDGES_HZ[b] <= f < BAND_EDGES_HZ[b + 1]`; the last band
/// also holds the Nyquist bin.
///
/// Provisional: DESIGN.md § 6.3 names eight logarithmic bands and no edges (OQ-15).
pub const BAND_EDGES_HZ: [f64; 9] = [
    0.0, 187.5, 375.0, 750.0, 1_500.0, 3_000.0, 6_000.0, 12_000.0, 24_000.0,
];

/// The floor of every logarithmic value, in dB: a zero magnitude or energy is reported as this
/// rather than as minus infinity. Provisional.
pub const LOG_FLOOR_DB: f64 = -120.0;

/// Refuses the first non-finite value of `values`.
pub(crate) fn check_finite<T: Copy + Into<f64>>(values: &[T]) -> Result<(), AnalysisError> {
    match values.iter().position(|v| !(*v).into().is_finite()) {
        Some(index) => Err(AnalysisError::NonFiniteSamples { index }),
        None => Ok(()),
    }
}
