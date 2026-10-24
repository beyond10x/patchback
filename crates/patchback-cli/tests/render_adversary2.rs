//! Adversarial cases, second pass (story:subtractive-voice-render, wave a5, after correction
//! round 1): the escaping of the `out` record (D11). The module documentation of
//! `crates/patchback-cli/src/render.rs` says the record is "the WAV written, as given, with each
//! control character escaped as a refusal escapes it". A refusal quotes a path with `Path`'s
//! `Debug`, which escapes a backslash as `\\` and a byte that is not UTF-8 as `\xNN`; the `out`
//! record escapes control characters only, after a lossy `Path::display`. So two different paths
//! the operator gave print the same record, and the reader cannot tell which file was written.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-render-adversary2")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn inputs(dir: &Path) -> (PathBuf, PathBuf) {
    let patch = dir.join("voice.patch");
    fs::write(
        &patch,
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive 6\n\
         node 4 LowpassFilter\nparam 4 filter_cutoff 1200\nparam 4 resonance_q 0.7\n\
         node 5 AdsrEnvelope\nparam 5 attack 0.01\nparam 5 decay 0.2\nparam 5 sustain 0.6\n\
         param 5 release 0.3\nnode 6 Vca\nnode 7 Output\nparam 7 output_gain -6\n\
         edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
         edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
         edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n",
    )
    .unwrap();
    let notes = dir.join("notes.events");
    fs::write(
        &notes,
        "patchback-notes 1\nsample_rate_hz 48000\npatch_id 7f0c0000-0000-4000-8000-000000000911\n\
         note 0 2400 45 0.8\n",
    )
    .unwrap();
    (patch, notes)
}

fn render(patch: &Path, notes: &Path, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .arg("render")
        .arg("--patch")
        .arg(patch)
        .arg("--notes")
        .arg(notes)
        .arg("--out")
        .arg(out)
        .output()
        .expect("the patchback binary runs")
}

/// The value of the `out` record of a successful render.
fn out_record(patch: &Path, notes: &Path, out: &Path) -> String {
    let output = render(patch, notes, out);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(out.exists(), "{out:?} was not written");
    let stdout = text(&output.stdout);
    stdout
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("out "))
        .unwrap_or_else(|| panic!("no out record first: {stdout:?}"))
        .to_owned()
}

/// RED. A file named `a`, backslash, `n`, `b.wav` and a file named `a`, newline, `b.wav` are two
/// files; the `out` record prints `a\nb.wav` for both, because a backslash is not escaped.
#[test]
fn a_backslash_in_the_out_path_is_not_read_as_an_escaped_newline() {
    let dir = scratch("backslash");
    let (patch, notes) = inputs(&dir);
    let newline = out_record(&patch, &notes, &dir.join("a\nb.wav"));
    let backslash = out_record(&patch, &notes, &dir.join("a\\nb.wav"));
    assert_ne!(newline, backslash, "two files, one record");
}

/// RED. Two file names that differ only in a byte that is not UTF-8 (0xFE, 0xFF) are two files;
/// `Path::display` replaces each with U+FFFD, so the `out` record is the same for both and is
/// not the path as given.
#[test]
fn an_out_path_that_is_not_utf8_is_reported_as_given() {
    let dir = scratch("not-utf8");
    let (patch, notes) = inputs(&dir);
    let fe = out_record(&patch, &notes, &dir.join(OsStr::from_bytes(b"c\xfe.wav")));
    let ff = out_record(&patch, &notes, &dir.join(OsStr::from_bytes(b"c\xff.wav")));
    assert_ne!(fe, ff, "two files, one record");
}
