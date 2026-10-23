//! `patchback compare --reference <wav> --candidate <wav>` (DESIGN.md § 9.1, § 11): reads both
//! WAVs through `patchback-audio`, aligns the candidate with the reference and computes the
//! comparison loss with `patchback-fit` (its crate documentation defines the alignment and the
//! four components), and prints the result. It writes no file.
//!
//! The reference is read first, so a missing reference is reported even when the candidate is
//! missing too (`spec/scenarios/audio-comparison-unknown-reference.yaml`).
//!
//! # Output
//!
//! UTF-8 text on stdout, one `key value` line per record, each ending in `\n`, in this order.
//! Reals are printed as Rust prints an `f64` (the shortest form that reads back to the same
//! value, never NaN or infinite).
//!
//! | Key | Value |
//! |---|---|
//! | `loss` | the weighted sum of the four components (DESIGN.md § 9.1) |
//! | `log_spectral` | the multi-resolution log-spectral component |
//! | `envelope` | the short-time RMS envelope component |
//! | `transient` | the onset envelope component |
//! | `band_energy` | the band-energy distribution component |
//! | `weight_log_spectral` | its weight, `0.45` |
//! | `weight_envelope` | its weight, `0.25` |
//! | `weight_transient` | its weight, `0.15` |
//! | `weight_band_energy` | its weight, `0.15` |
//! | `lag_frames` | the lag the candidate was aligned by, in samples; positive when it was late |
//!
//! # Refusals
//!
//! One line on stderr, exit status 1:
//!
//! - `patchback: compare refused: patchback.fit.ClipNotFound: no clip at "<path>" (<role>)` for a
//!   reference or candidate path that does not exist;
//! - `patchback: compare refused: <audio error>: <role> "<path>": <message>` for a file
//!   `patchback-audio` refuses to import (`spec/domains/audio.yaml`; a rate other than 48 kHz is
//!   `patchback.audio.UnsupportedSampleRate`);
//! - `patchback: compare refused: <fit or analysis error>: <message>` for a comparison
//!   `patchback-fit` refuses: `patchback.fit.SampleRateMismatch`,
//!   `patchback.fit.LengthOutsideAlignment`, or the analysis bound
//!   `patchback.analysis.DurationOverBound` for a clip longer than 60 s;
//! - `patchback: compare refused: cannot read "<path>": <message>` for any other read failure.

use std::io;
use std::path::Path;

use patchback_audio::{AudioClip, ReadError, read_wav};
use patchback_fit::{CompareError, Comparison, compare};

/// Why `compare` printed nothing on stdout: the one line it prints on stderr.
pub struct Refusal(String);

impl Refusal {
    pub fn line(&self) -> &str {
        &self.0
    }

    /// A refusal is one line: any control character left in a message is escaped. Paths are
    /// quoted with `Path`'s `Debug`, which escapes them too.
    fn new(text: String) -> Self {
        Self(
            text.chars()
                .flat_map(|c| {
                    let escaped: Vec<char> = if c.is_control() {
                        c.escape_default().collect()
                    } else {
                        vec![c]
                    };
                    escaped
                })
                .collect(),
        )
    }
}

impl From<CompareError> for Refusal {
    fn from(error: CompareError) -> Self {
        Self::new(format!(
            "patchback: compare refused: {}: {error}",
            error.domain_error()
        ))
    }
}

/// Reads one clip; `role` is `reference` or `candidate`.
fn read(path: &Path, role: &str) -> Result<AudioClip, Refusal> {
    read_wav(path).map_err(|error| match error {
        ReadError::Io(error) if error.kind() == io::ErrorKind::NotFound => Refusal::new(format!(
            "patchback: compare refused: patchback.fit.ClipNotFound: no clip at {path:?} ({role})"
        )),
        ReadError::Io(error) => Refusal::new(format!(
            "patchback: compare refused: cannot read {path:?}: {error}"
        )),
        ReadError::Wav(error) => Refusal::new(format!(
            "patchback: compare refused: {}: {role} {path:?}: {error}",
            error.domain_error()
        )),
    })
}

/// Runs `compare`; on success returns the comparison.
pub fn run(reference: &Path, candidate: &Path) -> Result<Comparison, Refusal> {
    let reference = read(reference, "reference")?;
    let candidate = read(candidate, "candidate")?;
    Ok(compare(&reference, &candidate)?)
}

/// The stdout lines of `comparison`, in the order of the module documentation.
pub fn render(comparison: &Comparison) -> String {
    let c = &comparison.components;
    let w = &comparison.weights;
    let mut text = String::new();
    for (key, value) in [
        ("loss", comparison.total),
        ("log_spectral", c.log_spectral),
        ("envelope", c.envelope),
        ("transient", c.transient),
        ("band_energy", c.band_energy),
        ("weight_log_spectral", w.log_spectral),
        ("weight_envelope", w.envelope),
        ("weight_transient", w.transient),
        ("weight_band_energy", w.band_energy),
    ] {
        text.push_str(&format!("{key} {value}\n"));
    }
    text.push_str(&format!("lag_frames {}\n", comparison.lag_frames));
    text
}
