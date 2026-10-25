//! The two acceptance scenarios of story:native-predictor
//! (`spec/scenarios/native-predictor-*.yaml`), step by step against [`Predictor`] (domain
//! `patchback.model`, command `Predict`).
//!
//! The arrange steps run the crate's own commands: the repository's pin, the design model
//! registered and checked by a stand-in shell script for `modelspec`, a dataset split of the
//! scenarios' 50,016 rows Written in a [`DatasetStore`] (as `tests/domain.rs` writes one; no file
//! is rendered), a CPU training run whose stand-in prints the documented `param` lines of weights
//! this file chooses and exits 0, and [`WeightArtifact::export`] of that run. The `Predict` steps
//! then run on the exported file.
//!
//! The scenarios carry a `feature_contract_id` and a `feature_count`; in Rust the contract is the
//! version string `NormalizationStats::apply` compares (`features/1` for the scenarios'
//! `…0001`, another string for `…00a2`) and the count is the length of the vector (Da10-3,
//! Da10-4). Whether an output leaves [0, 1] and whether a value is finite are the scenarios'
//! external decisions; here they are the chosen weights and inputs.
//!
//! Order (Da10-4): the spec answers `feature-contract-mismatch` before `corrupt-artifact`; in
//! Rust a corrupt artifact is refused by [`Predictor::load`] and never reaches `predict`, where
//! the contract is compared. Each step exercises one refusal on its own input, so every step
//! holds.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use patchback_analysis::{FEATURES_1_VALUE_COUNT, FEATURES_1_VERSION};
use patchback_dataset::{
    DatasetStore, DenseFileKind, GenerateDataset, Pipeline, PrngAlgorithm, SplitRole, SplitState,
    WriteDenseFile,
};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, ModelSource, ModelSourceState, ModelSpec, ModelSpecPin,
    PAYLOAD_BYTES, PredictError, Predictor, RECOMMENDED_FILE_NAME, TrainingRun, TrainingRunState,
    W1_ELEMENTS, W1_NAME, W1_SHAPE, W2_ELEMENTS, W2_NAME, W2_SHAPE, WeightArtifact,
};

const STEPS: u32 = 4;

fn pin() -> ModelSpecPin {
    ModelSpecPin::repository().expect("models/modelspec.pin parses")
}

fn design_model() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec")
}

/// The weights the stand-in prints: w1[i, i] = 1 for every input `i` (so h[i] = max(0, x[i]) and
/// h[32..64] = 0), w2[k, k] = 0.25 for every output `k` (so y[k] = 0.25 · max(0, x[k])).
fn weights() -> (Vec<f32>, Vec<f32>) {
    let mut w1 = vec![0.0f32; W1_ELEMENTS];
    let mut w2 = vec![0.0f32; W2_ELEMENTS];
    for i in 0..W1_SHAPE[0] {
        w1[i * W1_SHAPE[1] + i] = 1.0;
    }
    for k in 0..W2_SHAPE[1] {
        w2[k * W2_SHAPE[1] + k] = 0.25;
    }
    (w1, w2)
}

fn param(name: &str, shape: [usize; 2], values: &[f32]) -> String {
    let tokens: Vec<String> = values.iter().map(|v| v.to_string()).collect();
    format!(
        "param {name}: Tensor<f32, [{}, {}]> = [{}]",
        shape[0],
        shape[1],
        tokens.join(", ")
    )
}

/// The stand-in: reports the pinned version, accepts every `check`, and on `train` prints two
/// `step` lines and the `param` lines of [`weights`], exiting 0. Written once, before anything is
/// spawned (ETXTBSY).
fn standin() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let dir =
            Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-predictor-standins");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (w1, w2) = weights();
        let stdout = dir.join("train.stdout");
        fs::write(
            &stdout,
            format!(
                "step 0 loss 0.25\nstep 4 loss 0.125\n{}\n{}\n",
                param(W1_NAME, W1_SHAPE, &w1),
                param(W2_NAME, W2_SHAPE, &w2)
            ),
        )
        .unwrap();
        let path = dir.join("modelspec");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nif [ \"$1\" = --version ]; then echo \"modelspec {}\"; exit 0; fi\nif [ \"$1\" = train ]; then cat {}; fi\nexit 0\n",
                pin().version(),
                stdout.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    })
}

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-model-predictor-scenarios")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The arrange and the first twelve steps of both scenarios, which are the same: the artifact
/// exported to `<dir>/regressor.weights` and its path.
fn exported(dir: &Path) -> (WeightArtifact, PathBuf) {
    // 12:00:00 PinModelSpec -> pinned: the repository's pin.
    let pin = pin();
    let modelspec = ModelSpec::open(standin(), &pin).unwrap();
    // 12:00:01 RegisterModelSource models/patch_regressor.mspec, PatchRegressor, batch 16.
    let mut source = ModelSource::register(design_model(), "PatchRegressor", 16, &pin);
    // 12:00:02 CheckModelSource -> checked.
    assert_eq!(source.check(&modelspec).unwrap(), ModelSourceState::Checked);

    // 12:00:03 GenerateDataset seed 42, batch 16, out_dir data/ -> generated.
    let mut store = DatasetStore::new();
    let dataset = store.generate_dataset(GenerateDataset {
        seed: 42,
        prng_algorithm: PrngAlgorithm("std-only-generator".to_owned()),
        batch_size: 16,
        feature_contract_id: Pipeline::new().feature_contract_id(),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    });
    // 12:00:04 OpenSplit Train, 50,016 samples -> opened.
    let train = store.open_split(dataset, SplitRole::Train, 50_016).unwrap();
    // 12:00:05 WriteDenseFile X, data/train/x.f32, 50,016 × 32, 6,402,048 bytes -> written;
    // 12:00:06 WriteDenseFile Y, data/train/y.f32, 50,016 × 8, 1,600,512 bytes -> written.
    for (kind, columns, byte_count) in [
        (DenseFileKind::X, 32, 6_402_048),
        (DenseFileKind::Y, 8, 1_600_512),
    ] {
        let content: Vec<u8> = (0..50_016 * columns)
            .flat_map(|_| 0.5f32.to_le_bytes())
            .collect();
        store
            .write_dense_file(
                WriteDenseFile {
                    dataset_split_id: train,
                    kind,
                    path: PathBuf::from("data/train").join(kind.file_name()),
                    rows: 50_016,
                    columns,
                    byte_count,
                },
                &content,
            )
            .unwrap();
    }
    // 12:00:07 FinishSplit -> written.
    store.finish_split(train).unwrap();
    assert_eq!(store.split(train).unwrap().state, SplitState::Written);

    // 12:00:08 PlanTrainingRun Cpu -> planned; 12:00:09 BindModelSource -> bound.
    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    run.bind(&source.trainable().unwrap()).unwrap();
    // 12:00:10 StartTrainingRun, eval_split_id null -> started.
    run.start(
        &store,
        train,
        None,
        &modelspec,
        STEPS,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    // 12:00:11 FinishTrainingRun exit_status 0 -> completed.
    assert_eq!(run.finish().unwrap(), TrainingRunState::Completed);

    // 12:00:13 ExportWeights, feature contract …0001 (features/1), w1 "@PatchRegressor.w1"
    // [32, 64] 2048, w2 "@PatchRegressor.w2" [64, 8] 512 -> exported.
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let artifact = WeightArtifact::export(&run, FEATURES_1_VERSION, &path).unwrap();
    let (w1, w2) = weights();
    assert_eq!(artifact.w1(), w1.as_slice());
    assert_eq!(artifact.w2(), w2.as_slice());
    assert_eq!(artifact.feature_contract(), FEATURES_1_VERSION);
    (artifact, path)
}

/// native-predictor-predicted: a 32-value vector of the artifact's feature contract is predicted;
/// a prediction with an output outside [0, 1] is clamped and counted.
#[test]
fn native_predictor_predicted() {
    let dir = scratch("predicted");
    // 12:00:00 PinModelSpec .. 12:00:13 ExportWeights -> exported.
    let (artifact, path) = exported(&dir);
    let mut predictor = Predictor::load(&path).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(predictor.artifact(), &artifact);
    assert_eq!(predictor.clamped_predictions(), 0, "clamped_predictions: 0");

    // 12:00:14 Predict, feature contract …0001, feature_count 32 -> predicted: every output in
    // [0, 1], returned unchanged; the artifact preserved (the counter stays 0).
    let x = vec![1.0; FEATURES_1_VALUE_COUNT];
    let prediction = predictor.predict(FEATURES_1_VERSION, &x).unwrap();
    assert_eq!(prediction.raw, [0.25; 8]);
    assert_eq!(prediction.values, prediction.raw);
    assert_eq!(prediction.clamped, 0);
    assert_eq!(predictor.clamped_predictions(), 0);

    // 12:00:15 Predict, feature contract …0001, feature_count 32 -> predicted-clamped: y[0] =
    // 0.25 · 6 = 1.5 is clamped to 1 and the prediction counted (clamped_predictions
    // {increment: 1}; PredictionClamped is `clamped > 0`, Da10-6).
    let mut x = vec![1.0; FEATURES_1_VALUE_COUNT];
    x[0] = 6.0;
    x[1] = -2.0;
    let prediction = predictor.predict(FEATURES_1_VERSION, &x).unwrap();
    assert_eq!(
        prediction.raw,
        [1.5, 0.0, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25]
    );
    assert_eq!(
        prediction.values,
        [1.0, 0.0, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25]
    );
    assert_eq!(prediction.clamped, 1);
    assert_eq!(predictor.clamped_predictions(), 1);

    // Nothing is written: the artifact's bytes are as exported (Da10-6, Da9-13).
    assert_eq!(WeightArtifact::read(&path).unwrap(), artifact);
}

/// native-predictor-refused-input: a vector of another feature contract, a 31-value vector, a
/// corrupt artifact, a NaN feature and an unknown artifact are each refused.
#[test]
fn native_predictor_refused_input() {
    let dir = scratch("refused-input");
    // 12:00:00 PinModelSpec .. 12:00:13 ExportWeights -> exported.
    let (_, path) = exported(&dir);
    let mut predictor = Predictor::load(&path).unwrap_or_else(|e| panic!("{e}"));
    let x = vec![1.0; FEATURES_1_VALUE_COUNT];

    // 12:00:14 Predict, feature contract …00a2, feature_count 32 -> feature-contract-mismatch.
    // In the spec this outcome comes before corrupt-artifact; in Rust it is predict's, after load
    // (Da10-4).
    match predictor.predict("features/2", &x) {
        Err(error @ PredictError::FeatureContractMismatch { .. }) => {
            assert_eq!(
                error.domain_error(),
                Some("patchback.model.FeatureContractMismatch")
            );
            let PredictError::FeatureContractMismatch { artifact, input } = error else {
                unreachable!()
            };
            assert_eq!(
                (artifact.as_str(), input.as_str()),
                ("features/1", "features/2")
            );
        }
        other => panic!("{other:?}"),
    }

    // 12:00:15 Predict, feature contract …0001, feature_count 31 -> wrong-feature-count.
    match predictor.predict(FEATURES_1_VERSION, &x[..31]) {
        Err(error @ PredictError::WrongFeatureCount { .. }) => {
            assert_eq!(
                error.domain_error(),
                Some("patchback.model.WrongFeatureCount")
            );
            assert!(matches!(
                error,
                PredictError::WrongFeatureCount { count: 31 }
            ));
        }
        other => panic!("{other:?}"),
    }

    // 12:00:16 Predict, feature contract …0001, feature_count 32 -> corrupt-artifact: one payload
    // byte of the written artifact flipped, refused when it is loaded (Da10-4: a corrupt artifact
    // never loads, so this answers before any contract comparison).
    let mut bytes = fs::read(&path).unwrap();
    let at = bytes.len() - PAYLOAD_BYTES + 100;
    bytes[at] ^= 0x01;
    let corrupt = dir.join("corrupt.weights");
    fs::write(&corrupt, &bytes).unwrap();
    match Predictor::load(&corrupt) {
        Err(error @ PredictError::CorruptWeightArtifact { .. }) => {
            assert_eq!(
                error.domain_error(),
                Some("patchback.model.CorruptWeightArtifact")
            );
            let PredictError::CorruptWeightArtifact { reason } = error else {
                unreachable!()
            };
            assert!(reason.contains("SHA-256"), "{reason}");
        }
        other => panic!("{:?}", other.map(drop)),
    }

    // 12:00:17 Predict, feature contract …0001, feature_count 32 -> non-finite-feature.
    let mut nan = x.clone();
    nan[7] = f64::NAN;
    match predictor.predict(FEATURES_1_VERSION, &nan) {
        Err(error @ PredictError::NonFiniteFeature { .. }) => {
            assert_eq!(
                error.domain_error(),
                Some("patchback.model.NonFiniteFeature")
            );
            assert!(matches!(error, PredictError::NonFiniteFeature { index: 7 }));
        }
        other => panic!("{other:?}"),
    }

    // 12:00:18 Predict, weight artifact …00fb, which does not exist -> unknown-artifact: a path
    // with no file (Da10-4).
    let unknown = dir.join("6c000000-0000-4000-8000-0000000000fb.weights");
    match Predictor::load(&unknown) {
        Err(error @ PredictError::UnknownWeightArtifact { .. }) => {
            assert_eq!(
                error.domain_error(),
                Some("patchback.model.UnknownWeightArtifact")
            );
            let PredictError::UnknownWeightArtifact { path } = error else {
                unreachable!()
            };
            assert_eq!(path, unknown);
        }
        other => panic!("{:?}", other.map(drop)),
    }

    // Every refusal left the predictor as it was: nothing counted, and it still predicts.
    assert_eq!(predictor.clamped_predictions(), 0);
    assert_eq!(
        predictor.predict(FEATURES_1_VERSION, &x).unwrap().raw,
        [0.25; 8]
    );
}
