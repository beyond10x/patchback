//! Adversary pass 2 on story:native-predictor (wave a10): the correction round (Da10-16, Da10-17).
//!
//! - Da10-17 as behaviour: the `NonFinitePrediction` documentation says an `inf · 0` (a hidden
//!   value past `f32::MAX` times a zero weight) refuses the whole prediction, outputs that are
//!   `±inf` included. The case below states that with `-inf`, `+inf` and NaN in one output vector,
//!   the lowest NaN index named, the `inf − inf` route beside it, and the counter untouched.
//! - Da10-16 as a limit with teeth: the pinned case measures the defects whose relative difference
//!   to ModelSpec's `eval loss` falls between the new limit (`1e-5`) and the old one (`1e-3`), so
//!   the tightening is shown to reject something the old limit let through.
//!
//! Each artifact is written here from chosen weights (the `weights/1` bytes of
//! `website/docs/reference/files.md` § Exported weights), never downloaded.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use patchback_analysis::{FEATURES_1_VALUE_COUNT, FEATURES_1_VERSION};
use patchback_core::digest;
use patchback_dataset::{Generated, Generator, SplitCounts, SplitRole};
use patchback_dataset::{dense, training_stats};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, ModelSource, ModelSourceState, ModelSpec, ModelSpecError,
    ModelSpecPin, PAYLOAD_BYTES, PredictError, Prediction, Predictor, RECOMMENDED_FILE_NAME,
    TrainingRun, TrainingRunState, W1_ELEMENTS, W1_SHAPE, W2_ELEMENTS, W2_SHAPE, WeightArtifact,
    locate,
};

const INPUTS: usize = W1_SHAPE[0];
const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];

fn say(line: &str) {
    let _ = writeln!(std::io::stderr(), "{line}");
}

fn w1_at(i: usize, j: usize) -> usize {
    i * HIDDEN + j
}

fn w2_at(j: usize, k: usize) -> usize {
    j * OUTPUTS + k
}

fn zeros() -> (Vec<f32>, Vec<f32>) {
    (vec![0.0; W1_ELEMENTS], vec![0.0; W2_ELEMENTS])
}

fn artifact_bytes(w1: &[f32], w2: &[f32]) -> Vec<u8> {
    let payload: Vec<u8> = w1.iter().chain(w2).flat_map(|v| v.to_le_bytes()).collect();
    assert_eq!(payload.len(), PAYLOAD_BYTES);
    let pin = ModelSpecPin::repository().unwrap();
    let header = format!(
        "weights/1\nmodel PatchRegressor\nmodelspec {} {}\nfeatures {FEATURES_1_VERSION}\nw1 32 64\nw2 64 8\nsha256 {}\n\n",
        pin.version(),
        pin.commit(),
        digest::hex(&digest::sha256(&payload))
    );
    [header.as_bytes(), &payload].concat()
}

fn predictor(name: &str, w1: &[f32], w2: &[f32]) -> Predictor {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-adversary-predictor-2");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.weights"));
    fs::write(&path, artifact_bytes(w1, w2)).unwrap();
    Predictor::load(&path).unwrap_or_else(|e| panic!("{e}"))
}

fn input(entries: &[(usize, f64)]) -> Vec<f64> {
    let mut x = vec![0.0; FEATURES_1_VALUE_COUNT];
    for &(i, value) in entries {
        x[i] = value;
    }
    x
}

fn predicted(p: &mut Predictor, x: &[f64]) -> Prediction {
    p.predict(FEATURES_1_VERSION, x)
        .unwrap_or_else(|e| panic!("{e}"))
}

fn refused_at(p: &mut Predictor, x: &[f64]) -> usize {
    match p.predict(FEATURES_1_VERSION, x) {
        Err(PredictError::NonFinitePrediction { index }) => index,
        other => panic!("expected NonFinitePrediction, got {other:?}"),
    }
}

/// Da10-7, Da10-17 (`src/predict.rs`, the `NonFinitePrediction` documentation). `x[0] = 3e38` is
/// a finite `f32`; with `w1[0, j] = 2` the hidden value `h[j]` overflows to `+inf`.
#[test]
fn an_inf_times_zero_refuses_the_whole_prediction_infinite_outputs_included() {
    let overflow = input(&[(0, 3e38)]);

    // h[0] = +inf. w2[0, ·] = (-1, 1, 0, 0.5, …): y = (-inf, +inf, NaN, +inf, …). The NaN at 2
    // refuses the prediction although outputs 0 and 1 alone would be clamped and counted.
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 2.0;
    for k in 0..OUTPUTS {
        w2[w2_at(0, k)] = 0.5;
    }
    w2[w2_at(0, 0)] = -1.0;
    w2[w2_at(0, 1)] = 1.0;
    w2[w2_at(0, 2)] = 0.0;
    let mut p = predictor("inf-times-zero", &w1, &w2);
    assert_eq!(refused_at(&mut p, &overflow), 2);
    assert_eq!(p.clamped_predictions(), 0);

    // The same weights on a finite hidden value: y = (-2, 2, 0, 1, …), clamped 2, counted once.
    let finite = predicted(&mut p, &input(&[(0, 1.0)]));
    assert_eq!(finite.raw, [-2.0, 2.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    assert_eq!(finite.values, [0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    assert_eq!((finite.clamped, p.clamped_predictions()), (2, 1));
    // A second refusal leaves the counter where it was.
    assert_eq!(refused_at(&mut p, &overflow), 2);
    assert_eq!(p.clamped_predictions(), 1);

    // Without the zero weight the same overflow is clamped, not refused: eight infinite outputs.
    let mut w2_no_zero = w2.clone();
    w2_no_zero[w2_at(0, 2)] = -0.5;
    let mut q = predictor("inf-without-zero", &w1, &w2_no_zero);
    let infinite = predicted(&mut q, &overflow);
    assert!(infinite.raw.iter().all(|v| v.is_infinite()), "{infinite:?}");
    assert_eq!(
        infinite.values,
        [0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        "±inf is clamped"
    );
    assert_eq!((infinite.clamped, q.clamped_predictions()), (8, 1));

    // Two NaN outputs (zero weights at 0 and 2, -inf at 1): the lowest index is named.
    let mut w2_two = w2.clone();
    w2_two[w2_at(0, 0)] = 0.0;
    w2_two[w2_at(0, 1)] = -1.0;
    let mut r = predictor("inf-times-zero-twice", &w1, &w2_two);
    assert_eq!(refused_at(&mut r, &overflow), 0);
    assert_eq!(r.clamped_predictions(), 0);

    // The documented other route, `inf − inf`: h[0] = h[1] = +inf, w2[0, k] = 1 everywhere,
    // w2[1, k] = 1 except w2[1, 5] = -1. y[5] = inf − inf = NaN, every other output +inf.
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 2.0;
    w1[w1_at(0, 1)] = 2.0;
    for k in 0..OUTPUTS {
        w2[w2_at(0, k)] = 1.0;
        w2[w2_at(1, k)] = 1.0;
    }
    w2[w2_at(1, 5)] = -1.0;
    let mut s = predictor("inf-minus-inf", &w1, &w2);
    assert_eq!(refused_at(&mut s, &overflow), 5);
    assert_eq!(s.clamped_predictions(), 0);
}

// The pinned lane: Da10-16's limit against defects between it and the old 1e-3.

/// Da10-16, the value of `EQUIVALENCE` in `tests/pinned_predict.rs`, restated here so this file
/// fails if a defect that limit is meant to reject comes in under it.
const DA10_16: f64 = 1e-5;
/// Da10-9's limit before the correction.
const DA10_9: f64 = 1e-3;
const TRAIN_ROWS: i64 = 64;
const EVAL_ROWS: usize = 16;
const STEPS: u32 = 4;

fn binary(test: &str, pin: &ModelSpecPin) -> Option<PathBuf> {
    match locate(None) {
        Ok(binary) => Some(binary),
        Err(ModelSpecError::NotFound { .. }) => {
            assert!(
                std::env::var_os("PATCHBACK_REQUIRE_MODELSPEC").is_none(),
                "PATCHBACK_REQUIRE_MODELSPEC is set and no modelspec binary was found"
            );
            say(&format!(
                "SKIPPED {test}: no modelspec {} binary",
                pin.version()
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

fn floats(path: &Path) -> Vec<f32> {
    fs::read(path)
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect()
}

/// `tests/pinned_predict.rs`'s comparison: the denominator is ModelSpec's `eval loss`.
fn relative(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

fn mse(predictions: &[[f32; OUTPUTS]], y: &[f32]) -> f64 {
    let mut squared = 0.0f64;
    for (row, prediction) in predictions.iter().enumerate() {
        for (k, raw) in prediction.iter().enumerate() {
            let error = f64::from(*raw) - f64::from(y[row * OUTPUTS + k]);
            squared += error * error;
        }
    }
    squared / (predictions.len() * OUTPUTS) as f64
}

/// `v` with its mantissa rounded to nearest-even at `keep` bits (10: half precision, 7: bfloat16).
fn rounded(v: f32, keep: u32) -> f32 {
    let drop = 23 - keep;
    let bits = v.to_bits();
    let half = (1u32 << (drop - 1)) - 1 + ((bits >> drop) & 1);
    f32::from_bits((bits + half) & !((1u32 << drop) - 1))
}

/// The Da10-5 forward pass, with the hidden units in `dropped` forced to 0 and ReLU optional.
fn forward(w1: &[f32], w2: &[f32], x: &[f32], dropped: &[usize], relu: bool) -> [f32; OUTPUTS] {
    let mut h = [0.0f32; HIDDEN];
    for (j, hidden) in h.iter_mut().enumerate() {
        let mut sum = 0.0f32;
        for (i, value) in x.iter().enumerate() {
            sum += value * w1[w1_at(i, j)];
        }
        *hidden = if dropped.contains(&j) || (relu && sum < 0.0) {
            0.0
        } else {
            sum
        };
    }
    let mut y = [0.0f32; OUTPUTS];
    for (k, output) in y.iter_mut().enumerate() {
        let mut sum = 0.0f32;
        for (j, hidden) in h.iter().enumerate() {
            sum += hidden * w2[w2_at(j, k)];
        }
        *output = sum;
    }
    y
}

#[test]
fn the_tightened_limit_rejects_defects_the_old_one_let_through() {
    let test = "the_tightened_limit_rejects_defects_the_old_one_let_through";
    let pin = ModelSpecPin::repository().unwrap();
    let Some(binary) = binary(test, &pin) else {
        return;
    };
    let modelspec = ModelSpec::open(&binary, &pin).unwrap_or_else(|e| panic!("{e}"));
    let model =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec");
    let mut source = ModelSource::register(&model, "PatchRegressor", 16, &pin);
    assert_eq!(source.check(&modelspec).unwrap(), ModelSourceState::Checked);

    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-adversary-2-pinned");
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    let counts = SplitCounts {
        train: TRAIN_ROWS,
        validation: EVAL_ROWS as i64,
        test: 16,
    };
    let mut generated = Generator::new(42, counts, dir.clone()).run().unwrap();
    write(&mut generated, SplitRole::Train, &dir);
    write(&mut generated, SplitRole::Validation, &dir);

    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    run.bind(&source.trainable().unwrap()).unwrap();
    run.start(
        &generated.store,
        generated.split_id(SplitRole::Train),
        Some(generated.split_id(SplitRole::Validation)),
        &modelspec,
        STEPS,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    assert_eq!(run.finish().unwrap(), TrainingRunState::Completed);
    let eval_loss = run.eval_loss().expect("the run printed an eval line");
    let path = dir.join(RECOMMENDED_FILE_NAME);
    WeightArtifact::export(&run, FEATURES_1_VERSION, &path).unwrap_or_else(|e| panic!("{e}"));
    let mut predictor = Predictor::load(&path).unwrap();
    let (w1, w2) = (
        predictor.artifact().w1().to_vec(),
        predictor.artifact().w2().to_vec(),
    );

    let eval_dir = dir.join(SplitRole::Validation.dir());
    let x = floats(&eval_dir.join("x.f32"));
    let y = floats(&eval_dir.join("y.f32"));
    assert_eq!(
        (x.len(), y.len()),
        (EVAL_ROWS * INPUTS, EVAL_ROWS * OUTPUTS)
    );
    let rows: Vec<&[f32]> = x.chunks(INPUTS).collect();

    let mut raw = Vec::new();
    let mut values = Vec::new();
    for row in &rows {
        let input: Vec<f64> = row.iter().map(|&v| f64::from(v)).collect();
        let prediction = predicted(&mut predictor, &input);
        assert_eq!(prediction.raw, forward(&w1, &w2, row, &[], true));
        raw.push(prediction.raw);
        values.push(prediction.values);
    }
    let native = relative(mse(&raw, &y), eval_loss);
    assert!(
        native <= DA10_16,
        "native relative {native:e} > Da10-16's {DA10_16:e}"
    );

    let mutated =
        |w1: &[f32], w2: &[f32], dropped: &[usize], relu: bool, input: &dyn Fn(f32) -> f32| {
            let predictions: Vec<[f32; OUTPUTS]> = rows
                .iter()
                .map(|row| {
                    let row: Vec<f32> = row.iter().map(|&v| input(v)).collect();
                    forward(w1, w2, &row, dropped, relu)
                })
                .collect();
            relative(mse(&predictions, &y), eval_loss)
        };
    let same = |v: f32| v;
    let half = |v: f32| rounded(v, 10);
    let w1_half: Vec<f32> = w1.iter().map(|&v| rounded(v, 10)).collect();
    let w2_half: Vec<f32> = w2.iter().map(|&v| rounded(v, 10)).collect();
    let w1_bf16: Vec<f32> = w1.iter().map(|&v| rounded(v, 7)).collect();
    let w2_bf16: Vec<f32> = w2.iter().map(|&v| rounded(v, 7)).collect();

    let mut measured: Vec<(String, f64)> = vec![
        ("native".to_owned(), native),
        (
            "weights at half precision".to_owned(),
            mutated(&w1_half, &w2_half, &[], true, &same),
        ),
        (
            "weights at bfloat16".to_owned(),
            mutated(&w1_bf16, &w2_bf16, &[], true, &same),
        ),
        (
            "inputs at half precision".to_owned(),
            mutated(&w1, &w2, &[], true, &half),
        ),
        ("no ReLU".to_owned(), mutated(&w1, &w2, &[], false, &same)),
        (
            "clamped values instead of raw".to_owned(),
            relative(mse(&values, &y), eval_loss),
        ),
        (
            "last eval row left out".to_owned(),
            relative(mse(&raw[..EVAL_ROWS - 1], &y), eval_loss),
        ),
    ];
    for i in 0..INPUTS {
        let mut w1_without = w1.clone();
        for j in 0..HIDDEN {
            w1_without[w1_at(i, j)] = 0.0;
        }
        measured.push((
            format!("input {i} ignored"),
            mutated(&w1_without, &w2, &[], true, &same),
        ));
    }
    for j in 0..HIDDEN {
        measured.push((
            format!("hidden unit {j} dropped"),
            mutated(&w1, &w2, &[j], true, &same),
        ));
    }

    let in_gap: Vec<&(String, f64)> = measured
        .iter()
        .filter(|(_, r)| *r > DA10_16 && *r <= DA10_9)
        .collect();
    let under: Vec<&(String, f64)> = measured
        .iter()
        .skip(1)
        .filter(|(_, r)| *r <= DA10_16)
        .collect();
    say(&format!(
        "{test}: eval loss {eval_loss}; {}",
        measured
            .iter()
            .map(|(name, r)| format!("{name} {r:e}"))
            .collect::<Vec<_>>()
            .join("; ")
    ));
    say(&format!(
        "{test}: {} defects between {DA10_16:e} and {DA10_9:e}: {in_gap:?}; {} under {DA10_16:e}: {under:?}",
        in_gap.len(),
        under.len()
    ));
    // Every defect is rejected by Da10-16's limit; measured, the closest is the inputs at half
    // precision, 6.5e-5 (6.5 times the limit).
    assert!(under.is_empty(), "defects under {DA10_16:e}: {under:?}");
    // The tightening is not cosmetic: measured, weights at half precision (1.1e-4), at bfloat16
    // (4.4e-4), inputs at half precision (6.5e-5) and input 12 ignored (6.8e-4) passed 1e-3.
    let names: Vec<&str> = in_gap.iter().map(|(name, _)| name.as_str()).collect();
    for expected in [
        "weights at half precision",
        "weights at bfloat16",
        "inputs at half precision",
    ] {
        assert!(names.contains(&expected), "{expected} not in {in_gap:?}");
    }
}
