//! `patchback compare --reference <wav> --candidate <wav>` on WAV files the tests synthesize: the
//! acceptance scenarios of story:audio-comparison (`spec/scenarios/audio-comparison-*.yaml`)
//! through the binary, with exit 0 (compared), 1 (refused, one stderr line naming the domain
//! error) and 2 (usage error).

use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_audio::{AudioClip, WavSampleFormat, write_wav};

/// The `key value` lines `compare` prints, in order.
const KEYS: [&str; 10] = [
    "loss",
    "log_spectral",
    "envelope",
    "transient",
    "band_energy",
    "weight_log_spectral",
    "weight_envelope",
    "weight_transient",
    "weight_band_energy",
    "lag_frames",
];

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-compare")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(args)
        .output()
        .expect("the patchback binary runs")
}

fn compare(reference: &Path, candidate: &Path) -> Output {
    run(&[
        "compare",
        "--reference",
        reference.to_str().unwrap(),
        "--candidate",
        candidate.to_str().unwrap(),
    ])
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("output is UTF-8")
}

fn sine(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (amplitude * (2.0 * PI * frequency_hz * i as f64 / 48_000.0).sin()) as f32)
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

/// A mono float32 WAV written byte by byte, at a rate `write_wav` does not write.
fn raw_wav(dir: &Path, name: &str, sample_rate: u32, samples: &[f32]) -> PathBuf {
    let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&3u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

/// The `key value` lines of a successful comparison, checked for shape: exactly [`KEYS`], in
/// order, one value each.
fn records(out: &Output) -> Vec<(String, String)> {
    let stdout = text(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));
    assert!(stdout.ends_with('\n'), "{stdout}");
    let records: Vec<(String, String)> = stdout
        .lines()
        .map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            assert_eq!(fields.len(), 2, "{line}");
            (fields[0].to_owned(), fields[1].to_owned())
        })
        .collect();
    let keys: Vec<&str> = records.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, KEYS);
    records
}

fn real(records: &[(String, String)], key: &str) -> f64 {
    let value = &records.iter().find(|(k, _)| k == key).unwrap().1;
    let number: f64 = value.parse().unwrap_or_else(|_| panic!("{key} {value}"));
    assert!(number.is_finite(), "{key} {value}");
    number
}

/// Exit 1, nothing on stdout, exactly one stderr line naming `error` and containing `names`.
fn assert_refused(out: &Output, error: &str, names: &str) {
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty(), "{}", text(&out.stdout));
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(&format!("patchback: compare refused: {error}: ")),
        "{stderr}"
    );
    assert!(stderr.contains(names), "{stderr} does not name {names}");
}

/// audio-comparison-records-components: two different one-second clips are compared; the loss,
/// its four components and the weights are printed, and the loss is their weighted sum.
#[test]
fn compare_prints_the_loss_and_its_components() {
    let dir = scratch("records-components");
    let reference = wav(&dir, "reference.wav", sine(440.0, 0.5, 48_000));
    let candidate = wav(&dir, "candidate.wav", sine(466.0, 0.3, 48_000));
    let out = compare(&reference, &candidate);
    let records = records(&out);
    let weights = [
        real(&records, "weight_log_spectral"),
        real(&records, "weight_envelope"),
        real(&records, "weight_transient"),
        real(&records, "weight_band_energy"),
    ];
    assert_eq!(weights, [0.45, 0.25, 0.15, 0.15]);
    let components = [
        real(&records, "log_spectral"),
        real(&records, "envelope"),
        real(&records, "transient"),
        real(&records, "band_energy"),
    ];
    let expected: f64 = weights.iter().zip(components).map(|(w, c)| w * c).sum();
    let loss = real(&records, "loss");
    assert!(loss > 0.0, "{loss}");
    assert!((loss - expected).abs() <= 1e-12, "{loss} != {expected}");
    let lag: i64 = records[9].1.parse().unwrap();
    assert!(lag.unsigned_abs() <= 4_800, "{lag}");
    let entries: Vec<_> = fs::read_dir(&dir).unwrap().collect();
    assert_eq!(entries.len(), 2, "compare writes nothing");
}

#[test]
fn a_clip_compared_with_itself_has_loss_zero() {
    let dir = scratch("identical");
    let reference = wav(&dir, "reference.wav", sine(220.0, 0.5, 24_000));
    let out = compare(&reference, &reference);
    let records = records(&out);
    for key in [
        "loss",
        "log_spectral",
        "envelope",
        "transient",
        "band_energy",
    ] {
        assert_eq!(real(&records, key), 0.0, "{key}");
    }
    assert_eq!(records[9].1, "0");
}

/// A candidate rendered 1 000 samples late is aligned back before the loss.
#[test]
fn a_late_candidate_is_aligned_and_its_lag_printed() {
    let dir = scratch("late");
    let mut burst = vec![0.0f32; 3_000];
    burst.extend(sine(330.0, 0.5, 20_000));
    burst.extend(vec![0.0f32; 5_000]);
    let mut late = vec![0.0f32; 1_000];
    late.extend_from_slice(&burst[..burst.len() - 1_000]);
    let reference = wav(&dir, "reference.wav", burst);
    let candidate = wav(&dir, "candidate.wav", late);
    let records = records(&compare(&reference, &candidate));
    assert_eq!(records[9].1, "1000");
}

/// audio-comparison-unknown-reference: a reference that does not exist is refused with
/// ClipNotFound naming it, before the candidate (which does not exist either) is looked at.
#[test]
fn an_unknown_reference_is_refused_with_clip_not_found() {
    let dir = scratch("unknown-reference");
    let candidate = wav(&dir, "candidate.wav", sine(440.0, 0.5, 4_800));
    let missing = dir.join("never-imported.wav");
    assert_refused(
        &compare(&missing, &candidate),
        "patchback.fit.ClipNotFound",
        "never-imported.wav",
    );
    let also_missing = dir.join("also-missing.wav");
    let out = compare(&missing, &also_missing);
    assert_refused(&out, "patchback.fit.ClipNotFound", "never-imported.wav");
    assert!(!text(&out.stderr).contains("also-missing.wav"));
}

/// audio-comparison-records-components, its last step: a candidate never imported is refused.
#[test]
fn an_unknown_candidate_is_refused_with_clip_not_found() {
    let dir = scratch("unknown-candidate");
    let reference = wav(&dir, "reference.wav", sine(440.0, 0.5, 4_800));
    assert_refused(
        &compare(&reference, &dir.join("never-imported.wav")),
        "patchback.fit.ClipNotFound",
        "never-imported.wav",
    );
}

/// audio-comparison-length-outside-alignment: a candidate 4 801 samples longer than the
/// reference is refused; one 4 800 samples longer is compared.
#[test]
fn lengths_alignment_cannot_absorb_are_refused() {
    let dir = scratch("length");
    let reference = wav(&dir, "reference.wav", sine(440.0, 0.5, 48_000));
    let inside = wav(&dir, "inside.wav", sine(440.0, 0.5, 52_800));
    records(&compare(&reference, &inside));
    let outside = wav(&dir, "outside.wav", sine(440.0, 0.5, 52_801));
    assert_refused(
        &compare(&reference, &outside),
        "patchback.fit.LengthOutsideAlignment",
        "52801",
    );
}

/// audio-comparison-sample-rate-mismatch: import admits 48 kHz only, so a 44.1 kHz candidate is
/// refused by import (`patchback.audio.UnsupportedSampleRate`) before the comparison's own rate
/// check, which `crates/patchback-fit/tests/loss.rs` holds.
#[test]
fn a_candidate_at_another_sample_rate_is_refused() {
    let dir = scratch("sample-rate");
    let reference = wav(&dir, "reference.wav", sine(440.0, 0.5, 4_800));
    let candidate = raw_wav(&dir, "candidate.wav", 44_100, &sine(440.0, 0.5, 4_410));
    assert_refused(
        &compare(&reference, &candidate),
        "patchback.audio.UnsupportedSampleRate",
        "candidate.wav",
    );
}

#[test]
fn a_file_that_is_not_a_wav_is_refused_with_its_audio_error() {
    let dir = scratch("not-wav");
    let reference = dir.join("reference.wav");
    fs::write(&reference, b"not a wav file at all").unwrap();
    let candidate = wav(&dir, "candidate.wav", sine(440.0, 0.5, 4_800));
    let out = compare(&reference, &candidate);
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.starts_with("patchback: compare refused: patchback.audio."),
        "{stderr}"
    );
    assert!(stderr.contains("reference.wav"), "{stderr}");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
}

#[test]
fn compare_without_its_arguments_is_a_usage_error_and_exits_2() {
    let out = run(&["compare"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = text(&out.stderr);
    assert!(stderr.starts_with("error: "), "{stderr}");
    assert!(stderr.contains("--reference <REFERENCE>"), "{stderr}");
    assert!(stderr.contains("--candidate <CANDIDATE>"), "{stderr}");
    let out = run(&["compare", "--reference", "a.wav"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("--candidate <CANDIDATE>"));
}
