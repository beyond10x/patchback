//! Adversarial cases, pass 2, against `patchback analyze` (story:analysis-core, wave a3): the
//! refusals correction round 2 added, through the binary.

use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_audio::{AudioClip, WavSampleFormat, write_wav};

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-adversary2")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn analyze(input: &Path, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .arg("analyze")
        .arg("--input")
        .arg(input)
        .arg("--out")
        .arg(out)
        .output()
        .expect("the patchback binary runs")
}

fn sine(frequency_hz: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (0.5 * (2.0 * PI * frequency_hz * i as f64 / 48_000.0).sin()) as f32)
        .collect()
}

fn wav(dir: &Path, name: &str, samples: Vec<f32>) -> PathBuf {
    let path = dir.join(name);
    write_wav(
        &path,
        &AudioClip::from_mono(samples).unwrap(),
        WavSampleFormat::Float32,
    )
    .unwrap();
    path
}

fn assert_one_line_refusal(out: &Output, error: &str, out_dir: &Path) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty());
    assert_eq!(stderr.matches('\n').count(), 1, "{stderr:?}");
    assert!(
        stderr.starts_with(&format!("patchback: analyze refused: {error}: ")),
        "{stderr:?}"
    );
    assert!(!out_dir.exists(), "a refusal created {}", out_dir.display());
}

/// A 3 kHz sine: `PitchOutOfRange`, one line, nothing written. Green.
#[test]
fn adversary_cli2_pitch_above_the_range_is_refused() {
    let dir = scratch("out-of-range");
    let input = wav(&dir, "3k.wav", sine(3_000.0, 48_000));
    let out_dir = dir.join("analysis");
    let out = analyze(&input, &out_dir);
    assert_one_line_refusal(&out, "patchback.analysis.PitchOutOfRange", &out_dir);
}

/// 2100 samples of 200 Hz: `ClipTooShortForPitch`, one line, nothing written. Green.
#[test]
fn adversary_cli2_clip_too_short_for_pitch_is_refused() {
    let dir = scratch("too-short");
    let input = wav(&dir, "short.wav", sine(200.0, 2_100));
    let out_dir = dir.join("analysis");
    let out = analyze(&input, &out_dir);
    assert_one_line_refusal(&out, "patchback.analysis.ClipTooShortForPitch", &out_dir);
}

/// A refusal on a path holding a newline, a tab and an escape character stays one line. Green.
#[test]
fn adversary_cli2_control_characters_in_the_path_stay_on_one_line() {
    let dir = scratch("control");
    let input = dir.join("a\nb\tc\u{1b}[31md.wav");
    let out_dir = dir.join("analysis");
    let out = analyze(&input, &out_dir);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert_eq!(stderr.matches('\n').count(), 1, "{stderr:?}");
    assert!(
        !stderr.contains('\u{1b}') && !stderr.contains('\t'),
        "{stderr:?}"
    );
}
