//! The four acceptance scenarios of story:training-run (`spec/scenarios/training-run-*.yaml`),
//! step by step against [`TrainingRun`] (domain `patchback.model`, commands `PlanTrainingRun`,
//! `BindModelSource`, `StartTrainingRun`, `FinishTrainingRun`).
//!
//! The Written splits come from a real generation ([`Generator::run`]) written by
//! [`dense::write_split`] under the cargo target; the scenarios' 50,016 rows become 16, since
//! every step here observes the run's state machine, not the training. The binary is a stand-in
//! shell script this file writes, so the exit status and the output of `modelspec train` are
//! chosen here; `pinned_training.rs` runs the same commands against the real pinned binary.
//!
//! The steps that have no library counterpart (Da8-21) are cited where they fall: an unknown
//! source (`UnknownModelSource`) and an unknown pin (`UnknownPin`) name records `patchback-model`
//! does not keep; every [`TrainingRun`] is a value, so an unknown run (`UnknownTrainingRun`)
//! cannot be named either. `tests/domain.rs` accounts for them.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use patchback_dataset::{
    DatasetSplitId, DenseFileKind, GenerateDataset, Generated, Generator, Pipeline, PrngAlgorithm,
    SplitCounts, SplitRole, SplitState, WriteDenseFile,
};
use patchback_dataset::{dense, training_stats};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, LossCurve, ModelNotChecked, ModelSource, ModelSourceState,
    ModelSpec, ModelSpecPin, OutputError, TrainingError, TrainingFailure, TrainingRun,
    TrainingRunState,
};

const STEPS: u32 = 4;

fn pin() -> ModelSpecPin {
    ModelSpecPin::repository().expect("models/modelspec.pin parses")
}

/// `models/patch_regressor.mspec`, the source every scenario registers.
fn design_model() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec")
}

/// The stand-in directory. Every script is written before the first test spawns anything, so no
/// spawn can inherit a script's open write descriptor (ETXTBSY).
fn standins() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-training-standins");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let version = pin().version().to_owned();
        let header = format!("if [ \"$1\" = --version ]; then echo \"modelspec {version}\"; exit 0; fi\nif [ \"$1\" = check ]; then exit 0; fi");
        let eval = "case \" $* \" in *\" --eval \"*) echo \"eval loss=0.0625 accuracy=0\";; esac";
        let scripts = [
            (
                "trains",
                format!(
                    "{header}\nprintf '%s\\n' \"$@\" > {}\necho \"step 0 loss 0.25\"\necho \"step 4 loss 0.125\"\necho \"param @PatchRegressor.w1: Tensor<f32, [32, 64]> = [0]\"\n{eval}",
                    dir.join("trains.args").display()
                ),
            ),
            (
                "trains-quietly",
                format!("{header}\necho \"step 0 loss 0.25\"\necho \"step 4 loss 0.125\"\n{eval}"),
            ),
            (
                "fails",
                format!(
                    "{header}\necho \"E-DATA-008: the order file holds fewer than steps · B indices\" >&2\nexit 1"
                ),
            ),
            (
                "records",
                format!(
                    "{header}\ntouch {}\necho \"step 0 loss 0.25\"",
                    dir.join("records-was-asked-to-train").display()
                ),
            ),
            ("silent", format!("{header}\nexit 0")),
            (
                "no-eval-line",
                format!("{header}\necho \"step 0 loss 0.25\"\necho \"step 4 loss 0.125\""),
            ),
            ("sleeps", format!("{header}\nexec sleep 30")),
            (
                "rejects",
                format!(
                    "if [ \"$1\" = --version ]; then echo \"modelspec {version}\"; exit 0; fi\necho \"E-TYPE-001: matmul shapes do not agree\" >&2\nexit 1"
                ),
            ),
        ];
        for (name, body) in scripts {
            let path = dir.join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::write(
            dir.join("patch_regressor_draft.mspec"),
            "model PatchRegressor {}\n",
        )
        .unwrap();
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
        .join("patchback-model-training")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `GenerateDataset` and `OpenSplit` of each role, filled by the real pipeline: seed 42, batch
/// size 16 (the scenarios' input), 16 samples a split; every split still `Open`.
fn generate(dir: &Path) -> Generated {
    let counts = SplitCounts {
        train: 16,
        validation: 16,
        test: 16,
    };
    Generator::new(42, counts, dir.to_path_buf())
        .run()
        .expect("the generation fills its splits")
}

/// `WriteDenseFile` of x.f32 (z-scored under the training split's statistics, Da8-30), y.f32 (and
/// order.u32), then `FinishSplit`, through the writer, into `<dir>/<role>`; returns that directory.
fn write(generated: &mut Generated, role: SplitRole, dir: &Path) -> PathBuf {
    let split_dir = dir.join(role.dir());
    fs::create_dir_all(&split_dir).unwrap();
    let split = generated.split_id(role);
    let stats = training_stats(generated).unwrap();
    dense::write_split(
        &mut generated.store,
        split,
        &split_dir,
        &stats,
        dense::create_new,
    )
    .unwrap();
    split_dir
}

/// `RegisterModelSource` of the design model under the pin, then `CheckModelSource` with the
/// stand-in that accepts it.
fn checked_source() -> ModelSource {
    let mut source = ModelSource::register(design_model(), "PatchRegressor", 16, &pin());
    assert_eq!(
        source.check(&open("trains")).unwrap(),
        ModelSourceState::Checked
    );
    source
}

/// training-run-completed: a CPU run of a checked source on the written train split starts,
/// exits 0 and is Completed; finishing it again is refused.
#[test]
fn training_run_completed() {
    let dir = scratch("completed");
    // 12:00:00 PinModelSpec -> pinned.
    let pin = pin();
    // 12:00:01 RegisterModelSource, 12:00:02 CheckModelSource -> checked.
    let source = checked_source();
    // 12:00:03 GenerateDataset, 12:00:04 OpenSplit Train -> generated, opened.
    let mut generated = generate(&dir);
    let train = generated.split_id(SplitRole::Train);
    assert_eq!(
        generated.store.split(train).unwrap().state,
        SplitState::Open
    );
    // 12:00:05 WriteDenseFile X, 12:00:06 WriteDenseFile Y, 12:00:07 FinishSplit -> written.
    let train_dir = write(&mut generated, SplitRole::Train, &dir);
    let split = generated.store.split(train).unwrap();
    assert_eq!(split.state, SplitState::Written);
    let files = generated.store.dense_files_of(train);
    let x = files.iter().find(|f| f.kind == DenseFileKind::X).unwrap();
    let y = files.iter().find(|f| f.kind == DenseFileKind::Y).unwrap();
    assert_eq!((x.rows, x.columns, x.byte_count), (16, 32, 16 * 32 * 4));
    assert_eq!((y.rows, y.columns, y.byte_count), (16, 8, 16 * 8 * 4));

    // 12:00:08 PlanTrainingRun Cpu -> planned.
    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    assert_eq!(run.state(), TrainingRunState::Planned);
    assert_eq!(run.backend(), Backend::Cpu);
    assert_eq!(run.pin(), &pin);
    assert_eq!(run.model_source(), None);
    assert_eq!(run.data_split_id(), None);
    assert_eq!(run.eval_split_id(), None);
    assert_eq!(run.exit_status(), None);

    // 12:00:09 BindModelSource -> bound.
    run.bind(&source.trainable().unwrap()).unwrap();
    assert_eq!(run.state(), TrainingRunState::ModelBound);
    assert_eq!(run.model_source(), Some(design_model().as_path()));

    // 12:00:10 StartTrainingRun, eval_split_id null -> started.
    let modelspec = open("trains");
    run.start(
        &generated.store,
        train,
        None,
        &modelspec,
        STEPS,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    assert_eq!(run.state(), TrainingRunState::Running);
    assert_eq!(run.data_split_id(), Some(train));
    assert_eq!(run.data_role(), Some(SplitRole::Train));
    assert_eq!(run.eval_split_id(), None);
    assert_eq!(run.steps(), Some(STEPS));
    assert_eq!(run.exit_status(), None, "FinishTrainingRun records it");
    let args = fs::read_to_string(standins().join("trains.args")).unwrap();
    let expected = [
        "train".to_owned(),
        design_model().display().to_string(),
        "--backend".to_owned(),
        "cpu".to_owned(),
        "--data".to_owned(),
        train_dir.display().to_string(),
        "--steps".to_owned(),
        STEPS.to_string(),
    ];
    assert_eq!(args.lines().collect::<Vec<_>>(), expected);

    // 12:00:11 FinishTrainingRun exit_status 0 -> completed.
    assert_eq!(run.finish().unwrap(), TrainingRunState::Completed);
    assert_eq!(run.state(), TrainingRunState::Completed);
    assert_eq!(run.exit_status(), Some(0));
    assert_eq!(run.failure(), None);
    assert_eq!(run.output_error(), None);
    assert_eq!(run.losses(), [(0, 0.25), (4, 0.125)]);
    assert_eq!(run.eval_loss(), None, "no --eval, no eval line");
    assert!(run.stdout().contains("param @PatchRegressor.w1"));
    assert_eq!(run.stderr(), "");

    // 12:00:12 FinishTrainingRun again -> wrong-state, TrainingRunFinished.
    match run.finish() {
        Err(TrainingError::TrainingRunFinished { state }) => {
            assert_eq!(state, TrainingRunState::Completed)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(run.state(), TrainingRunState::Completed);
    assert_eq!(run.exit_status(), Some(0));
}

/// training-run-refused-unchecked-model: binding an unchecked, a rejected or an unknown model
/// source to a planned run is refused.
#[test]
fn training_run_refused_unchecked_model() {
    // 12:10:00 PinModelSpec -> pinned.
    let pin = pin();
    // 12:10:01 RegisterModelSource -> registered (unchecked).
    let unchecked = ModelSource::register(design_model(), "PatchRegressor", 16, &pin);
    // 12:10:02 RegisterModelSource, 12:10:03 CheckModelSource -> rejected.
    let mut rejected = ModelSource::register(
        standins().join("patch_regressor_draft.mspec"),
        "PatchRegressor",
        16,
        &pin,
    );
    assert_eq!(
        rejected.check(&open("rejects")).unwrap(),
        ModelSourceState::Rejected
    );
    // 12:10:04 PlanTrainingRun Cpu -> planned.
    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();

    // 12:10:05 BindModelSource unchecked -> model-not-checked. `bind` takes the source and
    // decides its state after the run's (Da8-39).
    match run.bind(&unchecked) {
        Err(TrainingError::ModelNotChecked(ModelNotChecked { state })) => {
            assert_eq!(state, ModelSourceState::Unchecked)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(run.state(), TrainingRunState::Planned);

    // 12:10:06 BindModelSource rejected -> model-not-checked.
    match run.bind(&rejected) {
        Err(TrainingError::ModelNotChecked(ModelNotChecked { state })) => {
            assert_eq!(state, ModelSourceState::Rejected)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(run.state(), TrainingRunState::Planned);
    assert_eq!(run.model_source(), None);

    // 12:10:07 BindModelSource of an unknown source id -> unknown-source: no library
    // counterpart (Da8-21); a ModelSource is a value, so there is no identity to look up.
    assert_eq!(
        TrainingError::from(ModelNotChecked {
            state: ModelSourceState::Unchecked
        })
        .domain_error(),
        Some("patchback.model.ModelNotChecked")
    );
}

/// Beyond the scenarios (Da8-39, the order the domain's interpreter answers): binding an
/// unchecked source to a run past Planned is refused for the run's state, before the source's;
/// a checked source of another pin to a Planned run is refused `PinMismatch` only after the
/// source's state. Each refusal leaves the run as it was.
#[test]
fn bind_decides_the_run_state_then_the_source_then_the_pin() {
    let pin = pin();
    let unchecked = ModelSource::register(design_model(), "PatchRegressor", 16, &pin);
    let source = checked_source();
    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    run.bind(&source).unwrap();
    let bound = run.clone();
    match run.bind(&unchecked) {
        Err(TrainingError::TrainingRunPastPlanning { state }) => {
            assert_eq!(state, TrainingRunState::ModelBound)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(run, bound);

    let other_pin = ModelSpecPin::parse(&format!(
        "version 0.4.0\ncommit {}\nsha256-x86_64-unknown-linux-gnu {}\n",
        "a".repeat(40),
        "b".repeat(64)
    ))
    .unwrap();
    let unchecked_elsewhere =
        ModelSource::register(design_model(), "PatchRegressor", 16, &other_pin);
    let mut planned = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    let before = planned.clone();
    match planned.bind(&unchecked_elsewhere) {
        Err(TrainingError::ModelNotChecked(ModelNotChecked { state })) => {
            assert_eq!(state, ModelSourceState::Unchecked)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(planned, before);
}

/// training-run-refused-unwritten-data: starting a run on an Open train split or an unknown
/// split, with an unwritten eval split, or an eval count off the batch size is refused; nothing
/// is spawned for any of them.
#[test]
fn training_run_refused_unwritten_data() {
    let dir = scratch("unwritten");
    let marker = standins().join("records-was-asked-to-train");
    let modelspec = open("records");
    // 12:20:00 PinModelSpec, 12:20:01 RegisterModelSource, 12:20:02 CheckModelSource.
    let pin = pin();
    let source = checked_source();
    // 12:20:03 GenerateDataset, 12:20:04 OpenSplit Train -> the train split is Open.
    let mut generated = generate(&dir);
    let train = generated.split_id(SplitRole::Train);
    let validation = generated.split_id(SplitRole::Validation);
    // 12:20:05 PlanTrainingRun Cpu -> planned.
    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    let start = |run: &mut TrainingRun,
                 store: &patchback_dataset::DatasetStore,
                 data: DatasetSplitId,
                 eval: Option<DatasetSplitId>| {
        run.start(
            store,
            data,
            eval,
            &modelspec,
            STEPS,
            DEFAULT_TRAINING_TIMEOUT,
        )
    };

    // 12:20:06 StartTrainingRun on the Planned run -> wrong-state, TrainingRunNotReady: the
    // run's state is decided before its data (the train split is Open here too).
    match start(&mut run, &generated.store, train, None) {
        Err(TrainingError::TrainingRunNotReady { state }) => {
            assert_eq!(state, TrainingRunState::Planned)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(run.state(), TrainingRunState::Planned);

    // 12:20:07 BindModelSource -> bound.
    run.bind(&source.trainable().unwrap()).unwrap();

    // 12:20:08 StartTrainingRun on the Open train split -> data-not-written.
    match start(&mut run, &generated.store, train, None) {
        Err(TrainingError::DataNotWritten {
            dataset_split_id,
            state,
        }) => {
            assert_eq!(dataset_split_id, train);
            assert_eq!(state, SplitState::Open);
        }
        other => panic!("{other:?}"),
    }

    // 12:20:09 StartTrainingRun on an unknown split -> unknown-data.
    let unknown = DatasetSplitId(0xfe);
    match start(&mut run, &generated.store, unknown, None) {
        Err(TrainingError::UnknownDataSplit { dataset_split_id }) => {
            assert_eq!(dataset_split_id, unknown)
        }
        other => panic!("{other:?}"),
    }

    // 12:20:10 WriteDenseFile X, 12:20:11 WriteDenseFile Y, 12:20:12 FinishSplit -> written.
    write(&mut generated, SplitRole::Train, &dir);
    assert_eq!(
        generated.store.split(train).unwrap().state,
        SplitState::Written
    );

    // 12:20:13 StartTrainingRun with an unknown eval split -> eval-not-written.
    let unknown_eval = DatasetSplitId(0xfd);
    match start(&mut run, &generated.store, train, Some(unknown_eval)) {
        Err(TrainingError::EvalNotWritten {
            eval_split_id,
            state,
        }) => {
            assert_eq!(eval_split_id, unknown_eval);
            assert_eq!(state, None);
        }
        other => panic!("{other:?}"),
    }
    // The other half of eval-not-written: a known eval split whose dense data is not written.
    match start(&mut run, &generated.store, train, Some(validation)) {
        Err(TrainingError::EvalNotWritten {
            eval_split_id,
            state,
        }) => {
            assert_eq!(eval_split_id, validation);
            assert_eq!(state, Some(SplitState::Open));
        }
        other => panic!("{other:?}"),
    }

    // 12:20:14 StartTrainingRun with an eval split whose count is not a multiple of the source's
    // batch size (16) -> eval-count-not-batch-multiple. The store admits only counts of its own
    // dataset's batch size, so the split is a written 24-sample split of a batch-8 dataset.
    let off_batch = off_batch_split(&mut generated);
    match start(&mut run, &generated.store, train, Some(off_batch)) {
        Err(TrainingError::EvalCountNotBatchMultiple {
            eval_split_id,
            sample_count,
            batch_size,
        }) => {
            assert_eq!(eval_split_id, off_batch);
            assert_eq!(sample_count, 24);
            assert_eq!(batch_size, 16);
        }
        other => panic!("{other:?}"),
    }

    assert_eq!(run.state(), TrainingRunState::ModelBound);
    assert_eq!(run.data_split_id(), None);
    assert!(!marker.exists(), "a refused start spawned the binary");
}

/// A Written validation split of 24 samples under a dataset of batch size 8, its x.f32 and y.f32
/// verified in memory through `WriteDenseFile`.
fn off_batch_split(generated: &mut Generated) -> DatasetSplitId {
    let store = &mut generated.store;
    let dataset = store.generate_dataset(GenerateDataset {
        seed: 7,
        prng_algorithm: PrngAlgorithm("xoshiro256**".to_owned()),
        batch_size: 8,
        feature_contract_id: Pipeline::new().feature_contract_id(),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    });
    let split = store
        .open_split(dataset, SplitRole::Validation, 24)
        .unwrap();
    for (kind, columns) in [(DenseFileKind::X, 32), (DenseFileKind::Y, 8)] {
        let content: Vec<u8> = (0..24 * columns)
            .flat_map(|_| 0.5f32.to_le_bytes())
            .collect();
        store
            .write_dense_file(
                WriteDenseFile {
                    dataset_split_id: split,
                    kind,
                    path: PathBuf::from("data/off-batch").join(kind.file_name()),
                    rows: 24,
                    columns,
                    byte_count: 24 * columns * 4,
                },
                &content,
            )
            .unwrap();
    }
    store.finish_split(split).unwrap();
    split
}

/// training-run-failed-and-vulkan-refused: a Vulkan run under a pin without a shipped Vulkan
/// backend is refused; a CPU run exiting 1 is Failed and takes no second exit status.
#[test]
fn training_run_failed_and_vulkan_refused() {
    let dir = scratch("failed");
    // 12:30:00 PinModelSpec -> pinned.
    let pin = pin();
    // 12:30:01 PlanTrainingRun Vulkan -> backend-not-shipped. `plan` takes no binary, so nothing
    // can be spawned for it.
    match TrainingRun::plan(Backend::Vulkan, &pin) {
        Err(TrainingError::BackendNotShipped {
            backend,
            model_spec_version,
        }) => {
            assert_eq!(backend, Backend::Vulkan);
            assert_eq!(model_spec_version, pin.version());
        }
        other => panic!("{other:?}"),
    }
    // 12:30:02 RegisterModelSource, 12:30:03 CheckModelSource -> checked.
    let source = checked_source();
    // 12:30:04 GenerateDataset .. 12:30:08 FinishSplit -> the train split is Written.
    let mut generated = generate(&dir);
    let train = generated.split_id(SplitRole::Train);
    write(&mut generated, SplitRole::Train, &dir);
    // 12:30:09 PlanTrainingRun Cpu -> planned; 12:30:10 BindModelSource -> bound.
    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    run.bind(&source.trainable().unwrap()).unwrap();
    // 12:30:11 StartTrainingRun -> started.
    run.start(
        &generated.store,
        train,
        None,
        &open("fails"),
        STEPS,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    assert_eq!(run.state(), TrainingRunState::Running);
    // 12:30:12 FinishTrainingRun exit_status 1 -> failed.
    assert_eq!(run.finish().unwrap(), TrainingRunState::Failed);
    assert_eq!(run.exit_status(), Some(1));
    assert_eq!(run.failure(), Some(&TrainingFailure::Exit { code: 1 }));
    assert!(run.stderr().contains("E-DATA-008"), "{}", run.stderr());
    assert!(run.losses().is_empty());
    // 12:30:13 FinishTrainingRun exit_status 0 -> wrong-state, TrainingRunFinished.
    match run.finish() {
        Err(TrainingError::TrainingRunFinished { state }) => {
            assert_eq!(state, TrainingRunState::Failed)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(run.state(), TrainingRunState::Failed);
    assert_eq!(run.exit_status(), Some(1));
}

/// Beyond the scenarios: a run on a written eval split passes `--eval <dir>` after `--data` and
/// records the evaluation loss.
#[test]
fn a_run_with_an_eval_split_records_the_evaluation_loss() {
    let dir = scratch("eval");
    let source = checked_source();
    let mut generated = generate(&dir);
    write(&mut generated, SplitRole::Train, &dir);
    write(&mut generated, SplitRole::Validation, &dir);
    let mut run = TrainingRun::plan(Backend::Cpu, &pin()).unwrap();
    run.bind(&source.trainable().unwrap()).unwrap();
    run.start(
        &generated.store,
        generated.split_id(SplitRole::Train),
        Some(generated.split_id(SplitRole::Validation)),
        &open("trains-quietly"),
        STEPS,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    assert_eq!(run.eval_role(), Some(SplitRole::Validation));
    assert_eq!(run.finish().unwrap(), TrainingRunState::Completed);
    assert_eq!(run.losses(), [(0, 0.25), (4, 0.125)]);
    assert_eq!(run.eval_loss(), Some(0.0625));
    assert_eq!(run.output_error(), None);
}

/// Beyond the scenarios: the exit status alone decides the state (`FinishTrainingRun`, Da8-40).
/// A run whose stdout holds no `step` line and a run given `--eval` that printed no `eval` line
/// exit 0 and are Completed, with the output's defect recorded beside the state; a run past its
/// timeout has no exit status 0 and is Failed with its named reason.
#[test]
fn exit_zero_completes_whatever_the_output_and_a_timeout_fails() {
    let dir = scratch("output");
    let source = checked_source();
    let mut generated = generate(&dir);
    write(&mut generated, SplitRole::Train, &dir);
    write(&mut generated, SplitRole::Validation, &dir);
    let train = generated.split_id(SplitRole::Train);
    let validation = generated.split_id(SplitRole::Validation);
    let finished = |standin: &str,
                    eval: Option<DatasetSplitId>,
                    timeout: Duration,
                    reached: TrainingRunState| {
        let mut run = TrainingRun::plan(Backend::Cpu, &pin()).unwrap();
        run.bind(&source.trainable().unwrap()).unwrap();
        run.start(
            &generated.store,
            train,
            eval,
            &open(standin),
            STEPS,
            timeout,
        )
        .unwrap();
        assert_eq!(run.state(), TrainingRunState::Running);
        assert_eq!(run.finish().unwrap(), reached);
        run
    };

    let run = finished(
        "silent",
        None,
        DEFAULT_TRAINING_TIMEOUT,
        TrainingRunState::Completed,
    );
    assert_eq!(run.exit_status(), Some(0));
    assert_eq!(run.failure(), None);
    assert_eq!(run.output_error(), Some(&OutputError::NoStepLine));
    assert!(run.losses().is_empty());
    assert_eq!(run.eval_loss(), None);

    let run = finished(
        "no-eval-line",
        Some(validation),
        DEFAULT_TRAINING_TIMEOUT,
        TrainingRunState::Completed,
    );
    assert_eq!(run.exit_status(), Some(0));
    assert_eq!(run.failure(), None);
    assert_eq!(run.output_error(), Some(&OutputError::NoEvalLine));
    assert_eq!(run.losses(), [(0, 0.25), (4, 0.125)]);
    assert_eq!(run.eval_loss(), None);

    let timeout = Duration::from_millis(300);
    let run = finished("sleeps", None, timeout, TrainingRunState::Failed);
    assert_eq!(run.exit_status(), None);
    assert_eq!(run.failure(), Some(&TrainingFailure::TimedOut { timeout }));
    assert_eq!(run.output_error(), None);
}

/// The stdout parser on fixed strings: the `step` lines in order and the `eval` line, whose
/// `accuracy` is read and dropped; the `init` and `param` lines are not read.
#[test]
fn the_parser_reads_the_step_lines_and_the_eval_line() {
    let stdout = "init @PatchRegressor.w1: Tensor<f32, [32, 64]> = [0.1, 0.2]\n\
                  step 0 loss 0.31622776\n\
                  step 50 loss 0.25\n\
                  step 120 loss 1e-3\n\
                  param @PatchRegressor.w1: Tensor<f32, [32, 64]> = [0.1, 0.2]\n\
                  eval loss=0.0123 accuracy=0.5\n";
    let curve = LossCurve::parse(stdout).unwrap();
    assert_eq!(curve.losses, [(0, 0.31622776), (50, 0.25), (120, 0.001)]);
    assert_eq!(curve.eval_loss, Some(0.0123));

    let curve = LossCurve::parse("step 0 loss 0\nstep 4 loss 0\n").unwrap();
    assert_eq!(curve.losses, [(0, 0.0), (4, 0.0)]);
    assert_eq!(curve.eval_loss, None);
}

#[test]
fn the_parser_refuses_a_missing_step_line_and_a_malformed_number() {
    assert_eq!(LossCurve::parse(""), Err(OutputError::NoStepLine));
    assert_eq!(
        LossCurve::parse(
            "param @PatchRegressor.w1: Tensor<f32, [32, 64]> = [0]\neval loss=1 accuracy=0\n"
        ),
        Err(OutputError::NoStepLine)
    );
    for (stdout, line, text) in [
        ("step 0 loss 0.5\nstep 4 loss abc\n", 2, "step 4 loss abc"),
        ("step x loss 0.5\n", 1, "step x loss 0.5"),
        ("step -1 loss 0.5\n", 1, "step -1 loss 0.5"),
        ("step 0 loss 0.5 extra\n", 1, "step 0 loss 0.5 extra"),
        ("step 0 loss\n", 1, "step 0 loss"),
        (
            "step 0 loss 0.5\neval loss=0.x accuracy=1\n",
            2,
            "eval loss=0.x accuracy=1",
        ),
        (
            "step 0 loss 0.5\neval loss=0.5 accuracy=?\n",
            2,
            "eval loss=0.5 accuracy=?",
        ),
        ("step 0 loss 0.5\neval loss=0.5\n", 2, "eval loss=0.5"),
        (
            "step 0 loss 0.5\neval loss=0.5 accuracy=1\neval loss=0.4 accuracy=1\n",
            3,
            "eval loss=0.4 accuracy=1",
        ),
    ] {
        assert_eq!(
            LossCurve::parse(stdout),
            Err(OutputError::Malformed {
                line,
                text: text.to_owned()
            }),
            "{stdout:?}"
        );
    }
}
