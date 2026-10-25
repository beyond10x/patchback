//! The acceptance of story:regressor-seeded-fit (DESIGN.md § 9.2 step 1, § 8.2, § 13 B): one
//! test per scenario of `spec/scenarios/`, named after it, each step cited, and the story's
//! numeric obligations: the seeded start is the predictor's output, finite and in [0, 1], and the
//! bounded-search obligations of `tests/search.rs` hold unchanged from the seeded start.
//!
//! Every weight artifact is written by [`artifact_bytes`] from weights the test chooses (the
//! `weights/1` bytes of `website/docs/reference/files.md` § Exported weights, copied from the a10
//! pattern of `crates/patchback-model/tests/predictor.rs`, Da11-14), every statistics table is a
//! `normalization.stats` text the test writes ([`stats_text`]) read through `read_stats`, and
//! every reference is a note the engine renders. The statistics are centred on the reference's
//! own features (mean `x[i] - 1`, standard deviation 1), so every normalized value is about 1 and
//! the chosen weights give a prediction near their targets.
//!
//! Scenario bounds: the scenarios name `max_renders 2000`, `max_wall_time_ms 600000`,
//! `max_restarts 4` and `improvement_threshold 0.001`; the outcomes they decide (the refusals and
//! the `init` of the started run) do not depend on the bounds, so the tests below run with small
//! render budgets to keep each fit to tens of renders.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use patchback_analysis::{
    Analysis, AnalysisConfig, FEATURES_1_VALUE_COUNT, FEATURES_1_VERSION,
    FeatureContractDefinition, FeatureRegistry, NormalizationTable, NoteSpan,
    RELEASE_OFFSET_FRAMES, RELEASE_WINDOW_FRAMES, SAMPLE_RATE_HZ, analyze_and_extract,
    analyze_clip, read_stats,
};
use patchback_audio::AudioClip;
use patchback_core::digest;
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, format_decimal, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    EvaluationOutcome, FitBounds, FitConfig, FitError, FitOutcome, FitRequest, FitState,
    InitSource, RunMeta, SeedInputs, StopReason, default_point, fit, fit_request, patch_at,
    report_text, restart_point, run_meta_text,
};
use patchback_model::{ModelSpecPin, PAYLOAD_BYTES, Predictor, W1_ELEMENTS, W1_SHAPE, W2_SHAPE};

const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];

/// The eight outputs the [`centred`] weights aim at: none at the default `0.5`, all inside
/// [0, 1].
const TARGETS: [f32; OUTPUTS] = [0.2, 0.3, 0.4, 0.6, 0.7, 0.8, 0.35, 0.65];

/// Targets three of which fall outside [0, 1]: two above, one below.
const CLAMPED_TARGETS: [f32; OUTPUTS] = [1.5, -0.5, 2.0, 0.5, 0.25, 0.75, 0.4, 0.6];

/// Written to the process's stderr, not through eprintln!, so the harness's output capture does
/// not hide it from the log.
fn say(line: &str) {
    let _ = writeln!(std::io::stderr(), "{line}");
}

fn dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-fit-regressor-seeded");
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The bytes of a `weights/1` artifact holding `w1` and `w2` for `features`, under the
/// repository's ModelSpec pin (the a10 writer, copied: Da11-14).
fn artifact_bytes(w1: &[f32], w2: &[f32], features: &str) -> Vec<u8> {
    assert_eq!(
        (w1.len(), w2.len()),
        (W1_ELEMENTS, HIDDEN * OUTPUTS),
        "the shapes of weights/1"
    );
    let payload: Vec<u8> = w1.iter().chain(w2).flat_map(|v| v.to_le_bytes()).collect();
    assert_eq!(payload.len(), PAYLOAD_BYTES);
    let pin = ModelSpecPin::repository().unwrap();
    let header = format!(
        "weights/1\nmodel PatchRegressor\nmodelspec {} {}\nfeatures {features}\nw1 32 64\nw2 64 8\nsha256 {}\n\n",
        pin.version(),
        pin.commit(),
        digest::hex(&digest::sha256(&payload))
    );
    [header.as_bytes(), &payload].concat()
}

/// Weights whose output `k` is `targets[k] · x[0]`: `w1[0, k] = 1` carries the first normalized
/// feature to hidden unit `k` (ReLU passes it, it is about 1), and `w2[k, k] = targets[k]`.
fn centred(targets: &[f32; OUTPUTS]) -> (Vec<f32>, Vec<f32>) {
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    let mut w2 = vec![0.0f32; HIDDEN * OUTPUTS];
    for (k, target) in targets.iter().enumerate() {
        w1[k] = 1.0;
        w2[k * OUTPUTS + k] = *target;
    }
    (w1, w2)
}

/// Writes the artifact of `targets` for `features` to `<name>.weights`; its path.
fn write_artifact(name: &str, targets: &[f32; OUTPUTS], features: &str) -> PathBuf {
    let (w1, w2) = centred(targets);
    let path = dir().join(format!("{name}.weights"));
    fs::write(&path, artifact_bytes(&w1, &w2, features)).unwrap();
    path
}

/// A `normalization.stats` text of `version` with the given means and standard deviations.
fn stats_text(version: &str, means: &[f64], stds: &[f64]) -> String {
    let mut text = format!(
        "patchback-normalization-stats 1\nfeature_contract {version}\nsplit train\nrows 16\nstd population\n"
    );
    for (i, (mean, std)) in means.iter().zip(stds).enumerate() {
        text.push_str(&format!(
            "feature {i} {} {}\n",
            format_decimal(*mean),
            format_decimal(*std)
        ));
    }
    text
}

/// The analysis the scenario's `AnalyzeAudio` step makes: the `features/1` configuration,
/// `AnalysisConfig::default()` (FFT lengths 512, 2048 and 8192).
fn analysis_of(reference: &AudioClip) -> Analysis {
    analyze_clip(reference, &AnalysisConfig::default()).expect("the reference analyses")
}

/// The raw `features/1` vector of the first note of `notes` in `reference`, its span clamped to
/// the reference's length (Da11-8), computed here through the analysis crate's own API as the
/// dataset computes a training example's, in the note's own time: the dataset renders one note
/// from sample 0 and extracts over `0..gate`, so here the clip is the reference from the note's
/// start, cut `gate + RELEASE_OFFSET_FRAMES + RELEASE_WINDOW_FRAMES` frames later, and the gate
/// is the note's length (Da11-21, Da11-25).
fn features_of(reference: &AudioClip, notes: &NoteSequence) -> Vec<f64> {
    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    registry.publish(id).unwrap();
    let note = notes.notes[0];
    let frames = reference.frame_count();
    let start = usize::try_from(note.start_sample).unwrap().min(frames);
    let gate = usize::try_from(note.end_sample).unwrap().min(frames) - start;
    let from_start = &reference.samples()[start..];
    let clip =
        &from_start[..(gate + RELEASE_OFFSET_FRAMES + RELEASE_WINDOW_FRAMES).min(from_start.len())];
    analyze_and_extract(
        registry.get(id).unwrap(),
        clip,
        SAMPLE_RATE_HZ,
        NoteSpan {
            start_sample: 0,
            end_sample: gate,
        },
    )
    .expect("the note's features")
    .values
}

/// The statistics centred on `features`: mean `x[i] - 1`, standard deviation 1.
fn centred_stats(version: &str, features: &[f64]) -> String {
    let means: Vec<f64> = features.iter().map(|x| x - 1.0).collect();
    stats_text(version, &means, &[1.0; FEATURES_1_VALUE_COUNT])
}

/// One note at `start..end`, MIDI 45, velocity 0.8, at 48 kHz.
fn notes(start: u64, end: u64) -> NoteSequence {
    NoteSequence {
        format_version: 1,
        sample_rate_hz: 48_000,
        patch_id: "4fa05d30-0000-4000-8000-0000000000f1".to_owned(),
        note_estimation_id: None,
        notes: vec![NoteEvent {
            start_sample: start,
            end_sample: end,
            midi_note: 45,
            velocity: 0.8,
        }],
    }
}

/// The default patch with the filter cutoff at `p = 0.25`, rendered with `notes` and padded with
/// silence (or cut) to `frames`.
fn reference(notes: &NoteSequence, frames: usize) -> AudioClip {
    let topology = Topology::subtractive_mono();
    let mut point = default_point(&topology);
    point[topology.contract("filter_cutoff").unwrap().index] = 0.25;
    let plan = compile(&validate(&patch_at(&topology, &point).unwrap()).unwrap());
    let mut samples = render(&plan, notes, &RenderConfig::default())
        .unwrap()
        .samples;
    samples.resize(frames, 0.0);
    AudioClip::from_mono(samples).unwrap()
}

fn config(seed: u64, max_renders: i64, max_restarts: i64, improvement_threshold: f64) -> FitConfig {
    FitConfig {
        bounds: FitBounds {
            max_renders,
            max_wall_time_ms: 600_000,
            max_restarts,
            improvement_threshold,
        },
        seed,
        ..FitConfig::default()
    }
}

/// The seed inputs of the artifact at `weights` and the statistics `stats`.
fn seed_inputs(weights: &Path, stats: &str) -> SeedInputs {
    SeedInputs {
        predictor: Predictor::load(weights).unwrap_or_else(|e| panic!("{e}")),
        normalization: read_stats(stats).unwrap_or_else(|e| panic!("{e}")),
        normalization_sha256: digest::sha256(stats.as_bytes()),
    }
}

fn request<'a>(
    reference: &'a AudioClip,
    notes: &'a NoteSequence,
    topology: &'a Topology,
    config: &'a FitConfig,
    analysis: Option<&'a Analysis>,
    seed: Option<SeedInputs>,
) -> FitRequest<'a> {
    FitRequest {
        reference,
        notes,
        topology,
        config,
        analysis,
        seed,
    }
}

/// The prediction the fit should start from, computed here independently of the fit: the
/// features normalized by the table, then the artifact's predictor.
fn expected_start(
    weights: &Path,
    table: &NormalizationTable,
    features: &[f64],
) -> patchback_model::Prediction {
    let x: [f64; FEATURES_1_VALUE_COUNT] = features.try_into().unwrap();
    let normalized = table.normalize(&x);
    Predictor::load(weights)
        .unwrap()
        .predict(FEATURES_1_VERSION, &normalized)
        .unwrap_or_else(|e| panic!("{e}"))
}

fn as_point(values: &[f32]) -> Vec<f64> {
    values.iter().map(|v| f64::from(*v)).collect()
}

/// spec/scenarios/regressor-seeded-fit-seeded.yaml: a fit naming a weight artifact and the
/// reference's analysis starts with init RegressorPrediction from the predicted patch; the same
/// fit without a weight artifact starts from DefaultPatch.
#[test]
fn regressor_seeded_fit_seeded() {
    // 00:00:01 DeclareTopology subtractive-mono/1 -> declared: the renderer's topology.
    let topology = Topology::subtractive_mono();
    assert_eq!(topology.name(), "subtractive-mono/1");
    // 00:00:02 LoadPatch (a SawOscillator patch of the topology) -> loaded: a fit takes no
    // starting patch as an input; the default patch is `default_point` of the topology.
    // 00:00:03 LoadNoteSequence: one note 0..24000, MIDI 45, velocity 0.8, at 48 kHz -> loaded.
    let notes = notes(0, 24_000);
    // 00:00:04 ImportWav: a Patchback-synthesized mono clip of 48000 frames at 48 kHz -> imported.
    let reference = reference(&notes, 48_000);
    assert_eq!(
        (reference.frame_count(), reference.sample_rate_hz()),
        (48_000, 48_000)
    );
    // 00:00:09 AnalyzeAudio with FFT lengths 512, 2048, 8192 -> analyzed (the features/1
    // configuration; its hops are half of each length).
    let analysis = analysis_of(&reference);
    let lengths: Vec<usize> = analysis.stfts.iter().map(|s| s.fft_length).collect();
    assert_eq!(lengths, [512, 2048, 8192]);
    let features = features_of(&reference, &notes);
    let stats = centred_stats(FEATURES_1_VERSION, &features);
    let weights = write_artifact("seeded", &TARGETS, FEATURES_1_VERSION);
    let table = read_stats(&stats).unwrap();
    let expected = expected_start(&weights, &table, &features);

    // 15:00:00 StartFit naming the weight artifact and the analysis, seed 42 -> started-seeded,
    // FitStarted { init: RegressorPrediction }. Threshold 10 ends restart 0 after one improving
    // sweep, so restart 1 begins within the budget.
    let config = config(42, 40, 4, 10.0);
    let started = Instant::now();
    let seeded = fit_request(request(
        &reference,
        &notes,
        &topology,
        &config,
        Some(&analysis),
        Some(seed_inputs(&weights, &stats)),
    ))
    .unwrap_or_else(|e| panic!("{}: {e}", e.domain_error()));
    say(&format!(
        "seeded fit: {} evaluations, {} restarts, wall {:?}",
        seeded.run.evaluations(),
        seeded.restarts,
        started.elapsed()
    ));
    assert_eq!(seeded.init, InitSource::RegressorPrediction);
    assert_eq!(seeded.init.name(), "RegressorPrediction");
    let start = seeded
        .seeded
        .as_ref()
        .expect("the seeded start is recorded");
    assert_eq!(start.prediction, expected);
    assert_eq!(start.prediction.clamped, 0);
    let point = as_point(&expected.values);
    // The start is the prediction, finite, in [0, 1], near the chosen targets, and not the
    // default patch.
    assert!(
        point
            .iter()
            .all(|p| p.is_finite() && (0.0..=1.0).contains(p))
    );
    for (value, target) in point.iter().zip(TARGETS) {
        assert!(
            (value - f64::from(target)).abs() < 1e-4,
            "{value} is not near {target}"
        );
    }
    assert_eq!(seeded.log[0].restart, 0);
    assert_eq!(
        seeded.log[0].normalized, point,
        "restart 0 starts at the prediction"
    );
    assert_eq!(seeded.run.initial_loss(), seeded.log[0].total);
    assert!(
        seeded
            .log
            .iter()
            .all(|e| e.normalized != default_point(&topology)),
        "a seeded run evaluates no default patch"
    );
    // Restarts 1 and later are unchanged.
    assert!(seeded.restarts >= 2, "the run reached restart 1");
    for restart in 1..seeded.restarts {
        let first = seeded.log.iter().find(|e| e.restart == restart).unwrap();
        assert_eq!(
            first.normalized,
            restart_point(42, restart, 8),
            "restart {restart}"
        );
    }
    assert_eq!(seeded.run.state(), FitState::Completed);

    // 15:00:01 StartFit without a weight artifact, seed 42 -> started, FitStarted { init:
    // DefaultPatch }: through `fit`, and through the request with neither input.
    let default = fit(&reference, &notes, &topology, &config).unwrap();
    assert_eq!(default.init, InitSource::DefaultPatch);
    assert_eq!(default.init.name(), "DefaultPatch");
    assert!(default.seeded.is_none());
    assert_eq!(default.log[0].normalized, default_point(&topology));
    let requested =
        fit_request(request(&reference, &notes, &topology, &config, None, None)).unwrap();
    assert_eq!(requested.init, InitSource::DefaultPatch);
    assert_eq!(requested.log, default.log);
    // A given analysis without a weight artifact changes nothing but who analysed.
    let analysed = fit_request(request(
        &reference,
        &notes,
        &topology,
        &config,
        Some(&analysis),
        None,
    ))
    .unwrap();
    assert_eq!(analysed.init, InitSource::DefaultPatch);
    assert_eq!(analysed.log, default.log);
}

/// spec/scenarios/regressor-seeded-fit-analysis-required.yaml: a fit naming a weight artifact
/// without an analysis is refused with AnalysisRequired; no run starts.
#[test]
fn regressor_seeded_fit_analysis_required() {
    let topology = Topology::subtractive_mono();
    let notes = notes(0, 24_000);
    let reference = reference(&notes, 48_000);
    let features = features_of(&reference, &notes);
    let stats = centred_stats(FEATURES_1_VERSION, &features);
    let weights = write_artifact("analysis-required", &TARGETS, FEATURES_1_VERSION);
    let checksum = Predictor::load(&weights)
        .unwrap()
        .artifact()
        .checksum()
        .to_owned();
    // 15:20:00 StartFit with weight_artifact_id and no analysis_id, seed 42, bounds 2000,
    // 600000, 4, 0.001 -> analysis-required, error patchback.fit.AnalysisRequired.
    let config = config(42, 2_000, 4, 0.001);
    let error = fit_request(request(
        &reference,
        &notes,
        &topology,
        &config,
        None,
        Some(seed_inputs(&weights, &stats)),
    ))
    .unwrap_err();
    assert_eq!(
        error,
        FitError::AnalysisRequired {
            weights_sha256: checksum.clone()
        }
    );
    assert_eq!(error.domain_error(), "patchback.fit.AnalysisRequired");
    assert!(error.to_string().contains(&checksum), "{error}");

    // In StartFit's order: the bounds come first; the missing analysis precedes the topology,
    // the feature-contract check and the reference's silence.
    let zero = config_with_renders(0);
    let error = fit_request(request(
        &reference,
        &notes,
        &topology,
        &zero,
        None,
        Some(seed_inputs(&weights, &stats)),
    ))
    .unwrap_err();
    assert_eq!(error.domain_error(), "patchback.fit.InvalidSearchBound");
    let unknown = unknown_topology();
    let mismatched = write_artifact("analysis-required-f2", &TARGETS, "features/2");
    let silent = AudioClip::from_mono(vec![0.0; 48_000]).unwrap();
    let error = fit_request(request(
        &silent,
        &notes,
        &unknown,
        &config,
        None,
        Some(seed_inputs(&mismatched, &stats)),
    ))
    .unwrap_err();
    assert_eq!(error.domain_error(), "patchback.fit.AnalysisRequired");
}

fn config_with_renders(max_renders: i64) -> FitConfig {
    config(42, max_renders, 4, 0.001)
}

/// A topology the renderer does not know: `subtractive-mono/1`'s contracts under another name.
fn unknown_topology() -> Topology {
    let known = Topology::subtractive_mono();
    Topology::new("subtractive-mono/9", known.contracts().to_vec()).unwrap()
}

/// spec/scenarios/regressor-seeded-fit-feature-contract-mismatch.yaml: a regressor-seeded fit
/// whose weight artifact and analysis differ in feature-contract version is refused with
/// FeatureContractMismatch; no run starts. The analysis is made under `features/1` (Da11-5,
/// Da11-7), so the mismatch is the weight artifact's contract or the statistics' against it.
#[test]
fn regressor_seeded_fit_feature_contract_mismatch() {
    // 00:00:01 DeclareTopology -> declared.
    let topology = Topology::subtractive_mono();
    // 00:00:02 LoadPatch -> loaded (no fit input). 00:00:03 LoadNoteSequence: 0..24000, MIDI 45.
    let notes = notes(0, 24_000);
    // 00:00:04 ImportWav: 48000 frames at 48 kHz -> imported.
    let reference = reference(&notes, 48_000);
    // 00:00:09 AnalyzeAudio, FFT lengths 512, 2048, 8192 -> analyzed.
    let analysis = analysis_of(&reference);
    let features = features_of(&reference, &notes);
    let stats = centred_stats(FEATURES_1_VERSION, &features);
    let config = config(42, 2_000, 4, 0.001);

    // 15:10:00 StartFit naming a weight artifact trained for features/2 and the features/1
    // analysis -> feature-contract-mismatch, error patchback.fit.FeatureContractMismatch.
    let weights = write_artifact("mismatch-weights", &TARGETS, "features/2");
    let error = fit_request(request(
        &reference,
        &notes,
        &topology,
        &config,
        Some(&analysis),
        Some(seed_inputs(&weights, &stats)),
    ))
    .unwrap_err();
    assert_eq!(
        error,
        FitError::FeatureContractMismatch {
            input: "weight artifact",
            expected: FEATURES_1_VERSION.to_owned(),
            found: "features/2".to_owned(),
        }
    );
    assert_eq!(
        error.domain_error(),
        "patchback.fit.FeatureContractMismatch"
    );
    let message = error.to_string();
    assert!(
        message.contains("weight artifact")
            && message.contains("features/1")
            && message.contains("features/2"),
        "{message}"
    );

    // Separately: statistics computed under another contract than features/1.
    let weights = write_artifact("mismatch-stats", &TARGETS, FEATURES_1_VERSION);
    let other = centred_stats("features/3", &features);
    let error = fit_request(request(
        &reference,
        &notes,
        &topology,
        &config,
        Some(&analysis),
        Some(seed_inputs(&weights, &other)),
    ))
    .unwrap_err();
    assert_eq!(
        error,
        FitError::FeatureContractMismatch {
            input: "normalization statistics",
            expected: FEATURES_1_VERSION.to_owned(),
            found: "features/3".to_owned(),
        }
    );
    assert_eq!(
        error.domain_error(),
        "patchback.fit.FeatureContractMismatch"
    );

    // In StartFit's order the mismatch follows the topology: an unknown topology is refused
    // first.
    let unknown = unknown_topology();
    let mismatched = write_artifact("mismatch-topology", &TARGETS, "features/2");
    let error = fit_request(request(
        &reference,
        &notes,
        &unknown,
        &config,
        Some(&analysis),
        Some(seed_inputs(&mismatched, &stats)),
    ))
    .unwrap_err();
    assert_eq!(error.domain_error(), "patchback.fit.TopologyNotFound");
}

// The numeric obligations: the bounded-search obligations of `tests/search.rs`, run from the
// seeded start with the same reference, notes and bounds.

/// One note of MIDI 45 at velocity 0.8, 0.25 s long, as `tests/search.rs` fits.
fn short_notes() -> NoteSequence {
    notes(0, 12_000)
}

/// The demo reference of `tests/search.rs`, unpadded.
fn short_reference() -> AudioClip {
    let notes = short_notes();
    let topology = Topology::subtractive_mono();
    let mut point = default_point(&topology);
    point[topology.contract("filter_cutoff").unwrap().index] = 0.25;
    let plan = compile(&validate(&patch_at(&topology, &point).unwrap()).unwrap());
    let samples = render(&plan, &notes, &RenderConfig::default())
        .unwrap()
        .samples;
    AudioClip::from_mono(samples).unwrap()
}

/// The inputs of a seeded fit of [`short_reference`]: the reference, its analysis, the artifact
/// of `targets` and the centred statistics.
struct Seeded {
    reference: AudioClip,
    notes: NoteSequence,
    analysis: Analysis,
    weights: PathBuf,
    stats: String,
}

impl Seeded {
    fn new(name: &str, targets: &[f32; OUTPUTS]) -> Self {
        let reference = short_reference();
        let notes = short_notes();
        let analysis = analysis_of(&reference);
        let features = features_of(&reference, &notes);
        Self {
            stats: centred_stats(FEATURES_1_VERSION, &features),
            weights: write_artifact(name, targets, FEATURES_1_VERSION),
            reference,
            notes,
            analysis,
        }
    }

    fn run(&self, config: &FitConfig) -> Result<FitOutcome, FitError> {
        fit_request(request(
            &self.reference,
            &self.notes,
            &Topology::subtractive_mono(),
            config,
            Some(&self.analysis),
            Some(seed_inputs(&self.weights, &self.stats)),
        ))
    }

    fn fit(&self, config: &FitConfig) -> FitOutcome {
        self.run(config)
            .unwrap_or_else(|e| panic!("{}: {e}", e.domain_error()))
    }
}

/// What every completed fit holds (`tests/search.rs`'s `assert_consistent`, copied), with the
/// first evaluation at the seeded start.
fn assert_consistent(outcome: &FitOutcome, config: &FitConfig) {
    let run = &outcome.run;
    let bounds = &config.bounds;
    assert_eq!(run.state(), FitState::Completed);
    assert_eq!(outcome.init, InitSource::RegressorPrediction);
    let start = outcome.seeded.as_ref().unwrap();
    assert_eq!(
        outcome.log[0].normalized,
        as_point(&start.prediction.values)
    );
    assert_eq!(run.evaluations() as usize, outcome.log.len());
    assert!(run.evaluations() <= bounds.max_renders);
    assert!(i64::from(outcome.restarts) <= bounds.max_restarts);
    assert!(outcome.restarts >= 1);
    let mut lowest: Option<f64> = None;
    for (n, evaluation) in outcome.log.iter().enumerate() {
        assert_eq!(evaluation.sequence, n as u64 + 1);
        match evaluation.outcome {
            EvaluationOutcome::Rejected => {
                assert!(evaluation.total.is_none() && evaluation.components.is_none());
                assert!(evaluation.rejection.is_some());
            }
            _ => {
                let total = evaluation.total.unwrap();
                assert!(total.is_finite() && total >= 0.0);
                let improves = lowest.is_none_or(|best| total < best);
                let expected = match (lowest, improves) {
                    (None, _) => EvaluationOutcome::FirstRetained,
                    (Some(_), true) => EvaluationOutcome::Improved,
                    (Some(_), false) => EvaluationOutcome::NotImproved,
                };
                assert_eq!(evaluation.outcome, expected, "evaluation {}", n + 1);
                if improves {
                    lowest = Some(total);
                }
            }
        }
        assert_eq!(evaluation.best_after, lowest, "evaluation {}", n + 1);
        assert_eq!(evaluation.normalized.len(), 8);
        assert!(
            evaluation
                .normalized
                .iter()
                .all(|p| (0.0..=1.0).contains(p))
        );
    }
    assert_eq!(run.best_loss(), lowest);
    assert_eq!(run.initial_loss(), outcome.log[0].total);
    let best = run.best().copied().unwrap();
    let retained = &outcome.log[best as usize - 1];
    assert_eq!(retained.total, run.best_loss());
    assert_eq!(retained.normalized, outcome.normalized);
    match run.stop_reason().unwrap() {
        StopReason::RenderBudgetExhausted => {
            assert_eq!(run.evaluations(), bounds.max_renders);
        }
        StopReason::TimeBudgetExhausted => {
            assert!(run.wall_time_ms().unwrap() >= bounds.max_wall_time_ms as u64);
        }
        StopReason::RestartsExhausted => {
            assert_eq!(i64::from(outcome.restarts), bounds.max_restarts);
        }
        StopReason::ImprovementBelowThreshold => {
            assert!(outcome.restarts >= 2);
        }
    }
}

/// § 9.2: from the seeded start every stop reason is reachable and agrees with the counters,
/// with `tests/search.rs`'s bounds.
#[test]
fn every_stop_reason_is_reachable_from_the_seeded_start() {
    let seeded = Seeded::new("stop-reasons", &TARGETS);
    let mut timed = config(0, 2_000, 4, 0.0001);
    timed.bounds.max_wall_time_ms = 1;
    let cases = [
        (config(0, 3, 4, 0.0001), StopReason::RenderBudgetExhausted),
        (timed, StopReason::TimeBudgetExhausted),
        (config(0, 500, 1, 10.0), StopReason::RestartsExhausted),
        (
            config(0, 500, 4, 10.0),
            StopReason::ImprovementBelowThreshold,
        ),
    ];
    for (config, reason) in cases {
        let outcome = seeded.fit(&config);
        assert_eq!(outcome.run.stop_reason(), Some(reason));
        assert_consistent(&outcome, &config);
    }
}

/// § 12.3: a wall-time bound of 1 ms stops a seeded run after its first render, the seeded
/// start, which every started run gets.
#[test]
fn a_one_millisecond_wall_time_bound_evaluates_the_seeded_start() {
    let seeded = Seeded::new("one-millisecond", &TARGETS);
    let mut config = config(0, 2_000, 4, 0.0001);
    config.bounds.max_wall_time_ms = 1;
    let outcome = seeded.fit(&config);
    assert_consistent(&outcome, &config);
    assert_eq!(
        outcome.run.stop_reason(),
        Some(StopReason::TimeBudgetExhausted)
    );
    assert_eq!(outcome.run.evaluations(), 1);
}

/// § 2 invariant 5: the same seed, inputs, artifact and statistics give the same run, bit for
/// bit; restarts 1 and later start at `restart_point`, as from the default start.
#[test]
fn the_same_seeded_inputs_give_the_same_run_and_the_same_restarts() {
    let seeded = Seeded::new("determinism", &TARGETS);
    let config = config(0x5eed, 60, 3, 10.0);
    let first = seeded.fit(&config);
    let second = seeded.fit(&config);
    assert_consistent(&first, &config);
    assert_eq!(first.log, second.log);
    assert_eq!(first.seeded, second.seeded);
    assert_eq!(first.render.samples(), second.render.samples());
    assert!(first.restarts >= 2, "the run reached a seeded restart");
    for restart in 1..first.restarts {
        let start = first.log.iter().find(|e| e.restart == restart).unwrap();
        assert_eq!(
            start.normalized,
            restart_point(0x5eed, restart, 8),
            "restart {restart}"
        );
    }
}

/// § 9.2 step 3: from the seeded start the search retains only improvements, so the best loss
/// is at most the seeded start's.
#[test]
fn the_search_from_the_seeded_start_never_ends_above_it() {
    let seeded = Seeded::new("never-above", &TARGETS);
    let config = config(0, 40, 1, 0.0001);
    let outcome = seeded.fit(&config);
    assert_consistent(&outcome, &config);
    assert!(outcome.run.best_loss().unwrap() <= outcome.run.initial_loss().unwrap());
}

/// § 5.2, § 8.2 (Da10-6, Da11-11): a prediction outside [0, 1] is clamped into it, the clamp is
/// counted, the start is the clamped values, and `report.txt` carries one warning line; a
/// prediction inside [0, 1] carries none.
#[test]
fn a_clamped_prediction_starts_clamped_and_warns() {
    let seeded = Seeded::new("clamped", &CLAMPED_TARGETS);
    let config = config(0, 6, 1, 0.0001);
    let outcome = seeded.fit(&config);
    assert_consistent(&outcome, &config);
    let start = outcome.seeded.as_ref().unwrap();
    assert_eq!(start.prediction.clamped, 3);
    let point = as_point(&start.prediction.values);
    assert_eq!(point[0], 1.0);
    assert_eq!(point[1], 0.0);
    assert_eq!(point[2], 1.0);
    assert!(point.iter().all(|p| (0.0..=1.0).contains(p)));
    let report = report_text(&outcome);
    let warnings: Vec<&str> = report
        .lines()
        .filter(|line| line.starts_with("warning "))
        .collect();
    assert_eq!(warnings, ["warning clamped 3 of 8 predictions"]);
    assert!(report.ends_with("warning clamped 3 of 8 predictions\n"));

    let unclamped = Seeded::new("unclamped", &TARGETS).fit(&config);
    assert_eq!(unclamped.seeded.as_ref().unwrap().prediction.clamped, 0);
    assert!(
        !report_text(&unclamped).contains("warning clamped"),
        "{}",
        report_text(&unclamped)
    );
}

/// Da11-10: `run.meta` of a seeded run records `init regressor-prediction` and, after it, the
/// artifact's checksum, its ModelSpec version and commit and the statistics' digest; the default
/// run's `init default` and records are unchanged.
#[test]
fn the_run_meta_of_a_seeded_run_names_its_weights_and_statistics() {
    let seeded = Seeded::new("run-meta", &TARGETS);
    let config = config(0, 4, 1, 0.0001);
    let outcome = seeded.fit(&config);
    let artifact = Predictor::load(&seeded.weights).unwrap();
    let artifact = artifact.artifact();
    let meta = RunMeta {
        seed: 0,
        block_size_frames: 256,
        modelspec_version: "0.5.0".to_owned(),
        reference_sha256: digest::sha256(b"reference"),
        notes_sha256: digest::sha256(b"notes"),
    };
    let text = run_meta_text(&outcome, &meta);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[2], "init regressor-prediction");
    assert_eq!(lines[3], format!("weights sha256 {}", artifact.checksum()));
    assert_eq!(
        lines[4],
        format!(
            "weights_modelspec {} {}",
            artifact.modelspec_version(),
            artifact.modelspec_commit()
        )
    );
    assert_eq!(
        lines[5],
        format!(
            "normalization sha256 {}",
            digest::hex(&digest::sha256(seeded.stats.as_bytes()))
        )
    );
    assert_eq!(lines[6], "seed 0");
    let keys: Vec<&str> = lines
        .iter()
        .map(|line| line.split_once(' ').unwrap().0)
        .collect();
    let mut unique = keys.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), keys.len(), "every record once: {keys:?}");

    let default = fit(
        &seeded.reference,
        &seeded.notes,
        &Topology::subtractive_mono(),
        &config,
    )
    .unwrap();
    let text = run_meta_text(&default, &meta);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[2], "init default");
    assert_eq!(lines[3], "seed 0");
    assert_eq!(lines.len(), 15);
}

/// Da11-7, Da11-12: a normalized value that is not finite as `f64` or as `f32` is refused before
/// the predictor, as the fit's NoValidCandidate naming the feature; no run starts.
#[test]
fn a_normalized_feature_that_is_not_finite_refuses_the_seeded_fit() {
    let seeded = Seeded::new("non-finite", &TARGETS);
    // Mean 0, standard deviation 1e-300: every feature above about 3.4e-262 becomes a z-score
    // past f32::MAX.
    let stats = stats_text(
        FEATURES_1_VERSION,
        &[0.0; FEATURES_1_VALUE_COUNT],
        &[1e-300; FEATURES_1_VALUE_COUNT],
    );
    let config = config(0, 4, 1, 0.0001);
    let error = fit_request(request(
        &seeded.reference,
        &seeded.notes,
        &Topology::subtractive_mono(),
        &config,
        Some(&seeded.analysis),
        Some(seed_inputs(&seeded.weights, &stats)),
    ))
    .unwrap_err();
    assert_eq!(error.domain_error(), "patchback.fit.NoValidCandidate");
    assert!(
        matches!(&error, FitError::PredictionFailed { message } if message.contains("normalized feature")),
        "{error:?}"
    );
}

/// Da11-8: the features are of the first note's span clamped to the reference; a note that
/// starts past the reference's end has no samples to read, and the seeded fit is refused as the
/// fit's NoValidCandidate naming the feature refusal; no run starts.
#[test]
fn a_first_note_past_the_reference_refuses_the_seeded_fit() {
    let seeded = Seeded::new("past-the-end", &TARGETS);
    let late = notes(1_000_000, 1_012_000);
    let config = config(0, 4, 1, 0.0001);
    let error = fit_request(request(
        &seeded.reference,
        &late,
        &Topology::subtractive_mono(),
        &config,
        Some(&seeded.analysis),
        Some(seed_inputs(&seeded.weights, &seeded.stats)),
    ))
    .unwrap_err();
    assert_eq!(error.domain_error(), "patchback.fit.NoValidCandidate");
    assert!(
        matches!(error, FitError::PredictionFailed { .. }),
        "{error:?}"
    );
}

/// Da11-25: the first note's clip starts at the note's start, as a training example's does, so
/// silence before the first note does not reach its features: the one-note render with 4,096
/// frames of silence prepended and its note shifted by 4,096 gives the seeded features of the
/// unshifted render, bit for bit.
#[test]
fn leading_silence_before_the_first_note_leaves_the_seeded_features_unchanged() {
    const LEAD_IN: usize = 4_096;
    let plain = Seeded::new("lead-in", &TARGETS);
    let mut samples = vec![0.0f32; LEAD_IN];
    samples.extend_from_slice(plain.reference.samples());
    let shifted = AudioClip::from_mono(samples).unwrap();
    let note = plain.notes.notes[0];
    let shifted_notes = notes(
        note.start_sample + LEAD_IN as u64,
        note.end_sample + LEAD_IN as u64,
    );
    let shifted_analysis = analysis_of(&shifted);
    let topology = Topology::subtractive_mono();
    let config = config(0, 1, 1, 0.0001);
    let features = |reference: &AudioClip, notes: &NoteSequence, analysis: &Analysis| {
        fit_request(request(
            reference,
            notes,
            &topology,
            &config,
            Some(analysis),
            Some(seed_inputs(&plain.weights, &plain.stats)),
        ))
        .unwrap_or_else(|e| panic!("{}: {e}", e.domain_error()))
        .seeded
        .expect("a seeded start")
        .features
    };
    let bits = |values: Vec<f64>| values.into_iter().map(f64::to_bits).collect::<Vec<_>>();
    let unshifted = features(&plain.reference, &plain.notes, &plain.analysis);
    let lead_in = features(&shifted, &shifted_notes, &shifted_analysis);
    assert_eq!(
        bits(lead_in.clone()),
        bits(unshifted.clone()),
        "the lead-in changed the features: {lead_in:?} vs {unshifted:?}"
    );
}
