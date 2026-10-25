//! The three acceptance scenarios of story:weight-export (`spec/scenarios/weight-export-*.yaml`),
//! step by step against [`WeightArtifact::export`] (domain `patchback.model`, command
//! `ExportWeights`) and [`WeightArtifact::read`].
//!
//! The run comes from the four training commands as `training_scenarios.rs` drives them: a real
//! generation of 16 samples a split written under the cargo target, and a stand-in shell script
//! for `modelspec`, whose stdout and exit status this file chooses. The stand-ins print the
//! documented stdout of a 4-step run (two `init` lines, two `step` lines, `param` w1 and w2) or
//! that stdout with one defect; `pinned_export.rs` exports from the real pinned binary.
//!
//! The scenarios give names, shapes and counts as `ExportWeights` inputs; in Rust they are read
//! from the run's stdout (Da9-6, Da9-7), so a step that passes a bias name, a transposed w1 or a
//! short w2 maps to a run whose stdout prints that tensor. The `unknown-run` step has no library
//! counterpart (Da8-21, Da9-7): a [`TrainingRun`] is a value, so there is no identity to miss.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use patchback_analysis::FEATURES_1_VERSION;
use patchback_core::digest;
use patchback_dataset::{Generated, Generator, SplitCounts, SplitRole, SplitState};
use patchback_dataset::{dense, training_stats};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, ExportError, ModelSource, ModelSourceState, ModelSpec,
    ModelSpecPin, PAYLOAD_BYTES, RECOMMENDED_FILE_NAME, ReadError, TrainingRun, TrainingRunState,
    W1_ELEMENTS, W1_NAME, W1_SHAPE, W2_ELEMENTS, W2_NAME, W2_SHAPE, WeightArtifact,
};

const STEPS: u32 = 4;

fn pin() -> ModelSpecPin {
    ModelSpecPin::repository().expect("models/modelspec.pin parses")
}

fn design_model() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec")
}

fn values(count: usize, salt: usize) -> Vec<f32> {
    (0..count)
        .map(|index| ((index * 7919 + salt * 104_729) % 4001) as f32 / 3001.0 - 0.6667)
        .collect()
}

/// The w1 every stand-in prints in its `param` line.
fn w1() -> Vec<f32> {
    values(W1_ELEMENTS, 1)
}

/// The w2 every stand-in prints in its `param` line.
fn w2() -> Vec<f32> {
    values(W2_ELEMENTS, 2)
}

fn tokens(values: &[f32]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn line(kind: &str, name: &str, shape: [usize; 2], tokens: &[String]) -> String {
    format!(
        "{kind} {name}: Tensor<f32, [{}, {}]> = [{}]",
        shape[0],
        shape[1],
        tokens.join(", ")
    )
}

/// The documented stdout of a 4-step run without `--eval`, one entry a line.
fn documented() -> Vec<String> {
    vec![
        line("init", W1_NAME, W1_SHAPE, &tokens(&values(W1_ELEMENTS, 3))),
        line("init", W2_NAME, W2_SHAPE, &tokens(&values(W2_ELEMENTS, 4))),
        "step 0 loss 0.25".to_owned(),
        "step 4 loss 0.125".to_owned(),
        line("param", W1_NAME, W1_SHAPE, &tokens(&w1())),
        line("param", W2_NAME, W2_SHAPE, &tokens(&w2())),
    ]
}

/// Each stand-in: its name, its stdout lines and its exit status.
fn standin_outputs() -> Vec<(&'static str, Vec<String>, i32)> {
    let mut bias = documented();
    bias[4] = line("param", "@PatchRegressor.b1", W1_SHAPE, &tokens(&w1()));
    let mut transposed = documented();
    transposed[4] = line("param", W1_NAME, [64, 32], &tokens(&w1()));
    let mut short_w2 = documented();
    short_w2[5] = line("param", W2_NAME, W2_SHAPE, &tokens(&w2()[..511]));
    let mut extra_line = documented();
    extra_line.insert(4, "checkpoint written to run.msckpt".to_owned());
    let mut nan = documented();
    let mut nan_tokens = tokens(&w2());
    nan_tokens[7] = "NaN".to_owned();
    nan[5] = line("param", W2_NAME, W2_SHAPE, &nan_tokens);
    vec![
        ("documented", documented(), 0),
        ("bias", bias, 0),
        ("transposed", transposed, 0),
        ("short-w2", short_w2, 0),
        ("extra-line", extra_line, 0),
        ("nan", nan, 0),
        ("fails", documented(), 1),
    ]
}

/// The stand-in directory. Every script is written before the first test spawns anything, so no
/// spawn can inherit a script's open write descriptor (ETXTBSY). A script answers `--version`
/// with the pinned version, accepts every `check`, and on `train` prints its stdout file and
/// exits with its status.
fn standins() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-export-standins");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let version = pin().version().to_owned();
        for (name, lines, status) in standin_outputs() {
            let stdout = dir.join(format!("{name}.stdout"));
            let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
            fs::write(&stdout, text).unwrap();
            let path = dir.join(name);
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nif [ \"$1\" = --version ]; then echo \"modelspec {version}\"; exit 0; fi\nif [ \"$1\" = check ]; then exit 0; fi\ncat {}\nexit {status}\n",
                    stdout.display()
                ),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir
    })
}

fn open(name: &str) -> ModelSpec {
    ModelSpec::open(&standins().join(name), &pin())
        .expect("the stand-in reports the pinned version")
}

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-model-export")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `PinModelSpec` .. `FinishSplit` of the scenarios' arrange: the design model registered and
/// checked, and a generation (seed 42, 16 samples a split) whose train split is Written.
fn arrange(dir: &Path) -> (ModelSource, Generated) {
    let mut source = ModelSource::register(design_model(), "PatchRegressor", 16, &pin());
    assert_eq!(
        source.check(&open("documented")).unwrap(),
        ModelSourceState::Checked
    );
    let counts = SplitCounts {
        train: 16,
        validation: 16,
        test: 16,
    };
    let mut generated = Generator::new(42, counts, dir.to_path_buf())
        .run()
        .expect("the generation fills its splits");
    let split_dir = dir.join(SplitRole::Train.dir());
    fs::create_dir_all(&split_dir).unwrap();
    let train = generated.split_id(SplitRole::Train);
    let stats = training_stats(&generated).unwrap();
    dense::write_split(
        &mut generated.store,
        train,
        &split_dir,
        &stats,
        dense::create_new,
    )
    .unwrap();
    assert_eq!(
        generated.store.split(train).unwrap().state,
        SplitState::Written
    );
    (source, generated)
}

/// `PlanTrainingRun` Cpu, `BindModelSource`, `StartTrainingRun` with `standin` as the binary:
/// the run is Running.
fn started(source: &ModelSource, generated: &Generated, standin: &str) -> TrainingRun {
    let mut run = TrainingRun::plan(Backend::Cpu, &pin()).unwrap();
    run.bind(&source.trainable().unwrap()).unwrap();
    run.start(
        &generated.store,
        generated.split_id(SplitRole::Train),
        None,
        &open(standin),
        STEPS,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    assert_eq!(run.state(), TrainingRunState::Running);
    run
}

/// [`started`], then `FinishTrainingRun` exit 0: the run is Completed.
fn completed(source: &ModelSource, generated: &Generated, standin: &str) -> TrainingRun {
    let mut run = started(source, generated, standin);
    assert_eq!(run.finish().unwrap(), TrainingRunState::Completed);
    run
}

fn payload() -> Vec<u8> {
    w1().iter()
        .chain(&w2())
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|value| value.to_bits()).collect()
}

/// The refusal of exporting `run` to `path`; the export wrote nothing.
fn refused(run: &TrainingRun, path: &Path) -> ExportError {
    let result = WeightArtifact::export(run, FEATURES_1_VERSION, path);
    assert!(!path.exists(), "a refused export wrote {}", path.display());
    match result {
        Err(error) => error,
        Ok(_) => panic!("exported"),
    }
}

/// weight-export-exported: a completed CPU run's w1 [32, 64] and w2 [64, 8] are exported into a
/// checksummed artifact for the feature contract.
#[test]
fn weight_export_exported() {
    let dir = scratch("exported");
    let pin = pin();
    // 12:00:00 PinModelSpec, 12:00:01 RegisterModelSource, 12:00:02 CheckModelSource -> checked;
    // 12:00:03 GenerateDataset .. 12:00:07 FinishSplit -> the train split is Written.
    let (source, generated) = arrange(&dir);
    // 12:00:08 PlanTrainingRun Cpu, 12:00:09 BindModelSource, 12:00:10 StartTrainingRun with
    // eval_split_id null, 12:00:11 FinishTrainingRun exit_status 0 -> completed.
    let run = completed(&source, &generated, "documented");
    assert_eq!(run.output_error(), None);

    // 12:00:13 ExportWeights, w1 "@PatchRegressor.w1" [32, 64] 2048, w2 "@PatchRegressor.w2"
    // [64, 8] 512 -> exported (WeightsExported is the Ok).
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let artifact = WeightArtifact::export(&run, FEATURES_1_VERSION, &path).unwrap();
    assert_eq!(bits(artifact.w1()), bits(&w1()));
    assert_eq!(bits(artifact.w2()), bits(&w2()));
    assert_eq!(artifact.model(), "PatchRegressor");
    assert_eq!(artifact.modelspec_version(), pin.version());
    assert_eq!(artifact.modelspec_commit(), pin.commit());
    assert_eq!(artifact.feature_contract(), "features/1");
    let payload = payload();
    let checksum = digest::hex(&digest::sha256(&payload));
    assert_eq!(artifact.checksum(), checksum);
    let header = format!(
        "weights/1\nmodel PatchRegressor\nmodelspec {} {}\nfeatures features/1\nw1 32 64\nw2 64 8\nsha256 {checksum}\n\n",
        pin.version(),
        pin.commit()
    );
    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.len(), header.len() + PAYLOAD_BYTES);
    assert_eq!(&bytes[..header.len()], header.as_bytes());
    assert_eq!(&bytes[header.len()..], payload.as_slice());
    // The checksum verifies on read (story, Numeric obligations).
    assert_eq!(WeightArtifact::read(&path).unwrap(), artifact);

    // 12:00:14 ExportWeights of training run ...00ff, which does not exist -> unknown-run,
    // UnknownTrainingRun: no library counterpart (Da8-21, Da9-7). `export` takes the run itself;
    // tests/domain.rs lists the error as not emitted.
    assert!(
        !ExportError::DOMAIN_ERRORS.contains(&"patchback.model.UnknownTrainingRun"),
        "{:?}",
        ExportError::DOMAIN_ERRORS
    );

    // Beyond the scenario (Da9-11): the file is created new; exporting onto it again is refused
    // with an I/O error and leaves it as it was. The same run exported elsewhere gives the same
    // bytes (DESIGN.md § 12.1, deterministic output).
    match WeightArtifact::export(&run, FEATURES_1_VERSION, &path) {
        Err(error @ ExportError::Io { .. }) => assert_eq!(error.domain_error(), None),
        other => panic!("{:?}", other.map(drop)),
    }
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let again = dir.join("again.weights");
    assert_eq!(
        WeightArtifact::export(&run, FEATURES_1_VERSION, &again).unwrap(),
        artifact
    );
    assert_eq!(fs::read(&again).unwrap(), bytes);
}

/// weight-export-refused-output: a bias parameter name, transposed w1, a short w2, unrecognized
/// output lines and a NaN weight are each refused; no artifact is written.
#[test]
fn weight_export_refused_output() {
    let dir = scratch("refused-output");
    // 12:00:00 PinModelSpec .. 12:00:07 FinishSplit -> checked source, Written train split.
    let (source, generated) = arrange(&dir);
    // 12:00:08 PlanTrainingRun .. 12:00:11 FinishTrainingRun exit_status 0 -> completed: one
    // completed run a step, each printing the documented stdout with the step's defect.
    let path = dir.join(RECOMMENDED_FILE_NAME);

    // 12:00:13 ExportWeights w1_name "@PatchRegressor.b1" -> unrecognized-parameter.
    let run = completed(&source, &generated, "bias");
    match refused(&run, &path) {
        error @ ExportError::UnrecognizedParameter { .. } => {
            assert_eq!(
                error.domain_error(),
                Some("patchback.model.UnrecognizedParameter")
            );
            let ExportError::UnrecognizedParameter { w1_name, w2_name } = error else {
                unreachable!()
            };
            assert_eq!(w1_name, "@PatchRegressor.b1");
            assert_eq!(w2_name, W2_NAME);
        }
        other => panic!("{other:?}"),
    }

    // 12:00:14 ExportWeights w1 [64, 32] -> wrong-shape.
    let run = completed(&source, &generated, "transposed");
    match refused(&run, &path) {
        ExportError::WrongWeightShape {
            w1_rows,
            w1_columns,
            w2_rows,
            w2_columns,
        } => assert_eq!((w1_rows, w1_columns, w2_rows, w2_columns), (64, 32, 64, 8)),
        other => panic!("{other:?}"),
    }

    // 12:00:15 ExportWeights w2_elements 511 -> wrong-element-count.
    let run = completed(&source, &generated, "short-w2");
    match refused(&run, &path) {
        ExportError::WrongElementCount {
            w1_elements,
            w2_elements,
        } => assert_eq!((w1_elements, w2_elements), (2048, 511)),
        other => panic!("{other:?}"),
    }

    // 12:00:16 ExportWeights -> unrecognized-output: a line outside the documented grammar.
    let run = completed(&source, &generated, "extra-line");
    match refused(&run, &path) {
        error @ ExportError::UnrecognizedTrainingOutput { line, .. } => {
            assert_eq!(line, Some(5));
            assert_eq!(
                error.domain_error(),
                Some("patchback.model.UnrecognizedTrainingOutput")
            );
        }
        other => panic!("{other:?}"),
    }

    // 12:00:17 ExportWeights -> non-finite-weight: w2's eighth value printed `NaN`.
    let run = completed(&source, &generated, "nan");
    match refused(&run, &path) {
        ExportError::NonFiniteWeight { name, index, value } => {
            assert_eq!((name, index), (W2_NAME, 7));
            assert!(value.is_nan());
        }
        other => panic!("{other:?}"),
    }

    // The documented stdout, from the same arrangement, exports: each refusal above is its
    // stand-in's one defect.
    let run = completed(&source, &generated, "documented");
    WeightArtifact::export(&run, FEATURES_1_VERSION, &path).unwrap();
}

/// weight-export-run-not-completed: exporting from a run still Running is refused with
/// TrainingNotCompleted, and from the same run after it exits 1.
#[test]
fn weight_export_run_not_completed() {
    let dir = scratch("run-not-completed");
    // 12:00:00 PinModelSpec .. 12:00:07 FinishSplit -> checked source, Written train split.
    let (source, generated) = arrange(&dir);
    let path = dir.join(RECOMMENDED_FILE_NAME);
    // 12:00:08 PlanTrainingRun, 12:00:09 BindModelSource, 12:00:10 StartTrainingRun -> started.
    // The stand-in prints the whole documented stdout, then exits 1.
    let mut run = started(&source, &generated, "fails");
    assert!(run.stdout().contains("param @PatchRegressor.w2"));

    // 12:00:11 ExportWeights -> run-not-completed, TrainingNotCompleted (Running).
    match refused(&run, &path) {
        error @ ExportError::TrainingNotCompleted { .. } => {
            assert_eq!(
                error.domain_error(),
                Some("patchback.model.TrainingNotCompleted")
            );
            let ExportError::TrainingNotCompleted { state } = error else {
                unreachable!()
            };
            assert_eq!(state, TrainingRunState::Running);
        }
        other => panic!("{other:?}"),
    }

    // 12:00:12 FinishTrainingRun exit_status 1 -> failed.
    assert_eq!(run.finish().unwrap(), TrainingRunState::Failed);
    assert_eq!(run.exit_status(), Some(1));

    // 12:00:13 ExportWeights -> run-not-completed, TrainingNotCompleted (Failed), though its
    // stdout holds both documented `param` lines.
    match refused(&run, &path) {
        ExportError::TrainingNotCompleted { state } => {
            assert_eq!(state, TrainingRunState::Failed)
        }
        other => panic!("{other:?}"),
    }

    // Beyond the scenario: a Planned and a ModelBound run, with no stdout, are refused the same.
    let mut planned = TrainingRun::plan(Backend::Cpu, &pin()).unwrap();
    match refused(&planned, &path) {
        ExportError::TrainingNotCompleted { state } => {
            assert_eq!(state, TrainingRunState::Planned)
        }
        other => panic!("{other:?}"),
    }
    planned.bind(&source.trainable().unwrap()).unwrap();
    match refused(&planned, &path) {
        ExportError::TrainingNotCompleted { state } => {
            assert_eq!(state, TrainingRunState::ModelBound)
        }
        other => panic!("{other:?}"),
    }
}

/// Beyond the scenarios: a feature contract the header cannot hold as one token is refused
/// before anything is written (not a domain error; Da9-9 leaves its value unchecked), and a
/// missing file is an I/O error of the reader, not a corrupt artifact.
#[test]
fn an_unwritable_feature_contract_and_a_missing_file() {
    let dir = scratch("unwritable");
    let (source, generated) = arrange(&dir);
    let run = completed(&source, &generated, "documented");
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let long = "f".repeat(1024);
    for contract in [
        "",
        "features 1",
        "features/1\n",
        "features/1\tx",
        "é",
        long.as_str(),
    ] {
        match WeightArtifact::export(&run, contract, &path) {
            Err(error @ ExportError::Unwritable { .. }) => {
                assert_eq!(error.domain_error(), None)
            }
            other => panic!("{contract:?}: {:?}", other.map(drop)),
        }
        assert!(!path.exists(), "{contract:?}");
    }
    // Any other single token is recorded as given.
    let artifact = WeightArtifact::export(&run, "features/2-draft", &path).unwrap();
    assert_eq!(artifact.feature_contract(), "features/2-draft");
    assert_eq!(WeightArtifact::read(&path).unwrap(), artifact);

    match WeightArtifact::read(&dir.join("missing.weights")) {
        Err(error @ ReadError::Io { .. }) => assert_eq!(error.domain_error(), None),
        other => panic!("{other:?}"),
    }
    let corrupt = dir.join("corrupt.weights");
    let mut bytes = fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x80;
    fs::write(&corrupt, &bytes).unwrap();
    match WeightArtifact::read(&corrupt) {
        Err(error @ ReadError::Corrupt { .. }) => assert_eq!(
            error.domain_error(),
            Some("patchback.model.CorruptWeightArtifact")
        ),
        other => panic!("{other:?}"),
    }
}
