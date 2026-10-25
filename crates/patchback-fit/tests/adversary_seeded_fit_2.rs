//! Adversary cases for story:regressor-seeded-fit (wave a11, pass 2: the correction Da11-21 to
//! Da11-23), through the library entry `fit_request`. Every artifact, statistics text and
//! reference is synthesized here.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-fit-adversary-seeded-2");
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

fn write_artifact(name: &str) -> PathBuf {
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    let mut w2 = vec![0.0f32; HIDDEN * OUTPUTS];
    for (k, target) in TARGETS.iter().enumerate() {
        w1[k] = 1.0;
        w2[k * OUTPUTS + k] = *target;
    }
    let path = dir().join(format!("{name}.weights"));
    fs::write(&path, artifact_bytes(&w1, &w2, FEATURES_1_VERSION)).unwrap();
    path
}

/// Statistics of mean 0 and standard deviation 1 for every value: `normalize` is then the
/// identity, bit for bit (`(x - 0) / 1`), so the seeded `features` are the raw vector.
fn identity_stats() -> String {
    let mut text = format!(
        "patchback-normalization-stats 1\nfeature_contract {FEATURES_1_VERSION}\nsplit train\nrows 16\nstd population\n"
    );
    for i in 0..FEATURES_1_VALUE_COUNT {
        text.push_str(&format!(
            "feature {i} {} {}\n",
            format_decimal(0.0),
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
        patch_id: "4fa05d30-0000-4000-8000-0000000000f2".to_owned(),
        note_estimation_id: None,
        notes,
    }
}

/// The default patch with the cutoff at `p = 0.25`, rendered with `notes` as the renderer
/// renders a dataset example (its own length: the last gate plus the release tail).
fn rendered(notes: &NoteSequence) -> Vec<f32> {
    let topology = Topology::subtractive_mono();
    let mut point = default_point(&topology);
    point[topology.contract("filter_cutoff").unwrap().index] = 0.25;
    let plan = compile(&validate(&patch_at(&topology, &point).unwrap()).unwrap());
    render(&plan, notes, &RenderConfig::default())
        .unwrap()
        .samples
}

fn clip(samples: Vec<f32>) -> AudioClip {
    AudioClip::from_mono(samples).unwrap()
}

fn analysis_of(clip: &AudioClip) -> Analysis {
    analyze_clip(clip, &AnalysisConfig::default()).expect("the clip analyses")
}

fn config(max_renders: i64) -> FitConfig {
    FitConfig {
        bounds: FitBounds {
            max_renders,
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
    max_renders: i64,
) -> Result<FitOutcome, FitError> {
    fit_request(FitRequest {
        reference,
        notes,
        topology: &Topology::subtractive_mono(),
        config: &config(max_renders),
        analysis: Some(analysis),
        seed: Some(seed),
    })
}

/// The `features/1` vector `patchback-dataset` computes for a training example
/// (`generate.rs` `Renderer::run`): `analyze_and_extract` of the published contract over the
/// whole render, span `0..gate`.
fn dataset_features(samples: &[f32], gate: usize) -> Vec<f64> {
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
    .expect("the dataset features")
    .values
}

/// The seeded run's raw features (the statistics are the identity).
fn seeded_features(reference: &AudioClip, notes: &NoteSequence, name: &str) -> Vec<f64> {
    let analysis = analysis_of(reference);
    let outcome = seeded_fit(
        reference,
        notes,
        &analysis,
        seed_inputs(&write_artifact(name), &identity_stats()),
        1,
    )
    .unwrap_or_else(|e| panic!("{name}: {}: {e}", e.domain_error()));
    assert_eq!(outcome.init, InitSource::RegressorPrediction);
    outcome.seeded.expect("a seeded run").features
}

fn differing(a: &[f64], b: &[f64]) -> Vec<(usize, f64, f64)> {
    a.iter()
        .zip(b)
        .enumerate()
        .filter(|(_, (x, y))| x.to_bits() != y.to_bits())
        .map(|(i, (x, y))| (i, *x, *y))
        .collect()
}

/// Da11-21: the seeded vector is, bit for bit, what the dataset computes for a render of that one
/// note, for a note other than the scenarios' (another pitch, velocity and gate, longer than the
/// attack window). The identity statistics make the comparison of the raw vectors exact.
#[test]
fn adversary2_the_seeded_vector_is_the_dataset_vector_bit_for_bit() {
    for (gate, midi, velocity) in [(30_000u64, 57u8, 0.6f32), (7_000, 40, 1.0)] {
        let notes = sequence(vec![note(0, gate, midi, velocity)]);
        let samples = rendered(&notes);
        assert!(samples.len() >= gate as usize + 9_600, "{}", samples.len());
        let expected = dataset_features(&samples, gate as usize);
        let got = seeded_features(&clip(samples), &notes, "adversary2-dataset");
        assert_eq!(
            differing(&expected, &got),
            [],
            "gate {gate}: (index, dataset, seeded)"
        );
    }
}

/// Da11-21: what the dataset never sees after the first note's release window does not change
/// the vector, down to a second note that starts on the first sample past the window
/// (`end + 9600`), so any cut later than the window's end would read it. A second note that
/// starts inside the window does change it (the window is read).
#[test]
fn adversary2_a_note_from_the_release_window_end_on_does_not_change_the_vector() {
    let first = note(0, 12_000, 45, 0.5);
    let single = sequence(vec![first]);
    let samples = rendered(&single);
    let expected = dataset_features(&samples, 12_000);

    let after = sequence(vec![first, note(21_600, 33_600, 52, 1.0)]);
    let phrase = rendered(&after);
    assert_eq!(phrase[..21_600], samples[..21_600]);
    assert!(phrase[21_600..].iter().any(|s| *s != 0.0));
    let got = seeded_features(&clip(phrase), &after, "adversary2-after");
    assert_eq!(differing(&expected, &got), [], "(index, dataset, seeded)");

    let inside = sequence(vec![first, note(16_000, 28_000, 52, 1.0)]);
    let got = seeded_features(&clip(rendered(&inside)), &inside, "adversary2-inside");
    assert_ne!(
        differing(&expected, &got),
        [],
        "a note inside the release window is read"
    );
}

/// Da11-21 measured: leading silence before the first note. The window frames are placed from
/// the clip's sample 0 (centres `i * hop + fft / 2`), so a shift that is a multiple of every
/// hop places the same frames over the note; the vector is printed for the report and the run
/// must start seeded either way.
#[test]
fn adversary2_leading_silence_before_the_first_note_is_measured() {
    let first = note(0, 12_000, 45, 0.8);
    let samples = rendered(&sequence(vec![first]));
    let expected = dataset_features(&samples, 12_000);
    for shift in [4_096usize, 1_000] {
        let mut shifted = vec![0.0f32; shift];
        shifted.extend_from_slice(&samples);
        let notes = sequence(vec![note(shift as u64, shift as u64 + 12_000, 45, 0.8)]);
        let got = seeded_features(&clip(shifted), &notes, "adversary2-leading");
        let diff = differing(&expected, &got);
        let largest = diff
            .iter()
            .map(|(_, x, y)| (x - y).abs())
            .fold(0.0f64, f64::max);
        eprintln!(
            "leading silence {shift}: {} of {FEATURES_1_VALUE_COUNT} values differ, largest |d| {largest}, at {:?}",
            diff.len(),
            diff.iter().map(|(i, _, _)| *i).collect::<Vec<_>>()
        );
        assert_eq!(got.len(), FEATURES_1_VALUE_COUNT);
        assert!(got.iter().all(|v| v.is_finite()));
    }
}

/// Da11-21: the given analysis admits the reference and is otherwise unread. The analysis of
/// another clip of the same length (another pitch and velocity) gives the same features, the
/// same prediction and the same evaluations as the reference's own.
#[test]
fn adversary2_a_foreign_analysis_of_the_same_length_changes_nothing() {
    let notes = sequence(vec![note(0, 12_000, 45, 0.8)]);
    let reference = clip(rendered(&notes));
    let other = clip(rendered(&sequence(vec![note(0, 12_000, 64, 0.4)])));
    assert_eq!(other.frame_count(), reference.frame_count());
    let own = analysis_of(&reference);
    let foreign = analysis_of(&other);
    assert_ne!(own, foreign);
    let weights = write_artifact("adversary2-foreign");
    let stats = identity_stats();
    let a = seeded_fit(&reference, &notes, &own, seed_inputs(&weights, &stats), 3).unwrap();
    let b = seeded_fit(
        &reference,
        &notes,
        &foreign,
        seed_inputs(&weights, &stats),
        3,
    )
    .unwrap();
    assert_eq!(a.seeded, b.seeded);
    assert_eq!(a.log, b.log);
    assert_eq!(a.normalized, b.normalized);
}

/// Module documentation step 5 at pass 1: "the feature extraction below refuses one whose frame
/// count is not the reference's". Da11-21 removed that extraction from the given analysis; the
/// correction brief keeps the analysis as the reference's admission. An analysis of a clip of
/// another length (or of another rate) is not the reference's, and nothing else checks it: the
/// fit's own `check_reference` is skipped when an analysis is given. Expected: refused before a
/// run starts, as it was at pass 1.
#[test]
fn adversary2_an_analysis_of_another_length_does_not_admit_the_reference() {
    let notes = sequence(vec![note(0, 12_000, 45, 0.8)]);
    let reference = clip(rendered(&notes));
    let mut longer = rendered(&notes);
    longer.resize(48_000, 0.0);
    let foreign = analysis_of(&clip(longer));
    assert_ne!(foreign.frame_count, reference.frame_count());
    let mut other_rate = analysis_of(&reference);
    other_rate.sample_rate_hz = 44_100;
    let weights = write_artifact("adversary2-other-length");
    let stats = identity_stats();
    for (name, analysis) in [("frame count", &foreign), ("sample rate", &other_rate)] {
        match seeded_fit(
            &reference,
            &notes,
            analysis,
            seed_inputs(&weights, &stats),
            1,
        ) {
            Err(_) => {}
            Ok(outcome) => panic!(
                "an analysis of another {name} admitted the reference: the run started ({}), {} evaluation(s)",
                outcome.init,
                outcome.run.evaluations()
            ),
        }
    }
}

/// Da11-22, the boundary: in a 48000-frame clip the STFT of 2048 (hop 1024) has frames
/// `0..47`, centres `1024 * (i + 1)`; the last centre before the clip's end is 47104. A first
/// note ending at 47104 leaves a release window `47104..48000` holding one frame centre and is
/// predicted from; one ending at 47105 holds none and is refused `PredictionFailed`, named
/// `patchback.fit.NoValidCandidate`, "holds no frame centre". The default fit of both runs. The
/// `.min(frames)` of `tail_end` is what keeps the slice in the clip for both (without it,
/// `..57104` panics).
#[test]
fn adversary2_the_last_end_sample_with_a_release_frame_is_47104() {
    let held = sequence(vec![note(0, 48_000, 45, 0.8)]);
    let mut samples = rendered(&held);
    samples.truncate(48_000);
    let reference = clip(samples);
    assert_eq!(reference.frame_count(), 48_000);
    let analysis = analysis_of(&reference);
    let weights = write_artifact("adversary2-boundary");
    let stats = identity_stats();
    let topology = Topology::subtractive_mono();
    for end in [47_104u64, 47_105] {
        let notes = sequence(vec![note(0, end, 45, 0.8)]);
        let default = fit(&reference, &notes, &topology, &config(1))
            .unwrap_or_else(|e| panic!("default, end_sample {end}: {}: {e}", e.domain_error()));
        assert_eq!(default.init, InitSource::DefaultPatch);
        let seeded = seeded_fit(
            &reference,
            &notes,
            &analysis,
            seed_inputs(&weights, &stats),
            1,
        );
        match (end, seeded) {
            (47_104, Ok(outcome)) => {
                assert_eq!(outcome.init, InitSource::RegressorPrediction);
            }
            (47_105, Err(error @ FitError::PredictionFailed { .. })) => {
                assert_eq!(error.domain_error(), "patchback.fit.NoValidCandidate");
                let message = error.to_string();
                assert!(message.contains("holds no frame centre"), "{message}");
                assert!(message.contains("from sample 47105 to 48000"), "{message}");
            }
            (end, Ok(outcome)) => panic!("end_sample {end}: started {}", outcome.init),
            (end, Err(error)) => panic!("end_sample {end}: {}: {error}", error.domain_error()),
        }
    }
}

/// Da11-23 measured: on a 10 s reference of two notes, the CLI's analysis of the whole
/// reference (outside the fit's clock), the first note's analysis (inside it) and one seeded
/// run of one evaluation, in milliseconds of this build. Printed for the report; the run
/// evaluates exactly once and records a wall time.
#[test]
fn adversary2_the_two_analyses_of_a_long_seeded_run_are_measured() {
    let notes = sequence(vec![
        note(0, 96_000, 45, 0.8),
        note(240_000, 420_000, 52, 0.8),
    ]);
    let mut samples = rendered(&notes);
    samples.resize(480_000, 0.0);
    let reference = clip(samples);

    let started = Instant::now();
    let analysis = analysis_of(&reference);
    let whole_ms = started.elapsed().as_millis();

    let started = Instant::now();
    let first = dataset_features(&reference.samples()[..96_000 + 9_600], 96_000);
    let first_ms = started.elapsed().as_millis();

    let started = Instant::now();
    let default = fit(
        &reference,
        &notes,
        &Topology::subtractive_mono(),
        &config(1),
    )
    .unwrap();
    let default_call_ms = started.elapsed().as_millis();

    let started = Instant::now();
    let outcome = seeded_fit(
        &reference,
        &notes,
        &analysis,
        seed_inputs(&write_artifact("adversary2-long"), &identity_stats()),
        1,
    )
    .unwrap();
    let seeded_call_ms = started.elapsed().as_millis();

    eprintln!(
        "10 s, two notes: whole-reference analysis {whole_ms} ms (CLI, outside the clock); \
         first-note analysis {first_ms} ms; default run of 1 evaluation: wall_time_ms {:?}, call {default_call_ms} ms; \
         seeded run of 1 evaluation: wall_time_ms {:?}, call {seeded_call_ms} ms",
        default.run.wall_time_ms(),
        outcome.run.wall_time_ms()
    );
    assert_eq!(outcome.seeded.unwrap().features, first);
    assert_eq!(outcome.run.evaluations(), 1);
    assert!(outcome.run.wall_time_ms().is_some());
}
