//! Adversary cases for `patchback fit` (story:bounded-search-fit, wave a6, pass 1), through the
//! binary: the wall-time obligation as an operator sees it, the block size across processes,
//! and the refusal order with two faults at once (D8).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-fit-adversary")
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

/// The fixed chain with the filter cutoff at `p = 0.25`, every other parameter at its default.
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

fn notes_at(dir: &Path, name: &str, rate: u32) -> PathBuf {
    write(
        dir,
        name,
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

fn reference(dir: &Path) -> PathBuf {
    let patch = reference_patch(dir);
    let notes = notes_at(dir, "reference.events", 48_000);
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

fn silent(dir: &Path) -> PathBuf {
    let clip = patchback_audio::AudioClip::from_mono(vec![0.0; 24_000]).unwrap();
    let path = dir.join("silent.wav");
    patchback_audio::write_wav(&path, &clip, patchback_audio::WavSampleFormat::Float32).unwrap();
    path
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

/// The story's wall-time obligation as Da6-22 states it, through the binary: with
/// `--max-wall-time-ms 1` only the run's first candidate renders, the run stops with
/// `TimeBudgetExhausted`, the printed `wall_time_ms` is at least the bound, and `patch.patch` is
/// written.
#[test]
fn a_one_millisecond_bound_flag_prints_one_evaluation_and_writes_the_patch() {
    let dir = scratch("wall-time");
    let input = reference(&dir);
    let notes = notes_at(&dir, "notes.events", 48_000);
    let out = dir.join("results");
    let output = fit(&input, &notes, &out, &["--max-wall-time-ms", "1"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let records = report(&output);
    eprintln!("{}", text(&output.stdout));
    assert_eq!(value(&records, "evaluations"), "1");
    assert_eq!(value(&records, "stop_reason"), "TimeBudgetExhausted");
    let wall: u64 = value(&records, "wall_time_ms").parse().unwrap();
    assert!(
        wall >= 1,
        "wall_time_ms {wall} is below --max-wall-time-ms 1"
    );
    assert!(out.join("patch.patch").is_file(), "patch.patch is written");
}

/// § 2 invariant 5 across processes: `--block-size 512` and `--block-size 3` write the same
/// `patch.patch` bytes and print the same lines (but `out` and `wall_time_ms`) as the default.
#[test]
fn the_block_size_does_not_change_the_written_patch_or_the_report() {
    let dir = scratch("block-size");
    let input = reference(&dir);
    let notes = notes_at(&dir, "notes.events", 48_000);
    let args = ["--max-renders", "10", "--max-restarts", "2", "--seed", "5"];
    type Run = (Vec<u8>, Vec<(String, String)>);
    let runs: Vec<Run> = [None, Some("512"), Some("3")]
        .iter()
        .enumerate()
        .map(|(n, block)| {
            let out = dir.join(format!("run-{n}"));
            let mut extra = args.to_vec();
            if let Some(block) = block {
                extra.extend_from_slice(&["--block-size", block]);
            }
            let output = fit(&input, &notes, &out, &extra);
            assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
            let lines = report(&output)
                .into_iter()
                .filter(|(key, _)| key != "out" && key != "wall_time_ms")
                .collect();
            (fs::read(out.join("patch.patch")).unwrap(), lines)
        })
        .collect();
    for other in &runs[1..] {
        assert_eq!(runs[0].0, other.0, "patch.patch bytes");
        assert_eq!(runs[0].1, other.1, "report lines");
    }
}

/// D8 with two faults at once, each adjacent pair in the documented order (bounds, `--input`,
/// `--notes`, an existing `patch.patch`, silence and pitch): the earlier fault answers.
#[test]
fn with_two_faults_the_earlier_in_the_documented_order_answers() {
    let dir = scratch("two-faults");
    let quiet = silent(&dir);
    let notes = notes_at(&dir, "notes.events", 48_000);
    let notes_44k = notes_at(&dir, "notes-44k.events", 44_100);
    let missing_wav = dir.join("missing.wav");
    let missing_notes = dir.join("missing.events");
    let taken = dir.join("taken");
    fs::create_dir_all(&taken).unwrap();
    write(&taken, "patch.patch", "keep me\n");
    let fresh = dir.join("fresh");
    let cases: [(&Path, &Path, &Path, &[&str], &str); 5] = [
        (
            &missing_wav,
            &missing_notes,
            &fresh,
            &["--max-renders", "0"],
            "patchback.fit.InvalidSearchBound",
        ),
        (
            &missing_wav,
            &missing_notes,
            &taken,
            &[],
            "patchback.fit.ClipNotFound",
        ),
        (
            &quiet,
            &missing_notes,
            &taken,
            &[],
            "patchback.fit.NoteSequenceNotFound",
        ),
        (&quiet, &notes, &taken, &[], "cannot write"),
        (
            &quiet,
            &notes_44k,
            &fresh,
            &["--max-renders", "4"],
            "patchback.fit.ReferenceSilent",
        ),
    ];
    for (input, notes, out, extra, first) in cases {
        let output = fit(input, notes, out, extra);
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{first}: {stderr}");
        assert!(
            stderr.starts_with(&format!("patchback: fit refused: {first}")),
            "{first}: {stderr}"
        );
        assert!(
            !fresh.exists(),
            "{first}: a refused fit creates no directory"
        );
        assert_eq!(
            fs::read_to_string(taken.join("patch.patch")).unwrap(),
            "keep me\n"
        );
    }
}

/// Pass 2, Da6-22 through the binary: with `--max-wall-time-ms 1` and up to 4 restarts the
/// report says `restarts 1`, and the `patch.patch` written is the default patch (the run's first
/// candidate, the only one rendered).
#[test]
fn a_one_millisecond_bound_flag_reports_one_restart_and_writes_the_default_patch() {
    use patchback_core::parameter::Topology;
    let dir = scratch("wall-time-default");
    let input = reference(&dir);
    let notes = notes_at(&dir, "notes.events", 48_000);
    let out = dir.join("results");
    let output = fit(
        &input,
        &notes,
        &out,
        &["--max-wall-time-ms", "1", "--max-restarts", "4"],
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let records = report(&output);
    assert_eq!(value(&records, "evaluations"), "1");
    assert_eq!(value(&records, "restarts"), "1");
    assert_eq!(
        value(&records, "initial_loss"),
        value(&records, "best_loss")
    );
    let topology = Topology::subtractive_mono();
    let default =
        patchback_fit::patch_at(&topology, &patchback_fit::default_point(&topology)).unwrap();
    assert_eq!(
        fs::read_to_string(out.join("patch.patch")).unwrap(),
        patchback_core::patch::write_patch(&default).unwrap()
    );
}

/// Pass 2, Da6-25 with two faults at once around the new `<out>` step (D8 order: bounds,
/// `--input`, `--notes`, `<out>`, silence and pitch): with `--out` a regular file, a zero bound,
/// a missing `--input` and a missing `--notes` each answer first; with a silent reference the
/// `<out>` refusal answers, for the file itself and for a path beneath it. The file is never
/// changed and nothing is created. (`--out` a file together with an existing
/// `<out>/patch.patch` cannot be built: nothing exists beneath a regular file.)
#[test]
fn an_out_file_with_a_second_fault_answers_in_the_documented_order() {
    let dir = scratch("out-file-two-faults");
    let input = reference(&dir);
    let quiet = silent(&dir);
    let notes = notes_at(&dir, "notes.events", 48_000);
    let missing_wav = dir.join("missing.wav");
    let missing_notes = dir.join("missing.events");
    let file = write(&dir, "results", "a file\n");
    let nested = file.join("nested");
    let not_a_directory = format!("cannot write {file:?}: is not a directory\n");
    let cases: [(&Path, &Path, &Path, &[&str], &str); 5] = [
        (
            &input,
            &notes,
            &file,
            &["--max-restarts", "0"],
            "patchback.fit.InvalidSearchBound: ",
        ),
        (
            &missing_wav,
            &notes,
            &file,
            &[],
            "patchback.fit.ClipNotFound: ",
        ),
        (
            &input,
            &missing_notes,
            &file,
            &[],
            "patchback.fit.NoteSequenceNotFound: ",
        ),
        (&quiet, &notes, &file, &[], &not_a_directory),
        (&quiet, &notes, &nested, &[], &not_a_directory),
    ];
    for (input, notes, out, extra, first) in cases {
        let mut extra = extra.to_vec();
        extra.extend_from_slice(&["--max-renders", "4"]);
        let output = fit(input, notes, out, &extra);
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{first}: {stderr}");
        assert!(
            output.stdout.is_empty(),
            "{first}: {}",
            text(&output.stdout)
        );
        assert!(
            stderr.starts_with(&format!("patchback: fit refused: {first}")),
            "expected {first:?} first, got {stderr}"
        );
        assert_eq!(fs::read(&file).unwrap(), b"a file\n", "{first}");
    }
}

/// Pass 2, Da6-25's class ("an `--out` that exists and is not a directory is refused before the
/// search"): a dangling symbolic link as `--out` exists as a directory entry and is not a
/// directory, and no directory can ever be created at it. `fs::metadata` follows the link and
/// fails, so the check walks to its parent; the search then runs (`--max-renders 20`) and only
/// `create_dir_all` refuses it. The refusal must come before the search, as for a regular file.
#[test]
fn a_dangling_symlink_as_out_is_refused_before_the_search() {
    let dir = scratch("out-dangling-link");
    let input = reference(&dir);
    let notes = notes_at(&dir, "notes.events", 48_000);
    let gone = dir.join("gone").join("results");
    let out = dir.join("results");
    std::os::unix::fs::symlink(&gone, &out).unwrap();
    let started = std::time::Instant::now();
    let output = fit(&input, &notes, &out, &["--max-renders", "20"]);
    let elapsed = started.elapsed();
    let stderr = text(&output.stderr);
    eprintln!(
        "dangling --out: {elapsed:?}, exit {:?}, {stderr}",
        output.status.code()
    );
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert_eq!(
        stderr,
        format!("patchback: fit refused: cannot write {out:?}: is not a directory\n")
    );
    assert_eq!(fs::read_link(&out).unwrap(), gone);
    assert!(!dir.join("gone").exists());
}
