//! `patchback analyze --input <wav> --out <dir>` (DESIGN.md § 11): reads the WAV through
//! `patchback-audio`, analyses it with `patchback-analysis` at the default configuration
//! (FFT lengths 512, 2048 and 8192, hop half the FFT length, Hann window) and writes one
//! analysis file, [`FILE`], into `<dir>`.
//!
//! All or nothing: the file is written only after the whole analysis succeeded, so a refusal
//! (the f0 estimate included) writes nothing, and `<dir>` is created (with its parents) only
//! then. An existing `<dir>/analysis.txt` is never overwritten.
//!
//! # The analysis file, version 1
//!
//! UTF-8 text, one record per line, every line ending in `\n`, fields separated by one space.
//! The first line is the header `patchback-analysis 1`. Every other line is a key followed by its
//! values. Numbers are decimal: integers as such, reals as Rust prints an `f64` (shortest form
//! that reads back to the same value, never NaN or infinite). In this order:
//!
//! | Key | Values |
//! |---|---|
//! | `sample_rate_hz` | the clip's sample rate, `48000` |
//! | `frame_count` | the clip's number of samples |
//! | `window` | `hann` (the periodic Hann window) |
//! | `fft_lengths` | each STFT's FFT length, in order |
//! | `hop_lengths` | each STFT's hop in samples, in the same order |
//! | `band_edges_hz` | the nine edges of the eight log-band energies, in Hz |
//! | `rms_dbfs` | the clip's RMS level in dBFS |
//! | `peak` | the clip's largest absolute sample |
//! | `f0_hz` | the estimated fundamental frequency in Hz |
//! | `f0_confidence` | its confidence, in `[0, 1]` |
//!
//! Then, for each STFT in the order of `fft_lengths`, one `stft` line followed by its `frame`
//! lines:
//!
//! | Key | Values |
//! |---|---|
//! | `stft` | FFT length, hop length, number of frames |
//! | `frame` | FFT length, frame index from 0, then `rms peak centroid_hz rolloff_hz flatness flux` and the eight band energies in dB, low band first |
//!
//! Frame `i` starts at sample `i * hop`; its spectrum is the Hann-windowed FFT of the samples
//! from there, zero past the end of the clip.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use patchback_analysis::{Analysis, AnalysisConfig, AnalysisError, BAND_EDGES_HZ, analyze_clip};
use patchback_audio::{ReadError, WavError, read_wav};

/// The analysis file's name inside `--out`.
pub const FILE: &str = "analysis.txt";

/// The analysis file's header line.
pub const HEADER: &str = "patchback-analysis 1";

/// Why `analyze` wrote nothing: the one line it prints on stderr.
pub struct Refusal(String);

impl Refusal {
    pub fn line(&self) -> &str {
        &self.0
    }

    fn domain(error: &str, message: impl std::fmt::Display) -> Self {
        Self::one_line(format!("patchback: analyze refused: {error}: {message}"))
    }

    /// The path is quoted with its control characters escaped (`Path` `Debug`), so a newline in
    /// a file name cannot split the refusal.
    fn io(what: &str, path: &Path, error: impl std::fmt::Display) -> Self {
        Self::one_line(format!(
            "patchback: analyze refused: {what} {path:?}: {error}"
        ))
    }

    /// A refusal is one line: any control character left in a message is escaped.
    fn one_line(text: String) -> Self {
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

impl From<AnalysisError> for Refusal {
    fn from(error: AnalysisError) -> Self {
        Self::domain(error.domain_error(), error)
    }
}

/// A WAV refusal that the analysis domain names too is reported with the analysis error
/// (`spec/domains/analysis.yaml` `AnalyzeAudio`): no frames, a rate other than 48 kHz, a
/// non-finite sample, and a clip past the import bound, which is also past the analysis
/// duration bound. Every other refusal keeps its `patchback.audio` error.
fn wav_refusal(error: WavError) -> Refusal {
    let analysis = match error {
        WavError::EmptyAudio => Some("patchback.analysis.EmptyAudio"),
        WavError::UnsupportedSampleRate { .. } => Some("patchback.analysis.UnsupportedSampleRate"),
        WavError::NonFiniteSample { .. } => Some("patchback.analysis.NonFiniteSamples"),
        WavError::InputTooLong { .. } => Some("patchback.analysis.DurationOverBound"),
        _ => None,
    };
    Refusal::domain(analysis.unwrap_or(error.domain_error()), error)
}

/// Runs `analyze`; on success returns the path of the file it wrote.
pub fn run(input: &Path, out: &Path) -> Result<PathBuf, Refusal> {
    let target = out.join(FILE);
    if target.exists() {
        return Err(Refusal::io(
            "the analysis file",
            &target,
            "already exists; it is never overwritten",
        ));
    }
    let clip = read_wav(input).map_err(|error| match error {
        ReadError::Wav(error) => wav_refusal(error),
        ReadError::Io(error) => Refusal::io("cannot read", input, error),
    })?;
    let analysis = analyze_clip(&clip, &AnalysisConfig::default())?;
    let text = render(&analysis);
    fs::create_dir_all(out).map_err(|error| Refusal::io("cannot create", out, error))?;
    write_new(&target, text.as_bytes())
        .map_err(|error| Refusal::io("cannot write", &target, error))?;
    Ok(target)
}

/// Writes `bytes` to a file that must not exist yet; a failed write removes the partial file.
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if written.is_err() {
        let _ = fs::remove_file(path);
    }
    written
}

/// The analysis file, version 1, of `analysis`.
pub fn render(analysis: &Analysis) -> String {
    let mut text = String::new();
    let mut line = |key: &str, values: &[String]| {
        text.push_str(key);
        for value in values {
            text.push(' ');
            text.push_str(value);
        }
        text.push('\n');
    };
    line(HEADER, &[]);
    line("sample_rate_hz", &[analysis.sample_rate_hz.to_string()]);
    line("frame_count", &[analysis.frame_count.to_string()]);
    line("window", &["hann".to_owned()]);
    line(
        "fft_lengths",
        &analysis
            .stfts
            .iter()
            .map(|s| s.fft_length.to_string())
            .collect::<Vec<_>>(),
    );
    line(
        "hop_lengths",
        &analysis
            .stfts
            .iter()
            .map(|s| s.hop_length.to_string())
            .collect::<Vec<_>>(),
    );
    line(
        "band_edges_hz",
        &BAND_EDGES_HZ.iter().map(f64::to_string).collect::<Vec<_>>(),
    );
    line("rms_dbfs", &[analysis.rms_dbfs.to_string()]);
    line("peak", &[analysis.peak.to_string()]);
    line("f0_hz", &[analysis.f0.f0_hz.to_string()]);
    line("f0_confidence", &[analysis.f0.confidence.to_string()]);
    for stft in &analysis.stfts {
        line(
            "stft",
            &[
                stft.fft_length.to_string(),
                stft.hop_length.to_string(),
                stft.frames.len().to_string(),
            ],
        );
        for (index, frame) in stft.frames.iter().enumerate() {
            let mut values = vec![stft.fft_length.to_string(), index.to_string()];
            values.extend(
                [
                    frame.rms,
                    frame.peak,
                    frame.centroid_hz,
                    frame.rolloff_hz,
                    frame.flatness,
                    frame.flux,
                ]
                .iter()
                .chain(&frame.band_energies_db)
                .map(f64::to_string),
            );
            line("frame", &values);
        }
    }
    text
}
