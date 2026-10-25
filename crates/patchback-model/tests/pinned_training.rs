//! The real pinned ModelSpec binary trains `models/patch_regressor.mspec` on real generated data
//! (DESIGN.md § 8.1, § 8.2 step 1, § 12.2 "ModelSpec bridge").
//!
//! The binary is found as `pinned_binary.rs` finds it: `MODELSPEC`, then a `modelspec` on
//! `PATH`. When neither names one, the binary tests print a `SKIPPED` line and pass, unless
//! `PATCHBACK_REQUIRE_MODELSPEC` is set, as CI sets it: then a missing binary fails. The Vulkan
//! refusal needs no binary and always runs.
//!
//! The data is one generation of 64 / 16 / 16 samples, shared by the binary tests: the smallest
//! per-split counts that are multiples of the batch size 16 and admit `--steps 4` (4 × 16 = 64 <=
//! 64 training rows, so the 64-entry `order.u32` suffices). Training loss decreases (§ 12.2) is
//! held as the last `step` loss strictly below the first (Da8-27). Every split's `x.f32`
//! is z-scored under the training split's statistics, as `patchback generate-dataset` writes it
//! (Da8-30).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use patchback_dataset::{DatasetSplitId, Generated, Generator, SplitCounts, SplitRole};
use patchback_dataset::{dense, training_stats};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, ModelSource, ModelSourceState, ModelSpec, ModelSpecError,
    ModelSpecPin, TrainingError, TrainingFailure, TrainingRun, TrainingRunState, locate,
};

const TRAIN_ROWS: i64 = 64;
const STEPS: u32 = 4;

/// Written to the process's stderr, not through eprintln!, so the harness's output capture does
/// not hide it from the log.
fn say(line: &str) {
    let _ = writeln!(std::io::stderr(), "{line}");
}

/// The pinned binary, or `None` after a `SKIPPED` line when there is none and none is required.
fn binary(test: &str, pin: &ModelSpecPin) -> Option<PathBuf> {
    match locate(None) {
        Ok(binary) => Some(binary),
        Err(ModelSpecError::NotFound { searched }) => {
            assert!(
                std::env::var_os("PATCHBACK_REQUIRE_MODELSPEC").is_none(),
                "PATCHBACK_REQUIRE_MODELSPEC is set and no modelspec binary was found in {searched:?}"
            );
            say(&format!(
                "SKIPPED {test}: no modelspec {} binary (MODELSPEC unset, none of {} PATH entries has one)",
                pin.version(),
                searched.len()
            ));
            None
        }
        Err(other) => panic!("{other}"),
    }
}

/// The binary, the design model checked by it, and the written generation.
struct Fixture {
    pin: ModelSpecPin,
    modelspec: ModelSpec,
    source: ModelSource,
    generated: Generated,
}

impl Fixture {
    fn split(&self, role: SplitRole) -> DatasetSplitId {
        self.generated.split_id(role)
    }

    /// A run planned on the CPU and bound to the checked design model.
    fn bound_run(&self) -> TrainingRun {
        let mut run = TrainingRun::plan(Backend::Cpu, &self.pin).unwrap();
        run.bind(&self.source.trainable().unwrap()).unwrap();
        run
    }
}

/// The dense files of `role` in `<dir>/<role>`, `x.f32` z-scored under the training split's
/// statistics.
fn write(generated: &mut Generated, role: SplitRole, dir: &Path) {
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
}

/// The fixture for `test`, built once; `None` after a `SKIPPED` line when there is no binary.
fn fixture(test: &str) -> Option<&'static Fixture> {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    let pin = ModelSpecPin::repository().unwrap();
    let binary = binary(test, &pin)?;
    Some(FIXTURE.get_or_init(|| {
        let modelspec = ModelSpec::open(&binary, &pin).unwrap_or_else(|e| panic!("{e}"));
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec");
        let mut source = ModelSource::register(&path, "PatchRegressor", 16, &pin);
        assert_eq!(source.check(&modelspec).unwrap(), ModelSourceState::Checked);

        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-pinned-training");
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        fs::create_dir_all(&dir).unwrap();
        let started = Instant::now();
        let counts = SplitCounts {
            train: TRAIN_ROWS,
            validation: 16,
            test: 16,
        };
        let mut generated = Generator::new(42, counts, dir.clone()).run().unwrap();
        write(&mut generated, SplitRole::Train, &dir);
        write(&mut generated, SplitRole::Validation, &dir);
        let elapsed: Duration = started.elapsed();
        say(&format!(
            "pinned_training: generation and writing of {TRAIN_ROWS}/16/16 samples {elapsed:?}"
        ));
        Fixture {
            pin,
            modelspec,
            source,
            generated,
        }
    }))
}

/// (c) `Backend::Vulkan` is refused by `PlanTrainingRun`, which takes no binary: nothing is
/// spawned, with or without the pinned binary.
#[test]
fn vulkan_is_refused_before_anything_is_spawned() {
    let pin = ModelSpecPin::repository().unwrap();
    match TrainingRun::plan(Backend::Vulkan, &pin) {
        Err(error @ TrainingError::BackendNotShipped { .. }) => {
            assert_eq!(
                error.domain_error(),
                Some("patchback.model.BackendNotShipped")
            )
        }
        other => panic!("{other:?}"),
    }
}

/// (a) A CPU run of the checked design model on the real 64-row training split with `--steps 4`
/// and `--eval` on the 16-row validation split completes; its loss falls and it reports a finite
/// evaluation loss.
#[test]
fn the_pinned_modelspec_trains_on_real_generated_data() {
    let test = "the_pinned_modelspec_trains_on_real_generated_data";
    let Some(fixture) = fixture(test) else {
        return;
    };
    let mut run = fixture.bound_run();
    let started = Instant::now();
    run.start(
        &fixture.generated.store,
        fixture.split(SplitRole::Train),
        Some(fixture.split(SplitRole::Validation)),
        &fixture.modelspec,
        STEPS,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    let elapsed = started.elapsed();
    let state = run.finish().unwrap();
    say(&format!(
        "{test}: modelspec train --steps {STEPS} {elapsed:?}; {state:?}; losses {:?}; eval loss {:?}",
        run.losses(),
        run.eval_loss()
    ));
    for line in run.stdout().lines() {
        if line.starts_with("step ") || line.starts_with("eval ") {
            say(&format!("{test}: modelspec printed: {line}"));
        }
    }
    assert_eq!(
        state,
        TrainingRunState::Completed,
        "{:?}: {}",
        run.failure(),
        run.stderr()
    );
    assert_eq!(run.exit_status(), Some(0));
    assert_eq!(
        run.output_error(),
        None,
        "the loss curve (Da8-27) is absent"
    );
    let losses = run.losses();
    assert!(losses.len() >= 2, "{losses:?}");
    assert_eq!(losses.first().map(|(step, _)| *step), Some(0));
    assert_eq!(losses.last().map(|(step, _)| *step), Some(u64::from(STEPS)));
    let (first, last) = (losses[0].1, losses[losses.len() - 1].1);
    assert!(
        last < first,
        "the training loss did not fall: {first} -> {last}"
    );
    let eval_loss = run.eval_loss().expect("--eval prints an eval line");
    assert!(eval_loss.is_finite(), "{eval_loss}");
    assert!(run.stdout().contains("param @PatchRegressor.w1"));
    assert!(run.stdout().contains("param @PatchRegressor.w2"));
}

/// (b) The same data with `--steps 5` (5 × 16 = 80 > 64 entries of `order.u32`) ends Failed
/// with the binary's own refusal on stderr (Da8-24).
#[test]
fn the_pinned_modelspec_refuses_more_steps_than_the_order_file_holds() {
    let test = "the_pinned_modelspec_refuses_more_steps_than_the_order_file_holds";
    let Some(fixture) = fixture(test) else {
        return;
    };
    let mut run = fixture.bound_run();
    run.start(
        &fixture.generated.store,
        fixture.split(SplitRole::Train),
        Some(fixture.split(SplitRole::Validation)),
        &fixture.modelspec,
        STEPS + 1,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    let state = run.finish().unwrap();
    say(&format!(
        "{test}: --steps {} on {TRAIN_ROWS} rows: {state:?}, exit {:?}, stderr:\n{}",
        STEPS + 1,
        run.exit_status(),
        run.stderr()
    ));
    assert_eq!(state, TrainingRunState::Failed);
    assert_eq!(run.exit_status(), Some(1));
    assert_eq!(run.failure(), Some(&TrainingFailure::Exit { code: 1 }));
    assert!(run.stderr().contains("E-DATA-008"), "{}", run.stderr());
    assert!(run.losses().is_empty());
}
