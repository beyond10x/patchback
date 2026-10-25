//! Adversary case for story:ml-init-benchmark (wave a12, pass 2): [`predicted_arm`]'s refusal of
//! a point the patch builder refuses, by its kind and its message. The unit's own case
//! (`tests/ml_init_benchmark.rs`, `a_predicted_point_the_builder_refuses_is_refused`) asserts
//! only `is_err()` for one NaN, so a refusal of the wrong kind or wording would pass it.

use patchback_audio::AudioClip;
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{DefaultArmError, PredictedArmError, default_point, patch_at, predicted_arm};

fn notes() -> NoteSequence {
    NoteSequence {
        format_version: patchback_core::score::FORMAT_VERSION,
        sample_rate_hz: 48_000,
        patch_id: "7f0c0000-0000-4000-8000-000000000c22".to_owned(),
        note_estimation_id: None,
        notes: vec![NoteEvent {
            start_sample: 0,
            end_sample: 9_600,
            midi_note: 57,
            velocity: 0.8,
        }],
    }
}

fn reference(topology: &Topology, notes: &NoteSequence) -> AudioClip {
    let mut point = default_point(topology);
    point[1] = 0.3;
    let patch = patch_at(topology, &point).unwrap();
    let rendered = render(
        &compile(&validate(&patch).unwrap()),
        notes,
        &RenderConfig::default(),
    )
    .unwrap();
    AudioClip::from_mono(rendered.samples).unwrap()
}

/// `predicted_arm`'s documentation: "a NaN or infinite one is refused". Every index, each of
/// NaN, +inf and -inf: a `DefaultArmError::Patch` refusal (no error of `spec/`), worded for the
/// predicted patch, never a loss and never the default patch's wording.
#[test]
fn a_non_finite_predicted_value_is_refused_as_the_predicted_patch_at_every_index() {
    let topology = Topology::subtractive_mono();
    let notes = notes();
    let reference = reference(&topology, &notes);
    let len = default_point(&topology).len();
    assert_eq!(len, 8);
    for index in 0..len {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut point = default_point(&topology);
            point[index] = bad;
            let refused = predicted_arm(
                &reference,
                &notes,
                &topology,
                &point,
                &RenderConfig::default(),
            );
            let error = match refused {
                Err(error) => error,
                Ok(arm) => panic!("index {index} {bad}: a loss {:?}", arm.comparison.total),
            };
            assert!(
                matches!(error, PredictedArmError(DefaultArmError::Patch(_))),
                "index {index} {bad}: {error:?}"
            );
            assert_eq!(error.domain_error(), None, "index {index} {bad}");
            let message = error.to_string();
            assert!(
                message.starts_with("the predicted patch is refused: "),
                "index {index} {bad}: {message}"
            );
            assert!(!message.contains("default patch"), "{message}");
        }
    }
}
