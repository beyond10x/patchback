//! Adversarial cases for `patchback render` (story:subtractive-voice-render, wave a5): the
//! acceptance scenarios read as the specification, the `key value` output contract, and guards on
//! the refusal order and on determinism with the limiter engaged.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_audio::read_wav;

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-render-adversary")
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

const EDGES: &str = "edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
     edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
     edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n";

/// The voice of `spec/scenarios/subtractive-voice-render-renders.yaml` (drive 6, cutoff 1200,
/// Q 0.7, ADSR 0.01/0.2/0.6/0.3, output gain -6), or a hot one.
fn patch_file(dir: &Path, rate: u32, hot: bool) -> PathBuf {
    let (drive, cutoff, q, gain) = if hot {
        ("24", "110", "12", "0")
    } else {
        ("6", "1200", "0.7", "-6")
    };
    let text = format!(
        "patchback-patch 1\nsample_rate_hz {rate}\ntopology_id subtractive-mono/1\n\
         node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive {drive}\n\
         node 4 LowpassFilter\nparam 4 filter_cutoff {cutoff}\nparam 4 resonance_q {q}\n\
         node 5 AdsrEnvelope\nparam 5 attack 0.01\nparam 5 decay 0.2\nparam 5 sustain 0.6\n\
         param 5 release 0.3\nnode 6 Vca\nnode 7 Output\nparam 7 output_gain {gain}\n{EDGES}"
    );
    let path = dir.join(if hot { "hot.patch" } else { "voice.patch" });
    fs::write(&path, text).unwrap();
    path
}

fn notes_file(dir: &Path, rate: u32, body: &str) -> PathBuf {
    let path = dir.join("notes.events");
    fs::write(
        &path,
        format!(
            "patchback-notes 1\nsample_rate_hz {rate}\npatch_id 7f0c0000-0000-4000-8000-000000000911\n{body}"
        ),
    )
    .unwrap();
    path
}

fn render(patch: &Path, notes: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .arg("render")
        .arg("--patch")
        .arg(patch)
        .arg("--notes")
        .arg(notes)
        .arg("--out")
        .arg(out)
        .args(extra)
        .output()
        .expect("the patchback binary runs")
}

fn refused_with(output: &Output, error: &str) {
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with(&format!("patchback: render refused: {error}: ")),
        "{stderr}"
    );
}

/// RED. `spec/scenarios/subtractive-voice-render-renders.yaml:120-131` imports the WAV the
/// default-block render wrote (`path: out.wav`) with `frame_count: 24000`, the length of its one
/// note (0 to 24000). The binary writes the note, the 300 ms release and a 4800-frame tail. The
/// expected count is read from the scenario, so the case follows whichever side is corrected.
#[test]
fn the_rendered_wav_has_the_frame_count_the_renders_scenario_imports() {
    let dir = scratch("scenario-frame-count");
    let patch = patch_file(&dir, 48_000, false);
    let notes = notes_file(&dir, 48_000, "note 0 24000 45 0.8\n");
    let out = dir.join("out.wav");
    let output = render(&patch, &notes, &out, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let clip = read_wav(&out).unwrap();
    let scenario = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/scenarios/subtractive-voice-render-renders.yaml"
    ))
    .unwrap();
    let imported: usize = scenario
        .split_once("path: out.wav\n")
        .expect("the scenario imports out.wav")
        .1
        .lines()
        .find_map(|line| line.trim().strip_prefix("frame_count: "))
        .expect("the import states its frame_count")
        .parse()
        .unwrap();
    assert_eq!(
        clip.frame_count(),
        imported,
        "the renders scenario imports out.wav with frame_count {imported}"
    );
}

/// RED. The module documentation of `crates/patchback-cli/src/render.rs` promises "one
/// `key value` line per record"; the refusal path escapes control characters to keep its one
/// line, the success path prints `out` with `Path::display`, so a path holding a newline splits
/// the `out` record over two lines and a reader of the output sees seven records.
#[test]
fn an_out_path_with_a_newline_keeps_one_key_value_record_per_line() {
    let dir = scratch("newline-out");
    let patch = patch_file(&dir, 48_000, false);
    let notes = notes_file(&dir, 48_000, "note 0 2400 45 0.8\n");
    let out = dir.join("a\nb.wav");
    let output = render(&patch, &notes, &out, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    let keys: Vec<&str> = stdout
        .lines()
        .map(|line| line.split_once(' ').map_or(line, |(key, _)| key))
        .collect();
    assert_eq!(
        keys,
        [
            "out",
            "sample_rate_hz",
            "block_size_frames",
            "frame_count",
            "limited",
            "limited_samples"
        ],
        "{stdout:?}"
    );
    assert_eq!(
        stdout.lines().next(),
        Some(format!("out \"{}/a\\nb.wav\"", dir.display()).as_str()),
        "{stdout:?}"
    );
}

/// Passing regression guard: the input guards answer before the lookups in every pairing the
/// existing cases leave out (a patch at 44.1 kHz with block size 0; a sequence at 44.1 kHz with a
/// patch that does not load; block size 0 with a patch that loads and does not validate).
#[test]
fn the_input_guards_answer_first_in_the_remaining_pairings() {
    let dir = scratch("guard-pairings");
    let out = dir.join("out.wav");
    let good_notes = notes_file(&dir, 48_000, "note 0 2400 45 0.8\n");
    let patch_44 = patch_file(&dir, 44_100, false);
    refused_with(
        &render(&patch_44, &good_notes, &out, &["--block-size", "0"]),
        "patchback.audio.UnsupportedSampleRate",
    );
    let unloadable = dir.join("bad.patch");
    fs::write(&unloadable, "patchback-patch 9\n").unwrap();
    let notes_44 = notes_file(&dir, 44_100, "note 0 2400 45 0.8\n");
    refused_with(
        &render(&unloadable, &notes_44, &out, &[]),
        "patchback.audio.UnsupportedSampleRate",
    );
    let good_notes = notes_file(&dir, 48_000, "note 0 2400 45 0.8\n");
    let open = dir.join("open.patch");
    let open_text = fs::read_to_string(patch_file(&dir, 48_000, false))
        .unwrap()
        .replace("edge 6 out Audio 7 in Audio\n", "");
    fs::write(&open, open_text).unwrap();
    refused_with(
        &render(&open, &good_notes, &out, &["--block-size=-0"]),
        "patchback.audio.InvalidBlockSize",
    );
    assert!(!out.exists());
}

/// Passing regression guard: a block size one past `i64::MAX` is a usage error, not a refusal or
/// a render.
#[test]
fn a_block_size_past_i64_is_a_usage_error() {
    let dir = scratch("block-overflow");
    let patch = patch_file(&dir, 48_000, false);
    let notes = notes_file(&dir, 48_000, "note 0 2400 45 0.8\n");
    let out = dir.join("out.wav");
    let output = render(
        &patch,
        &notes,
        &out,
        &["--block-size", "9223372036854775808"],
    );
    assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
    assert!(!out.exists());
}

/// Passing regression guard: with the limiter engaged and adjacent notes retriggering, two
/// processes at block sizes 1 and 257 write the same bytes and report the same count.
#[test]
fn a_limited_render_is_byte_identical_across_processes_and_block_sizes() {
    let dir = scratch("limited-determinism");
    let patch = patch_file(&dir, 48_000, true);
    let notes = notes_file(
        &dir,
        48_000,
        "note 0 12000 45 1\nnote 12000 20000 57 0.5\nnote 30000 40000 33 1\n",
    );
    let (a, b) = (dir.join("a.wav"), dir.join("b.wav"));
    let first = render(&patch, &notes, &a, &["--block-size", "1"]);
    let second = render(&patch, &notes, &b, &["--block-size", "257"]);
    assert_eq!(first.status.code(), Some(0), "{}", text(&first.stderr));
    assert_eq!(second.status.code(), Some(0), "{}", text(&second.stderr));
    let count = |output: &Output| {
        text(&output.stdout)
            .lines()
            .find_map(|line| line.strip_prefix("limited_samples "))
            .unwrap()
            .to_owned()
    };
    assert_ne!(count(&first), "0");
    assert_eq!(count(&first), count(&second));
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
}
