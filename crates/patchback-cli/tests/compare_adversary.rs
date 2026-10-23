//! Adversary pass 1 on `patchback compare`: real WAV files at the boundaries the command
//! documents (`website/docs/reference/cli.md`, `crates/patchback-cli/src/compare.rs`).

use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_audio::{AudioClip, WavSampleFormat, write_wav};

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-compare-adversary")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn compare(reference: &Path, candidate: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args([
            "compare",
            "--reference",
            reference.to_str().unwrap(),
            "--candidate",
            candidate.to_str().unwrap(),
        ])
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn note(onset: usize, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            if i < onset {
                return 0.0;
            }
            let t = (i - onset) as f64 / 48_000.0;
            let env = (t / 0.005).min(1.0) * (-t / 0.25).exp();
            (0.6 * env * (2.0 * PI * 330.0 * t).sin()) as f32
        })
        .collect()
}

fn wav(dir: &Path, name: &str, samples: Vec<f32>, format: WavSampleFormat) -> PathBuf {
    let path = dir.join(name);
    write_wav(&path, &AudioClip::from_mono(samples).unwrap(), format).unwrap();
    path
}

/// A float32 WAV of `channels` interleaved channels, written byte by byte.
fn raw_wav(dir: &Path, name: &str, channels: u16, interleaved: &[f32]) -> PathBuf {
    let data: Vec<u8> = interleaved.iter().flat_map(|s| s.to_le_bytes()).collect();
    let block = 4 * channels;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&3u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&48_000u32.to_le_bytes());
    bytes.extend_from_slice(&(48_000 * u32::from(block)).to_le_bytes());
    bytes.extend_from_slice(&block.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn records(out: &Output) -> Vec<(String, String)> {
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));
    text(&out.stdout)
        .lines()
        .map(|line| {
            let (k, v) = line.split_once(' ').unwrap();
            (k.to_string(), v.to_string())
        })
        .collect()
}

fn value(records: &[(String, String)], key: &str) -> f64 {
    records
        .iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("no {key}"))
        .1
        .parse()
        .unwrap()
}

fn assert_refused(out: &Output, error: &str) {
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty());
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(&format!("patchback: compare refused: {error}: ")),
        "{stderr}"
    );
}

/// The printed loss is exactly the weighted sum of the printed components with the printed
/// weights: the lines read back to the values the total was computed from.
#[test]
fn adversary_printed_loss_is_exactly_the_weighted_sum_of_printed_values() {
    let dir = scratch("weighted-sum");
    let reference = wav(&dir, "r.wav", note(1_000, 48_000), WavSampleFormat::Float32);
    let other: Vec<f32> = note(1_500, 48_000).iter().map(|s| s * 0.7).collect();
    let candidate = wav(&dir, "c.wav", other, WavSampleFormat::Pcm16);
    let r = records(&compare(&reference, &candidate));
    let total = value(&r, "weight_log_spectral") * value(&r, "log_spectral")
        + value(&r, "weight_envelope") * value(&r, "envelope")
        + value(&r, "weight_transient") * value(&r, "transient")
        + value(&r, "weight_band_energy") * value(&r, "band_energy");
    assert_eq!(value(&r, "loss"), total);
    assert!(value(&r, "loss") > 0.0);
}

/// A stereo candidate whose two channels both hold the reference downmixes to the reference:
/// loss 0, lag 0.
#[test]
fn adversary_a_stereo_copy_of_a_mono_reference_compares_to_zero() {
    let dir = scratch("stereo");
    let samples = note(2_000, 30_000);
    let reference = wav(&dir, "r.wav", samples.clone(), WavSampleFormat::Float32);
    let interleaved: Vec<f32> = samples.iter().flat_map(|s| [*s, *s]).collect();
    let candidate = raw_wav(&dir, "c.wav", 2, &interleaved);
    let r = records(&compare(&reference, &candidate));
    assert_eq!(value(&r, "loss"), 0.0);
    assert_eq!(value(&r, "lag_frames"), 0.0);
}

/// Through WAV files: a candidate delayed by exactly the 4 800-sample tolerance is accepted and
/// aligned back to 4 800 with loss 0; one sample more is refused with one line, exit 1.
#[test]
fn adversary_the_tolerance_boundary_holds_through_wav_files() {
    let dir = scratch("tolerance");
    let samples = note(6_000, 48_000);
    let reference = wav(&dir, "r.wav", samples.clone(), WavSampleFormat::Float32);
    for (delay, accepted) in [(4_800usize, true), (4_801, false)] {
        let mut late = vec![0.0f32; delay];
        late.extend_from_slice(&samples);
        let candidate = wav(
            &dir,
            &format!("c{delay}.wav"),
            late,
            WavSampleFormat::Float32,
        );
        let out = compare(&reference, &candidate);
        if accepted {
            let r = records(&out);
            assert_eq!(value(&r, "lag_frames"), delay as f64);
            assert_eq!(value(&r, "loss"), 0.0);
        } else {
            assert_refused(&out, "patchback.fit.LengthOutsideAlignment");
        }
    }
}

/// An empty WAV (a data chunk of 0 frames) is refused at import with one stderr line.
#[test]
fn adversary_an_empty_wav_is_refused_with_one_line() {
    let dir = scratch("empty");
    let reference = raw_wav(&dir, "r.wav", 1, &[]);
    let candidate = wav(&dir, "c.wav", note(0, 4_800), WavSampleFormat::Float32);
    assert_refused(
        &compare(&reference, &candidate),
        "patchback.audio.EmptyAudio",
    );
}

/// A clip one sample past the 60 s analysis bound is refused as documented, with one line.
#[test]
fn adversary_a_clip_past_60_seconds_is_refused_with_duration_over_bound() {
    let dir = scratch("too-long");
    let n = 60 * 48_000 + 1;
    let reference = wav(&dir, "r.wav", vec![0.1; n], WavSampleFormat::Float32);
    let candidate = wav(&dir, "c.wav", vec![0.1; n], WavSampleFormat::Float32);
    assert_refused(
        &compare(&reference, &candidate),
        "patchback.analysis.DurationOverBound",
    );
}
