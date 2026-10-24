//! Adversarial cases for the `subtractive-mono/1` renderer (story:subtractive-voice-render, wave
//! a5): the documented note semantics of `render`, the polyBLEP residual before the wrap, the
//! clip bound one frame past it, and block-size invariance with the limiter engaged.

use patchback_audio::MAX_FRAMES;
use patchback_core::patch::{ExecutionPlan, compile, parse_patch, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{
    PolyBlepSaw, RENDER_TAIL_FRAMES, RenderConfig, RenderError, render, render_frame_count,
};

fn plan(hot: bool) -> ExecutionPlan {
    let (drive, cutoff, q, attack, decay, sustain, release, gain) = if hot {
        (24.0, 110.0, 12.0, 0.001, 2.0, 1.0, 0.005, 0.0)
    } else {
        (6.0, 1200.0, 0.7, 0.01, 0.2, 0.6, 0.3, -6.0)
    };
    let text = format!(
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive {drive}\n\
         node 4 LowpassFilter\nparam 4 filter_cutoff {cutoff}\nparam 4 resonance_q {q}\n\
         node 5 AdsrEnvelope\nparam 5 attack {attack}\nparam 5 decay {decay}\n\
         param 5 sustain {sustain}\nparam 5 release {release}\nnode 6 Vca\nnode 7 Output\n\
         param 7 output_gain {gain}\n\
         edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
         edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
         edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n"
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

fn bits(samples: &[f32]) -> Vec<u32> {
    samples.iter().map(|s| s.to_bits()).collect()
}

/// State built by hand: `read_events` refuses a note with `end_sample <= start_sample`
/// (`patchback.score.InvalidNoteTiming`). Rendered, such a note had its note-off sorted before its
/// own note-on, the gate never closed, and the voice sounded through the [`RENDER_TAIL_FRAMES`]
/// tail to a cut-off at a non-zero level. `render` refuses it as the CLI refuses a sequence that
/// does not load: `patchback.audio.NoteSequenceNotFound`, naming the score error (D12).
#[test]
fn a_zero_length_note_of_a_hand_built_sequence_is_refused_as_a_sequence_that_does_not_load() {
    let notes = sequence(&[(1_000, 1_000, 45, 0.8)]);
    let error = render(&plan(false), &notes, &RenderConfig::default()).err();
    assert_eq!(
        error,
        Some(RenderError::InvalidNoteTiming {
            start_sample: 1_000,
            end_sample: 1_000
        })
    );
    let error = error.unwrap();
    assert_eq!(error.domain_error(), "patchback.audio.NoteSequenceNotFound");
    assert!(
        error
            .to_string()
            .contains("patchback.score.InvalidNoteTiming"),
        "{error}"
    );
}

/// Passing regression guard against the mutant that drops the pre-wrap polyBLEP branch
/// (`t > 1 - dt`), which no golden of `tests/units.rs` reaches: at 750 Hz the phase is k/64
/// exactly and never exceeds `1 - dt`. At 18 kHz (`dt` = 3/8) the phases cycle 0, 3/8, 6/8, 1/8,
/// 4/8, 7/8, 2/8, 5/8, so both branches fire; the values are the closed form of the two-sample
/// polyBLEP saw, and the cycle is odd about its midpoint as a band-limited saw is.
#[test]
fn the_saw_at_18_khz_matches_its_golden_cycle_through_both_polyblep_branches() {
    let golden = [
        0.0,
        -1.0 / 4.0,
        7.0 / 18.0,
        -11.0 / 36.0,
        0.0,
        11.0 / 36.0,
        -7.0 / 18.0,
        1.0 / 4.0,
    ];
    let mut saw = PolyBlepSaw::new();
    saw.set_frequency(18_000.0);
    for k in 0..80 {
        let y = saw.next_sample();
        let expected = golden[k % 8];
        assert!(
            (y - expected).abs() < 1e-12,
            "sample {k}: {y} against {expected}"
        );
    }
}

/// Passing regression guard: one frame past the clip bound is refused, before anything is
/// allocated, with the frame count it would have had (the existing case is 19200 frames past).
#[test]
fn a_render_one_frame_past_max_frames_is_refused() {
    let release_and_tail = 14_400 + RENDER_TAIL_FRAMES;
    let end = MAX_FRAMES as u64 + 1 - release_and_tail;
    let notes = sequence(&[(0, end, 45, 0.8)]);
    assert_eq!(
        render_frame_count(&plan(false), &notes),
        MAX_FRAMES as u64 + 1
    );
    assert_eq!(
        render(&plan(false), &notes, &RenderConfig::default()),
        Err(RenderError::RenderTooLong {
            frame_count: MAX_FRAMES as u64 + 1
        })
    );
}

/// Passing regression guard: with the limiter engaged, retriggered adjacent notes and a gap, every
/// block size (1, 7, 255, 257, 4096, the frame count and one past it) gives the samples and the
/// limited count of the default 256.
#[test]
fn a_limited_render_is_identical_at_every_block_size() {
    let notes = sequence(&[
        (0, 12_000, 45, 1.0),
        (12_000, 20_000, 57, 0.5),
        (30_000, 40_000, 33, 1.0),
    ]);
    let hot = plan(true);
    let reference = render(&hot, &notes, &RenderConfig::default()).unwrap();
    assert!(reference.report.limited_samples > 0);
    let frames = reference.samples.len() as i64;
    for block in [1, 7, 255, 257, 4096, frames, frames + 1] {
        let rendered = render(
            &hot,
            &notes,
            &RenderConfig {
                block_size_frames: block,
            },
        )
        .unwrap();
        assert_eq!(
            bits(&rendered.samples),
            bits(&reference.samples),
            "block {block}"
        );
        assert_eq!(
            rendered.report.limited_samples, reference.report.limited_samples,
            "block {block}"
        );
    }
}
