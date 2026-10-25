//! The native predictor against the pinned ModelSpec (DESIGN.md § 8.2 steps 4–5, § 12.2 "inference
//! test passes"; Da10-9): the weights of a real run, exported and loaded, predict the run's
//! evaluation split, and the mean squared error of the raw predictions against `y.f32` is the
//! `eval loss` ModelSpec printed for the same weights and rows.
//!
//! The binary is found as `pinned_export.rs` finds it: `MODELSPEC`, then a `modelspec` on `PATH`.
//! When neither names one, the test prints a `SKIPPED` line and passes, unless
//! `PATCHBACK_REQUIRE_MODELSPEC` is set, as CI sets it: then a missing binary fails.
//!
//! The run is `pinned_export.rs`'s: one generation of 64 / 16 / 16 samples, the train and
//! validation splits written z-scored, `modelspec train --steps 4 --eval` over the 16-row
//! validation split. Each row of the validation `x.f32` is handed to the predictor as `f64`
//! (`f32 as f64` is exact, so the cast back inside is the identity) and compared with its row of
//! `y.f32`. This is the only ModelSpec equivalence available: 0.5.0's `infer` takes no input, and
//! DESIGN.md § 1.1 excludes checkpoint use (Da9-15).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use patchback_analysis::{FEATURES_1_VALUE_COUNT, FEATURES_1_VERSION};
use patchback_dataset::{Generated, Generator, SplitCounts, SplitRole};
use patchback_dataset::{dense, training_stats};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, ModelSource, ModelSourceState, ModelSpec, ModelSpecError,
    ModelSpecPin, Predictor, RECOMMENDED_FILE_NAME, TrainingRun, TrainingRunState, W2_SHAPE,
    WeightArtifact, locate,
};

const TRAIN_ROWS: i64 = 64;
const EVAL_ROWS: usize = 16;
const STEPS: u32 = 4;
const OUTPUTS: usize = W2_SHAPE[1];
/// Da10-9, Da10-16, provisional: the relative difference allowed between the predictor's mean
/// squared error and ModelSpec's `eval loss`. Measured 6.9e-8; a transposed w1 gives 5.87e-3 and
/// eval rows misaligned by one 7.85e-3.
const EQUIVALENCE: f64 = 1e-5;

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

/// A dense file of little-endian `f32`, `rows` × `columns`.
fn matrix(path: &Path, rows: usize, columns: usize) -> Vec<Vec<f32>> {
    let bytes = fs::read(path).unwrap();
    assert_eq!(bytes.len(), rows * columns * 4, "{}", path.display());
    let values: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect();
    values.chunks(columns).map(<[f32]>::to_vec).collect()
}

fn relative(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

#[test]
fn the_predictor_reproduces_the_eval_loss_of_a_pinned_run() {
    let test = "the_predictor_reproduces_the_eval_loss_of_a_pinned_run";
    let pin = ModelSpecPin::repository().unwrap();
    let Some(binary) = binary(test, &pin) else {
        return;
    };
    let modelspec = ModelSpec::open(&binary, &pin).unwrap_or_else(|e| panic!("{e}"));
    let model =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec");
    let mut source = ModelSource::register(&model, "PatchRegressor", 16, &pin);
    assert_eq!(source.check(&modelspec).unwrap(), ModelSourceState::Checked);

    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-pinned-predict");
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    let started = Instant::now();
    let counts = SplitCounts {
        train: TRAIN_ROWS,
        validation: EVAL_ROWS as i64,
        test: 16,
    };
    let mut generated = Generator::new(42, counts, dir.clone()).run().unwrap();
    write(&mut generated, SplitRole::Train, &dir);
    write(&mut generated, SplitRole::Validation, &dir);
    say(&format!(
        "{test}: generation and writing of {TRAIN_ROWS}/{EVAL_ROWS}/16 samples {:?}",
        started.elapsed()
    ));

    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    run.bind(&source.trainable().unwrap()).unwrap();
    let started = Instant::now();
    run.start(
        &generated.store,
        generated.split_id(SplitRole::Train),
        Some(generated.split_id(SplitRole::Validation)),
        &modelspec,
        STEPS,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    let trained = started.elapsed();
    let state = run.finish().unwrap();
    say(&format!(
        "{test}: modelspec train --steps {STEPS} --eval {trained:?}; {state:?}"
    ));
    assert_eq!(
        state,
        TrainingRunState::Completed,
        "{:?}: {}",
        run.failure(),
        run.stderr()
    );
    assert_eq!(run.output_error(), None);
    let eval_loss = run.eval_loss().expect("the run printed an eval line");

    let path = dir.join(RECOMMENDED_FILE_NAME);
    let artifact =
        WeightArtifact::export(&run, FEATURES_1_VERSION, &path).unwrap_or_else(|e| panic!("{e}"));
    let mut predictor = Predictor::load(&path).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(predictor.artifact(), &artifact);

    let eval_dir = dir.join(SplitRole::Validation.dir());
    let x = matrix(&eval_dir.join("x.f32"), EVAL_ROWS, FEATURES_1_VALUE_COUNT);
    let y = matrix(&eval_dir.join("y.f32"), EVAL_ROWS, OUTPUTS);
    let rows: Vec<Vec<f64>> = x
        .iter()
        .map(|row| row.iter().map(|&v| f64::from(v)).collect())
        .collect();

    let mut squared = 0.0f64;
    let mut clamped = 0;
    for (row, target) in rows.iter().zip(&y) {
        let prediction = predictor
            .predict(FEATURES_1_VERSION, row)
            .unwrap_or_else(|e| panic!("{e}"));
        clamped += prediction.clamped;
        for (raw, target) in prediction.raw.iter().zip(target) {
            let error = f64::from(*raw) - f64::from(*target);
            squared += error * error;
        }
    }
    let mean = squared / (EVAL_ROWS * OUTPUTS) as f64;
    let per_row = squared / EVAL_ROWS as f64;
    let last_step = run.losses().last().map(|&(_, loss)| loss);
    say(&format!(
        "{test}: eval loss {eval_loss}; predictor mean over {EVAL_ROWS} x {OUTPUTS} {mean} (relative {:e}); sum over outputs, mean over rows {per_row} (relative {:e}); last step loss {last_step:?}; {clamped} outputs clamped in {} predictions",
        relative(mean, eval_loss),
        relative(per_row, eval_loss),
        predictor.clamped_predictions()
    ));
    assert!(mean.is_finite() && eval_loss.is_finite());
    assert!(
        relative(mean, eval_loss) <= EQUIVALENCE,
        "the predictor's mean squared error {mean} is not ModelSpec's eval loss {eval_loss}"
    );

    // Da10-15: the wall time of one prediction.
    let rounds = 1000;
    let started = Instant::now();
    for n in 0..rounds {
        predictor
            .predict(FEATURES_1_VERSION, &rows[n % EVAL_ROWS])
            .unwrap();
    }
    say(&format!(
        "{test}: one prediction {:?} (mean of {rounds}, {} build)",
        started.elapsed() / rounds as u32,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    ));
}
