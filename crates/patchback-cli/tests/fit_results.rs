//! The files of a fit run through the binary (story:fit-run-artifacts, DESIGN.md § 11): any of
//! the six existing in `--out` is refused before the search and the directory is left as it was
//! (Da7-6); a write that fails after the search removes everything the run wrote; `patchback
//! compare` of the reference and `reconstructed.wav` prints the best loss the fit printed; and
//! `run.meta` records the digests of the input files and the compiled-in ModelSpec pin.
//!
//! References are rendered by `patchback render` from a known patch, 0.25 s notes, small budgets.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_core::digest::{hex, sha256};
use patchback_model::ModelSpecPin;

/// The six files of a fit run, in the order the existence check names them.
const RESULT_FILES: [&str; 6] = [
    "patch.patch",
    "notes.events",
    "reconstructed.wav",
    "report.txt",
    "run.meta",
    "trace.events",
];

const SMALL: [&str; 4] = ["--max-renders", "12", "--max-restarts", "1"];

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-fit-results")
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

fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).unwrap();
    path
}

/// The fixed chain with the filter cutoff at 179 Hz (`p = 0.25`) and every other parameter at
/// its contract default (`p = 0.5`).
fn reference_patch(dir: &Path) -> PathBuf {
    write(
        dir,
        "reference.patch",
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive 12\n\
         node 4 LowpassFilter\nparam 4 filter_cutoff 178.88543819998318\nparam 4 resonance_q 6.25\n\
         node 5 AdsrEnvelope\nparam 5 attack 0.044721359549995794\nparam 5 decay 0.1\n\
         param 5 sustain 0.5\nparam 5 release 0.1\nnode 6 Vca\nnode 7 Output\n\
         param 7 output_gain -12\n\
         edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
         edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
         edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n",
    )
}

/// One note of MIDI 45 at velocity 0.8, 0.25 s.
fn notes(dir: &Path) -> PathBuf {
    write(
        dir,
        "notes.events",
        "patchback-notes 1\nsample_rate_hz 48000\npatch_id 7f0c0000-0000-4000-8000-000000000911\n\
         note 0 12000 45 0.8\n",
    )
}

fn patchback(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(args)
        .output()
        .expect("the patchback binary runs")
}

/// The reference WAV rendered by `patchback render` from [`reference_patch`].
fn reference(dir: &Path) -> PathBuf {
    let patch = reference_patch(dir);
    let notes = notes(dir);
    let out = dir.join("reference.wav");
    let rendered = patchback(&[
        "render",
        "--patch",
        patch.to_str().unwrap(),
        "--notes",
        notes.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(
        rendered.status.code(),
        Some(0),
        "{}",
        text(&rendered.stderr)
    );
    out
}

/// A silent reference, which the search refuses (`ReferenceSilent`): a refusal that names an
/// existing file instead was answered before the search.
fn silent(dir: &Path) -> PathBuf {
    let clip = patchback_audio::AudioClip::from_mono(vec![0.0; 24_000]).unwrap();
    let path = dir.join("silent.wav");
    patchback_audio::write_wav(&path, &clip, patchback_audio::WavSampleFormat::Float32).unwrap();
    path
}

fn fit_args<'a>(
    input: &'a Path,
    notes: &'a Path,
    out: &'a Path,
    extra: &[&'a str],
) -> Vec<&'a str> {
    let mut args = vec![
        "fit",
        "--input",
        input.to_str().unwrap(),
        "--notes",
        notes.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    args
}

fn fit(input: &Path, notes: &Path, out: &Path, extra: &[&str]) -> Output {
    patchback(&fit_args(input, notes, out, extra))
}

/// The value of the `key value` line `key` of `text`.
fn value(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{key} ")))
        .unwrap_or_else(|| panic!("no {key} in {text}"))
        .to_owned()
}

/// Every entry under `dir`, with its bytes for a file, sorted by path.
fn listing(dir: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut entries = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.is_dir() {
                pending.push(path.clone());
                entries.push((path, None));
            } else if meta.is_file() {
                let bytes = fs::read(&path).unwrap();
                entries.push((path, Some(bytes)));
            } else {
                entries.push((path, None));
            }
        }
    }
    entries.sort();
    entries
}

/// Da7-6: each of the six files existing in `--out` is refused before the search (the silent
/// reference would be refused by the search), naming it, and the directory is left as it was:
/// exit 1, one stderr line, nothing on stdout.
#[test]
fn each_existing_result_file_is_refused_and_the_directory_is_unchanged() {
    let dir = scratch("existing");
    let input = silent(&dir);
    let notes = notes(&dir);
    for name in RESULT_FILES {
        let out = dir.join(format!("out-{name}"));
        fs::create_dir_all(&out).unwrap();
        let existing = write(&out, name, "keep me\n");
        let before = listing(&out);
        let output = fit(&input, &notes, &out, &SMALL);
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{name}: {stderr}");
        assert!(output.stdout.is_empty(), "{name}: {}", text(&output.stdout));
        assert_eq!(
            stderr,
            format!(
                "patchback: fit refused: cannot write {existing:?}: already exists; it is never \
                 overwritten\n"
            ),
            "{name}"
        );
        assert_eq!(listing(&out), before, "{name}");
    }
}

/// With several of the six existing, the first in the documented order is named.
#[test]
fn the_first_existing_file_in_the_documented_order_is_named() {
    let dir = scratch("existing-several");
    let input = silent(&dir);
    let notes = notes(&dir);
    let out = dir.join("results");
    fs::create_dir_all(&out).unwrap();
    write(&out, "trace.events", "keep\n");
    let report = write(&out, "report.txt", "keep\n");
    let output = fit(&input, &notes, &out, &SMALL);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        text(&output.stderr),
        format!(
            "patchback: fit refused: cannot write {report:?}: already exists; it is never \
             overwritten\n"
        )
    );
}

/// A dangling symbolic link at one of the six names exists too: it is refused before the search
/// and left in place, never followed to create its target.
#[cfg(unix)]
#[test]
fn a_dangling_symlink_at_a_result_name_is_refused() {
    let dir = scratch("dangling");
    let input = silent(&dir);
    let notes = notes(&dir);
    let out = dir.join("results");
    fs::create_dir_all(&out).unwrap();
    let target = dir.join("elsewhere.wav");
    let link = out.join("reconstructed.wav");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let output = fit(&input, &notes, &out, &SMALL);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stderr),
        format!(
            "patchback: fit refused: cannot write {link:?}: already exists; it is never \
             overwritten\n"
        )
    );
    assert!(!target.exists());
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(listing(&out).len(), 1);
}

/// A write that fails after the search removes every file and directory the run wrote. The
/// failure is a file size limit (`ulimit -f 16`, at most 16 KiB a file, with `SIGXFSZ`
/// ignored so the write fails with `EFBIG` instead of killing the process): `patch.patch` and
/// `notes.events` are written below it, `reconstructed.wav` (about 100 KB) is not. Exit 1, one
/// stderr line naming the WAV, `--out` gone.
#[cfg(unix)]
#[test]
fn a_write_that_fails_after_the_search_removes_everything_the_run_wrote() {
    let dir = scratch("failed-write");
    let input = reference(&dir);
    let notes = notes(&dir);
    let out = dir.join("results").join("nested");
    let args = fit_args(&input, &notes, &out, &SMALL);
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg("ulimit -f 16 && trap '' XFSZ && exec \"$0\" \"$@\"")
        .arg(env!("CARGO_BIN_EXE_patchback"))
        .args(&args)
        .output()
        .expect("sh runs");
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    let wav = out.join("reconstructed.wav");
    assert!(
        stderr.starts_with(&format!("patchback: fit refused: cannot write {wav:?}: ")),
        "{stderr}"
    );
    assert!(!dir.join("results").exists(), "{:?}", listing(&dir));
}

/// `patchback compare --reference <input> --candidate <out>/reconstructed.wav` prints, as
/// `loss`, the `best_loss` the fit printed, and `report.txt` records the same value.
#[test]
fn compare_of_the_reconstructed_wav_prints_the_best_loss() {
    let dir = scratch("compare");
    let input = reference(&dir);
    let notes = notes(&dir);
    let out = dir.join("results");
    let fitted = fit(&input, &notes, &out, &SMALL);
    assert_eq!(fitted.status.code(), Some(0), "{}", text(&fitted.stderr));
    let best = value(&text(&fitted.stdout), "best_loss");
    let compared = patchback(&[
        "compare",
        "--reference",
        input.to_str().unwrap(),
        "--candidate",
        out.join("reconstructed.wav").to_str().unwrap(),
    ]);
    assert_eq!(
        compared.status.code(),
        Some(0),
        "{}",
        text(&compared.stderr)
    );
    assert_eq!(value(&text(&compared.stdout), "loss"), best);
    let report = fs::read_to_string(out.join("report.txt")).unwrap();
    assert_eq!(value(&report, "best_loss"), best);
    for key in ["log_spectral", "envelope", "transient", "band_energy"] {
        assert_eq!(
            value(&text(&compared.stdout), key),
            value(&text(&fitted.stdout), key),
            "{key}"
        );
    }
}

/// `run.meta` names the digests of the `--input` and `--notes` bytes, the compiled-in ModelSpec
/// pin's version and the flags the run was given; `report.txt` the values the fit printed.
#[test]
fn run_meta_records_the_input_digests_the_pin_and_the_flags() {
    let dir = scratch("run-meta");
    let input = reference(&dir);
    let notes = notes(&dir);
    let out = dir.join("results");
    let fitted = fit(
        &input,
        &notes,
        &out,
        &[
            "--max-renders",
            "9",
            "--max-restarts",
            "1",
            "--seed",
            "7",
            "--block-size",
            "128",
            "--improvement-threshold",
            "0.25",
            "--max-wall-time-ms",
            "600000",
        ],
    );
    let stdout = text(&fitted.stdout);
    assert_eq!(fitted.status.code(), Some(0), "{}", text(&fitted.stderr));
    let meta = fs::read_to_string(out.join("run.meta")).unwrap();
    let keys: Vec<&str> = meta
        .lines()
        .map(|line| line.split_once(' ').unwrap().0)
        .collect();
    assert_eq!(
        keys,
        [
            "patchback-fit-run-meta",
            "patchback",
            "init",
            "seed",
            "topology",
            "feature_contract",
            "prng",
            "modelspec",
            "reference",
            "notes",
            "max_renders",
            "max_wall_time_ms",
            "max_restarts",
            "improvement_threshold",
            "block_size",
        ]
    );
    assert_eq!(value(&meta, "patchback-fit-run-meta"), "1");
    assert_eq!(value(&meta, "patchback"), env!("CARGO_PKG_VERSION"));
    assert_eq!(value(&meta, "init"), "default");
    assert_eq!(value(&meta, "seed"), "7");
    assert_eq!(value(&meta, "topology"), "subtractive-mono/1");
    assert_eq!(value(&meta, "feature_contract"), "features/1");
    assert_eq!(value(&meta, "prng"), "xoshiro256**");
    assert_eq!(
        value(&meta, "modelspec"),
        ModelSpecPin::repository().unwrap().version()
    );
    assert_eq!(
        value(&meta, "reference"),
        format!("sha256 {}", hex(&sha256(&fs::read(&input).unwrap())))
    );
    assert_eq!(
        value(&meta, "notes"),
        format!("sha256 {}", hex(&sha256(&fs::read(&notes).unwrap())))
    );
    assert_eq!(value(&meta, "max_renders"), "9");
    assert_eq!(value(&meta, "max_wall_time_ms"), "600000");
    assert_eq!(value(&meta, "max_restarts"), "1");
    assert_eq!(value(&meta, "improvement_threshold"), "0.25");
    assert_eq!(value(&meta, "block_size"), "128");

    let report = fs::read_to_string(out.join("report.txt")).unwrap();
    assert!(report.starts_with("patchback-fit-report 1\n"), "{report}");
    for key in [
        "initial_loss",
        "best_loss",
        "log_spectral",
        "envelope",
        "transient",
        "band_energy",
        "evaluations",
        "restarts",
        "stop_reason",
        "wall_time_ms",
    ] {
        assert_eq!(value(&report, key), value(&stdout, key), "{key}");
    }
    let trace = fs::read_to_string(out.join("trace.events")).unwrap();
    let evaluations: usize = value(&stdout, "evaluations").parse().unwrap();
    assert_eq!(trace.lines().count(), evaluations + 3);
}
