//! The acceptance scenarios of story:ml-init-benchmark (`spec/scenarios/ml-init-benchmark-*.yaml`)
//! as tests of [`Benchmark`], the value of `spec/domains/fit.yaml` `Benchmark` with its commands
//! `StartBenchmark` ([`Benchmark::start`]) and `RecordMlInitResult`
//! ([`Benchmark::record_ml_init_result`]). One test per scenario file, named after it; each maps
//! every step of its timeline.
//!
//! As in `tests/engine_baseline.rs`: a declared topology is `subtractive-mono/1`, the one the
//! renderer knows; `DeclareTopology` steps have no counterpart (the topology is compiled in);
//! `capture` of a `benchmark_id` has none either, since a benchmark is a value (Da8-10), so the
//! later steps act on the value the start returned, and `no-such-benchmark`
//! (`BenchmarkNotFound`) cannot be reached. A `weight_artifact_id` is an opaque identifier here.
//!
//! The crate models no events: `MlInitCompared`'s payload is the four medians the completed
//! benchmark holds, read back through its getters.
//!
//! Beside the scenarios: the model's outcome order of `RecordMlInitResult` (`kind-mismatch`
//! before `wrong-state`), and the raw regressor arm ([`predicted_arm`], Da12-4): the default arm
//! generalised to any point, equal to [`default_arm`] at the default point and to evaluation 1
//! of a regressor-seeded fit at its prediction.

use std::fs;
use std::path::{Path, PathBuf};

use patchback_analysis::{
    AnalysisConfig, FEATURES_1_VALUE_COUNT, NormalizationTable, analyze_clip, read_stats,
};
use patchback_audio::AudioClip;
use patchback_core::digest;
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, format_decimal, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    BaselineResult, Benchmark, BenchmarkKind, BenchmarkState, FitBounds, FitConfig, FitError,
    FitRequest, InitSource, MlInitResult, SeedInputs, compare, default_arm, default_point,
    fit_request, patch_at, predicted_arm,
};
use patchback_model::{ModelSpecPin, Predictor, W1_ELEMENTS, W1_SHAPE, W2_SHAPE};

/// The topology the scenarios declare: the one the renderer knows.
const DECLARED: &str = "subtractive-mono/1";

/// The scenarios' weight artifact identifier (`weight_artifact_id`), opaque here.
const ARTIFACT: &str = "3e9a4c20-0000-4000-8000-000000000102";

#[allow(clippy::too_many_arguments)]
fn ml_init_result(
    default_median_loss: f64,
    default_search_median_loss: f64,
    raw_regressor_median_loss: f64,
    regressor_search_median_loss: f64,
    default_search_median_evaluations: i64,
    regressor_search_median_evaluations: i64,
    wall_time_ms: u64,
    listening_examples: &[&str],
) -> MlInitResult {
    MlInitResult {
        default_median_loss,
        default_search_median_loss,
        raw_regressor_median_loss,
        regressor_search_median_loss,
        default_search_median_evaluations,
        regressor_search_median_evaluations,
        wall_time_ms,
        listening_examples: listening_examples.iter().map(|s| (*s).to_owned()).collect(),
    }
}

/// The scenarios' `RecordMlInitResult` input: 0.8, 0.58, 0.62, 0.5, 1400, 600, 2 400 000 ms.
fn scenario_result(listening_examples: &[&str]) -> MlInitResult {
    ml_init_result(
        0.8,
        0.58,
        0.62,
        0.5,
        1400,
        600,
        2_400_000,
        listening_examples,
    )
}

/// The started benchmark: Running, its inputs set, every result field absent.
fn assert_started(benchmark: &Benchmark, kind: BenchmarkKind, artifact: Option<&str>) {
    assert_eq!(benchmark.state(), BenchmarkState::Running);
    assert_eq!(benchmark.kind(), kind);
    assert_eq!(benchmark.topology(), DECLARED);
    assert_eq!(benchmark.seed(), 1234);
    assert_eq!(benchmark.example_count(), 200);
    assert_eq!(benchmark.weight_artifact_id(), artifact);
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

/// The completed benchmark after `RecordMlInitResult` (`recorded`): the eight inputs set as the
/// model's `sets:` lists them, Completed, and no reduction and no verdict (`fit.yaml`: "no verdict
/// is recorded for an ML-init benchmark", Da12-9).
fn assert_recorded(benchmark: &Benchmark, recorded: &MlInitResult) {
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
        benchmark.raw_regressor_median_loss(),
        Some(recorded.raw_regressor_median_loss)
    );
    assert_eq!(
        benchmark.regressor_search_median_loss(),
        Some(recorded.regressor_search_median_loss)
    );
    assert_eq!(
        benchmark.default_search_median_evaluations(),
        Some(recorded.default_search_median_evaluations)
    );
    assert_eq!(
        benchmark.regressor_search_median_evaluations(),
        Some(recorded.regressor_search_median_evaluations)
    );
    assert_eq!(benchmark.wall_time_ms(), Some(recorded.wall_time_ms));
    assert_eq!(
        benchmark.listening_examples(),
        Some(recorded.listening_examples.as_slice())
    );
    assert_eq!(benchmark.median_loss_reduction_percent(), None);
    assert_eq!(benchmark.target_verdict(), None);
    let row = benchmark.summary();
    assert_eq!(row.state, BenchmarkState::Completed);
    assert_eq!(row.kind, BenchmarkKind::MlInit);
    assert_eq!(row.target_verdict, None);
    assert_eq!(
        row.regressor_search_median_evaluations,
        Some(recorded.regressor_search_median_evaluations)
    );
}

/// ml-init-benchmark-four-arms-reported: DeclareTopology (compiled in); StartBenchmark MlInit,
/// seed 1234, 200 examples, a weight artifact is `started`; RecordMlInitResult with the four
/// medians 0.8, 0.58, 0.62 and 0.5, evaluations 1400 and 600, 2 400 000 ms and one listening
/// example is `recorded`, Completed, and `MlInitCompared` carries the four medians: the values the
/// benchmark now holds.
#[test]
fn ml_init_benchmark_four_arms_reported() {
    let mut bench = Benchmark::start(BenchmarkKind::MlInit, DECLARED, 1234, 200, Some(ARTIFACT))
        .expect("started");
    assert_started(&bench, BenchmarkKind::MlInit, Some(ARTIFACT));
    let recorded = scenario_result(&["listen/ml-01.wav"]);
    assert_eq!(bench.record_ml_init_result(recorded.clone()), Ok(()));
    assert_recorded(&bench, &recorded);
    // MlInitCompared's payload.
    assert_eq!(
        (
            bench.default_median_loss(),
            bench.default_search_median_loss(),
            bench.raw_regressor_median_loss(),
            bench.regressor_search_median_loss(),
        ),
        (Some(0.8), Some(0.58), Some(0.62), Some(0.5))
    );
}

/// ml-init-benchmark-kind-mismatch: DeclareTopology (compiled in); StartBenchmark EngineBaseline
/// is `started` (baseline); StartBenchmark MlInit with a weight artifact is `started` (mlinit);
/// RecordMlInitResult for the baseline is `kind-mismatch`, `BenchmarkKindMismatch` naming
/// EngineBaseline, the baseline left Running; RecordBaselineResult for the ML-init benchmark is
/// `kind-mismatch`, naming MlInit, it left Running; RecordBaselineResult for an unknown
/// benchmark (`no-such-benchmark`, `BenchmarkNotFound`) has no counterpart: a benchmark is a
/// value, no call can name one that does not exist (Da8-10; `tests/domain.rs` accounts for it).
#[test]
fn ml_init_benchmark_kind_mismatch() {
    let mut baseline = Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1234, 200, None)
        .expect("started");
    assert_started(&baseline, BenchmarkKind::EngineBaseline, None);
    let mut mlinit = Benchmark::start(
        BenchmarkKind::MlInit,
        DECLARED,
        1234,
        200,
        Some("3e9a4c20-0000-4000-8000-000000000202"),
    )
    .expect("started");
    assert_started(
        &mlinit,
        BenchmarkKind::MlInit,
        Some("3e9a4c20-0000-4000-8000-000000000202"),
    );

    let refused = baseline.record_ml_init_result(scenario_result(&[]));
    assert_eq!(
        refused,
        Err(FitError::BenchmarkKindMismatch {
            kind: BenchmarkKind::EngineBaseline
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.fit.BenchmarkKindMismatch"
    );
    assert_started(&baseline, BenchmarkKind::EngineBaseline, None);

    let refused = mlinit.record_baseline_result(BaselineResult {
        default_median_loss: 0.8,
        default_search_median_loss: 0.58,
        median_loss_reduction_percent: 27.5,
        default_search_median_evaluations: 1400,
        wall_time_ms: 1_800_000,
        listening_examples: Vec::new(),
    });
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
    assert_started(
        &mlinit,
        BenchmarkKind::MlInit,
        Some("3e9a4c20-0000-4000-8000-000000000202"),
    );
}

/// ml-init-benchmark-weight-artifact-required: StartBenchmark MlInit, 200 examples, without a
/// weight artifact is `weight-artifact-required`, `WeightArtifactRequired` naming the topology;
/// StartBenchmark MlInit with 0 examples and a weight artifact is `invalid-holdout-size`,
/// `InvalidHoldoutSize` naming 0. Neither starts a benchmark (no value is returned).
#[test]
fn ml_init_benchmark_weight_artifact_required() {
    let refused = Benchmark::start(BenchmarkKind::MlInit, DECLARED, 1234, 200, None);
    assert_eq!(
        refused,
        Err(FitError::WeightArtifactRequired {
            topology: DECLARED.to_owned()
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.fit.WeightArtifactRequired"
    );
    let refused = Benchmark::start(
        BenchmarkKind::MlInit,
        DECLARED,
        1234,
        0,
        Some("3e9a4c20-0000-4000-8000-000000000302"),
    );
    assert_eq!(
        refused,
        Err(FitError::InvalidHoldoutSize { example_count: 0 })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.fit.InvalidHoldoutSize"
    );
}

/// `RecordMlInitResult`'s `wrong-state` (`fit.yaml`): a second result for a Completed ML-init
/// benchmark is refused `BenchmarkStateConflict` naming Completed, and changes nothing.
#[test]
fn a_second_ml_init_result_is_a_state_conflict() {
    let mut bench = Benchmark::start(BenchmarkKind::MlInit, DECLARED, 1234, 200, Some(ARTIFACT))
        .expect("started");
    let recorded = scenario_result(&["listen/ml-01.wav"]);
    bench.record_ml_init_result(recorded.clone()).unwrap();
    let refused = bench.record_ml_init_result(ml_init_result(0.9, 0.7, 0.6, 0.4, 1, 1, 1, &[]));
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
    assert_recorded(&bench, &recorded);
}

/// `kind-mismatch` comes before `wrong-state` (`fit.yaml` lists it first): an ML-init result for
/// a Completed engine baseline is a kind mismatch, not a state conflict, and changes nothing.
#[test]
fn kind_mismatch_is_answered_before_wrong_state() {
    let mut baseline = Benchmark::start(BenchmarkKind::EngineBaseline, DECLARED, 1234, 200, None)
        .expect("started");
    let recorded = BaselineResult {
        default_median_loss: 0.8,
        default_search_median_loss: 0.58,
        median_loss_reduction_percent: 27.5,
        default_search_median_evaluations: 1400,
        wall_time_ms: 1_800_000,
        listening_examples: Vec::new(),
    };
    baseline.record_baseline_result(recorded).unwrap();
    let before = baseline.clone();
    assert_eq!(
        baseline.record_ml_init_result(scenario_result(&[])),
        Err(FitError::BenchmarkKindMismatch {
            kind: BenchmarkKind::EngineBaseline
        })
    );
    assert_eq!(baseline, before);
}

/// A 0.4 s note of MIDI 52 at 48 kHz, from sample 0.
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

/// The render of the patch at `point`, whole.
fn rendered(topology: &Topology, point: &[f64], notes: &NoteSequence) -> AudioClip {
    let patch = patch_at(topology, point).unwrap();
    let plan = compile(&validate(&patch).unwrap());
    let rendered = render(&plan, notes, &RenderConfig::default()).unwrap();
    AudioClip::from_mono(rendered.samples).unwrap()
}

/// The reference of the arm tests: the default patch with the drive and the release moved.
fn reference(topology: &Topology, notes: &NoteSequence) -> AudioClip {
    let mut point = default_point(topology);
    point[3] = 0.2;
    point[0] = 0.8;
    rendered(topology, &point, notes)
}

/// Da12-4: the raw regressor arm is the default arm's pattern at any point. At the default point
/// it is the default arm, bit for bit; at another point its loss is `compare` of the reference
/// with that point's render cut or padded with silence to the reference's length (another
/// release gives another length), and its clip is that cut or padded render.
#[test]
fn the_predicted_arm_is_the_default_arm_at_any_point() {
    let topology = Topology::subtractive_mono();
    let notes = notes();
    let reference = reference(&topology, &notes);
    let config = RenderConfig::default();

    let default = default_arm(&reference, &notes, &topology, &config).expect("compared");
    let at_default = predicted_arm(
        &reference,
        &notes,
        &topology,
        &default_point(&topology),
        &config,
    )
    .expect("compared");
    assert_eq!(at_default, default);

    let point = [0.6, 0.3, 0.7, 0.4, 0.55, 0.2, 0.45, 0.65];
    let whole = rendered(&topology, &point, &notes);
    assert_ne!(
        whole.frame_count(),
        reference.frame_count(),
        "another release, another length: the cut or pad is exercised"
    );
    let mut samples = whole.samples().to_vec();
    samples.resize(reference.frame_count(), 0.0);
    let candidate = AudioClip::from_mono(samples).unwrap();
    let expected = compare(&reference, &candidate).expect("compared");
    let arm = predicted_arm(&reference, &notes, &topology, &point, &config).expect("compared");
    assert_eq!(arm.comparison, expected);
    assert_eq!(arm.clip, candidate);
}

/// A point the patch builder refuses (a NaN value) is a refusal of the arm, never a loss.
#[test]
fn a_predicted_point_the_builder_refuses_is_refused() {
    let topology = Topology::subtractive_mono();
    let notes = notes();
    let reference = reference(&topology, &notes);
    let mut point = default_point(&topology);
    point[2] = f64::NAN;
    let refused = predicted_arm(
        &reference,
        &notes,
        &topology,
        &point,
        &RenderConfig::default(),
    );
    assert!(refused.is_err(), "{refused:?}");
}

const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];

/// The bytes of a `weights/1` artifact whose output `k` is `targets[k]` times the first
/// normalized feature, under the repository's ModelSpec pin (the a10 writer, copied from
/// `tests/regressor_seeded_fit_scenarios.rs`).
fn artifact_bytes(targets: &[f32; OUTPUTS]) -> Vec<u8> {
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    let mut w2 = vec![0.0f32; HIDDEN * OUTPUTS];
    for (k, target) in targets.iter().enumerate() {
        w1[k] = 1.0;
        w2[k * OUTPUTS + k] = *target;
    }
    let payload: Vec<u8> = w1.iter().chain(&w2).flat_map(|v| v.to_le_bytes()).collect();
    let pin = ModelSpecPin::repository().unwrap();
    let header = format!(
        "weights/1\nmodel PatchRegressor\nmodelspec {} {}\nfeatures features/1\nw1 32 64\nw2 64 8\nsha256 {}\n\n",
        pin.version(),
        pin.commit(),
        digest::hex(&digest::sha256(&payload))
    );
    [header.as_bytes(), &payload].concat()
}

/// Statistics whose every feature has mean `-10^6` and standard deviation `10^6`: a feature `x`
/// normalizes to `1 + x / 10^6`, about 1 for every feature of a short note, so the prediction is
/// about the targets whatever the clip.
fn wide_stats() -> (NormalizationTable, String) {
    let mut text = "patchback-normalization-stats 1\nfeature_contract features/1\nsplit train\nrows 16\nstd population\n".to_owned();
    for i in 0..FEATURES_1_VALUE_COUNT {
        text.push_str(&format!(
            "feature {i} {} {}\n",
            format_decimal(-1e6),
            format_decimal(1e6)
        ));
    }
    (read_stats(&text).expect("the statistics read"), text)
}

fn dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-fit-ml-init-benchmark");
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Da12-4: arm 3 renders the seeded start once. On a regressor-seeded fit restart 0 starts at
/// the prediction ([`patchback_fit::SeededStart::point`]), so evaluation 1 of its log is that
/// point; the raw regressor arm at that point has evaluation 1's total, bit for bit.
#[test]
fn the_predicted_arm_equals_the_first_evaluation_of_the_seeded_fit() {
    let topology = Topology::subtractive_mono();
    let notes = notes();
    let reference = reference(&topology, &notes);
    let path = dir().join("targets.weights");
    fs::write(
        &path,
        artifact_bytes(&[0.2, 0.3, 0.4, 0.6, 0.7, 0.8, 0.35, 0.65]),
    )
    .unwrap();
    let (normalization, text) = wide_stats();
    let seed = SeedInputs {
        predictor: Predictor::load(&path).expect("the artifact loads"),
        normalization,
        normalization_sha256: digest::sha256(text.as_bytes()),
    };
    let analysis = analyze_clip(&reference, &AnalysisConfig::default()).expect("analysed");
    let config = FitConfig {
        bounds: FitBounds {
            max_renders: 4,
            max_wall_time_ms: 600_000,
            max_restarts: 1,
            improvement_threshold: 1e-4,
        },
        seed: 3,
        render: RenderConfig::default(),
    };
    let outcome = fit_request(FitRequest {
        reference: &reference,
        notes: &notes,
        topology: &topology,
        config: &config,
        analysis: Some(&analysis),
        seed: Some(seed),
    })
    .expect("the seeded fit completes");
    assert_eq!(outcome.init, InitSource::RegressorPrediction);
    let seeded = outcome.seeded.as_ref().expect("a seeded start");
    assert_eq!(seeded.prediction.clamped, 0);
    let first = &outcome.log[0];
    assert_eq!(first.sequence, 1);
    assert_eq!(first.normalized, seeded.point());
    let total = first.total.expect("the predicted render is valid");

    let arm = predicted_arm(
        &reference,
        &notes,
        &topology,
        &seeded.point(),
        &config.render,
    )
    .expect("compared");
    assert_eq!(arm.comparison.total.to_bits(), total.to_bits());
    assert_eq!(arm.comparison.components, first.components.unwrap());
    assert_eq!(arm.clip.frame_count(), reference.frame_count());
}
