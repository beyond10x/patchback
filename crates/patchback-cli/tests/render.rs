//! `patchback render --patch <file> --notes <file> --out <wav> [--block-size <frames>]` on files
//! the tests write: the acceptance scenarios of story:subtractive-voice-render
//! (`spec/scenarios/subtractive-voice-render-*.yaml`) through the binary, with exit 0 (rendered),
//! 1 (refused, one stderr line naming the error of `spec/domains/audio.yaml`, nothing written)
//! and 2 (usage error).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_audio::{WavSampleFormat, read_wav};

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-render")
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

/// The errors `spec/domains/audio.yaml` declares.
fn audio_errors() -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/domains/audio.yaml");
    let spec = fs::read_to_string(path).unwrap();
    let errors = spec.split_once("\nerrors:\n").unwrap().1;
    let errors = errors.split_once("\ncommands:\n").unwrap().0;
    errors
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- name: "))
        .map(str::to_owned)
        .collect()
}

/// The parameters of the voice as the `.patch` file carries them.
struct Voice {
    drive: &'static str,
    cutoff: &'static str,
    q: &'static str,
    gain: &'static str,
}

/// The voice of the acceptance scenarios.
const SCENARIO: Voice = Voice {
    drive: "6",
    cutoff: "1200",
    q: "0.7",
    gain: "-6",
};

/// Full drive, a resonance peak on the fundamental of MIDI 45, no output attenuation.
const HOT: Voice = Voice {
    drive: "24",
    cutoff: "110",
    q: "12",
    gain: "0",
};

const EDGES: &str = "edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
     edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
     edge 5 env Control 6 gain Control\n";
const LAST_EDGE: &str = "edge 6 out Audio 7 in Audio\n";

fn patch_text(voice: &Voice, rate: u32, edges: &str) -> String {
    format!(
        "patchback-patch 1\nsample_rate_hz {rate}\ntopology_id subtractive-mono/1\n\
         node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive {}\n\
         node 4 LowpassFilter\nparam 4 filter_cutoff {}\nparam 4 resonance_q {}\n\
         node 5 AdsrEnvelope\nparam 5 attack 0.01\nparam 5 decay 0.2\nparam 5 sustain 0.6\n\
         param 5 release 0.3\nnode 6 Vca\nnode 7 Output\nparam 7 output_gain {}\n{edges}",
        voice.drive, voice.cutoff, voice.q, voice.gain
    )
}

fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).unwrap();
    path
}

/// The compiled scenario patch.
fn patch(dir: &Path, voice: &Voice) -> PathBuf {
    write(
        dir,
        "voice.patch",
        &patch_text(voice, 48_000, &format!("{EDGES}{LAST_EDGE}")),
    )
}

/// The scenario note sequence: MIDI 45 from 0 to 24000 at velocity 0.8.
fn notes_at(dir: &Path, rate: u32, velocity: &str) -> PathBuf {
    write(
        dir,
        "notes.events",
        &format!(
            "patchback-notes 1\nsample_rate_hz {rate}\npatch_id 7f0c0000-0000-4000-8000-000000000911\n\
             note 0 24000 45 {velocity}\n"
        ),
    )
}

fn notes(dir: &Path) -> PathBuf {
    notes_at(dir, 48_000, "0.8")
}

fn render(patch: &Path, notes: &Path, out: &Path, extra: &[&str]) -> Output {
    render_env(patch, notes, out, extra, &[])
}

fn render_env(
    patch: &Path,
    notes: &Path,
    out: &Path,
    extra: &[&str],
    env: &[(&str, &str)],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_patchback"));
    command.args([
        "render",
        "--patch",
        patch.to_str().unwrap(),
        "--notes",
        notes.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    command.args(extra);
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().expect("the patchback binary runs")
}

/// The `key value` lines of a successful render.
fn report(out: &Output) -> Vec<(String, String)> {
    text(&out.stdout)
        .lines()
        .map(|line| {
            let (key, value) = line.split_once(' ').expect("key value");
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

/// The scenario render: 24000 note frames, a 300 ms release and the tail.
const SCENARIO_FRAMES: usize = 24_000 + 14_400 + patchback_dsp::RENDER_TAIL_FRAMES as usize;

/// Exit 1, nothing on stdout, one stderr line `patchback: render refused: <error>: …` naming
/// `error`, which `spec/domains/audio.yaml` declares, and no file at `out`.
fn assert_refused(output: &Output, error: &str, out: &Path) -> String {
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty());
    let stderr = text(&output.stderr);
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 1, "{stderr}");
    let prefix = format!("patchback: render refused: {error}: ");
    assert!(lines[0].starts_with(&prefix), "{stderr}");
    assert!(
        audio_errors().iter().any(|declared| declared == error),
        "{error} is not an error of spec/domains/audio.yaml"
    );
    assert!(!out.exists(), "a refused render writes nothing");
    lines[0].to_owned()
}

// subtractive-voice-render-renders

#[test]
fn render_writes_a_float32_wav_with_the_default_block_size_and_with_512() {
    let dir = scratch("renders");
    let (patch, notes) = (patch(&dir, &SCENARIO), notes(&dir));
    let out = dir.join("out.wav");
    let first = render(&patch, &notes, &out, &[]);
    assert_eq!(first.status.code(), Some(0), "{}", text(&first.stderr));
    assert!(first.stderr.is_empty());
    let frames = SCENARIO_FRAMES.to_string();
    let expected = |path: &Path, block: &str| {
        vec![
            ("out".to_owned(), format!("\"{}\"", path.display())),
            ("sample_rate_hz".to_owned(), "48000".to_owned()),
            ("block_size_frames".to_owned(), block.to_owned()),
            ("frame_count".to_owned(), frames.clone()),
            ("limited".to_owned(), "false".to_owned()),
            ("limited_samples".to_owned(), "0".to_owned()),
        ]
    };
    assert_eq!(report(&first), expected(&out, "256"));
    let clip = read_wav(&out).unwrap();
    assert_eq!(clip.sample_rate_hz(), 48_000);
    assert_eq!(clip.source_channel_count(), 1);
    assert_eq!(clip.source_format(), Some(WavSampleFormat::Float32));
    assert_eq!(clip.frame_count(), SCENARIO_FRAMES);
    assert!(clip.samples().iter().any(|s| *s != 0.0));

    let out_512 = dir.join("out-512.wav");
    let second = render(&patch, &notes, &out_512, &["--block-size", "512"]);
    assert_eq!(second.status.code(), Some(0), "{}", text(&second.stderr));
    assert_eq!(report(&second), expected(&out_512, "512"));
    // The block size changes how the work is cut, not the samples.
    assert_eq!(fs::read(&out).unwrap(), fs::read(&out_512).unwrap());
}

#[test]
fn the_same_inputs_render_the_same_bytes() {
    let dir = scratch("repeatable");
    let (patch, notes) = (patch(&dir, &SCENARIO), notes(&dir));
    let (a, b) = (dir.join("a.wav"), dir.join("b.wav"));
    assert_eq!(render(&patch, &notes, &a, &[]).status.code(), Some(0));
    assert_eq!(
        render(&patch, &notes, &b, &["--block-size", "1"])
            .status
            .code(),
        Some(0)
    );
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
}

#[test]
fn the_recorded_patch_of_the_sequence_is_not_compared_with_the_rendered_patch() {
    let dir = scratch("other-patch-id");
    let patch = patch(&dir, &SCENARIO);
    let notes = write(
        &dir,
        "notes.events",
        "patchback-notes 1\nsample_rate_hz 48000\npatch_id 00000000-0000-4000-8000-000000000000\n\
         note 0 24000 45 0.8\n",
    );
    let out = dir.join("out.wav");
    let output = render(&patch, &notes, &out, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(out.exists());
}

// subtractive-voice-render-limited

#[test]
fn a_hot_patch_renders_and_reports_limited_true_with_its_count() {
    let dir = scratch("limited");
    let (patch, notes) = (patch(&dir, &HOT), notes_at(&dir, 48_000, "1"));
    let out = dir.join("hot.wav");
    let output = render(&patch, &notes, &out, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let lines = report(&output);
    assert!(
        lines.contains(&("limited".to_owned(), "true".to_owned())),
        "{lines:?}"
    );
    let count: u64 = lines
        .iter()
        .find(|(key, _)| key == "limited_samples")
        .unwrap()
        .1
        .parse()
        .unwrap();
    assert!(count > 0);
    let clip = read_wav(&out).unwrap();
    assert!(clip.samples().iter().all(|s| s.abs() <= 1.0));
    let at_threshold = clip.samples().iter().filter(|s| s.abs() == 1.0).count() as u64;
    assert_eq!(count, at_threshold);
}

// subtractive-voice-render-refused-configuration

#[test]
fn a_sequence_at_44100_hz_is_refused_as_an_unsupported_sample_rate() {
    let dir = scratch("rate");
    let (patch, notes) = (patch(&dir, &SCENARIO), notes_at(&dir, 44_100, "0.8"));
    let out = dir.join("out.wav");
    let line = assert_refused(
        &render(&patch, &notes, &out, &[]),
        "patchback.audio.UnsupportedSampleRate",
        &out,
    );
    assert!(line.contains("44100"), "{line}");
}

#[test]
fn a_patch_at_44100_hz_is_refused_as_an_unsupported_sample_rate() {
    let dir = scratch("patch-rate");
    let patch = write(
        &dir,
        "voice.patch",
        &patch_text(&SCENARIO, 44_100, &format!("{EDGES}{LAST_EDGE}")),
    );
    let out = dir.join("out.wav");
    let line = assert_refused(
        &render(&patch, &notes(&dir), &out, &[]),
        "patchback.audio.UnsupportedSampleRate",
        &out,
    );
    assert!(line.contains("44100"), "{line}");
}

#[test]
fn a_block_size_of_zero_or_less_is_refused() {
    let dir = scratch("block");
    let (patch, notes) = (patch(&dir, &SCENARIO), notes(&dir));
    let out = dir.join("out.wav");
    for block in ["0", "-1", "-9223372036854775808"] {
        let line = assert_refused(
            &render(&patch, &notes, &out, &["--block-size", block]),
            "patchback.audio.InvalidBlockSize",
            &out,
        );
        assert!(line.contains(block), "{line}");
    }
}

#[test]
fn the_configuration_guards_precede_the_patch_and_sequence_lookups() {
    let dir = scratch("guards-first");
    let missing = dir.join("missing.patch");
    let out = dir.join("out.wav");
    // A rate other than 48 kHz outranks the block size.
    assert_refused(
        &render(
            &patch(&dir, &SCENARIO),
            &notes_at(&dir, 44_100, "0.8"),
            &out,
            &["--block-size", "0"],
        ),
        "patchback.audio.UnsupportedSampleRate",
        &out,
    );
    // Both outrank a missing patch.
    assert_refused(
        &render(&missing, &notes_at(&dir, 44_100, "0.8"), &out, &[]),
        "patchback.audio.UnsupportedSampleRate",
        &out,
    );
    assert_refused(
        &render(&missing, &notes(&dir), &out, &["--block-size", "0"]),
        "patchback.audio.InvalidBlockSize",
        &out,
    );
}

#[test]
fn a_block_size_that_is_not_a_number_is_a_usage_error() {
    let dir = scratch("block-usage");
    let out = dir.join("out.wav");
    for block in ["abc", "1.5", "99999999999999999999"] {
        let output = render(
            &patch(&dir, &SCENARIO),
            &notes(&dir),
            &out,
            &["--block-size", block],
        );
        assert_eq!(output.status.code(), Some(2), "{block}");
        assert!(text(&output.stderr).starts_with("error: "), "{block}");
        assert!(!out.exists());
    }
}

// subtractive-voice-render-refused-patch

#[test]
fn a_patch_that_does_not_exist_is_refused_as_patch_not_found() {
    let dir = scratch("no-patch");
    let missing = dir.join("missing.patch");
    let out = dir.join("out.wav");
    let line = assert_refused(
        &render(&missing, &notes(&dir), &out, &[]),
        "patchback.audio.PatchNotFound",
        &out,
    );
    assert!(line.contains("missing.patch"), "{line}");
    // A missing patch outranks a missing sequence.
    assert_refused(
        &render(&missing, &dir.join("missing.events"), &out, &[]),
        "patchback.audio.PatchNotFound",
        &out,
    );
}

#[test]
fn a_patch_file_that_does_not_load_is_refused_as_patch_not_found_with_its_reason() {
    let dir = scratch("unloadable-patch");
    let patch = write(&dir, "bad.patch", "patchback-patch 9\n");
    let out = dir.join("out.wav");
    let line = assert_refused(
        &render(&patch, &notes(&dir), &out, &[]),
        "patchback.audio.PatchNotFound",
        &out,
    );
    assert!(
        line.contains("patchback.patch.UnknownFormatVersion"),
        "{line}"
    );
    // An unknown topology is a LoadPatch refusal the graph check finds: still not loaded.
    let unknown = write(
        &dir,
        "unknown.patch",
        &patch_text(&SCENARIO, 48_000, &format!("{EDGES}{LAST_EDGE}"))
            .replace("subtractive-mono/1", "subtractive-mono/9"),
    );
    let line = assert_refused(
        &render(&unknown, &notes(&dir), &out, &[]),
        "patchback.audio.PatchNotFound",
        &out,
    );
    assert!(line.contains("patchback.patch.UnknownTopology"), "{line}");
}

#[test]
fn a_sequence_that_does_not_exist_is_refused_as_note_sequence_not_found() {
    let dir = scratch("no-notes");
    let missing = dir.join("missing.events");
    let out = dir.join("out.wav");
    let line = assert_refused(
        &render(&patch(&dir, &SCENARIO), &missing, &out, &[]),
        "patchback.audio.NoteSequenceNotFound",
        &out,
    );
    assert!(line.contains("missing.events"), "{line}");
}

#[test]
fn a_sequence_file_that_does_not_load_is_refused_as_note_sequence_not_found_with_its_reason() {
    let dir = scratch("unloadable-notes");
    let notes = write(
        &dir,
        "bad.events",
        "patchback-notes 1\nsample_rate_hz 48000\npatch_id 7f0c0000-0000-4000-8000-000000000911\n\
         note 0 24000 45 2\n",
    );
    let out = dir.join("out.wav");
    let line = assert_refused(
        &render(&patch(&dir, &SCENARIO), &notes, &out, &[]),
        "patchback.audio.NoteSequenceNotFound",
        &out,
    );
    assert!(
        line.contains("patchback.score.VelocityOutOfRange"),
        "{line}"
    );
}

#[test]
fn a_patch_that_loads_but_does_not_validate_is_refused_as_not_compiled_in_state_loaded() {
    let dir = scratch("not-compiled");
    // The fixed chain without its last edge: it loads, and validation refuses it.
    let patch = write(&dir, "open.patch", &patch_text(&SCENARIO, 48_000, EDGES));
    let out = dir.join("out.wav");
    let line = assert_refused(
        &render(&patch, &notes(&dir), &out, &[]),
        "patchback.audio.PatchNotCompiled",
        &out,
    );
    assert!(line.contains("state Loaded"), "{line}");
    assert!(
        line.contains("patchback.patch.FixedChainMismatch"),
        "{line}"
    );
    // A missing sequence outranks an uncompiled patch.
    assert_refused(
        &render(&patch, &dir.join("missing.events"), &out, &[]),
        "patchback.audio.NoteSequenceNotFound",
        &out,
    );
}

// subtractive-voice-render-non-finite

/// The test seam of debug builds: the binary replaces the sample at this frame with NaN after
/// rendering, standing in for an unstable voice (a bounded voice cannot produce one).
const NON_FINITE_SEAM: &str = "PATCHBACK_TEST_RENDER_NON_FINITE_FRAME";

#[cfg(debug_assertions)]
#[test]
fn a_render_with_a_non_finite_sample_is_refused_and_writes_nothing() {
    let dir = scratch("non-finite");
    let (patch, notes) = (patch(&dir, &SCENARIO), notes(&dir));
    let out = dir.join("unstable.wav");
    let line = assert_refused(
        &render_env(&patch, &notes, &out, &[], &[(NON_FINITE_SEAM, "1000")]),
        "patchback.audio.NonFiniteRender",
        &out,
    );
    assert!(line.contains("1000"), "{line}");
    let entries: Vec<_> = fs::read_dir(&dir).unwrap().collect();
    assert_eq!(
        entries.len(),
        2,
        "only the patch and the notes: {entries:?}"
    );
}

// The output file.

#[test]
fn an_existing_output_is_never_overwritten() {
    let dir = scratch("existing");
    let out = write(&dir, "out.wav", "keep me");
    let output = render(&patch(&dir, &SCENARIO), &notes(&dir), &out, &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = text(&output.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with("patchback: render refused: "),
        "{stderr}"
    );
    assert_eq!(fs::read_to_string(&out).unwrap(), "keep me");
}

#[test]
fn an_output_in_a_missing_directory_is_refused_and_leaves_nothing() {
    let dir = scratch("missing-dir");
    let out = dir.join("absent").join("out.wav");
    let output = render(&patch(&dir, &SCENARIO), &notes(&dir), &out, &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).starts_with("patchback: render refused: "));
    assert!(!out.exists());
}

/// The `out` record escapes control characters as a refusal does, so a path holding a newline or
/// a tab stays one `key value` line; the WAV is written at the path as given.
#[test]
fn the_out_record_escapes_control_characters_as_a_refusal_does() {
    let dir = scratch("escaped-out");
    let out = dir.join("a\nb\tc.wav");
    let output = render(&patch(&dir, &SCENARIO), &notes(&dir), &out, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let lines = report(&output);
    assert_eq!(lines.len(), 6, "{lines:?}");
    assert_eq!(lines[0].0, "out");
    assert_eq!(
        lines[0].1,
        format!("\"{}/a\\nb\\tc.wav\"", dir.display()),
        "{lines:?}"
    );
    assert!(out.exists());
}
