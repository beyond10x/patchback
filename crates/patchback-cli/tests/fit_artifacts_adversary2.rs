//! Adversary cases for story:fit-run-artifacts (wave a7, pass 2) through the binary: the stdout
//! `limited` record, `report.txt`'s `warning` line (Da7-15) and the written files agree, with
//! and without limiting, and for a run whose best render is limited only after the reference
//! ends.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_audio::{AudioClip, WavSampleFormat, read_wav, write_wav};
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, load_patch_file, validate};
use patchback_core::score::read_events_file;
use patchback_dsp::{RenderConfig, render, render_frame_count};
use patchback_fit::{default_point, patch_at};

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-fit-artifacts-adversary2")
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

fn value(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{key} ")))
        .unwrap_or_else(|| panic!("no {key} in {text}"))
        .to_owned()
}

/// Fits `point`'s render of `notes` (written to `dir`), cut to `frames`, with 20 renders.
fn fit_of(dir: &Path, point: &[f64], notes: &str, frames: usize) -> (Output, PathBuf, PathBuf) {
    let notes_path = dir.join("notes.events");
    fs::write(&notes_path, notes).unwrap();
    let sequence = read_events_file(&notes_path).unwrap();
    let topology = Topology::subtractive_mono();
    let plan = compile(&validate(&patch_at(&topology, point).unwrap()).unwrap());
    let mut samples = render(&plan, &sequence, &RenderConfig::default())
        .unwrap()
        .samples;
    samples.truncate(frames);
    let input = dir.join("reference.wav");
    write_wav(
        &input,
        &AudioClip::from_mono(samples).unwrap(),
        WavSampleFormat::Float32,
    )
    .unwrap();
    let out = dir.join("results");
    let output = Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args([
            "fit",
            "--input",
            input.to_str().unwrap(),
            "--notes",
            notes_path.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--max-renders",
            "20",
            "--max-restarts",
            "1",
            "--seed",
            "3",
        ])
        .output()
        .expect("the patchback binary runs");
    (output, out, notes_path)
}

/// The `<n>` and `<m>` of the warning, from a render re-made from the written `patch.patch`.
fn counts(out: &Path, notes: &Path) -> (u64, u64) {
    let loaded = load_patch_file(out.join("patch.patch")).unwrap();
    let plan = compile(&validate(loaded.patch()).unwrap());
    let sequence = read_events_file(notes).unwrap();
    let rendered = render(&plan, &sequence, &RenderConfig::default()).unwrap();
    let m = render_frame_count(&plan, &sequence);
    assert_eq!(u64::try_from(rendered.samples.len()).unwrap(), m);
    (rendered.report.limited_samples, m)
}

fn warnings(out: &Path) -> Vec<String> {
    fs::read_to_string(out.join("report.txt"))
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("warning "))
        .map(str::to_owned)
        .collect()
}

/// No limiting: stdout `limited false`, and `report.txt` has no `warning` line at all.
#[test]
fn adversary2_an_unlimited_run_prints_limited_false_and_writes_no_warning() {
    let dir = scratch("unlimited");
    let topology = Topology::subtractive_mono();
    let mut point = default_point(&topology);
    point[topology.contract("filter_cutoff").unwrap().index] = 0.25;
    let notes = "patchback-notes 1\nsample_rate_hz 48000\n\
                 patch_id 7f0c0000-0000-4000-8000-000000000a04\nnote 0 12000 45 0.8\n";
    let (output, out, notes_path) = fit_of(&dir, &point, notes, usize::MAX);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert_eq!(value(&stdout, "limited"), "false", "{stdout}");
    assert_eq!(counts(&out, &notes_path).0, 0);
    assert_eq!(warnings(&out), Vec::<String>::new());
    assert!(
        !fs::read_to_string(out.join("report.txt"))
            .unwrap()
            .contains("warning")
    );
}

/// Da7-15 through the binary, for a best render limited only after the reference ends: stdout
/// says `limited true`, `report.txt` carries one `warning limiter engaged on <n> of <m> rendered
/// samples` with `<n>` and `<m>` those of the whole render of the written patch, and
/// `reconstructed.wav`, the reference's length, holds no sample at the threshold (the gap
/// `cli-doc.md` documents).
#[test]
fn adversary2_a_run_limited_after_the_reference_ends_prints_limited_true_and_warns() {
    let dir = scratch("late-limit");
    let topology = Topology::subtractive_mono();
    let mut point = default_point(&topology);
    for path in ["output_gain", "drive", "resonance_q", "sustain"] {
        point[topology.contract(path).unwrap().index] = 1.0;
    }
    let notes = "patchback-notes 1\nsample_rate_hz 48000\n\
                 patch_id 7f0c0000-0000-4000-8000-000000000a05\n\
                 note 0 12000 45 0.25\nnote 36000 48000 45 1\n";
    let (output, out, notes_path) = fit_of(&dir, &point, notes, 24_000);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert_eq!(value(&stdout, "limited"), "true", "{stdout}");
    let (n, m) = counts(&out, &notes_path);
    assert!((1..=m).contains(&n), "{n} of {m}");
    assert!(m > 48_000, "{m}");
    assert_eq!(
        warnings(&out),
        [format!("limiter engaged on {n} of {m} rendered samples")]
    );
    let wav = read_wav(&out.join("reconstructed.wav")).unwrap();
    assert_eq!(wav.frame_count(), 24_000);
    assert_eq!(
        wav.samples().iter().filter(|s| s.abs() >= 1.0).count(),
        0,
        "the written clip holds no limited sample"
    );
}
