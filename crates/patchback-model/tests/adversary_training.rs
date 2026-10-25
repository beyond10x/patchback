//! Adversary cases for story:training-run (wave a8, pass 1): the command order the domain's
//! interpreter answers, the domain's FinishTrainingRun outcome, the argv with `--eval`, the
//! refusals that leave the run as it was, the timeout bounds, both pipes, and the stdout parser.
//!
//! The splits are recorded in memory through `WriteDenseFile` and `FinishSplit`, as
//! `tests/domain.rs` records them; the binary is a stand-in shell script, so no case here needs
//! the pinned binary or a generation.

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
    Backend, DEFAULT_TRAINING_TIMEOUT, LossCurve, ModelSource, ModelSpec, ModelSpecError,
    ModelSpecPin, OutputError, TrainingError, TrainingFailure, TrainingRun, TrainingRunState,
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

/// Every stand-in, written before anything in this binary spawns (no inherited write descriptor).
fn standins() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir =
            Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-adversary-training");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let header = |version: &str| {
            format!(
                "if [ \"$1\" = --version ]; then echo \"modelspec {version}\"; exit 0; fi\nif [ \"$1\" = check ]; then exit 0; fi"
            )
        };
        let ours = header(pin().version());
        let scripts = [
            (
                "argv",
                format!(
                    "{ours}\nprintf '%s\\n' \"$@\" > {}\necho \"step 0 loss 0.25\"\necho \"eval loss=0.5 accuracy=0\"",
                    dir.join("argv.args").display()
                ),
            ),
            (
                "trains",
                format!("{ours}\necho \"step 0 loss 0.25\"\necho \"step 4 loss 0.125\""),
            ),
            (
                "records",
                format!(
                    "{ours}\ntouch {}\necho \"step 0 loss 0.25\"",
                    dir.join("records-was-asked-to-train").display()
                ),
            ),
            (
                "other",
                format!(
                    "{}\ntouch {}\necho \"step 0 loss 0.25\"",
                    header("0.4.0"),
                    dir.join("other-was-asked-to-train").display()
                ),
            ),
            ("unspawnable", format!("{ours}\necho \"step 0 loss 0.25\"")),
            ("sleeps", format!("{ours}\nexec sleep 30")),
            ("signalled", format!("{ours}\nkill -9 $$")),
            (
                "loud",
                format!(
                    "{ours}\necho \"step 0 loss 0.25\"\nhead -c 1048576 /dev/zero | tr '\\000' a\necho\nhead -c 1048576 /dev/zero | tr '\\000' b >&2\necho \"step 4 loss 0.125\""
                ),
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
    source.check(&open("trains")).unwrap();
    source
}

/// The splits of one store: `open` (Open, 16), `train` and `validation` (Written, 16, under
/// `<root>/train` and `<root>/validation`), `off_batch` (Written, 24 of a batch-8 dataset).
struct Splits {
    store: DatasetStore,
    open: DatasetSplitId,
    train: DatasetSplitId,
    validation: DatasetSplitId,
    off_batch: DatasetSplitId,
    root: PathBuf,
}

fn split(
    store: &mut DatasetStore,
    batch_size: u64,
    role: SplitRole,
    count: i64,
    dir: Option<&Path>,
) -> DatasetSplitId {
    let dataset = store.generate_dataset(GenerateDataset {
        seed: 1,
        prng_algorithm: PrngAlgorithm("xoshiro256**".to_owned()),
        batch_size,
        feature_contract_id: Pipeline::new().feature_contract_id(),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    });
    let split = store.open_split(dataset, role, count).unwrap();
    if let Some(dir) = dir {
        for (kind, columns) in [(DenseFileKind::X, 32), (DenseFileKind::Y, 8)] {
            let content: Vec<u8> = (0..count * columns)
                .flat_map(|_| 0.5f32.to_le_bytes())
                .collect();
            store
                .write_dense_file(
                    WriteDenseFile {
                        dataset_split_id: split,
                        kind,
                        path: dir.join(kind.file_name()),
                        rows: count,
                        columns,
                        byte_count: count * columns * 4,
                    },
                    &content,
                )
                .unwrap();
        }
        store.finish_split(split).unwrap();
    }
    split
}

fn splits(test: &str) -> Splits {
    let root = standins().join("splits").join(test);
    let mut store = DatasetStore::new();
    let open = split(&mut store, 16, SplitRole::Train, 16, None);
    let train = split(
        &mut store,
        16,
        SplitRole::Train,
        16,
        Some(&root.join("train")),
    );
    let validation = split(
        &mut store,
        16,
        SplitRole::Validation,
        16,
        Some(&root.join("validation")),
    );
    let off_batch = split(
        &mut store,
        8,
        SplitRole::Validation,
        24,
        Some(&root.join("off-batch")),
    );
    Splits {
        store,
        open,
        train,
        validation,
        off_batch,
        root,
    }
}

fn bound_run(source: &ModelSource) -> TrainingRun {
    let mut run = TrainingRun::plan(Backend::Cpu, &pin()).unwrap();
    run.bind(&source.trainable().unwrap()).unwrap();
    run
}

/// spec/domains/model.yaml `BindModelSource`: `wrong-state` (TrainingRunPastPlanning) is a
/// held-state refusal and `model-not-checked` a related-row predicate refusal, and the domain's
/// interpreter answers the first: a scenario binding an unchecked source to a ModelBound run
/// with `outcome: wrong-state` passes `ess verify conform run --target interpreted` (ess 0.68.0).
/// The route the module documents (`bind(&source.trainable()?)`) answers ModelNotChecked.
#[test]
fn adversary_bind_on_a_bound_run_with_an_unchecked_source_answers_past_planning() {
    let source = checked_source();
    let unchecked = ModelSource::register(design_model(), "PatchRegressor", 16, &pin());
    let mut run = bound_run(&source);
    let before = run.clone();
    let result = run.bind(&unchecked);
    match result {
        Err(TrainingError::TrainingRunPastPlanning { state }) => {
            assert_eq!(state, TrainingRunState::ModelBound)
        }
        other => panic!("the domain answers TrainingRunPastPlanning first; got {other:?}"),
    }
    assert_eq!(run, before);
}

/// spec/domains/model.yaml `FinishTrainingRun`: `failed` is `when: exit_status != 0`, and every
/// other status is `completed` ("Record the subprocess exit status; 0 completes the run"). A run
/// given `--eval` whose child exits 0 with its `step` lines and no `eval` line is a Failed run with
/// exit status 0, an outcome the domain does not declare.
#[test]
fn adversary_exit_zero_with_eval_and_no_eval_line_is_what_the_domain_declares() {
    let s = splits("no-eval-line");
    let mut run = bound_run(&checked_source());
    run.start(
        &s.store,
        s.train,
        Some(s.validation),
        &open("trains"),
        4,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    let state = run.finish().unwrap();
    assert_eq!(run.exit_status(), Some(0));
    assert_eq!(
        state,
        TrainingRunState::Completed,
        "exit status 0 is `completed` in FinishTrainingRun; failure {:?}",
        run.failure()
    );
}

/// The argv with `--eval`: the eval split's own directory after `--data`, then `--steps 0`
/// passed through as given. No other case asserts the `--eval` argument.
#[test]
fn adversary_start_passes_the_eval_split_directory_and_steps_zero() {
    let s = splits("argv");
    let mut run = bound_run(&checked_source());
    run.start(
        &s.store,
        s.train,
        Some(s.validation),
        &open("argv"),
        0,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    let args = fs::read_to_string(standins().join("argv.args")).unwrap();
    let expected = [
        "train".to_owned(),
        design_model().display().to_string(),
        "--backend".to_owned(),
        "cpu".to_owned(),
        "--data".to_owned(),
        s.root.join("train").display().to_string(),
        "--eval".to_owned(),
        s.root.join("validation").display().to_string(),
        "--steps".to_owned(),
        "0".to_owned(),
    ];
    assert_eq!(args.lines().collect::<Vec<_>>(), expected);
    assert_eq!(run.eval_split_id(), Some(s.validation));
    assert_eq!(run.finish().unwrap(), TrainingRunState::Completed);
    assert_eq!(run.losses(), [(0, 0.25)]);
    assert_eq!(run.eval_loss(), Some(0.5));
}

/// `start` with a binary opened under another pin than the run's is refused `OtherPin` before
/// anything is spawned, and the run is unchanged. No other case reaches this branch.
#[test]
fn adversary_start_refuses_a_binary_of_another_pin_and_spawns_nothing() {
    let s = splits("other-pin");
    let mut run = bound_run(&checked_source());
    let before = run.clone();
    let other = ModelSpec::open(&standins().join("other"), &other_pin()).unwrap();
    match run.start(&s.store, s.train, None, &other, 4, DEFAULT_TRAINING_TIMEOUT) {
        Err(error @ TrainingError::OtherPin { .. }) => {
            assert_eq!(error.domain_error(), None);
            let TrainingError::OtherPin { run: ours, binary } = error else {
                unreachable!()
            };
            assert_eq!(ours, pin().version());
            assert_eq!(binary, "0.4.0");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(run, before);
    assert!(!standins().join("other-was-asked-to-train").exists());
}

/// A binary that was opened and then cannot be started leaves the run ModelBound and unchanged,
/// with the spawn error named. No other case reaches this branch.
#[test]
fn adversary_a_binary_that_cannot_be_spawned_leaves_the_run_model_bound() {
    let s = splits("unspawnable");
    let mut run = bound_run(&checked_source());
    let modelspec = open("unspawnable");
    let path = standins().join("unspawnable");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let before = run.clone();
    match run.start(
        &s.store,
        s.train,
        None,
        &modelspec,
        4,
        DEFAULT_TRAINING_TIMEOUT,
    ) {
        Err(error @ TrainingError::ModelSpec(ModelSpecError::Spawn { .. })) => {
            assert_eq!(error.domain_error(), None);
            assert!(error.to_string().contains("unspawnable"), "{error}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(run, before);
    assert_eq!(run.state(), TrainingRunState::ModelBound);
}

/// Every refusal of every command leaves the value as it was (compared whole, before and after).
#[test]
fn adversary_every_refusal_leaves_the_run_unchanged() {
    let s = splits("unchanged");
    let modelspec = open("records");
    let source = checked_source();

    let mut planned = TrainingRun::plan(Backend::Cpu, &pin()).unwrap();
    let before = planned.clone();
    assert!(matches!(
        planned.start(
            &s.store,
            s.train,
            None,
            &modelspec,
            4,
            DEFAULT_TRAINING_TIMEOUT
        ),
        Err(TrainingError::TrainingRunNotReady {
            state: TrainingRunState::Planned
        })
    ));
    assert!(matches!(
        planned.finish(),
        Err(TrainingError::TrainingRunFinished {
            state: TrainingRunState::Planned
        })
    ));
    let mut elsewhere = ModelSource::register(design_model(), "PatchRegressor", 16, &other_pin());
    elsewhere
        .check(&ModelSpec::open(&standins().join("other"), &other_pin()).unwrap())
        .unwrap();
    assert!(matches!(
        planned.bind(&elsewhere.trainable().unwrap()),
        Err(TrainingError::PinMismatch { .. })
    ));
    assert_eq!(planned, before);

    let mut run = bound_run(&source);
    let before = run.clone();
    let cases: [(DatasetSplitId, Option<DatasetSplitId>, &str); 6] = [
        (DatasetSplitId(0), None, "patchback.model.UnknownDataSplit"),
        (
            DatasetSplitId(0xff),
            Some(s.validation),
            "patchback.model.UnknownDataSplit",
        ),
        (
            s.open,
            Some(DatasetSplitId(0xff)),
            "patchback.model.DataNotWritten",
        ),
        (
            s.train,
            Some(DatasetSplitId(0xff)),
            "patchback.model.EvalNotWritten",
        ),
        (s.train, Some(s.open), "patchback.model.EvalNotWritten"),
        (
            s.train,
            Some(s.off_batch),
            "patchback.model.EvalCountNotBatchMultiple",
        ),
    ];
    for (data, eval, expected) in cases {
        let error = run
            .start(
                &s.store,
                data,
                eval,
                &modelspec,
                4,
                DEFAULT_TRAINING_TIMEOUT,
            )
            .unwrap_err();
        assert_eq!(
            error.domain_error(),
            Some(expected),
            "{data:?} {eval:?}: {error:?}"
        );
        assert_eq!(run, before, "{expected}");
    }
    assert!(matches!(
        run.finish(),
        Err(TrainingError::TrainingRunFinished {
            state: TrainingRunState::ModelBound
        })
    ));
    assert!(matches!(
        run.bind(&source.trainable().unwrap()),
        Err(TrainingError::TrainingRunPastPlanning {
            state: TrainingRunState::ModelBound
        })
    ));
    assert_eq!(run, before);
    assert!(!standins().join("records-was-asked-to-train").exists());
}

/// A zero timeout kills the child at once: the run is Failed `TimedOut`, well inside the stand-in's
/// 30 s sleep.
#[test]
fn adversary_a_zero_timeout_fails_the_run_at_once() {
    let s = splits("zero-timeout");
    let mut run = bound_run(&checked_source());
    let started = Instant::now();
    run.start(&s.store, s.train, None, &open("sleeps"), 4, Duration::ZERO)
        .unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(run.finish().unwrap(), TrainingRunState::Failed);
    assert_eq!(
        run.failure(),
        Some(&TrainingFailure::TimedOut {
            timeout: Duration::ZERO
        })
    );
    assert_eq!(run.exit_status(), None);
}

/// A caller that wants no practical bound passes `Duration::MAX`; `start` must run the child or
/// refuse, not panic.
#[test]
fn adversary_an_unbounded_timeout_does_not_panic() {
    let s = splits("max-timeout");
    let mut run = bound_run(&checked_source());
    let result = run.start(&s.store, s.train, None, &open("trains"), 4, Duration::MAX);
    if result.is_ok() {
        assert_eq!(run.finish().unwrap(), TrainingRunState::Completed);
    }
}

/// A child killed by a signal is a Failed run without an exit status.
#[test]
fn adversary_a_child_killed_by_a_signal_fails_without_an_exit_status() {
    let s = splits("signalled");
    let mut run = bound_run(&checked_source());
    run.start(
        &s.store,
        s.train,
        None,
        &open("signalled"),
        4,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    assert_eq!(run.finish().unwrap(), TrainingRunState::Failed);
    assert_eq!(run.failure(), Some(&TrainingFailure::Signalled));
    assert_eq!(run.exit_status(), None);
}

/// One MiB on stdout and one on stderr, with a `step` line after both: no deadlock, both kept.
#[test]
fn adversary_a_megabyte_on_each_pipe_completes() {
    let s = splits("loud");
    let mut run = bound_run(&checked_source());
    run.start(
        &s.store,
        s.train,
        None,
        &open("loud"),
        4,
        Duration::from_secs(30),
    )
    .unwrap();
    assert_eq!(
        run.finish().unwrap(),
        TrainingRunState::Completed,
        "{:?}",
        run.failure()
    );
    assert_eq!(run.losses(), [(0, 0.25), (4, 0.125)]);
    assert_eq!(run.stderr().len(), 1 << 20);
    assert!(run.stdout().len() > 1 << 20);
}

/// The number forms and line ends the parser meets, and the pinned binary's own lines (modelspec
/// 0.5.0, `train … --eval … --steps 0`: `step 0 loss 0`, `eval loss=0 accuracy=1`).
#[test]
fn adversary_the_parser_reads_every_number_form() {
    let curve = LossCurve::parse(
        "init @PatchRegressor.w1: Tensor<f32, [32, 64]> = [0.1]\r\nstep 0 loss 1e-3\r\nsteps 4 loss 9\r\nstep 1 loss -0.5\r\nstep 2 loss inf\r\nstep 3 loss 1.\r\nstep 4 loss nan\r\neval loss=nan accuracy=0\r\n",
    )
    .unwrap();
    assert_eq!(curve.losses.len(), 5);
    assert_eq!(
        &curve.losses[..4],
        [(0, 0.001), (1, -0.5), (2, f64::INFINITY), (3, 1.0)]
    );
    assert_eq!(curve.losses[4].0, 4);
    assert!(curve.losses[4].1.is_nan());
    assert!(curve.eval_loss.unwrap().is_nan());

    let curve = LossCurve::parse("step 0 loss 0\neval loss=0 accuracy=1\n").unwrap();
    assert_eq!(curve.losses, [(0, 0.0)]);
    assert_eq!(curve.eval_loss, Some(0.0));
}

/// `LossCurve` documents its `step` lines "in order (step 0, every 50th step and the last)", and
/// the § 12.2 check (Da8-27) reads the first and the last of them as the start and end of the
/// curve. Step numbers that go back are accepted as a curve.
#[test]
fn adversary_the_parser_refuses_step_numbers_that_go_back() {
    assert_eq!(
        LossCurve::parse("step 0 loss 2\nstep 4 loss 1\nstep 2 loss 3\n"),
        Err(OutputError::Malformed {
            line: 3,
            text: "step 2 loss 3".to_owned()
        })
    );
}
