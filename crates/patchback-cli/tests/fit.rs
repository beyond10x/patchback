//! `patchback fit --input <wav> --notes <events> --out <dir> …` on files the tests write: the
//! acceptance scenarios of story:bounded-search-fit (`spec/scenarios/bounded-search-fit-*.yaml`)
//! that need clips and note sequences named by path, through the binary, with exit 0 (fitted,
//! `patch.patch` written), 1 (refused, one stderr line naming an error of `spec/`, nothing
//! written) and 2 (usage error). The search itself is held by `crates/patchback-fit/tests/`.
//!
//! References are rendered by `patchback render` from a known patch, 0.25 s notes, small budgets.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-fit")
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

/// The errors a domain of `spec/` declares.
fn declared_errors(domain: &str) -> Vec<String> {
    let path = format!(
        "{}/../../spec/domains/{domain}.yaml",
        env!("CARGO_MANIFEST_DIR")
    );
    let spec = fs::read_to_string(path).unwrap();
    let errors = spec.split_once("\nerrors:\n").unwrap().1;
    let errors = errors.split_once("\ncommands:\n").unwrap().0;
    errors
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- name: "))
        .map(str::to_owned)
        .collect()
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

/// One note of MIDI 45 at velocity 0.8, 0.25 s, at `rate`.
fn notes_at(dir: &Path, rate: u32) -> PathBuf {
    write(
        dir,
        "notes.events",
        &format!(
            "patchback-notes 1\nsample_rate_hz {rate}\npatch_id 7f0c0000-0000-4000-8000-000000000911\n\
             note 0 12000 45 0.8\n"
        ),
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
    let notes = notes_at(dir, 48_000);
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

fn fit(input: &Path, notes: &Path, out: &Path, extra: &[&str]) -> Output {
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
    patchback(&args)
}

/// The `key value` lines of a successful fit.
fn report(out: &Output) -> Vec<(String, String)> {
    text(&out.stdout)
        .lines()
        .map(|line| {
            let (key, value) = line.split_once(' ').expect("key value");
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

fn value<'a>(records: &'a [(String, String)], key: &str) -> &'a str {
    &records.iter().find(|(k, _)| k == key).unwrap().1
}

/// Exit 1, nothing on stdout, one stderr line `patchback: fit refused: <error>: …` naming
/// `error`, which `spec/domains/<domain>.yaml` declares, and nothing at `out`.
fn assert_refused(output: &Output, domain: &str, error: &str, out: &Path) -> String {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 1, "{stderr}");
    assert!(
        lines[0].starts_with(&format!("patchback: fit refused: {error}: ")),
        "{stderr}"
    );
    assert!(
        declared_errors(domain)
            .iter()
            .any(|declared| declared == error),
        "{error} is not an error of spec/domains/{domain}.yaml"
    );
    assert!(!out.exists(), "a refused fit writes nothing");
    lines[0].to_owned()
}

const SMALL: [&str; 4] = ["--max-renders", "12", "--max-restarts", "1"];

/// A fit of a reference one parameter away from the default: exit 0, the report lines in their
/// order, a `patch.patch` the loader accepts, the best loss below the initial loss.
#[test]
fn fit_writes_the_best_patch_and_reports_the_run() {
    let dir = scratch("fits");
    let input = reference(&dir);
    let notes = notes_at(&dir, 48_000);
    let out = dir.join("results");
    let output = fit(&input, &notes, &out, &SMALL);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    let records = report(&output);
    let keys: Vec<&str> = records.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "out",
            "init",
            "evaluations",
            "restarts",
            "initial_loss",
            "best_loss",
            "log_spectral",
            "envelope",
            "transient",
            "band_energy",
            "stop_reason",
            "wall_time_ms",
            "limited",
        ]
    );
    assert_eq!(value(&records, "out"), format!("{out:?}"));
    assert_eq!(value(&records, "init"), "default");
    let evaluations: u64 = value(&records, "evaluations").parse().unwrap();
    assert!((1..=12).contains(&evaluations), "{evaluations}");
    assert_eq!(value(&records, "restarts"), "1");
    let initial: f64 = value(&records, "initial_loss").parse().unwrap();
    let best: f64 = value(&records, "best_loss").parse().unwrap();
    assert!(best < initial, "{best} is not below {initial}");
    for key in ["log_spectral", "envelope", "transient", "band_energy"] {
        let component: f64 = value(&records, key).parse().unwrap();
        assert!(component.is_finite() && component >= 0.0, "{key}");
    }
    assert!(
        [
            "RenderBudgetExhausted",
            "TimeBudgetExhausted",
            "RestartsExhausted",
            "ImprovementBelowThreshold"
        ]
        .contains(&value(&records, "stop_reason"))
    );
    let _: u64 = value(&records, "wall_time_ms").parse().unwrap();
    assert!(["true", "false"].contains(&value(&records, "limited")));

    let patch = out.join("patch.patch");
    let validated = patchback(&["patch", "validate", patch.to_str().unwrap()]);
    assert_eq!(
        validated.status.code(),
        Some(0),
        "{}",
        text(&validated.stderr)
    );
    let written = fs::read_to_string(&patch).unwrap();
    let canonical =
        patchback_core::patch::write_patch(&patchback_core::patch::parse_patch(&written).unwrap())
            .unwrap();
    assert_eq!(written, canonical, "patch.patch is the canonical text");
    let entries: Vec<_> = fs::read_dir(&out).unwrap().collect();
    assert_eq!(entries.len(), 1, "a6 writes patch.patch only");
}

/// § 2 invariant 5: two processes with the same seed and inputs write the same `patch.patch`,
/// byte for byte, and report the same run.
#[test]
fn the_same_seed_and_inputs_write_the_same_patch_in_two_processes() {
    let dir = scratch("repeatable");
    let input = reference(&dir);
    let notes = notes_at(&dir, 48_000);
    let args = [
        "--max-renders",
        "20",
        "--max-restarts",
        "2",
        "--seed",
        "42",
        "--improvement-threshold",
        "10",
    ];
    let (a, b) = (dir.join("a"), dir.join("b"));
    let first = fit(&input, &notes, &a, &args);
    let second = fit(&input, &notes, &b, &args);
    assert_eq!(first.status.code(), Some(0), "{}", text(&first.stderr));
    assert_eq!(second.status.code(), Some(0), "{}", text(&second.stderr));
    assert_eq!(
        fs::read(a.join("patch.patch")).unwrap(),
        fs::read(b.join("patch.patch")).unwrap()
    );
    let without = |records: Vec<(String, String)>| -> Vec<(String, String)> {
        records
            .into_iter()
            .filter(|(key, _)| key != "out" && key != "wall_time_ms")
            .collect()
    };
    assert_eq!(without(report(&first)), without(report(&second)));
}

/// An existing `patch.patch` is never overwritten: the fit is refused before it searches and
/// the file is unchanged.
#[test]
fn an_existing_patch_file_is_refused_and_kept() {
    let dir = scratch("existing");
    let input = reference(&dir);
    let notes = notes_at(&dir, 48_000);
    let out = dir.join("results");
    fs::create_dir_all(&out).unwrap();
    let existing = write(&out, "patch.patch", "keep me\n");
    let output = fit(&input, &notes, &out, &SMALL);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty());
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with("patchback: fit refused: cannot write ")
            && stderr.contains("patch.patch"),
        "{stderr}"
    );
    assert_eq!(fs::read_to_string(existing).unwrap(), "keep me\n");
}

/// Da6-25 (adversary F5): an `--out` that exists and is not a directory is refused at the
/// existing-patch step, before the search, naming it; so is an `--out` beneath such a file,
/// naming the file. Exit 1, one stderr line, nothing on stdout, the file's bytes unchanged.
#[test]
fn an_out_that_is_not_a_directory_is_refused_before_the_search() {
    let dir = scratch("out-is-a-file");
    let input = reference(&dir);
    let notes = notes_at(&dir, 48_000);
    let file = write(&dir, "results", "not a directory\n");
    for out in [file.clone(), file.join("nested")] {
        let output = fit(&input, &notes, &out, &["--max-renders", "20"]);
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{out:?}: {stderr}");
        assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
        assert_eq!(
            stderr,
            format!("patchback: fit refused: cannot write {file:?}: is not a directory\n"),
            "--out {out:?}"
        );
        assert_eq!(fs::read(&file).unwrap(), b"not a directory\n");
    }
}

/// bounded-search-fit-unknown-references: a reference never imported is refused with
/// ClipNotFound naming it, before the note sequence (missing too) is looked at.
#[test]
fn an_unknown_reference_is_refused_with_clip_not_found() {
    let dir = scratch("unknown-reference");
    let out = dir.join("results");
    let line = assert_refused(
        &fit(
            &dir.join("never-imported.wav"),
            &dir.join("never-loaded.events"),
            &out,
            &[],
        ),
        "fit",
        "patchback.fit.ClipNotFound",
        &out,
    );
    assert!(line.contains("never-imported.wav"), "{line}");
    assert!(!line.contains("never-loaded.events"), "{line}");
}

/// A file that is not a WAV the importer accepts is no imported clip either.
#[test]
fn a_refused_reference_is_refused_with_clip_not_found_and_its_import_error() {
    let dir = scratch("refused-reference");
    let input = write(&dir, "reference.wav", "RIFF but not a WAV");
    let notes = notes_at(&dir, 48_000);
    let out = dir.join("results");
    let line = assert_refused(
        &fit(&input, &notes, &out, &[]),
        "fit",
        "patchback.fit.ClipNotFound",
        &out,
    );
    assert!(line.contains("patchback.audio."), "{line}");
}

/// bounded-search-fit-unknown-references: a note sequence never loaded (missing, or refused by
/// the parser) is refused with NoteSequenceNotFound naming it.
#[test]
fn an_unknown_note_sequence_is_refused_with_note_sequence_not_found() {
    let dir = scratch("unknown-notes");
    let input = reference(&dir);
    let out = dir.join("results");
    let line = assert_refused(
        &fit(&input, &dir.join("never-loaded.events"), &out, &[]),
        "fit",
        "patchback.fit.NoteSequenceNotFound",
        &out,
    );
    assert!(line.contains("never-loaded.events"), "{line}");
    let malformed = write(&dir, "malformed.events", "patchback-notes 9\n");
    let line = assert_refused(
        &fit(&input, &malformed, &out, &[]),
        "fit",
        "patchback.fit.NoteSequenceNotFound",
        &out,
    );
    assert!(line.contains("patchback.score."), "{line}");
}

/// bounded-search-fit-invalid-bounds: a zero render, wall-time or restart budget is refused with
/// InvalidSearchBound naming the bound, before the inputs (missing here) are looked at.
#[test]
fn a_zero_bound_is_refused_with_invalid_search_bound_before_the_inputs() {
    let dir = scratch("invalid-bounds");
    let out = dir.join("results");
    for (flag, bound) in [
        ("--max-renders", "RenderEvaluations"),
        ("--max-wall-time-ms", "WallTime"),
        ("--max-restarts", "Restarts"),
    ] {
        let line = assert_refused(
            &fit(
                &dir.join("never-imported.wav"),
                &dir.join("never-loaded.events"),
                &out,
                &[flag, "0"],
            ),
            "fit",
            "patchback.fit.InvalidSearchBound",
            &out,
        );
        assert!(line.contains(bound), "{line}");
    }
}

/// bounded-search-fit-silence-and-pitch through the binary: a silent reference is refused with
/// ReferenceSilent and nothing is written.
#[test]
fn a_silent_reference_is_refused_with_reference_silent() {
    let dir = scratch("silent");
    let notes = notes_at(&dir, 48_000);
    let clip = patchback_audio::AudioClip::from_mono(vec![0.0; 24_000]).unwrap();
    let input = dir.join("silent.wav");
    patchback_audio::write_wav(&input, &clip, patchback_audio::WavSampleFormat::Float32).unwrap();
    let out = dir.join("results");
    assert_refused(
        &fit(&input, &notes, &out, &[]),
        "fit",
        "patchback.fit.ReferenceSilent",
        &out,
    );
}

/// A run whose every render is refused (a note sequence at 44.1 kHz) fails with
/// NoValidCandidate and writes nothing.
#[test]
fn a_failed_run_is_refused_with_no_valid_candidate() {
    let dir = scratch("failed");
    let input = reference(&dir);
    let notes = notes_at(&dir, 44_100);
    let out = dir.join("results");
    assert_refused(
        &fit(&input, &notes, &out, &SMALL),
        "fit",
        "patchback.fit.NoValidCandidate",
        &out,
    );
}

/// Flags out of their parse range are usage errors: a negative bound, a non-finite or negative
/// threshold, a block size of zero.
#[test]
fn malformed_flags_are_usage_errors() {
    let dir = scratch("usage");
    let (input, notes, out) = (dir.join("a.wav"), dir.join("a.events"), dir.join("run"));
    for extra in [
        ["--max-wall-time-ms", "-1"],
        ["--max-restarts", "-1"],
        ["--improvement-threshold", "NaN"],
        ["--improvement-threshold", "inf"],
        ["--improvement-threshold", "-0.5"],
        ["--seed", "-1"],
        ["--block-size", "0"],
    ] {
        let output = fit(&input, &notes, &out, &extra);
        assert_eq!(output.status.code(), Some(2), "{extra:?}");
        assert!(output.stdout.is_empty());
        assert!(text(&output.stderr).starts_with("error: "), "{extra:?}");
        assert!(!out.exists());
    }
}
