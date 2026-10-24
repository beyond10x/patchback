//! Rendering a compiled `subtractive-mono/1` plan with a note sequence (DESIGN.md § 5.3, § 2
//! invariants 4 and 5, § 12.2 DSP engine gate).

use patchback_audio::MAX_FRAMES;
use patchback_core::patch::{ExecutionPlan, compile, parse_patch, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{
    DEFAULT_BLOCK_FRAMES, LIMITER_THRESHOLD, RENDER_TAIL_FRAMES, RenderConfig, RenderError, render,
    render_frame_count,
};

/// The eight physical parameters of the voice, in § 5.2 order.
#[derive(Clone, Copy, Debug)]
struct Voice {
    attack: f64,
    decay: f64,
    sustain: f64,
    release: f64,
    cutoff: f64,
    q: f64,
    drive: f64,
    gain: f64,
}

/// The voice of the acceptance scenarios (`spec/scenarios/subtractive-voice-render-*.yaml`).
const SCENARIO: Voice = Voice {
    attack: 0.01,
    decay: 0.2,
    sustain: 0.6,
    release: 0.3,
    cutoff: 1200.0,
    q: 0.7,
    drive: 6.0,
    gain: -6.0,
};

/// Full drive, a resonance peak on the fundamental of MIDI 45 and no output attenuation.
const HOT: Voice = Voice {
    attack: 0.001,
    decay: 2.0,
    sustain: 1.0,
    release: 0.005,
    cutoff: 110.0,
    q: 12.0,
    drive: 24.0,
    gain: 0.0,
};

fn plan(voice: Voice) -> ExecutionPlan {
    let text = format!(
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive {}\n\
         node 4 LowpassFilter\nparam 4 filter_cutoff {}\nparam 4 resonance_q {}\n\
         node 5 AdsrEnvelope\nparam 5 attack {}\nparam 5 decay {}\nparam 5 sustain {}\n\
         param 5 release {}\nnode 6 Vca\nnode 7 Output\nparam 7 output_gain {}\n\
         edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
         edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
         edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n",
        voice.drive,
        voice.cutoff,
        voice.q,
        voice.attack,
        voice.decay,
        voice.sustain,
        voice.release,
        voice.gain
    );
    compile(&validate(&parse_patch(&text).unwrap()).unwrap())
}

fn sequence(notes: &[(u64, u64, u8, f32)]) -> NoteSequence {
    NoteSequence {
        format_version: 1,
        sample_rate_hz: 48_000,
        patch_id: "7f0c0000-0000-4000-8000-000000000911".to_owned(),
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
    }
}

/// The note of the acceptance scenarios: MIDI 45 for half a second at velocity 0.8.
fn scenario_notes() -> NoteSequence {
    sequence(&[(0, 24_000, 45, 0.8)])
}

fn config(block_size_frames: i64) -> RenderConfig {
    RenderConfig { block_size_frames }
}

fn bits(samples: &[f32]) -> Vec<u32> {
    samples.iter().map(|s| s.to_bits()).collect()
}

#[test]
fn the_default_block_size_is_256_frames() {
    assert_eq!(DEFAULT_BLOCK_FRAMES, 256);
    assert_eq!(RenderConfig::default().block_size_frames, 256);
}

#[test]
fn the_scenario_voice_renders_finite_unlimited_audio_of_the_stated_length() {
    let rendered = render(&plan(SCENARIO), &scenario_notes(), &RenderConfig::default()).unwrap();
    let expected = 24_000 + 14_400 + RENDER_TAIL_FRAMES;
    assert_eq!(rendered.samples.len() as u64, expected);
    assert_eq!(rendered.report.frame_count as u64, expected);
    assert_eq!(rendered.report.block_size_frames, 256);
    assert!(!rendered.report.limited);
    assert_eq!(rendered.report.limited_samples, 0);
    assert!(rendered.samples.iter().all(|s| s.is_finite()));
    let peak = rendered.samples.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
    assert!(peak > 0.05 && peak < 1.0, "peak {peak}");
    // The release ends 300 ms after the note-off; the tail after it is silent.
    assert!(rendered.samples[38_400..].iter().all(|s| *s == 0.0));
    assert!(rendered.samples[38_000..38_399].iter().any(|s| *s != 0.0));
}

#[test]
fn the_render_length_is_the_last_note_end_plus_release_plus_tail() {
    assert_eq!(
        render_frame_count(&plan(SCENARIO), &scenario_notes()),
        24_000 + 14_400 + RENDER_TAIL_FRAMES
    );
    let notes = sequence(&[
        (0, 100, 60, 1.0),
        (50_000, 60_000, 62, 0.5),
        (200, 300, 64, 1.0),
    ]);
    assert_eq!(
        render_frame_count(&plan(HOT), &notes),
        60_000 + 240 + RENDER_TAIL_FRAMES
    );
    let empty = sequence(&[]);
    let rendered = render(&plan(SCENARIO), &empty, &RenderConfig::default()).unwrap();
    assert_eq!(rendered.samples.len() as u64, 14_400 + RENDER_TAIL_FRAMES);
    assert!(rendered.samples.iter().all(|s| *s == 0.0));
}

#[test]
fn the_same_inputs_render_bit_identical_samples() {
    let first = render(&plan(SCENARIO), &scenario_notes(), &RenderConfig::default()).unwrap();
    let second = render(&plan(SCENARIO), &scenario_notes(), &RenderConfig::default()).unwrap();
    assert_eq!(bits(&first.samples), bits(&second.samples));
    assert_eq!(first.report, second.report);
}

#[test]
fn the_block_size_does_not_change_the_samples() {
    let notes = sequence(&[
        (100, 9_000, 45, 0.8),
        (9_000, 15_000, 57, 0.3),
        (20_001, 30_000, 81, 1.0),
    ]);
    let reference = render(&plan(SCENARIO), &notes, &config(256)).unwrap();
    for block in [1, 7, 255, 512, 4096, 1_000_000, i64::MAX] {
        let rendered = render(&plan(SCENARIO), &notes, &config(block)).unwrap();
        assert_eq!(
            bits(&rendered.samples),
            bits(&reference.samples),
            "block {block}"
        );
        assert_eq!(
            rendered.report.limited_samples,
            reference.report.limited_samples
        );
    }
}

#[test]
fn notes_take_effect_at_their_exact_sample_offsets() {
    let notes = sequence(&[(1_000, 2_000, 45, 0.8), (30_000, 31_000, 69, 0.8)]);
    let voice = Voice {
        release: 0.005,
        ..SCENARIO
    };
    for block in [1, 256, 333] {
        let samples = render(&plan(voice), &notes, &config(block))
            .unwrap()
            .samples;
        assert!(samples[..1_000].iter().all(|s| *s == 0.0), "block {block}");
        assert_ne!(samples[1_000], 0.0, "block {block}");
        // The release of the first note (240 samples) ends before the second note starts.
        assert!(
            samples[2_240..30_000].iter().all(|s| *s == 0.0),
            "block {block}"
        );
        assert!(
            samples[2_000..2_239].iter().any(|s| *s != 0.0),
            "block {block}"
        );
        assert_ne!(samples[30_000], 0.0, "block {block}");
    }
}

#[test]
fn a_note_starting_where_the_last_ends_retriggers_without_silence() {
    let notes = sequence(&[(0, 10_000, 45, 0.8), (10_000, 20_000, 52, 0.8)]);
    let samples = render(&plan(SCENARIO), &notes, &RenderConfig::default())
        .unwrap()
        .samples;
    let around: f32 = samples[9_990..10_010].iter().map(|s| s.abs()).sum();
    assert!(around > 0.0);
    assert!(samples[..20_000].iter().all(|s| s.is_finite()));
}

#[test]
fn a_hot_patch_engages_the_limiter_and_counts_each_limited_sample() {
    let notes = sequence(&[(0, 24_000, 45, 1.0)]);
    let rendered = render(&plan(HOT), &notes, &RenderConfig::default()).unwrap();
    assert!(rendered.report.limited);
    assert!(rendered.report.limited_samples > 0);
    let at_threshold = rendered
        .samples
        .iter()
        .filter(|s| f64::from(s.abs()) == LIMITER_THRESHOLD)
        .count() as u64;
    assert_eq!(rendered.report.limited_samples, at_threshold);
    assert!(
        rendered
            .samples
            .iter()
            .all(|s| f64::from(s.abs()) <= LIMITER_THRESHOLD)
    );
}

#[test]
fn bounded_controls_and_notes_give_finite_samples() {
    let lows = Voice {
        attack: 0.001,
        decay: 0.005,
        sustain: 0.0,
        release: 0.005,
        cutoff: 40.0,
        q: 0.5,
        drive: 0.0,
        gain: -24.0,
    };
    let highs = Voice {
        attack: 2.0,
        decay: 2.0,
        sustain: 1.0,
        release: 2.0,
        cutoff: 16_000.0,
        q: 12.0,
        drive: 24.0,
        gain: 0.0,
    };
    let notes = sequence(&[
        (0, 4_800, 0, 1.0),
        (4_800, 9_600, 127, 0.0),
        (9_600, 12_000, 127, 1.0),
    ]);
    for corner in 0..64u32 {
        let pick = |bit: u32| corner & (1 << bit) != 0;
        let voice = Voice {
            attack: if pick(0) { highs.attack } else { lows.attack },
            decay: if pick(0) { highs.decay } else { lows.decay },
            release: if pick(0) { highs.release } else { lows.release },
            sustain: if pick(1) { highs.sustain } else { lows.sustain },
            cutoff: if pick(2) { highs.cutoff } else { lows.cutoff },
            q: if pick(3) { highs.q } else { lows.q },
            drive: if pick(4) { highs.drive } else { lows.drive },
            gain: if pick(5) { highs.gain } else { lows.gain },
        };
        let rendered = render(&plan(voice), &notes, &config(512)).unwrap();
        assert!(
            rendered
                .samples
                .iter()
                .all(|s| s.is_finite() && f64::from(s.abs()) <= LIMITER_THRESHOLD),
            "{voice:?}"
        );
    }
}

#[test]
fn the_recorded_patch_of_the_sequence_is_not_compared() {
    let mut notes = scenario_notes();
    notes.patch_id = "00000000-0000-4000-8000-000000000000".to_owned();
    let other = render(&plan(SCENARIO), &notes, &RenderConfig::default()).unwrap();
    let own = render(&plan(SCENARIO), &scenario_notes(), &RenderConfig::default()).unwrap();
    assert_eq!(bits(&other.samples), bits(&own.samples));
}

#[test]
fn a_sequence_at_another_rate_is_refused() {
    let mut notes = scenario_notes();
    notes.sample_rate_hz = 44_100;
    assert_eq!(
        render(&plan(SCENARIO), &notes, &RenderConfig::default()),
        Err(RenderError::UnsupportedSampleRate {
            sample_rate_hz: 44_100
        })
    );
}

#[test]
fn a_non_positive_block_size_is_refused() {
    for block in [0, -1, i64::MIN] {
        assert_eq!(
            render(&plan(SCENARIO), &scenario_notes(), &config(block)),
            Err(RenderError::InvalidBlockSize {
                block_size_frames: block
            })
        );
    }
}

#[test]
fn the_rate_is_refused_before_the_block_size() {
    let mut notes = scenario_notes();
    notes.sample_rate_hz = 44_100;
    assert_eq!(
        render(&plan(SCENARIO), &notes, &config(0)),
        Err(RenderError::UnsupportedSampleRate {
            sample_rate_hz: 44_100
        })
    );
}

#[test]
fn a_render_past_the_clip_bound_is_refused_before_it_runs() {
    let end = MAX_FRAMES as u64;
    let notes = sequence(&[(0, end, 45, 0.8)]);
    let frames = end + 14_400 + RENDER_TAIL_FRAMES;
    assert_eq!(
        render(&plan(SCENARIO), &notes, &RenderConfig::default()),
        Err(RenderError::RenderTooLong {
            frame_count: frames
        })
    );
    let notes = sequence(&[(0, u64::MAX, 45, 0.8)]);
    assert_eq!(
        render(&plan(SCENARIO), &notes, &RenderConfig::default()),
        Err(RenderError::RenderTooLong {
            frame_count: u64::MAX
        })
    );
}

#[test]
fn out_of_range_velocities_of_a_hand_built_sequence_stay_finite() {
    let mut notes = sequence(&[(0, 4_800, 45, 0.8), (4_800, 9_600, 45, 0.8)]);
    notes.notes[0].velocity = f32::NAN;
    notes.notes[1].velocity = f32::MAX;
    let rendered = render(&plan(SCENARIO), &notes, &RenderConfig::default()).unwrap();
    assert!(rendered.samples.iter().all(|s| s.is_finite()));
}

/// Every note of a hand-built sequence that does not end after it starts is refused before
/// anything is rendered, as the sequence loader refuses it (`patchback.score.InvalidNoteTiming`),
/// wherever it stands in the sequence.
#[test]
fn a_note_that_does_not_end_after_it_starts_is_refused_as_a_sequence_that_does_not_load() {
    for notes in [
        vec![(1_000, 1_000, 45, 0.8)],
        vec![(0, 0, 45, 0.8)],
        vec![(2_000, 1_000, 45, 0.8)],
        vec![(0, 4_800, 45, 0.8), (9_600, 9_600, 45, 0.8)],
        vec![(9_600, 4_800, 45, 0.8), (0, 4_800, 45, 0.8)],
    ] {
        let refused = render(&plan(SCENARIO), &sequence(&notes), &RenderConfig::default()).err();
        let Some(error) = refused else {
            panic!("{notes:?} rendered");
        };
        assert_eq!(
            error.domain_error(),
            "patchback.audio.NoteSequenceNotFound",
            "{notes:?}"
        );
        assert!(
            error
                .to_string()
                .contains("patchback.score.InvalidNoteTiming"),
            "{error}"
        );
    }
}

/// The rest of what the sequence loader refuses (overlapping notes, a MIDI number above 127, a
/// velocity outside `[0, 1]`) has defined semantics in `render` (a later note takes the voice,
/// the number and the velocity are clamped): each such hand-built sequence renders finite samples
/// and closes its gate, so the frames after the last release are silent.
#[test]
fn the_other_sequences_the_loader_refuses_render_and_close_their_gate() {
    for notes in [
        vec![(0, 9_600, 45, 0.8), (4_800, 7_200, 57, 0.8)],
        vec![(0, 9_600, 45, 0.8), (4_800, 14_400, 57, 0.8)],
        vec![(0, 4_800, 45, 0.8), (0, 9_600, 57, 0.8)],
        vec![(0, 9_600, 200, 0.8)],
        vec![(0, 9_600, 45, f32::NAN)],
        vec![(0, 9_600, 45, -1.0)],
        vec![(0, 9_600, 45, f32::MAX)],
        vec![(0, 9_600, 45, f32::INFINITY)],
    ] {
        let rendered =
            render(&plan(SCENARIO), &sequence(&notes), &RenderConfig::default()).unwrap();
        assert!(rendered.samples.iter().all(|s| s.is_finite()), "{notes:?}");
        let last_end = notes.iter().map(|n| n.1).max().unwrap() as usize;
        assert!(
            rendered.samples[last_end + 14_400..]
                .iter()
                .all(|s| *s == 0.0),
            "{notes:?}"
        );
    }
}
