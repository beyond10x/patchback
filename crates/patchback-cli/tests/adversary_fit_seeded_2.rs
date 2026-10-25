//! Adversary cases for `patchback fit --weights --normalization` (story:regressor-seeded-fit,
//! wave a11, pass 2: the correction Da11-21 to Da11-23), through the binary. Every artifact,
//! statistics file and reference is written here.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_audio::{AudioClip, WavSampleFormat, decode, encode};
use patchback_core::digest::{hex, sha256};
use patchback_core::patch::format_decimal;
use patchback_model::{ModelSpecPin, W1_ELEMENTS, W1_SHAPE, W2_SHAPE};

const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];
const TARGETS: [f32; OUTPUTS] = [0.2, 0.3, 0.4, 0.6, 0.7, 0.8, 0.35, 0.65];

const REFERENCE_PATCH: &str = "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
     node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive 12\n\
     node 4 LowpassFilter\nparam 4 filter_cutoff 178.88543819998318\nparam 4 resonance_q 6.25\n\
     node 5 AdsrEnvelope\nparam 5 attack 0.044721359549995794\nparam 5 decay 0.1\n\
     param 5 sustain 0.5\nparam 5 release 0.1\nnode 6 Vca\nnode 7 Output\n\
     param 7 output_gain -12\n\
     edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
     edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
     edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n";

fn notes_text(end: u64) -> String {
    format!(
        "patchback-notes 1\nsample_rate_hz 48000\npatch_id 7f0c0000-0000-4000-8000-000000000912\n\
         note 0 {end} 45 0.8\n"
    )
}

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-adversary-fit-seeded-2")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("output is UTF-8")
}

fn write(dir: &Path, name: &str, contents: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).unwrap();
    path
}

fn patchback(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(args)
        .output()
        .expect("the patchback binary runs")
}

/// The reference patch rendered with one note `0..end` by `patchback render`, cut to `frames`
/// when given, written as float32.
fn reference(dir: &Path, end: u64, frames: Option<usize>) -> PathBuf {
    let patch = write(dir, "reference.patch", REFERENCE_PATCH.as_bytes());
    let notes = write(dir, "render.events", notes_text(end).as_bytes());
    let rendered = dir.join("rendered.wav");
    let output = patchback(&[
        "render",
        "--patch",
        patch.to_str().unwrap(),
        "--notes",
        notes.to_str().unwrap(),
        "--out",
        rendered.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let Some(frames) = frames else {
        return rendered;
    };
    let mut samples = decode(&fs::read(&rendered).unwrap())
        .unwrap()
        .samples()
        .to_vec();
    assert!(samples.len() >= frames);
    samples.truncate(frames);
    let clip = AudioClip::from_mono(samples).unwrap();
    write(
        dir,
        "reference.wav",
        &encode(&clip, WavSampleFormat::Float32).unwrap(),
    )
}

fn targets_artifact() -> Vec<u8> {
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    let mut w2 = vec![0.0f32; HIDDEN * OUTPUTS];
    for (k, target) in TARGETS.iter().enumerate() {
        w1[k] = 1.0;
        w2[k * OUTPUTS + k] = *target;
    }
    let payload: Vec<u8> = w1.iter().chain(&w2).flat_map(|v| v.to_le_bytes()).collect();
    let pin = ModelSpecPin::repository().unwrap();
    let header = format!(
        "weights/1\nmodel PatchRegressor\nmodelspec {} {}\nfeatures features/1\nw1 32 64\nw2 64 8\nsha256 {}\n\n",
        pin.version(),
        pin.commit(),
        hex(&sha256(&payload))
    );
    [header.as_bytes(), &payload].concat()
}

fn identity_stats() -> String {
    let mut text = "patchback-normalization-stats 1\nfeature_contract features/1\nsplit train\nrows 16\nstd population\n".to_owned();
    for i in 0..32 {
        text.push_str(&format!(
            "feature {i} {} {}\n",
            format_decimal(0.0),
            format_decimal(1.0)
        ));
    }
    text
}

/// `patchback fit` of `reference` with `notes` into `out`, with the seed files when given.
fn fit(
    reference: &Path,
    notes: &Path,
    out: &Path,
    seed: Option<(&Path, &Path)>,
    bounds: &[&str],
) -> Output {
    let mut args = vec![
        "fit",
        "--input",
        reference.to_str().unwrap(),
        "--notes",
        notes.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--seed",
        "42",
    ];
    if let Some((weights, stats)) = seed {
        args.extend([
            "--weights",
            weights.to_str().unwrap(),
            "--normalization",
            stats.to_str().unwrap(),
        ]);
    }
    args.extend_from_slice(bounds);
    patchback(&args)
}

fn record<'a>(stdout: &'a str, key: &str) -> &'a str {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key} ")))
        .unwrap_or_else(|| panic!("no {key} line in {stdout}"))
}

/// Da11-22 through the binary: a 48000-frame recording whose first note ends at 48000 or
/// 60000 is refused on a seeded run: exit 1, nothing on stdout, one stderr line
/// `patchback: fit refused: patchback.fit.NoValidCandidate: …holds no frame centre…`, nothing
/// written at `--out`. The same inputs without the two flags fit (exit 0, `init default`).
#[test]
fn adversary2_a_first_note_held_to_the_recording_end_is_refused_through_the_binary() {
    let dir = scratch("held");
    let reference = reference(&dir, 48_000, Some(48_000));
    let weights = write(&dir, "regressor.weights", &targets_artifact());
    let stats = write(&dir, "normalization.stats", identity_stats().as_bytes());
    let small = ["--max-renders", "1", "--max-restarts", "1"];
    for end in [48_000u64, 60_000] {
        let notes = write(
            &dir,
            &format!("notes-{end}.events"),
            notes_text(end).as_bytes(),
        );
        let seeded_out = dir.join(format!("seeded-{end}"));
        let output = fit(
            &reference,
            &notes,
            &seeded_out,
            Some((&weights, &stats)),
            &small,
        );
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "end {end}: {stderr}");
        assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
        assert_eq!(stderr.lines().count(), 1, "{stderr}");
        assert!(
            stderr.starts_with("patchback: fit refused: patchback.fit.NoValidCandidate: "),
            "{stderr}"
        );
        assert!(stderr.contains("holds no frame centre"), "{stderr}");
        assert!(!seeded_out.exists(), "a refused fit writes nothing");

        let default_out = dir.join(format!("default-{end}"));
        let output = fit(&reference, &notes, &default_out, None, &small);
        let stdout = text(&output.stdout);
        assert_eq!(
            output.status.code(),
            Some(0),
            "end {end}: {}",
            text(&output.stderr)
        );
        assert_eq!(record(&stdout, "init"), "default");
    }
}

/// Da11-23 through the binary: a seeded run with `--max-wall-time-ms 1` evaluates its first
/// candidate, the prediction, and stops `TimeBudgetExhausted` with a `wall_time_ms` above the
/// bound (the first note's analysis, the prediction and one evaluation are inside the clock).
/// The default run of the same inputs and bounds is printed beside it for the report.
#[test]
fn adversary2_a_one_millisecond_seeded_run_reports_its_clock_through_the_binary() {
    let dir = scratch("clock");
    let reference = reference(&dir, 12_000, None);
    let notes = write(&dir, "notes.events", notes_text(12_000).as_bytes());
    let weights = write(&dir, "regressor.weights", &targets_artifact());
    let stats = write(&dir, "normalization.stats", identity_stats().as_bytes());
    let bounds = ["--max-wall-time-ms", "1", "--max-renders", "50"];
    let output = fit(
        &reference,
        &notes,
        &dir.join("seeded"),
        Some((&weights, &stats)),
        &bounds,
    );
    let stdout = text(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(record(&stdout, "init"), "regressor-prediction");
    assert_eq!(record(&stdout, "evaluations"), "1");
    assert_eq!(record(&stdout, "stop_reason"), "TimeBudgetExhausted");
    let seeded_ms: u64 = record(&stdout, "wall_time_ms").parse().unwrap();
    assert!(seeded_ms > 1, "{stdout}");

    let output = fit(&reference, &notes, &dir.join("default"), None, &bounds);
    let stdout = text(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(record(&stdout, "evaluations"), "1");
    eprintln!(
        "--max-wall-time-ms 1: seeded wall_time_ms {seeded_ms}, default wall_time_ms {}",
        record(&stdout, "wall_time_ms")
    );
}
