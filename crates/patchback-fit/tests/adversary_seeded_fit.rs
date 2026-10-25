//! Adversary cases for story:regressor-seeded-fit (wave a11, pass 1), through the library
//! entry `fit_request`. Every artifact, statistics text and reference is synthesized here.

use std::fs;
use std::path::{Path, PathBuf};

use patchback_analysis::{
    Analysis, AnalysisConfig, FEATURES_1_VALUE_COUNT, FEATURES_1_VERSION,
    FeatureContractDefinition, FeatureRegistry, NoteSpan, SAMPLE_RATE_HZ, analyze_and_extract,
    analyze_clip, read_stats,
};
use patchback_audio::AudioClip;
use patchback_core::digest;
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, format_decimal, validate};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    FitBounds, FitConfig, FitError, FitOutcome, FitRequest, InitSource, SeedInputs, default_point,
    fit, fit_request, patch_at,
};
use patchback_model::{ModelSpecPin, PAYLOAD_BYTES, Predictor, W1_ELEMENTS, W1_SHAPE, W2_SHAPE};

const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];
const TARGETS: [f32; OUTPUTS] = [0.2, 0.3, 0.4, 0.6, 0.7, 0.8, 0.35, 0.65];

fn dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-fit-adversary-seeded");
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn artifact_bytes(w1: &[f32], w2: &[f32], features: &str) -> Vec<u8> {
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

fn write_artifact(name: &str, features: &str) -> PathBuf {
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    let mut w2 = vec![0.0f32; HIDDEN * OUTPUTS];
    for (k, target) in TARGETS.iter().enumerate() {
        w1[k] = 1.0;
        w2[k * OUTPUTS + k] = *target;
    }
    let path = dir().join(format!("{name}.weights"));
    fs::write(&path, artifact_bytes(&w1, &w2, features)).unwrap();
    path
}

fn stats_text(version: &str, features: &[f64]) -> String {
    let mut text = format!(
        "patchback-normalization-stats 1\nfeature_contract {version}\nsplit train\nrows 16\nstd population\n"
    );
    for (i, x) in features.iter().enumerate() {
        text.push_str(&format!(
            "feature {i} {} {}\n",
            format_decimal(x - 1.0),
            format_decimal(1.0)
        ));
    }
    text
}

fn seed_inputs(weights: &Path, stats: &str) -> SeedInputs {
    SeedInputs {
        predictor: Predictor::load(weights).unwrap_or_else(|e| panic!("{e}")),
        normalization: read_stats(stats).unwrap_or_else(|e| panic!("{e}")),
        normalization_sha256: digest::sha256(stats.as_bytes()),
    }
}

fn note(start: u64, end: u64, midi_note: u8, velocity: f32) -> NoteEvent {
    NoteEvent {
        start_sample: start,
        end_sample: end,
        midi_note,
        velocity,
    }
}

fn sequence(notes: Vec<NoteEvent>) -> NoteSequence {
    NoteSequence {
        format_version: 1,
        sample_rate_hz: 48_000,
        patch_id: "4fa05d30-0000-4000-8000-0000000000f1".to_owned(),
        note_estimation_id: None,
        notes,
    }
}

/// The default patch with the cutoff at `p = 0.25`, rendered with `notes`, cut or padded to
/// `frames` (or left at its rendered length when `frames` is `None`).
fn rendered(notes: &NoteSequence, frames: Option<usize>) -> AudioClip {
    let topology = Topology::subtractive_mono();
    let mut point = default_point(&topology);
    point[topology.contract("filter_cutoff").unwrap().index] = 0.25;
    let plan = compile(&validate(&patch_at(&topology, &point).unwrap()).unwrap());
    let mut samples = render(&plan, notes, &RenderConfig::default())
        .unwrap()
        .samples;
    if let Some(frames) = frames {
        samples.resize(frames, 0.0);
    }
    AudioClip::from_mono(samples).unwrap()
}

fn analysis_of(clip: &AudioClip) -> Analysis {
    analyze_clip(clip, &AnalysisConfig::default()).expect("the reference analyses")
}

fn small() -> FitConfig {
    FitConfig {
        bounds: FitBounds {
            max_renders: 1,
            max_wall_time_ms: 600_000,
            max_restarts: 1,
            improvement_threshold: 0.0001,
        },
        seed: 42,
        ..FitConfig::default()
    }
}

fn seeded_fit(
    reference: &AudioClip,
    notes: &NoteSequence,
    analysis: &Analysis,
    seed: SeedInputs,
) -> Result<FitOutcome, FitError> {
    fit_request(FitRequest {
        reference,
        notes,
        topology: &Topology::subtractive_mono(),
        config: &small(),
        analysis: Some(analysis),
        seed: Some(seed),
    })
}

/// The `features/1` vector the dataset generator trains on (`patchback_dataset::generate`,
/// `Renderer::run`): `analyze_and_extract` of the published contract over `0..gate_samples`.
fn training_features(samples: &[f32], gate: usize) -> Vec<f64> {
    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    registry.publish(id).unwrap();
    analyze_and_extract(
        registry.get(id).unwrap(),
        samples,
        SAMPLE_RATE_HZ,
        NoteSpan {
            start_sample: 0,
            end_sample: gate,
        },
    )
    .expect("the training features")
    .values
}

/// Da11-8 (the features are of the first note's span): a reference whose first note is the
/// same as a single-note reference's, followed after the first note's release window by a
/// louder note of another pitch, gives the seeded fit the same `features/1` vector, so the same
/// start. The STFT frames of the first note's windows are identical in both clips (the render is
/// causal and the second note starts after them); only a value read from outside the span can
/// differ.
#[test]
fn adversary_a_later_note_does_not_change_the_first_notes_features() {
    let first = note(0, 12_000, 45, 0.5);
    let one = sequence(vec![first]);
    let two = sequence(vec![first, note(30_000, 42_000, 52, 1.0)]);
    let single = rendered(&one, Some(48_000));
    let phrase = rendered(&two, Some(48_000));
    assert_eq!(
        single.samples()[..30_000],
        phrase.samples()[..30_000],
        "the clips agree before the second note"
    );
    let single_analysis = analysis_of(&single);
    let phrase_analysis = analysis_of(&phrase);
    let weights = write_artifact("adversary-phrase", FEATURES_1_VERSION);
    let raw = training_features(single.samples(), 12_000);
    let stats = stats_text(FEATURES_1_VERSION, &raw);

    let a = seeded_fit(
        &single,
        &one,
        &single_analysis,
        seed_inputs(&weights, &stats),
    )
    .unwrap_or_else(|e| panic!("single: {}: {e}", e.domain_error()));
    let b = seeded_fit(
        &phrase,
        &two,
        &phrase_analysis,
        seed_inputs(&weights, &stats),
    )
    .unwrap_or_else(|e| panic!("phrase: {}: {e}", e.domain_error()));
    let a = a.seeded.unwrap();
    let b = b.seeded.unwrap();
    let differing: Vec<(usize, f64, f64)> = a
        .features
        .iter()
        .zip(&b.features)
        .enumerate()
        .filter(|(_, (x, y))| x != y)
        .map(|(i, (x, y))| (i, *x, *y))
        .collect();
    assert!(
        differing.is_empty(),
        "the first note's normalized features differ at (index, single, phrase) {differing:?}; \
         f0 single {} Hz, phrase {} Hz",
        single_analysis.f0.f0_hz,
        phrase_analysis.f0.f0_hz
    );
    assert_eq!(a.prediction, b.prediction);
}

/// Da11-7: the seeded fit's features are the ones the regressor was trained on: the dataset
/// generator's `analyze_and_extract` of the published `features/1` contract over `0..gate`,
/// normalized by the table. Any other analysis configuration or span changes the vector.
#[test]
fn adversary_the_seeded_features_are_the_training_features() {
    let notes = sequence(vec![note(0, 12_000, 45, 0.8)]);
    let reference = rendered(&notes, None);
    assert!(
        reference.frame_count() > 12_000,
        "the render holds a release tail"
    );
    let analysis = analysis_of(&reference);
    let raw = training_features(reference.samples(), 12_000);
    let stats = stats_text(FEATURES_1_VERSION, &raw);
    let weights = write_artifact("adversary-training", FEATURES_1_VERSION);
    let outcome = seeded_fit(&reference, &notes, &analysis, seed_inputs(&weights, &stats))
        .unwrap_or_else(|e| panic!("{}: {e}", e.domain_error()));
    let table = read_stats(&stats).unwrap();
    let raw: [f64; FEATURES_1_VALUE_COUNT] = raw.as_slice().try_into().unwrap();
    assert_eq!(
        outcome.seeded.unwrap().features,
        table.normalize(&raw).to_vec()
    );
}

/// Da11-5 order: an artifact and statistics both of another contract name the artifact, the
/// first of the two checks (the module documentation: "the artifact's, then the statistics'").
/// The implementor's cases make one input wrong at a time, so swapping the two checks passes
/// them.
#[test]
fn adversary_both_inputs_of_another_contract_name_the_weight_artifact() {
    let notes = sequence(vec![note(0, 12_000, 45, 0.8)]);
    let reference = rendered(&notes, None);
    let analysis = analysis_of(&reference);
    let raw = training_features(reference.samples(), 12_000);
    let weights = write_artifact("adversary-both-wrong", "features/2");
    let stats = stats_text("features/3", &raw);
    let error =
        seeded_fit(&reference, &notes, &analysis, seed_inputs(&weights, &stats)).unwrap_err();
    assert_eq!(
        error,
        FitError::FeatureContractMismatch {
            input: "weight artifact",
            expected: FEATURES_1_VERSION.to_owned(),
            found: "features/2".to_owned(),
        }
    );
}

/// Da11-8 and Da11-22 (the span clamped to the reference's length; the refusal stays): a
/// recording that ends while its note still sounds (the note's `end_sample` at or past the
/// clip's end, a reference cut at the gate) is fitted from the default patch without complaint;
/// the seeded fit of the same inputs is refused, because the first note has no release window
/// and its release features cannot be computed: `FitError::PredictionFailed`, named
/// `patchback.fit.NoValidCandidate`, its message naming the window that holds no frame centre.
#[test]
fn adversary_a_note_held_to_the_reference_end_is_refused() {
    let topology = Topology::subtractive_mono();
    let held = sequence(vec![note(0, 48_000, 45, 0.8)]);
    let reference = rendered(&held, Some(48_000));
    assert_eq!(reference.frame_count(), 48_000);
    let analysis = analysis_of(&reference);

    let gate = training_features(rendered(&held, None).samples(), 48_000);
    let stats = stats_text(FEATURES_1_VERSION, &gate);
    let weights = write_artifact("adversary-held", FEATURES_1_VERSION);
    for end in [48_000, 60_000] {
        let notes = sequence(vec![note(0, end, 45, 0.8)]);
        let default = fit(&reference, &notes, &topology, &small())
            .unwrap_or_else(|e| panic!("default, end_sample {end}: {}: {e}", e.domain_error()));
        assert_eq!(default.init, InitSource::DefaultPatch);
        match seeded_fit(&reference, &notes, &analysis, seed_inputs(&weights, &stats)) {
            Err(error @ FitError::PredictionFailed { .. }) => {
                assert_eq!(error.domain_error(), "patchback.fit.NoValidCandidate");
                assert!(
                    error.to_string().contains("holds no frame centre"),
                    "end_sample {end}: {error}"
                );
            }
            Err(error) => panic!("end_sample {end}: {}: {error}", error.domain_error()),
            Ok(outcome) => panic!("end_sample {end}: seeded run started: {:?}", outcome.init),
        }
    }
}

/// Da11-7 (the regressor is fed the features it was trained on): the dataset generator extracts
/// from the bare render of one note (`Renderer::run`, no padding); a reference is a recording,
/// and the scenario's own reference holds trailing silence (48000 frames for a note of 24000).
/// The same note followed by silence is expected to give the seeded fit the training vector.
#[test]
fn adversary_trailing_silence_does_not_change_the_seeded_features() {
    let notes = sequence(vec![note(0, 12_000, 45, 0.8)]);
    let bare = rendered(&notes, None);
    let raw = training_features(bare.samples(), 12_000);
    let stats = stats_text(FEATURES_1_VERSION, &raw);
    let weights = write_artifact("adversary-silence", FEATURES_1_VERSION);
    let table = read_stats(&stats).unwrap();
    let expected = table
        .normalize(&raw.as_slice().try_into().unwrap())
        .to_vec();

    let padded = rendered(&notes, Some(48_000));
    assert_eq!(padded.samples()[..bare.frame_count()], bare.samples()[..]);
    let outcome = seeded_fit(
        &padded,
        &notes,
        &analysis_of(&padded),
        seed_inputs(&weights, &stats),
    )
    .unwrap_or_else(|e| panic!("{}: {e}", e.domain_error()));
    let features = outcome.seeded.unwrap().features;
    let differing: Vec<(usize, f64, f64)> = expected
        .iter()
        .zip(&features)
        .enumerate()
        .filter(|(_, (x, y))| x != y)
        .map(|(i, (x, y))| (i, *x, *y))
        .collect();
    assert!(
        differing.is_empty(),
        "bare render {} frames, padded 48000: normalized features differ at (index, training, \
         fit) {differing:?}",
        bare.frame_count()
    );
}
