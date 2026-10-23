//! `patchback analyze --input <wav> --out <dir>` on WAV files the tests synthesize: the
//! acceptance scenarios of story:analysis-core (`spec/scenarios/analysis-core-*.yaml`) through
//! the binary, with exit 0 (analysed), 1 (refused, one stderr line naming the domain error) and
//! 2 (usage error).

use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_audio::{AudioClip, WavSampleFormat, write_wav};

/// The analysis file `analyze` writes into `--out`.
const FILE: &str = "analysis.txt";

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-analyze")
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

fn analyze(input: &Path, out: &Path) -> Output {
    run(&[
        "analyze",
        "--input",
        input.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
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

fn noise(count: usize) -> Vec<f32> {
    let mut state: u64 = 5;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (((state >> 11) as f64 / (1u64 << 53) as f64) - 0.5) as f32
        })
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

/// A mono float32 WAV written byte by byte, for what `write_wav` refuses to write.
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

/// Exit 1, nothing on stdout, exactly one stderr line naming `error`, and no analysis file.
fn assert_refused(out: &Output, error: &str, dir: &Path) {
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty(), "{}", text(&out.stdout));
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with("patchback: analyze refused: ") && stderr.contains(error),
        "{stderr}"
    );
    assert!(!dir.join(FILE).exists(), "{error} left an analysis file");
}

/// The `key value...` lines of an analysis file, after its header.
fn records(analysis: &str) -> Vec<(String, Vec<String>)> {
    analysis
        .lines()
        .skip(1)
        .map(|line| {
            let mut fields = line.split(' ').map(str::to_owned);
            let key = fields.next().unwrap();
            (key, fields.collect())
        })
        .collect()
}

fn value<'a>(records: &'a [(String, Vec<String>)], key: &str) -> &'a [String] {
    let found: Vec<_> = records.iter().filter(|(k, _)| k == key).collect();
    assert_eq!(found.len(), 1, "exactly one {key} line");
    &found[0].1
}

/// analysis-core-analyzed: a one-second 440 Hz sine is analysed into a new nested directory,
/// and the file records the configuration, the f0 estimate and one line per frame.
#[test]
fn analyze_writes_one_versioned_analysis_file() {
    let dir = scratch("analyzed");
    let input = wav(&dir, "sine.wav", sine(440.0, 0.5, 48_000));
    let out_dir = dir.join("nested").join("analysis");
    let out = analyze(&input, &out_dir);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout).lines().collect::<Vec<_>>(),
        [out_dir.join(FILE).to_str().unwrap()]
    );
    let entries: Vec<_> = fs::read_dir(&out_dir).unwrap().collect();
    assert_eq!(entries.len(), 1, "one file in --out");

    let analysis = fs::read_to_string(out_dir.join(FILE)).unwrap();
    assert_eq!(analysis.lines().next(), Some("patchback-analysis 1"));
    assert!(analysis.ends_with('\n'));
    let records = records(&analysis);
    assert_eq!(value(&records, "sample_rate_hz"), ["48000"]);
    assert_eq!(value(&records, "frame_count"), ["48000"]);
    assert_eq!(value(&records, "window"), ["hann"]);
    assert_eq!(value(&records, "fft_lengths"), ["512", "2048", "8192"]);
    assert_eq!(value(&records, "hop_lengths"), ["256", "1024", "4096"]);
    assert_eq!(value(&records, "band_edges_hz").len(), 9);
    let f0: f64 = value(&records, "f0_hz")[0].parse().unwrap();
    assert!((f0 - 440.0).abs() < 1.0, "{f0}");
    let confidence: f64 = value(&records, "f0_confidence")[0].parse().unwrap();
    assert!((0.85..=1.0).contains(&confidence), "{confidence}");

    let stfts: Vec<&Vec<String>> = records
        .iter()
        .filter(|(k, _)| k == "stft")
        .map(|(_, v)| v)
        .collect();
    assert_eq!(
        stfts,
        [
            &vec!["512".to_owned(), "256".to_owned(), "188".to_owned()],
            &vec!["2048".to_owned(), "1024".to_owned(), "47".to_owned()],
            &vec!["8192".to_owned(), "4096".to_owned(), "12".to_owned()],
        ]
    );
    for (fft_length, count) in [("512", 188), ("2048", 47), ("8192", 12)] {
        let frames: Vec<&Vec<String>> = records
            .iter()
            .filter(|(k, v)| k == "frame" && v[0] == fft_length)
            .map(|(_, v)| v)
            .collect();
        assert_eq!(frames.len(), count, "{fft_length}");
        for (index, frame) in frames.iter().enumerate() {
            assert_eq!(frame.len(), 2 + 6 + 8, "{frame:?}");
            assert_eq!(frame[1], index.to_string());
            for field in &frame[2..] {
                let number: f64 = field.parse().unwrap();
                assert!(number.is_finite(), "{frame:?}");
            }
        }
    }
    let known = [
        "sample_rate_hz",
        "frame_count",
        "window",
        "fft_lengths",
        "hop_lengths",
        "band_edges_hz",
        "rms_dbfs",
        "peak",
        "f0_hz",
        "f0_confidence",
        "stft",
        "frame",
    ];
    for (key, _) in &records {
        assert!(known.contains(&key.as_str()), "unexpected key {key}");
    }
}

#[test]
fn analyze_refuses_to_overwrite_an_existing_analysis_file() {
    let dir = scratch("overwrite");
    let input = wav(&dir, "sine.wav", sine(220.0, 0.5, 24_000));
    let out_dir = dir.join("analysis");
    fs::create_dir_all(&out_dir).unwrap();
    fs::write(out_dir.join(FILE), "keep me\n").unwrap();
    let out = analyze(&input, &out_dir);
    assert_eq!(out.status.code(), Some(1));
    let stderr = text(&out.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.contains("already exists"), "{stderr}");
    assert_eq!(fs::read_to_string(out_dir.join(FILE)).unwrap(), "keep me\n");
}

/// analysis-core-refused-input, step 1.
#[test]
fn analyze_refuses_empty_audio() {
    let dir = scratch("empty");
    let input = raw_wav(&dir, "empty.wav", 48_000, &[]);
    assert_refused(
        &analyze(&input, &dir),
        "patchback.analysis.EmptyAudio",
        &dir,
    );
}

/// analysis-core-refused-input, step 2.
#[test]
fn analyze_refuses_a_44_1_khz_clip() {
    let dir = scratch("rate");
    let input = raw_wav(&dir, "cd.wav", 44_100, &sine(440.0, 0.5, 44_100));
    assert_refused(
        &analyze(&input, &dir),
        "patchback.analysis.UnsupportedSampleRate",
        &dir,
    );
}

/// analysis-core-refused-input, step 3.
#[test]
fn analyze_refuses_a_non_finite_sample() {
    let dir = scratch("non-finite");
    let mut samples = sine(440.0, 0.5, 48_000);
    samples[100] = f32::NAN;
    let input = raw_wav(&dir, "nan.wav", 48_000, &samples);
    assert_refused(
        &analyze(&input, &dir),
        "patchback.analysis.NonFiniteSamples",
        &dir,
    );
}

/// analysis-core-refused-bounds, step 4: one sample over the sixty-second bound.
#[test]
fn analyze_refuses_a_clip_over_the_duration_bound() {
    let dir = scratch("duration");
    let input = wav(
        &dir,
        "long.wav",
        sine(440.0, 0.5, patchback_analysis::MAX_DURATION_FRAMES + 1),
    );
    assert_refused(
        &analyze(&input, &dir),
        "patchback.analysis.DurationOverBound",
        &dir,
    );
}

/// analysis-core-silence-and-pitch, step 1: nothing is written, the directory included.
#[test]
fn analyze_refuses_silence() {
    let dir = scratch("silent");
    let input = wav(&dir, "silence.wav", vec![0.0; 48_000]);
    let out_dir = dir.join("analysis");
    assert_refused(
        &analyze(&input, &out_dir),
        "patchback.analysis.SilentInput",
        &out_dir,
    );
    assert!(!out_dir.exists());
}

/// analysis-core-silence-and-pitch, step 2: when f0 is refused, no other descriptor is written.
#[test]
fn analyze_refuses_an_ambiguous_pitch() {
    let dir = scratch("ambiguous");
    let input = wav(&dir, "noise.wav", noise(48_000));
    let out_dir = dir.join("analysis");
    assert_refused(
        &analyze(&input, &out_dir),
        "patchback.analysis.PitchAmbiguous",
        &out_dir,
    );
    assert!(!out_dir.exists());
}

/// A file that is not a WAV is refused with the audio domain's error.
#[test]
fn analyze_refuses_a_corrupt_wav() {
    let dir = scratch("corrupt");
    let input = dir.join("not.wav");
    fs::write(&input, b"this is not a RIFF file at all").unwrap();
    assert_refused(
        &analyze(&input, &dir),
        "patchback.audio.CorruptRiffChunk",
        &dir,
    );
}

#[test]
fn analyze_of_a_missing_input_exits_1() {
    let dir = scratch("missing");
    let out = analyze(&dir.join("absent.wav"), &dir);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stderr).lines().count(), 1);
    assert!(!dir.join(FILE).exists());
}

#[test]
fn analyze_without_input_or_out_is_a_usage_error() {
    for argv in [
        &["analyze"][..],
        &["analyze", "--input", "a.wav"],
        &["analyze", "--out", "analysis"],
    ] {
        let out = run(argv);
        assert_eq!(out.status.code(), Some(2), "{argv:?}");
        assert!(out.stdout.is_empty(), "{argv:?}");
        assert!(text(&out.stderr).starts_with("error: "), "{argv:?}");
    }
}
