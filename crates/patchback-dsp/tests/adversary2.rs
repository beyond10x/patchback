//! Adversarial cases, second pass (story:subtractive-voice-render, wave a5, after correction
//! round 1): the place of `RenderError::InvalidNoteTiming` in the refusal order, and the
//! documented semantics of the hand-built sequences the loader refuses and `render` renders
//! (a later note takes the voice, a note-off of a note no longer sounding is ignored, a MIDI
//! number above 127 is 127, a velocity outside `[0, 1]` is clamped, NaN is 0). The class test of
//! the correction (`tests/render.rs`, `the_other_sequences_the_loader_refuses_render_and_close_their_gate`)
//! asserts only finiteness and silence after the release; these assert the semantics bit for bit
//! against the legal sequence each one is documented to equal.

use patchback_core::patch::{ExecutionPlan, compile, parse_patch, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, RenderError, render};

fn plan() -> ExecutionPlan {
    let text = "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive 6\n\
         node 4 LowpassFilter\nparam 4 filter_cutoff 1200\nparam 4 resonance_q 0.7\n\
         node 5 AdsrEnvelope\nparam 5 attack 0.01\nparam 5 decay 0.2\n\
         param 5 sustain 0.6\nparam 5 release 0.3\nnode 6 Vca\nnode 7 Output\n\
         param 7 output_gain -6\n\
         edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
         edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
         edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n";
    compile(&validate(&parse_patch(text).unwrap()).unwrap())
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

fn samples(notes: &[(u64, u64, u8, f32)]) -> Vec<u32> {
    render(&plan(), &sequence(notes), &RenderConfig::default())
        .unwrap_or_else(|error| panic!("{notes:?}: {error}"))
        .samples
        .iter()
        .map(|s| s.to_bits())
        .collect()
}

/// Passing regression guard on the refusal order `render` documents: rate, block size, note
/// timing, length. Each pair puts a zero-length note beside the fault that must answer first,
/// or a too-long note beside it that must answer second; the payload names the first faulty
/// note in the sequence's own order, not in start order.
#[test]
fn invalid_note_timing_stands_after_rate_and_block_size_and_before_length() {
    let mut at_44 = sequence(&[(1_000, 1_000, 45, 0.8)]);
    at_44.sample_rate_hz = 44_100;
    assert_eq!(
        render(&plan(), &at_44, &RenderConfig::default()).err(),
        Some(RenderError::UnsupportedSampleRate {
            sample_rate_hz: 44_100
        })
    );
    assert_eq!(
        render(
            &plan(),
            &sequence(&[(1_000, 1_000, 45, 0.8)]),
            &RenderConfig {
                block_size_frames: 0
            }
        )
        .err(),
        Some(RenderError::InvalidBlockSize {
            block_size_frames: 0
        })
    );
    assert_eq!(
        render(
            &plan(),
            &sequence(&[(0, u64::MAX, 45, 0.8), (1_000, 1_000, 45, 0.8)]),
            &RenderConfig::default()
        )
        .err(),
        Some(RenderError::InvalidNoteTiming {
            start_sample: 1_000,
            end_sample: 1_000
        })
    );
    assert_eq!(
        render(
            &plan(),
            &sequence(&[
                (0, 4_800, 45, 0.8),
                (6_000, 6_000, 45, 0.8),
                (3_000, 2_000, 45, 0.8)
            ]),
            &RenderConfig::default()
        )
        .err(),
        Some(RenderError::InvalidNoteTiming {
            start_sample: 6_000,
            end_sample: 6_000
        })
    );
}

/// Passing regression guard. "A note-off of a note that is no longer the sounding one (a later
/// note started) is ignored": a note overlapped by a later one that outlasts it sounds exactly as
/// the legal sequence where the first note ends where the second starts (note-off then note-on
/// at one sample is a retrigger from the current level). The class test passes when the stale
/// note-off closes the gate at 9600 instead, because the release still ends before 28800.
#[test]
fn an_overlapped_note_ends_as_if_it_ended_where_the_later_note_starts() {
    assert_eq!(
        samples(&[(0, 9_600, 45, 0.8), (4_800, 14_400, 57, 0.8)]),
        samples(&[(0, 4_800, 45, 0.8), (4_800, 14_400, 57, 0.8)])
    );
}

/// Passing regression guard. Two notes starting on one sample: the later one in start order takes
/// the voice, and the earlier one's note-off is ignored, so the render is the later note alone.
#[test]
fn of_two_notes_starting_on_one_sample_the_later_one_alone_sounds() {
    assert_eq!(
        samples(&[(0, 4_800, 45, 0.8), (0, 9_600, 57, 0.8)]),
        samples(&[(0, 9_600, 57, 0.8)])
    );
}

/// Passing regression guard. "A MIDI number above 127 is 127": MIDI 200 renders the samples of
/// MIDI 127, bit for bit. Without the clamp the oscillator runs at its own 20 kHz limit instead
/// of 12543.9 Hz, which the class test (finite, silent after the release) does not see.
#[test]
fn midi_200_renders_as_midi_127() {
    assert_eq!(
        samples(&[(0, 9_600, 200, 0.8)]),
        samples(&[(0, 9_600, 127, 0.8)])
    );
}

/// Passing regression guard. "A velocity outside `[0, 1]` is clamped (NaN is 0)": -1 and NaN
/// render the all-zero samples of velocity 0, and `f32::MAX` and infinity render the samples of
/// velocity 1, bit for bit. A clamp that keeps only its upper bound renders -1 as a polarity-
/// inverted note, finite and silent after the release, so the class test passes it.
#[test]
fn out_of_range_velocities_render_as_their_clamped_values() {
    let zero = samples(&[(0, 9_600, 45, 0.0)]);
    assert!(zero.iter().all(|s| f32::from_bits(*s) == 0.0));
    let one = samples(&[(0, 9_600, 45, 1.0)]);
    assert!(one.iter().any(|s| f32::from_bits(*s) != 0.0));
    for (velocity, expected) in [
        (-1.0, &zero),
        (f32::NEG_INFINITY, &zero),
        (f32::NAN, &zero),
        (f32::MAX, &one),
        (f32::INFINITY, &one),
    ] {
        assert!(
            samples(&[(0, 9_600, 45, velocity)]) == *expected,
            "velocity {velocity}"
        );
    }
}
