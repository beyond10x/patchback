//! Adversarial cases against `patchback analyze` (story:analysis-core, wave a3).

use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_analysis::{AnalysisConfig, BAND_EDGES_HZ, analyze_clip};
use patchback_audio::{AudioClip, WavSampleFormat, read_wav, write_wav};

const FILE: &str = "analysis.txt";

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-adversary")
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

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
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

/// The documented grammar, built here from the documented table, against the file: no existing
/// test reads a single descriptor value from the file, so a mutant of `render` that swaps two
/// columns (rms and peak, centroid and rolloff) or prints the hop for the FFT length passes.
/// Every line is compared field by field; reals by exact `f64` equality, which the "shortest
/// form that reads back to the same value" claim promises.
#[test]
fn adversary_cli_file_is_the_documented_grammar_of_the_library_analysis() {
    let dir = scratch("grammar");
    let mut samples = sine(440.0, 0.5, 24_000);
    samples.extend(sine(660.0, 0.2, 24_000));
    let input = wav(&dir, "two-notes.wav", samples);
    let out_dir = dir.join("analysis");
    let out = analyze(&input, &out_dir);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let file = fs::read_to_string(out_dir.join(FILE)).unwrap();

    let analysis = analyze_clip(&read_wav(&input).unwrap(), &AnalysisConfig::default()).unwrap();
    enum V {
        Word(String),
        Real(f64),
    }
    let ints = |values: Vec<usize>| -> Vec<V> {
        values.into_iter().map(|v| V::Word(v.to_string())).collect()
    };
    let mut expected: Vec<(String, Vec<V>)> = vec![
        ("patchback-analysis".into(), vec![V::Word("1".into())]),
        ("sample_rate_hz".into(), vec![V::Word("48000".into())]),
        ("frame_count".into(), vec![V::Word("48000".into())]),
        ("window".into(), vec![V::Word("hann".into())]),
        (
            "fft_lengths".into(),
            ints(analysis.stfts.iter().map(|s| s.fft_length).collect()),
        ),
        (
            "hop_lengths".into(),
            ints(analysis.stfts.iter().map(|s| s.hop_length).collect()),
        ),
        (
            "band_edges_hz".into(),
            BAND_EDGES_HZ.iter().map(|e| V::Real(*e)).collect(),
        ),
        ("rms_dbfs".into(), vec![V::Real(analysis.rms_dbfs)]),
        ("peak".into(), vec![V::Real(analysis.peak)]),
        ("f0_hz".into(), vec![V::Real(analysis.f0.f0_hz)]),
        (
            "f0_confidence".into(),
            vec![V::Real(analysis.f0.confidence)],
        ),
    ];
    for s in &analysis.stfts {
        expected.push((
            "stft".into(),
            ints(vec![s.fft_length, s.hop_length, s.frames.len()]),
        ));
        for (index, f) in s.frames.iter().enumerate() {
            let mut values = ints(vec![s.fft_length, index]);
            values.extend(
                [
                    f.rms,
                    f.peak,
                    f.centroid_hz,
                    f.rolloff_hz,
                    f.flatness,
                    f.flux,
                ]
                .into_iter()
                .chain(f.band_energies_db)
                .map(V::Real),
            );
            expected.push(("frame".into(), values));
        }
    }

    let lines: Vec<&str> = file.lines().collect();
    assert_eq!(lines.len(), expected.len(), "line count");
    for (number, (line, (key, values))) in lines.iter().zip(&expected).enumerate() {
        let mut fields = line.split(' ');
        assert_eq!(fields.next(), Some(key.as_str()), "line {number}: {line}");
        let fields: Vec<&str> = fields.collect();
        assert_eq!(fields.len(), values.len(), "line {number}: {line}");
        for (column, (field, value)) in fields.iter().zip(values).enumerate() {
            match value {
                V::Word(word) => assert_eq!(field, word, "line {number} column {column}"),
                V::Real(real) => {
                    let parsed: f64 = field
                        .parse()
                        .unwrap_or_else(|_| panic!("line {number} column {column}: {field}"));
                    assert!(parsed.is_finite(), "line {number} column {column}");
                    assert_eq!(
                        parsed.to_bits(),
                        real.to_bits(),
                        "line {number} column {column}: {field} vs {real}"
                    );
                }
            }
        }
    }
}

/// "one line on stderr naming the reason": an input path holding a newline splits the refusal
/// over two lines.
#[test]
fn adversary_cli_refusal_is_one_line_whatever_the_path() {
    let dir = scratch("newline");
    let out = analyze(&dir.join("first\nsecond.wav"), &dir.join("analysis"));
    assert_eq!(out.status.code(), Some(1));
    let stderr = text(&out.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
}

/// `--out` naming an existing regular file: refused with one line, the file untouched, nothing
/// created beside it.
#[test]
fn adversary_cli_out_naming_a_file_is_refused_and_untouched() {
    let dir = scratch("out-is-file");
    let input = wav(&dir, "sine.wav", sine(440.0, 0.5, 24_000));
    let out_path = dir.join("taken");
    fs::write(&out_path, "keep me\n").unwrap();
    let out = analyze(&input, &out_path);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stderr).lines().count(),
        1,
        "{}",
        text(&out.stderr)
    );
    assert!(out.stdout.is_empty());
    assert_eq!(fs::read_to_string(&out_path).unwrap(), "keep me\n");
    let mut names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["sine.wav", "taken"]);
}

/// `<out>/analysis.txt` a dangling symlink: `exists()` is false through it, so the guard
/// passes; the write must still not follow the link and create its target.
#[cfg(unix)]
#[test]
fn adversary_cli_dangling_symlink_is_not_followed() {
    let dir = scratch("symlink");
    let input = wav(&dir, "sine.wav", sine(440.0, 0.5, 24_000));
    let out_dir = dir.join("analysis");
    fs::create_dir_all(&out_dir).unwrap();
    let victim = dir.join("victim.txt");
    std::os::unix::fs::symlink(&victim, out_dir.join(FILE)).unwrap();
    let out = analyze(&input, &out_dir);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stderr).lines().count(),
        1,
        "{}",
        text(&out.stderr)
    );
    assert!(!victim.exists(), "the symlink's target was created");
}
