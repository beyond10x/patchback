//! The acceptance scenarios of story:engine-baseline (`spec/scenarios/engine-baseline-*.yaml`) as
//! tests of [`Benchmark`], the value of `spec/domains/fit.yaml` `Benchmark` with its commands
//! `StartBenchmark` ([`Benchmark::start`]) and `RecordBaselineResult`
//! ([`Benchmark::record_baseline_result`]), and of its view `Benchmarks` ([`Benchmark::summary`]).
//! One test per scenario file, named after it; each maps every step of its timeline.
//!
//! The scenarios name topologies by Uuid and declare them with `patchback.patch.DeclareTopology`;
//! the code knows topologies by name (`patchback_core::patch::known_topology`, Da8-6). So a
//! declared topology is `subtractive-mono/1`, the one the renderer knows, and an undeclared one
//! is a name the renderer does not know. `DeclareTopology` steps have no counterpart: the
//! topology is compiled in. `capture` of a `benchmark_id` has none either: a benchmark is a
//! value (Da8-10), so the later steps act on the value the start returned.
//!
//! Beside the scenarios: [`median`] (the lower median, Da8-4), [`median_count`],
//! [`reduction_percent`] and the default arm's loss ([`default_arm`], Da8-5).

use patchback_audio::AudioClip;
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    BaselineResult, Benchmark, BenchmarkKind, BenchmarkRow, BenchmarkState, FitBounds, FitConfig,
    FitError, TARGET_REDUCTION_PERCENT, TargetVerdict, default_arm, default_point, fit, median,
    median_count, patch_at, reduction_percent,
};

/// The topology the scenarios declare: the one the renderer knows.
const DECLARED: &str = "subtractive-mono/1";

/// A topology no scenario step declares: a name the renderer does not know.
const UNDECLARED: &str = "no-such-topology/1";

fn result(
    default_median_loss: f64,
    default_search_median_loss: f64,
    median_loss_reduction_percent: f64,
    default_search_median_evaluations: i64,
    wall_time_ms: u64,
    listening_examples: &[&str],
) -> BaselineResult {
    BaselineResult {
        default_median_loss,
        default_search_median_loss,
        median_loss_reduction_percent,
        default_search_median_evaluations,
        wall_time_ms,
        listening_examples: listening_examples.iter().map(|s| (*s).to_owned()).collect(),
    }
}

/// The started benchmark the scenarios' `started` outcome creates: Running, its inputs set, every
/// result field absent.
fn assert_started(
    benchmark: &Benchmark,
    kind: BenchmarkKind,
    topology: &str,
    seed: u64,
    example_count: i64,
) {
    assert_eq!(benchmark.state(), BenchmarkState::Running);
    assert_eq!(benchmark.kind(), kind);
    assert_eq!(benchmark.topology(), topology);
    assert_eq!(benchmark.seed(), seed);
    assert_eq!(benchmark.example_count(), example_count);
    assert_eq!(benchmark.default_median_loss(), None);
    assert_eq!(benchmark.default_search_median_loss(), None);
    assert_eq!(benchmark.raw_regressor_median_loss(), None);
    assert_eq!(benchmark.regressor_search_median_loss(), None);
    assert_eq!(benchmark.median_loss_reduction_percent(), None);
    assert_eq!(benchmark.target_verdict(), None);
    assert_eq!(benchmark.default_search_median_evaluations(), None);
    assert_eq!(benchmark.regressor_search_median_evaluations(), None);
    assert_eq!(benchmark.wall_time_ms(), None);
    assert_eq!(benchmark.listening_examples(), None);
}

/// The completed benchmark after `RecordBaselineResult`: every field of `recorded` set, the
/// verdict, Completed.
fn assert_completed(benchmark: &Benchmark, recorded: &BaselineResult, verdict: TargetVerdict) {
    assert_eq!(benchmark.state(), BenchmarkState::Completed);
    assert_eq!(
        benchmark.default_median_loss(),
        Some(recorded.default_median_loss)
    );
    assert_eq!(
        benchmark.default_search_median_loss(),
        Some(recorded.default_search_median_loss)
    );
    assert_eq!(
        benchmark.median_loss_reduction_percent(),
        Some(recorded.median_loss_reduction_percent)
    );
    assert_eq!(
        benchmark.default_search_median_evaluations(),
        Some(recorded.default_search_median_evaluations)
    );
    assert_eq!(benchmark.wall_time_ms(), Some(recorded.wall_time_ms));
    assert_eq!(
        benchmark.listening_examples(),
        Some(recorded.listening_examples.as_slice())
    );
    assert_eq!(benchmark.target_verdict(), Some(verdict));
    assert_eq!(benchmark.raw_regressor_median_loss(), None);
    assert_eq!(benchmark.regressor_search_median_loss(), None);
    assert_eq!(benchmark.regressor_search_median_evaluations(), None);
}

/// engine-baseline-target-met: DeclareTopology (compiled in); StartBenchmark EngineBaseline,
/// seed 1234, 200 examples is `started` (`BenchmarkStarted` carries kind EngineBaseline and seed
/// 1234); RecordBaselineResult with a 27.5 % reduction is `target-met`, `BenchmarkCompleted`
/// with verdict Met.
#[test]
fn engine_baseline_target_met() {
    let mut bench = Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1234, 200, None)
        .expect("started");
    assert_started(&bench, BenchmarkKind::EngineBaseline, DECLARED, 1234, 200);
    let recorded = result(
        0.8,
        0.58,
        27.5,
        1400,
        1_800_000,
        &["listen/example-01.wav", "listen/example-02.wav"],
    );
    assert_eq!(
        bench.record_baseline_result(recorded.clone()),
        Ok(TargetVerdict::Met)
    );
    assert_completed(&bench, &recorded, TargetVerdict::Met);
}

/// engine-baseline-target-not-met: DeclareTopology (compiled in); StartBenchmark is `started`;
/// RecordBaselineResult with a 12.5 % reduction is `target-not-met` (verdict NotMet, the
/// benchmark Completed: a result, not a refusal); a second RecordBaselineResult (25 %) is
/// `wrong-state`, `BenchmarkStateConflict`, and changes nothing.
#[test]
fn engine_baseline_target_not_met() {
    let mut bench = Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1234, 200, None)
        .expect("started");
    assert_started(&bench, BenchmarkKind::EngineBaseline, DECLARED, 1234, 200);
    let recorded = result(0.8, 0.7, 12.5, 2000, 2_100_000, &["listen/example-01.wav"]);
    assert_eq!(
        bench.record_baseline_result(recorded.clone()),
        Ok(TargetVerdict::NotMet)
    );
    assert_completed(&bench, &recorded, TargetVerdict::NotMet);

    let second = result(0.8, 0.6, 25.0, 2000, 2_100_000, &["listen/example-01.wav"]);
    let refused = bench.record_baseline_result(second);
    assert_eq!(
        refused,
        Err(FitError::BenchmarkStateConflict {
            state: BenchmarkState::Completed
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.fit.BenchmarkStateConflict"
    );
    assert_completed(&bench, &recorded, TargetVerdict::NotMet);
}

/// engine-baseline-target-met-at-boundary: DeclareTopology (compiled in); StartBenchmark is
/// `started`; RecordBaselineResult with a reduction of exactly 20 is `target-met`: the bound is
/// inclusive (DESIGN.md § 12.2, "at least 20%").
#[test]
fn engine_baseline_target_met_at_boundary() {
    assert_eq!(TARGET_REDUCTION_PERCENT, 20.0);
    let mut bench = Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1234, 200, None)
        .expect("started");
    let recorded = result(0.8, 0.64, 20.0, 1400, 1_800_000, &["listen/example-01.wav"]);
    assert_eq!(
        bench.record_baseline_result(recorded.clone()),
        Ok(TargetVerdict::Met)
    );
    assert_completed(&bench, &recorded, TargetVerdict::Met);

    let mut below = Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1234, 200, None)
        .expect("started");
    let just_below = 20.0_f64.next_down();
    assert_eq!(
        below.record_baseline_result(result(0.8, 0.64, just_below, 1400, 1, &[])),
        Ok(TargetVerdict::NotMet)
    );
}

/// engine-baseline-unknown-topology: StartBenchmark EngineBaseline, seed 1234, 200 examples, in
/// an undeclared topology is `unknown-topology`, `TopologyNotFound` naming it; no benchmark
/// starts (`no_events: BenchmarkStarted`: no value is returned).
#[test]
fn engine_baseline_unknown_topology() {
    let refused = Benchmark::start(BenchmarkKind::EngineBaseline, UNDECLARED, 1234, 200, None);
    assert_eq!(
        refused,
        Err(FitError::TopologyNotFound {
            topology: UNDECLARED.to_owned()
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.fit.TopologyNotFound"
    );
}

/// engine-baseline-unknown-topology-ml-init: StartBenchmark MlInit with a weight artifact in an
/// undeclared topology is `unknown-topology` (the existence check holds for both kinds); after
/// DeclareTopology the same start is `started`, `BenchmarkStarted` carrying kind MlInit, the
/// topology and seed 1234.
#[test]
fn engine_baseline_unknown_topology_ml_init() {
    let artifact = "3e9a4c20-0000-4000-8000-00000000c002";
    assert_eq!(
        Benchmark::start(BenchmarkKind::MlInit, UNDECLARED, 1234, 200, Some(artifact)),
        Err(FitError::TopologyNotFound {
            topology: UNDECLARED.to_owned()
        })
    );
    let bench = Benchmark::start(BenchmarkKind::MlInit, DECLARED, 1234, 200, Some(artifact))
        .expect("started");
    assert_started(&bench, BenchmarkKind::MlInit, DECLARED, 1234, 200);
    assert_eq!(bench.weight_artifact_id(), Some(artifact));
}

/// engine-baseline-refusal-precedence: every step names an undeclared topology, so
/// `unknown-topology` holds on each, and the first declared refusal answers: MlInit with 0
/// examples is `invalid-holdout-size` (`InvalidHoldoutSize`, example_count 0); EngineBaseline
/// with -1 is `invalid-holdout-size` (-1); MlInit with 1 example and no weight artifact is
/// `weight-artifact-required` (`WeightArtifactRequired` naming the topology). No benchmark
/// starts on any.
#[test]
fn engine_baseline_refusal_precedence() {
    let first = Benchmark::start(BenchmarkKind::MlInit, UNDECLARED, 1234, 0, None);
    assert_eq!(
        first,
        Err(FitError::InvalidHoldoutSize { example_count: 0 })
    );
    assert_eq!(
        first.unwrap_err().domain_error(),
        "patchback.fit.InvalidHoldoutSize"
    );

    assert_eq!(
        Benchmark::start(BenchmarkKind::EngineBaseline, UNDECLARED, 1234, -1, None),
        Err(FitError::InvalidHoldoutSize { example_count: -1 })
    );

    let third = Benchmark::start(BenchmarkKind::MlInit, UNDECLARED, 1234, 1, None);
    assert_eq!(
        third,
        Err(FitError::WeightArtifactRequired {
            topology: UNDECLARED.to_owned()
        })
    );
    assert_eq!(
        third.unwrap_err().domain_error(),
        "patchback.fit.WeightArtifactRequired"
    );
}

/// engine-baseline-topology-recorded: DeclareTopology (compiled in); StartBenchmark
/// EngineBaseline in another topology than the declared one is `unknown-topology`; MlInit in the
/// declared one without a weight artifact is `weight-artifact-required`, the refusal carrying
/// that topology; EngineBaseline in the declared one, seed 77, 1 example, is `started` and
/// carries the topology and seed.
#[test]
fn engine_baseline_topology_recorded() {
    assert_eq!(
        Benchmark::start(BenchmarkKind::EngineBaseline, UNDECLARED, 1234, 1, None),
        Err(FitError::TopologyNotFound {
            topology: UNDECLARED.to_owned()
        })
    );
    assert_eq!(
        Benchmark::start(BenchmarkKind::MlInit, DECLARED, 1234, 1, None),
        Err(FitError::WeightArtifactRequired {
            topology: DECLARED.to_owned()
        })
    );
    let bench =
        Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 77, 1, None).expect("started");
    assert_started(&bench, BenchmarkKind::EngineBaseline, DECLARED, 77, 1);
}

/// engine-baseline-view-topology: DeclareTopology (compiled in); two EngineBaseline starts in
/// it (seed 1234, 1 example; seed 99, 200 examples) are each `started`; the `Benchmarks` view
/// holds a row for each with state Running, kind EngineBaseline, the topology, its own seed and
/// size.
#[test]
fn engine_baseline_view_topology() {
    let first =
        Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1234, 1, None).expect("started");
    let second =
        Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 99, 200, None).expect("started");
    let row = |seed, example_count| BenchmarkRow {
        state: BenchmarkState::Running,
        kind: BenchmarkKind::EngineBaseline,
        topology: DECLARED.to_owned(),
        seed,
        example_count,
        target_verdict: None,
        default_search_median_evaluations: None,
        regressor_search_median_evaluations: None,
        wall_time_ms: None,
        listening_examples: None,
    };
    assert_eq!(first.summary(), row(1234, 1));
    assert_eq!(second.summary(), row(99, 200));
}

/// The view of a completed benchmark carries its verdict, evaluations, wall time and listening
/// examples.
#[test]
fn the_view_of_a_completed_benchmark_carries_its_result() {
    let mut bench =
        Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 5, 3, None).expect("started");
    bench
        .record_baseline_result(result(0.5, 0.25, 50.0, 42, 1234, &["examples/0/fit.wav"]))
        .unwrap();
    let row = bench.summary();
    assert_eq!(row.state, BenchmarkState::Completed);
    assert_eq!(row.target_verdict, Some(TargetVerdict::Met));
    assert_eq!(row.default_search_median_evaluations, Some(42));
    assert_eq!(row.wall_time_ms, Some(1234));
    assert_eq!(
        row.listening_examples,
        Some(vec!["examples/0/fit.wav".to_owned()])
    );
}

/// `kind-mismatch` comes before the state: an ML-init benchmark is refused
/// `BenchmarkKindMismatch` naming its kind, and stays Running.
#[test]
fn a_baseline_result_for_an_ml_init_benchmark_is_a_kind_mismatch() {
    let mut bench =
        Benchmark::start(BenchmarkKind::MlInit, DECLARED, 1, 1, Some("weights")).expect("started");
    let refused = bench.record_baseline_result(result(0.8, 0.6, 25.0, 1, 1, &[]));
    assert_eq!(
        refused,
        Err(FitError::BenchmarkKindMismatch {
            kind: BenchmarkKind::MlInit
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.fit.BenchmarkKindMismatch"
    );
    assert_eq!(bench.state(), BenchmarkState::Running);
    assert_eq!(bench.target_verdict(), None);
}

/// The names the model gives the enums.
#[test]
fn the_enums_carry_the_model_names() {
    assert_eq!(BenchmarkKind::EngineBaseline.name(), "EngineBaseline");
    assert_eq!(BenchmarkKind::MlInit.name(), "MlInit");
    assert_eq!(TargetVerdict::Met.name(), "Met");
    assert_eq!(TargetVerdict::NotMet.name(), "NotMet");
    assert_eq!(BenchmarkState::Running.name(), "Running");
    assert_eq!(BenchmarkState::Completed.name(), "Completed");
}

/// Da8-4: the lower median, the element at index `(n - 1) / 2` of the sorted values.
#[test]
fn median_is_the_lower_median() {
    assert_eq!(median(&[]), None);
    assert_eq!(median(&[0.7]), Some(0.7));
    assert_eq!(median(&[0.3, 0.1, 0.2]), Some(0.2));
    assert_eq!(median(&[0.4, 0.1, 0.3, 0.2]), Some(0.2), "even: the lower");
    assert_eq!(median(&[0.9, 0.5]), Some(0.5));
    assert_eq!(median(&[0.5, 0.5, 0.1, 0.9, 0.2]), Some(0.5));
    assert_eq!(median_count(&[]), None);
    assert_eq!(median_count(&[17]), Some(17));
    assert_eq!(median_count(&[30, 10, 20]), Some(20));
    assert_eq!(median_count(&[40, 10, 30, 20]), Some(20));
}

/// Da8-4: reduction = (default − search) / default × 100.
#[test]
fn reduction_percent_is_relative_to_the_default() {
    assert_eq!(reduction_percent(1.0, 0.75), 25.0);
    assert_eq!(reduction_percent(0.5, 0.5), 0.0);
    assert_eq!(reduction_percent(0.5, 0.75), -50.0);
    assert!((reduction_percent(0.8, 0.58) - 27.5).abs() < 1e-12);
    assert!((reduction_percent(0.8, 0.7) - 12.5).abs() < 1e-12);
}

/// A 0.4 s note of MIDI 52 at 48 kHz.
fn notes() -> NoteSequence {
    NoteSequence {
        format_version: patchback_core::score::FORMAT_VERSION,
        sample_rate_hz: 48_000,
        patch_id: "7f0c0000-0000-4000-8000-000000000a81".to_owned(),
        note_estimation_id: None,
        notes: vec![NoteEvent {
            start_sample: 0,
            end_sample: 19_200,
            midi_note: 52,
            velocity: 0.9,
        }],
    }
}

/// The render of the patch at `point`, whole.
fn reference(topology: &Topology, point: &[f64], notes: &NoteSequence) -> AudioClip {
    let patch = patch_at(topology, point).unwrap();
    let plan = compile(&validate(&patch).unwrap());
    let rendered = render(&plan, notes, &RenderConfig::default()).unwrap();
    AudioClip::from_mono(rendered.samples).unwrap()
}

/// Da8-5: the default arm's loss is `compare` of the default patch's render cut or padded to the
/// reference, computed apart from the fit; it equals evaluation 1's `total` of the fit's log,
/// restart 0's start at the default point (`search.rs`), when that evaluation is valid.
#[test]
fn the_default_arm_equals_the_first_evaluation_of_the_fit() {
    let topology = Topology::subtractive_mono();
    let notes = notes();
    let mut point = default_point(&topology);
    point[3] = 0.2;
    point[0] = 0.8;
    let reference = reference(&topology, &point, &notes);
    let config = FitConfig {
        bounds: FitBounds {
            max_renders: 6,
            max_wall_time_ms: 600_000,
            max_restarts: 1,
            improvement_threshold: 1e-4,
        },
        seed: 3,
        render: RenderConfig::default(),
    };
    let outcome = fit(&reference, &notes, &topology, &config).expect("the fit completes");
    let first = &outcome.log[0];
    assert_eq!(first.sequence, 1);
    assert_eq!(first.normalized, default_point(&topology));
    let total = first.total.expect("the default render is valid");

    let arm = default_arm(&reference, &notes, &topology, &config.render).expect("compared");
    assert_eq!(arm.comparison.total.to_bits(), total.to_bits());
    assert_eq!(arm.comparison.components, first.components.unwrap());
    assert_eq!(arm.clip.frame_count(), reference.frame_count());
    assert!(
        arm.comparison.total > 0.0,
        "the reference is not the default"
    );
}
