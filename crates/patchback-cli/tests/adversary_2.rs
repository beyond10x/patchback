//! Adversary pass 2 on story:note-estimation: `patchback fit --estimation` and Dc1-14 ("`patchback
//! fit` refuses an estimation of another clip than its `--input`") through the binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_analysis::read_estimation;
use patchback_audio::{AudioClip, WavSampleFormat, write_wav};
use patchback_core::patch::{compile, parse_patch, validate};
use patchback_core::score::{NoteEvent, NoteSequence, parse_events, write_events};
use patchback_dsp::{RenderConfig, render};

const VOICE: [f64; 8] = [0.005, 0.08, 0.7, 0.03, 6_000.0, 0.7, 0.0, -6.0];

const PHRASE: [(u64, u64, u8, f32); 3] = [
    (4_800, 19_200, 45, 0.9),
    (20_160, 34_560, 52, 0.7),
    (34_560, 52_800, 57, 0.8),
];

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

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-estimate-adversary-2")
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
    String::from_utf8(bytes.to_vec()).unwrap()
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

/// `fit --estimation` is read and checked whether or not the sequence names an estimation (the
/// module documentation of `fit.rs`, step 3); Dc1-14 says `fit` refuses an estimation of another
/// clip than its `--input`. A sequence that names no estimation, given with the estimation of
/// another clip, is fitted all the same.
#[test]
fn fit_refuses_a_given_estimation_of_another_clip_even_when_the_notes_name_none() {
    let dir = scratch("unnamed-other-clip");
    let input = wav(&dir, "phrase.wav", render_notes(VOICE, &PHRASE));
    let estimation_dir = dir.join("estimation");
    let out = run(&[
        "estimate",
        "--input",
        input.to_str().unwrap(),
        "--out",
        estimation_dir.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let estimation = estimation_dir.join("estimation.txt");
    assert!(read_estimation(&fs::read_to_string(&estimation).unwrap()).is_ok());

    // The estimated notes, with no estimation named.
    let mut sequence =
        parse_events(&fs::read_to_string(estimation_dir.join("notes.events")).unwrap()).unwrap();
    sequence.note_estimation_id = None;
    let notes = dir.join("plain.events");
    fs::write(&notes, write_events(&sequence).unwrap()).unwrap();

    // Another clip: the same notes 2 dB louder.
    let louder: Vec<f32> = render_notes(VOICE, &PHRASE)
        .iter()
        .map(|s| s * 1.25)
        .collect();
    let other = wav(&dir, "other.wav", louder);
    let results = dir.join("out");
    let fit = run(&[
        "fit",
        "--input",
        other.to_str().unwrap(),
        "--notes",
        notes.to_str().unwrap(),
        "--out",
        results.to_str().unwrap(),
        "--max-renders",
        "3",
        "--max-restarts",
        "1",
        "--estimation",
        estimation.to_str().unwrap(),
    ]);
    let stderr = text(&fit.stderr);
    assert_eq!(
        fit.status.code(),
        Some(1),
        "fit of other.wav with the estimation of phrase.wav exited {:?}; stderr {stderr:?}",
        fit.status.code()
    );
    assert!(
        stderr.contains("patchback.score.EstimationClipMismatch"),
        "{stderr}"
    );
    assert!(!results.exists());
}
