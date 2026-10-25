//! Adversary cases for story:training-run, pass 2: the corrections Da8-39 (bind order),
//! Da8-40 (finish by exit status alone), Da8-41 (no deadline past `Instant`'s range) and Da8-42
//! (step numbers increase), each over the inputs pass 1 did not reach.
//!
//! The splits are recorded in memory through `WriteDenseFile` and `FinishSplit`; the binary is a
//! stand-in shell script, so no case here needs the pinned binary or a generation.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use patchback_dataset::{
    DatasetSplitId, DatasetStore, DenseFileKind, GenerateDataset, Pipeline, PrngAlgorithm,
    SplitRole, WriteDenseFile,
};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, LossCurve, ModelNotChecked, ModelSource, ModelSourceState,
    ModelSpec, ModelSpecPin, OutputError, TrainingError, TrainingFailure, TrainingRun,
    TrainingRunState,
};

fn pin() -> ModelSpecPin {
    ModelSpecPin::repository().unwrap()
}

fn other_pin() -> ModelSpecPin {
    ModelSpecPin::parse(&format!(
        "version 0.4.0\ncommit {}\nsha256-x86_64-unknown-linux-gnu {}\n",
        "a".repeat(40),
        "b".repeat(64)
    ))
    .unwrap()
}

fn design_model() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec")
}

/// Every stand-in, written before anything in this binary spawns.
fn standins() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir =
            Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-adversary-training-2");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let version = |v: &str| {
            format!("if [ \"$1\" = --version ]; then echo \"modelspec {v}\"; exit 0; fi")
        };
        let ours = format!(
            "{}\nif [ \"$1\" = check ]; then exit 0; fi",
            version(pin().version())
        );
        let scripts = [
            ("trains", format!("{ours}\necho \"step 0 loss 0.25\"\necho \"step 4 loss 0.125\"")),
            (
                "accepts-other",
                format!("{}\nif [ \"$1\" = check ]; then exit 0; fi", version("0.4.0")),
            ),
            (
                "rejects-other",
                format!(
                    "{}\necho \"E-TYPE-001: matmul shapes do not agree\" >&2\nexit 1",
                    version("0.4.0")
                ),
            ),
            (
                "malformed",
                format!("{ours}\necho \"step 0 loss 0.5\"\necho \"step 4 loss abc\""),
            ),
            (
                "repeated",
                format!("{ours}\necho \"step 0 loss 0.5\"\necho \"step 0 loss 0.4\""),
            ),
            (
                "eval-nan",
                format!(
                    "{ours}\necho \"step 0 loss 0.5\"\necho \"step 4 loss 0.25\"\necho \"eval loss=nan accuracy=0\""
                ),
            ),
            ("eval-only", format!("{ours}\necho \"eval loss=0.5 accuracy=0\"")),
            (
                "curve-then-exit-1",
                format!(
                    "{ours}\necho \"step 0 loss 0.5\"\necho \"step 4 loss 0.25\"\necho \"E-RUN: late\" >&2\nexit 1"
                ),
            ),
            (
                "curve-then-signal",
                format!("{ours}\necho \"step 0 loss 0.5\"\nkill -9 $$"),
            ),
            (
                "slow",
                format!("{ours}\nsleep 1\necho \"step 0 loss 0.25\"\necho \"step 4 loss 0.125\""),
            ),
        ];
        for (name, body) in scripts {
            let path = dir.join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir
    })
}

fn open(name: &str) -> ModelSpec {
    ModelSpec::open(&standins().join(name), &pin()).unwrap()
}

fn checked_source() -> ModelSource {
    let mut source = ModelSource::register(design_model(), "PatchRegressor", 16, &pin());
    assert_eq!(
        source.check(&open("trains")).unwrap(),
        ModelSourceState::Checked
    );
    source
}

fn split(store: &mut DatasetStore, role: SplitRole, dir: &Path) -> DatasetSplitId {
    let dataset = store.generate_dataset(GenerateDataset {
        seed: 1,
        prng_algorithm: PrngAlgorithm("xoshiro256**".to_owned()),
        batch_size: 16,
        feature_contract_id: Pipeline::new().feature_contract_id(),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    });
    let split = store.open_split(dataset, role, 16).unwrap();
    for (kind, columns) in [(DenseFileKind::X, 32), (DenseFileKind::Y, 8)] {
        let content: Vec<u8> = (0..16 * columns)
            .flat_map(|_| 0.5f32.to_le_bytes())
            .collect();
        store
            .write_dense_file(
                WriteDenseFile {
                    dataset_split_id: split,
                    kind,
                    path: dir.join(kind.file_name()),
                    rows: 16,
                    columns,
                    byte_count: 16 * columns * 4,
                },
                &content,
            )
            .unwrap();
    }
    store.finish_split(split).unwrap();
    split
}

/// A store with a Written train and validation split, and the run bound to `source` and finished
/// by the stand-in `standin`, given `--eval` when `eval`.
fn finished(standin: &str, eval: bool, timeout: Duration) -> (TrainingRun, TrainingRunState) {
    let root = standins().join("splits").join(standin);
    let mut store = DatasetStore::new();
    let train = split(&mut store, SplitRole::Train, &root.join("train"));
    let validation = split(&mut store, SplitRole::Validation, &root.join("validation"));
    let mut run = TrainingRun::plan(Backend::Cpu, &pin()).unwrap();
    run.bind(&checked_source()).unwrap();
    run.start(
        &store,
        train,
        eval.then_some(validation),
        &open(standin),
        4,
        timeout,
    )
    .unwrap();
    assert_eq!(run.state(), TrainingRunState::Running);
    let state = run.finish().unwrap();
    (run, state)
}

/// Da8-39 over the pairs pass 1 did not reach: a Planned run refuses a rejected source of another
/// pin `ModelNotChecked` (the source's state before its pin) and a checked source of another pin
/// `PinMismatch`, naming both pins; a Running, a Completed and a Failed run refuse even a checked
/// source of the run's own pin `TrainingRunPastPlanning`; each refusal leaves the run unchanged.
#[test]
fn adversary2_bind_refuses_in_the_interpreters_order_for_every_pair() {
    let source = checked_source();
    let mut rejected_elsewhere =
        ModelSource::register(design_model(), "PatchRegressor", 16, &other_pin());
    let rejects = ModelSpec::open(&standins().join("rejects-other"), &other_pin()).unwrap();
    assert_eq!(
        rejected_elsewhere.check(&rejects).unwrap(),
        ModelSourceState::Rejected
    );
    let mut checked_elsewhere =
        ModelSource::register(design_model(), "PatchRegressor", 16, &other_pin());
    let accepts = ModelSpec::open(&standins().join("accepts-other"), &other_pin()).unwrap();
    assert_eq!(
        checked_elsewhere.check(&accepts).unwrap(),
        ModelSourceState::Checked
    );

    let mut planned = TrainingRun::plan(Backend::Cpu, &pin()).unwrap();
    let before = planned.clone();
    match planned.bind(&rejected_elsewhere) {
        Err(TrainingError::ModelNotChecked(ModelNotChecked { state })) => {
            assert_eq!(state, ModelSourceState::Rejected)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(planned, before);
    match planned.bind(&checked_elsewhere) {
        Err(TrainingError::PinMismatch { run_pin, model_pin }) => {
            assert_eq!(*run_pin, pin());
            assert_eq!(*model_pin, other_pin());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(planned, before);

    let root = standins().join("splits").join("bind-matrix");
    let mut store = DatasetStore::new();
    let train = split(&mut store, SplitRole::Train, &root.join("train"));
    for (standin, reached) in [
        ("trains", TrainingRunState::Completed),
        ("curve-then-exit-1", TrainingRunState::Failed),
    ] {
        let mut run = TrainingRun::plan(Backend::Cpu, &pin()).unwrap();
        run.bind(&source).unwrap();
        run.start(
            &store,
            train,
            None,
            &open(standin),
            4,
            DEFAULT_TRAINING_TIMEOUT,
        )
        .unwrap();
        let running = run.clone();
        assert!(matches!(
            run.bind(&source),
            Err(TrainingError::TrainingRunPastPlanning {
                state: TrainingRunState::Running
            })
        ));
        assert_eq!(run, running);
        assert_eq!(run.finish().unwrap(), reached);
        let ended = run.clone();
        match run.bind(&source) {
            Err(TrainingError::TrainingRunPastPlanning { state }) => assert_eq!(state, reached),
            other => panic!("{other:?}"),
        }
        assert_eq!(run, ended);
    }
}

/// `bind(&source)` and `bind(&source.trainable()?)` bind the same run, and refuse a bound run
/// alike.
#[test]
fn adversary2_bind_of_the_source_and_of_its_checked_model_agree() {
    let source = checked_source();
    let mut by_source = TrainingRun::plan(Backend::Cpu, &pin()).unwrap();
    let mut by_model = by_source.clone();
    by_source.bind(&source).unwrap();
    by_model.bind(&source.trainable().unwrap()).unwrap();
    assert_eq!(by_source, by_model);
    assert_eq!(by_source.model_source(), Some(design_model().as_path()));
    let a = by_source.bind(&source).unwrap_err().domain_error();
    let b = by_model
        .bind(&source.trainable().unwrap())
        .unwrap_err()
        .domain_error();
    assert_eq!(a, Some("patchback.model.TrainingRunPastPlanning"));
    assert_eq!(a, b);
}

/// Da8-40, exit 0 with a malformed line: Completed, the line named, and no partial curve.
#[test]
fn adversary2_exit_zero_with_a_malformed_line_completes_without_a_curve() {
    let (run, state) = finished("malformed", false, DEFAULT_TRAINING_TIMEOUT);
    assert_eq!(state, TrainingRunState::Completed);
    assert_eq!(run.exit_status(), Some(0));
    assert_eq!(run.failure(), None);
    assert_eq!(
        run.output_error(),
        Some(&OutputError::Malformed {
            line: 2,
            text: "step 4 loss abc".to_owned()
        })
    );
    assert!(run.losses().is_empty(), "{:?}", run.losses());
    assert_eq!(run.eval_loss(), None);
    assert!(run.stdout().contains("step 0 loss 0.5"));
}

/// Da8-42 through `finish`: a repeated step number is malformed, named by its line.
#[test]
fn adversary2_exit_zero_with_a_repeated_step_completes_with_the_line_named() {
    let (run, state) = finished("repeated", false, DEFAULT_TRAINING_TIMEOUT);
    assert_eq!(state, TrainingRunState::Completed);
    assert_eq!(
        run.output_error(),
        Some(&OutputError::Malformed {
            line: 2,
            text: "step 0 loss 0.4".to_owned()
        })
    );
    assert!(run.losses().is_empty());
}

/// Exit 0 with `eval loss=nan`: Completed, no output error (the line is well formed), the NaN kept.
#[test]
fn adversary2_exit_zero_with_a_nan_eval_loss_completes_and_keeps_it() {
    let (run, state) = finished("eval-nan", true, DEFAULT_TRAINING_TIMEOUT);
    assert_eq!(state, TrainingRunState::Completed);
    assert_eq!(run.output_error(), None);
    assert_eq!(run.losses(), [(0, 0.5), (4, 0.25)]);
    assert!(run.eval_loss().unwrap().is_nan());
}

/// Exit 0 with only an `eval` line: Completed, `NoStepLine`, and neither curve nor eval loss.
#[test]
fn adversary2_exit_zero_with_only_an_eval_line_completes_without_a_curve() {
    let (run, state) = finished("eval-only", true, DEFAULT_TRAINING_TIMEOUT);
    assert_eq!(state, TrainingRunState::Completed);
    assert_eq!(run.output_error(), Some(&OutputError::NoStepLine));
    assert!(run.losses().is_empty());
    assert_eq!(run.eval_loss(), None);
}

/// A non-zero exit after a clean curve: Failed with its status; the curve is not read (only a
/// run that exited 0 is parsed), the stdout and stderr are kept.
#[test]
fn adversary2_a_non_zero_exit_after_a_clean_curve_fails_and_reads_no_curve() {
    let (run, state) = finished("curve-then-exit-1", false, DEFAULT_TRAINING_TIMEOUT);
    assert_eq!(state, TrainingRunState::Failed);
    assert_eq!(run.exit_status(), Some(1));
    assert_eq!(run.failure(), Some(&TrainingFailure::Exit { code: 1 }));
    assert_eq!(run.output_error(), None);
    assert!(run.losses().is_empty());
    assert!(run.stdout().contains("step 4 loss 0.25"));
    assert!(run.stderr().contains("E-RUN: late"));
}

/// A signal after a step line: Failed, no exit status, no output error, no curve.
#[test]
fn adversary2_a_signal_after_a_step_line_fails_without_an_exit_status() {
    let (run, state) = finished("curve-then-signal", false, DEFAULT_TRAINING_TIMEOUT);
    assert_eq!(state, TrainingRunState::Failed);
    assert_eq!(run.exit_status(), None);
    assert_eq!(run.failure(), Some(&TrainingFailure::Signalled));
    assert_eq!(run.output_error(), None);
    assert!(run.losses().is_empty());
}

/// Da8-41: `Duration::MAX` sets no deadline, so a child that takes 1 s completes.
#[test]
fn adversary2_an_unbounded_timeout_waits_for_a_slow_child() {
    let started = Instant::now();
    let (run, state) = finished("slow", false, Duration::MAX);
    assert!(started.elapsed() >= Duration::from_secs(1));
    assert_eq!(state, TrainingRunState::Completed, "{:?}", run.failure());
    assert_eq!(run.losses(), [(0, 0.25), (4, 0.125)]);
    assert_eq!(run.output_error(), None);
}

/// Da8-42 on fixed strings: a repeated step number is malformed and named; a gap (0, 4), as the
/// pinned binary prints for `--steps 4`, is not.
#[test]
fn adversary2_the_parser_refuses_a_repeated_step_and_accepts_a_gap() {
    assert_eq!(
        LossCurve::parse("step 0 loss 1\nstep 4 loss 0.5\nstep 4 loss 0.5\n"),
        Err(OutputError::Malformed {
            line: 3,
            text: "step 4 loss 0.5".to_owned()
        })
    );
    assert_eq!(
        LossCurve::parse("step 0 loss 1.0717294\nstep 4 loss 0.8882302\neval loss=0.9350907802581787 accuracy=0.0625\n")
            .unwrap(),
        LossCurve {
            losses: vec![(0, 1.0717294), (4, 0.8882302)],
            eval_loss: Some(0.9350907802581787),
        }
    );
}
