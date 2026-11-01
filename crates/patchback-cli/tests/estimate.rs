//! `patchback estimate --input <wav> --out <dir>` (story:note-estimation, approval-record:wave-c1
//! Dc1-6) and the `--estimation <estimation.txt>` flag of `render` and `fit` (Dc1-8) through the
//! binary: the acceptance scenarios `spec/scenarios/note-estimation-estimated.yaml`,
//! `note-estimation-low-confidence.yaml`, `note-estimation-polyphonic.yaml` and
//! `note-events-from-estimation.yaml`, with exit 0 (estimated), 1 (refused, one stderr line, nothing
//! written) and 2 (usage error).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_analysis::{estimate_notes, read_estimation};
use patchback_audio::{AudioClip, WavSampleFormat, write_wav};
use patchback_core::patch::{compile, parse_patch, validate};
use patchback_core::score::{NoteEvent, NoteSequence, parse_events, write_events};
use patchback_dsp::{RenderConfig, render};

/// The plain voice of the phrases: attack, decay, sustain, release, cutoff, Q, drive, gain.
const VOICE: [f64; 8] = [0.005, 0.08, 0.7, 0.03, 6_000.0, 0.7, 0.0, -6.0];

fn patch_text(voice: [f64; 8]) -> String {
    format!(
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive {}\n\
         node 4 LowpassFilter\nparam 4 filter_cutoff {}\nparam 4 resonance_q {}\n\
         node 5 AdsrEnvelope\nparam 5 attack {}\nparam 5 decay {}\nparam 5 sustain {}\n\
         param 5 release {}\nnode 6 Vca\nnode 7 Output\nparam 7 output_gain {}\n\
         edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
         edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
         edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n",
        voice[6], voice[4], voice[5], voice[0], voice[1], voice[2], voice[3], voice[7]
    )
}

fn render_notes(voice: [f64; 8], notes: &[(u64, u64, u8, f32)]) -> Vec<f32> {
    let plan = compile(&validate(&parse_patch(&patch_text(voice)).unwrap()).unwrap());
    let sequence = NoteSequence {
        format_version: 1,
        sample_rate_hz: 48_000,
        patch_id: "7f0c0000-0000-4000-8000-0000000009e1".to_owned(),
        note_estimation_id: None,
        notes: notes
            .iter()
            .map(
                |&(start_sample, end_sample, midi_note, velocity)| NoteEvent {
                    start_sample,
                    end_sample,
                    midi_note,
                    velocity,
                },
            )
            .collect(),
    };
    render(&plan, &sequence, &RenderConfig::default())
        .unwrap()
        .samples
}

const PHRASE: [(u64, u64, u8, f32); 3] = [
    (4_800, 19_200, 45, 0.9),
    (20_160, 34_560, 52, 0.7),
    (34_560, 52_800, 57, 0.8),
];

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-estimate")
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

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("output is UTF-8")
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

fn estimate(input: &Path, out: &Path) -> Output {
    run(&[
        "estimate",
        "--input",
        input.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ])
}

/// A refusal: exit 1, nothing on stdout, one stderr line starting with `prefix`.
fn assert_refused(out: &Output, prefix: &str) -> String {
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(out.stdout.is_empty(), "{}", text(&out.stdout));
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.ends_with('\n'), "{stderr:?}");
    assert!(stderr.starts_with(prefix), "{stderr}");
    stderr
}

// note-estimation-estimated

#[test]
fn estimate_writes_the_estimation_and_a_note_sequence_that_records_it() {
    let dir = scratch("estimated");
    let samples = render_notes(VOICE, &PHRASE);
    let input = wav(&dir, "phrase.wav", samples.clone());
    let out_dir = dir.join("nested").join("estimation");
    let out = estimate(&input, &out_dir);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));

    let written = fs::read_to_string(out_dir.join("estimation.txt")).unwrap();
    let estimation = read_estimation(&written).unwrap();
    assert_eq!(estimation, estimate_notes(&samples, 48_000).unwrap());
    assert_eq!(estimation.notes.len(), 3);

    let events = fs::read_to_string(out_dir.join("notes.events")).unwrap();
    let sequence = parse_events(&events).unwrap();
    assert_eq!(sequence, estimation.note_sequence().canonical());
    assert_eq!(events, write_events(&estimation.note_sequence()).unwrap());
    assert_eq!(
        sequence.note_estimation_id.as_deref(),
        Some(estimation.note_estimation_id.as_str())
    );

    assert_eq!(
        text(&out.stdout),
        format!(
            "out {out_dir:?}\nnote_estimation_id {}\nnotes 3\n",
            estimation.note_estimation_id
        )
    );
    let mut names: Vec<String> = fs::read_dir(&out_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["estimation.txt", "notes.events"]);
}

#[test]
fn two_estimates_of_one_clip_write_identical_bytes() {
    let dir = scratch("deterministic");
    let input = wav(&dir, "phrase.wav", render_notes(VOICE, &PHRASE));
    for out in ["first", "second"] {
        let run = estimate(&input, &dir.join(out));
        assert_eq!(run.status.code(), Some(0), "{}", text(&run.stderr));
    }
    for file in ["estimation.txt", "notes.events"] {
        assert_eq!(
            fs::read(dir.join("first").join(file)).unwrap(),
            fs::read(dir.join("second").join(file)).unwrap(),
            "{file}"
        );
    }
}

// note-estimation-low-confidence

#[test]
fn a_low_confidence_phrase_is_refused_naming_each_low_note_and_its_quantities() {
    let dir = scratch("low-confidence");
    let voice = [0.002, 0.08, 0.7, 0.01, 6_000.0, 0.7, 0.0, -6.0];
    let notes = [
        (4_800, 24_000, 45, 0.8),
        (33_600, 34_080, 57, 0.8),
        (43_200, 62_400, 50, 0.8),
    ];
    let input = wav(&dir, "phrase.wav", render_notes(voice, &notes));
    let out_dir = dir.join("estimation");
    let out = estimate(&input, &out_dir);
    let line = assert_refused(
        &out,
        "patchback: estimate refused: patchback.analysis.LowConfidenceNotes: ",
    );
    assert!(line.contains("note 1 (pitch"), "{line}");
    assert!(!line.contains("note 0 ("), "{line}");
    assert!(!line.contains("note 2 ("), "{line}");
    assert!(!out_dir.exists(), "a refusal writes nothing");
}

// note-estimation-polyphonic

#[test]
fn a_polyphonic_clip_is_refused_and_nothing_is_written() {
    let dir = scratch("polyphonic");
    let voice = [0.01, 0.1, 0.8, 0.05, 5_000.0, 0.7, 0.0, -3.0];
    let low = render_notes(voice, &[(4_800, 33_600, 52, 0.8)]);
    let high = render_notes(voice, &[(4_800, 33_600, 59, 0.8)]);
    let samples = low.iter().zip(&high).map(|(a, b)| 0.5 * (a + b)).collect();
    let input = wav(&dir, "dyad.wav", samples);
    let out_dir = dir.join("estimation");
    let out = estimate(&input, &out_dir);
    assert_refused(
        &out,
        "patchback: estimate refused: patchback.analysis.PolyphonicInput: ",
    );
    assert!(!out_dir.exists(), "a refusal writes nothing");
}

#[test]
fn estimate_refuses_unreadable_silent_and_existing_outputs_and_writes_nothing() {
    let dir = scratch("refusals");
    let out_dir = dir.join("estimation");

    let missing = estimate(&dir.join("absent.wav"), &out_dir);
    let line = assert_refused(&missing, "patchback: estimate refused: ");
    assert!(line.contains("absent.wav"), "{line}");
    assert!(!out_dir.exists());

    let silent = wav(&dir, "silent.wav", vec![0.0; 48_000]);
    assert_refused(
        &estimate(&silent, &out_dir),
        "patchback: estimate refused: patchback.analysis.SilentInput: ",
    );
    assert!(!out_dir.exists());

    let input = wav(&dir, "phrase.wav", render_notes(VOICE, &PHRASE));
    for existing in ["estimation.txt", "notes.events"] {
        let target = dir.join(format!("exists-{existing}"));
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join(existing), "keep\n").unwrap();
        let line = assert_refused(&estimate(&input, &target), "patchback: estimate refused: ");
        assert!(line.contains("already exists"), "{line}");
        assert!(line.contains(existing), "{line}");
        assert_eq!(fs::read_to_string(target.join(existing)).unwrap(), "keep\n");
        assert_eq!(
            fs::read_dir(&target).unwrap().count(),
            1,
            "nothing else written"
        );
    }

    let file = dir.join("a-file");
    fs::write(&file, "x").unwrap();
    let line = assert_refused(&estimate(&input, &file), "patchback: estimate refused: ");
    assert!(line.contains("is not a directory"), "{line}");
    assert_eq!(fs::read_to_string(&file).unwrap(), "x");
}

// note-events-from-estimation: `--estimation` on render and fit (Dc1-8)

/// The phrase's WAV, its estimation directory (from `estimate`) and a patch file.
fn estimated_phrase(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let input = wav(dir, "phrase.wav", render_notes(VOICE, &PHRASE));
    let out_dir = dir.join("estimation");
    let out = estimate(&input, &out_dir);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let patch = dir.join("voice.patch");
    fs::write(&patch, patch_text(VOICE)).unwrap();
    (input, out_dir, patch)
}

fn render_with(patch: &Path, notes: &Path, out: &Path, estimation: Option<&Path>) -> Output {
    let mut args = vec![
        "render".to_owned(),
        "--patch".to_owned(),
        patch.to_str().unwrap().to_owned(),
        "--notes".to_owned(),
        notes.to_str().unwrap().to_owned(),
        "--out".to_owned(),
        out.to_str().unwrap().to_owned(),
    ];
    if let Some(estimation) = estimation {
        args.push("--estimation".to_owned());
        args.push(estimation.to_str().unwrap().to_owned());
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&args)
}

#[test]
fn render_binds_a_sequence_to_the_estimation_it_names() {
    let dir = scratch("render-bound");
    let (_, estimation_dir, patch) = estimated_phrase(&dir);
    let notes = estimation_dir.join("notes.events");
    let estimation = estimation_dir.join("estimation.txt");
    let out = render_with(&patch, &notes, &dir.join("bound.wav"), Some(&estimation));
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(dir.join("bound.wav").exists());
}

#[test]
fn render_refuses_a_sequence_whose_estimation_is_not_given() {
    let dir = scratch("render-unbound");
    let (_, estimation_dir, patch) = estimated_phrase(&dir);
    let notes = estimation_dir.join("notes.events");
    let wav_out = dir.join("unbound.wav");
    let line = assert_refused(
        &render_with(&patch, &notes, &wav_out, None),
        "patchback: render refused: patchback.audio.NoteSequenceNotFound: ",
    );
    assert!(
        line.contains("patchback.score.NoteEstimationNotFound"),
        "{line}"
    );
    let id = read_estimation(&fs::read_to_string(estimation_dir.join("estimation.txt")).unwrap())
        .unwrap()
        .note_estimation_id;
    assert!(line.contains(&id), "{line}");
    assert!(!wav_out.exists());

    // Another clip's estimation does not bind it either.
    let other_dir = dir.join("other");
    fs::create_dir_all(&other_dir).unwrap();
    let other = wav(
        &other_dir,
        "other.wav",
        render_notes(
            VOICE,
            &[(4_800, 24_000, 60, 0.8), (28_800, 48_000, 64, 0.8)],
        ),
    );
    let other_out = other_dir.join("estimation");
    assert_eq!(estimate(&other, &other_out).status.code(), Some(0));
    let line = assert_refused(
        &render_with(
            &patch,
            &notes,
            &wav_out,
            Some(&other_out.join("estimation.txt")),
        ),
        "patchback: render refused: patchback.audio.NoteSequenceNotFound: ",
    );
    assert!(
        line.contains("patchback.score.NoteEstimationNotFound"),
        "{line}"
    );
    assert!(!wav_out.exists());

    // An estimation file that cannot be read is refused, never ignored.
    let broken = dir.join("broken.txt");
    fs::write(&broken, "patchback-note-estimation 1\n").unwrap();
    let line = assert_refused(
        &render_with(&patch, &notes, &wav_out, Some(&broken)),
        "patchback: render refused: patchback.audio.NoteSequenceNotFound: ",
    );
    assert!(
        line.contains("patchback.score.NoteEstimationNotFound"),
        "{line}"
    );
    assert!(line.contains("broken.txt"), "{line}");
    assert!(!wav_out.exists());
}

/// The score domain's precedence: an unknown patch is answered before an unknown estimation.
#[test]
fn render_answers_an_unknown_patch_before_an_unknown_estimation() {
    let dir = scratch("render-precedence");
    let (_, estimation_dir, _) = estimated_phrase(&dir);
    let notes = estimation_dir.join("notes.events");
    assert_refused(
        &render_with(&dir.join("absent.patch"), &notes, &dir.join("x.wav"), None),
        "patchback: render refused: patchback.audio.PatchNotFound: ",
    );
}

fn fit_with(input: &Path, notes: &Path, out: &Path, estimation: Option<&Path>) -> Output {
    let mut args = vec![
        "fit".to_owned(),
        "--input".to_owned(),
        input.to_str().unwrap().to_owned(),
        "--notes".to_owned(),
        notes.to_str().unwrap().to_owned(),
        "--out".to_owned(),
        out.to_str().unwrap().to_owned(),
        "--max-renders".to_owned(),
        "3".to_owned(),
        "--max-restarts".to_owned(),
        "1".to_owned(),
    ];
    if let Some(estimation) = estimation {
        args.push("--estimation".to_owned());
        args.push(estimation.to_str().unwrap().to_owned());
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&args)
}

#[test]
fn fit_binds_a_sequence_to_its_estimation_and_refuses_it_without_one() {
    let dir = scratch("fit-bound");
    let (input, estimation_dir, _) = estimated_phrase(&dir);
    let notes = estimation_dir.join("notes.events");
    let estimation = estimation_dir.join("estimation.txt");

    let unbound = dir.join("unbound");
    let line = assert_refused(
        &fit_with(&input, &notes, &unbound, None),
        "patchback: fit refused: patchback.fit.NoteSequenceNotFound: ",
    );
    assert!(
        line.contains("patchback.score.NoteEstimationNotFound"),
        "{line}"
    );
    assert!(!unbound.exists());

    let bound = dir.join("bound");
    let out = fit_with(&input, &notes, &bound, Some(&estimation));
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let written = parse_events(&fs::read_to_string(bound.join("notes.events")).unwrap()).unwrap();
    assert_eq!(
        written,
        parse_events(&fs::read_to_string(&notes).unwrap()).unwrap()
    );
}

/// Dc1-14: the estimation binds the sequence only to the clip it was estimated from.
#[test]
fn fit_refuses_an_estimation_of_another_clip_than_its_input() {
    let dir = scratch("fit-other-clip");
    let (_, estimation_dir, _) = estimated_phrase(&dir);
    let notes = estimation_dir.join("notes.events");
    let estimation = estimation_dir.join("estimation.txt");
    // The same notes, rendered a little louder: another clip.
    let mut louder = render_notes(VOICE, &PHRASE);
    for sample in &mut louder {
        *sample *= 1.25;
    }
    let other = wav(&dir, "other.wav", louder);
    let out = dir.join("out");
    let line = assert_refused(
        &fit_with(&other, &notes, &out, Some(&estimation)),
        "patchback: fit refused: patchback.fit.NoteSequenceNotFound: ",
    );
    assert!(
        line.contains("patchback.score.EstimationClipMismatch"),
        "{line}"
    );
    assert!(line.contains("other.wav"), "{line}");
    assert!(!out.exists());
}
