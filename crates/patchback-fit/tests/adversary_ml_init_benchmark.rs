//! Adversary cases for the library half of story:ml-init-benchmark (wave a12, pass 1):
//! `Benchmark::record_ml_init_result`, `record_baseline_result` on an ML-init benchmark, and
//! `predicted_arm` at points outside `[0, 1]` and at a short point.

use patchback_audio::AudioClip;
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    BaselineResult, Benchmark, BenchmarkKind, BenchmarkState, FitError, MlInitResult, default_arm,
    default_point, patch_at, predicted_arm,
};

fn result() -> MlInitResult {
    MlInitResult {
        default_median_loss: 0.8,
        default_search_median_loss: 0.58,
        raw_regressor_median_loss: 0.62,
        regressor_search_median_loss: 0.5,
        default_search_median_evaluations: 1400,
        regressor_search_median_evaluations: 600,
        wall_time_ms: 2_400_000,
        listening_examples: vec!["listen/ml-01.wav".to_owned()],
    }
}

/// `RecordBaselineResult` lists `kind-mismatch` before `wrong-state` (`fit.yaml`): a baseline
/// result for a Completed ML-init benchmark is `BenchmarkKindMismatch` naming MlInit, not a state
/// conflict, and changes nothing.
#[test]
fn a_baseline_result_on_a_completed_ml_init_benchmark_is_a_kind_mismatch() {
    let mut bench = Benchmark::start(
        BenchmarkKind::MlInit,
        "subtractive-mono/1",
        1234,
        200,
        Some("sha256:00"),
    )
    .unwrap();
    bench.record_ml_init_result(result()).unwrap();
    assert_eq!(bench.state(), BenchmarkState::Completed);
    let before = bench.clone();
    let refused = bench.record_baseline_result(BaselineResult {
        default_median_loss: 0.8,
        default_search_median_loss: 0.58,
        median_loss_reduction_percent: 27.5,
        default_search_median_evaluations: 1400,
        wall_time_ms: 1,
        listening_examples: Vec::new(),
    });
    assert_eq!(
        refused,
        Err(FitError::BenchmarkKindMismatch {
            kind: BenchmarkKind::MlInit
        })
    );
    assert_eq!(bench, before);
    assert_eq!(bench.raw_regressor_median_loss(), Some(0.62));
    assert_eq!(bench.regressor_search_median_evaluations(), Some(600));
}

fn notes() -> NoteSequence {
    NoteSequence {
        format_version: patchback_core::score::FORMAT_VERSION,
        sample_rate_hz: 48_000,
        patch_id: "7f0c0000-0000-4000-8000-000000000c12".to_owned(),
        note_estimation_id: None,
        notes: vec![NoteEvent {
            start_sample: 0,
            end_sample: 19_200,
            midi_note: 52,
            velocity: 0.9,
        }],
    }
}

fn reference(topology: &Topology, notes: &NoteSequence) -> AudioClip {
    let mut point = default_point(topology);
    point[3] = 0.2;
    point[0] = 0.8;
    let patch = patch_at(topology, &point).unwrap();
    let plan = compile(&validate(&patch).unwrap());
    AudioClip::from_mono(
        render(&plan, notes, &RenderConfig::default())
            .unwrap()
            .samples,
    )
    .unwrap()
}

/// Da12-4: a prediction that clamped renders the clamped point. `predicted_arm` at a point with
/// values above 1 and below 0 is `predicted_arm` at those values clamped to `[0, 1]`, bit for bit,
/// and its clip is the reference's length.
#[test]
fn the_predicted_arm_at_an_unclamped_point_is_the_arm_at_the_clamped_point() {
    let topology = Topology::subtractive_mono();
    let notes = notes();
    let reference = reference(&topology, &notes);
    let config = RenderConfig::default();
    let raw: [f64; 8] = [1.5, -0.5, 2.0, 0.5, 0.25, 0.75, 0.4, 0.6];
    let clamped: Vec<f64> = raw.iter().map(|v| v.clamp(0.0, 1.0)).collect();
    let at_raw = predicted_arm(&reference, &notes, &topology, &raw, &config).expect("compared");
    let at_clamped =
        predicted_arm(&reference, &notes, &topology, &clamped, &config).expect("compared");
    assert_eq!(
        at_raw.comparison.total.to_bits(),
        at_clamped.comparison.total.to_bits()
    );
    assert_eq!(at_raw, at_clamped);
    assert_eq!(at_raw.clip.frame_count(), reference.frame_count());
}

/// A point with no values is every contract's default (`patch_at`): the predicted arm there is the
/// default arm, and does not panic on the short point.
#[test]
fn the_predicted_arm_at_an_empty_point_is_the_default_arm() {
    let topology = Topology::subtractive_mono();
    let notes = notes();
    let reference = reference(&topology, &notes);
    let config = RenderConfig::default();
    let empty = predicted_arm(&reference, &notes, &topology, &[], &config).expect("compared");
    let default = default_arm(&reference, &notes, &topology, &config).expect("compared");
    assert_eq!(empty, default);
}
