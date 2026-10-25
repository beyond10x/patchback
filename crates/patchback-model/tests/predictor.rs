//! The native predictor's numeric obligations (story:native-predictor, DESIGN.md § 8.2 step 4,
//! § 8.1, § 12.1): the goldens and the reference check (Da10-9), determinism (Da10-8), the clamp,
//! the infinities and NaN (Da10-6, Da10-7) and every refusal with its payload (Da10-4).
//!
//! Every artifact here is written by [`write_artifact`] from weights the test chooses: the
//! `weights/1` bytes of `website/docs/reference/files.md` § Exported weights, the checksum from
//! `patchback_core::digest`, read back through [`Predictor::load`], so no training run is needed.
//! The scenarios are `tests/predictor_scenarios.rs`; the pinned binary is `tests/pinned_predict.rs`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use patchback_analysis::{FEATURES_1_VALUE_COUNT, FEATURES_1_VERSION};
use patchback_core::digest;
use patchback_core::prng::Xoshiro256StarStar;
use patchback_model::{
    ModelSpecPin, PAYLOAD_BYTES, PredictError, Prediction, Predictor, W1_ELEMENTS, W1_SHAPE,
    W2_ELEMENTS, W2_SHAPE, WeightArtifact,
};

const INPUTS: usize = W1_SHAPE[0];
const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];

/// Written to the process's stderr, not through eprintln!, so the harness's output capture does
/// not hide it from the log.
fn say(line: &str) {
    let _ = writeln!(std::io::stderr(), "{line}");
}

/// The index of w1 `[i, j]`, row-major `[32, 64]`.
fn w1_at(i: usize, j: usize) -> usize {
    i * HIDDEN + j
}

/// The index of w2 `[j, k]`, row-major `[64, 8]`.
fn w2_at(j: usize, k: usize) -> usize {
    j * OUTPUTS + k
}

/// All-zero weights, to set entries of.
fn zeros() -> (Vec<f32>, Vec<f32>) {
    (vec![0.0; W1_ELEMENTS], vec![0.0; W2_ELEMENTS])
}

/// The bytes of a `weights/1` artifact holding `w1` and `w2` for `features`, under the
/// repository's ModelSpec pin.
fn artifact_bytes(w1: &[f32], w2: &[f32], features: &str) -> Vec<u8> {
    assert_eq!((w1.len(), w2.len()), (W1_ELEMENTS, W2_ELEMENTS));
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

fn dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-predictor");
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes the artifact of `w1` and `w2` for `features/1` to `<name>.weights`, replacing an
/// earlier one; its path.
fn write_artifact(name: &str, w1: &[f32], w2: &[f32]) -> PathBuf {
    let path = dir().join(format!("{name}.weights"));
    fs::write(&path, artifact_bytes(w1, w2, FEATURES_1_VERSION)).unwrap();
    path
}

/// A predictor loaded from the artifact of `w1` and `w2`.
fn predictor(name: &str, w1: &[f32], w2: &[f32]) -> Predictor {
    let path = write_artifact(name, w1, w2);
    Predictor::load(&path).unwrap_or_else(|e| panic!("{e}"))
}

/// A `features/1` vector of zeros with the given entries set.
fn input(entries: &[(usize, f64)]) -> Vec<f64> {
    let mut x = vec![0.0; FEATURES_1_VALUE_COUNT];
    for &(i, value) in entries {
        x[i] = value;
    }
    x
}

fn predicted(predictor: &mut Predictor, x: &[f64]) -> Prediction {
    predictor
        .predict(FEATURES_1_VERSION, x)
        .unwrap_or_else(|e| panic!("{e}"))
}

fn outputs(entries: &[(usize, f32)]) -> [f32; OUTPUTS] {
    let mut y = [0.0f32; OUTPUTS];
    for &(k, value) in entries {
        y[k] = value;
    }
    y
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

/// Golden 1 (Da10-9), a one-hot path: x[3] = 1 reaches h[5] through w1[3, 5] = 0.5 and y[2]
/// through w2[5, 2] = 0.75, so y[2] = 1 · 0.5 · 0.75 = 0.375 exactly and every other output is 0.
/// A reader that took w1 or w2 column-major would read a zero there (w1 index 197, not 163; w2
/// index 42, not 133).
#[test]
fn golden_a_one_hot_path() {
    let (mut w1, mut w2) = zeros();
    w1[w1_at(3, 5)] = 0.5;
    w2[w2_at(5, 2)] = 0.75;
    let mut p = predictor("golden-one-hot", &w1, &w2);
    let prediction = predicted(&mut p, &input(&[(3, 1.0)]));
    assert_eq!(prediction.raw, outputs(&[(2, 0.375)]));
    assert_eq!(prediction.values, prediction.raw);
    assert_eq!(prediction.clamped, 0);
    // x[3] = 2: 2 · 0.5 · 0.75 = 0.75.
    let prediction = predicted(&mut p, &input(&[(3, 2.0)]));
    assert_eq!(prediction.raw, outputs(&[(2, 0.75)]));
    // Another input than the path's reaches nothing.
    let prediction = predicted(&mut p, &input(&[(4, 1.0)]));
    assert_eq!(prediction.raw, outputs(&[]));
    assert_eq!(p.clamped_predictions(), 0);
}

/// Golden 2 (Da10-9), a negative pre-activation cut by ReLU: x = (1, 2, 0, …); h[0]'s
/// pre-activation is 1 · 1 + 2 · (−1) = −1, cut to 0; h[1]'s is 1 · 0.25 + 2 · 0.25 = 0.75.
/// y[0] = 0 · 4 + 0.75 · 0.5 = 0.375 and y[1] = 0 · (−0.5) + 0.75 · 0 = 0. Without the cut they
/// would be −3.625 and 0.5.
#[test]
fn golden_a_negative_pre_activation_cut_by_relu() {
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 1.0;
    w1[w1_at(1, 0)] = -1.0;
    w1[w1_at(0, 1)] = 0.25;
    w1[w1_at(1, 1)] = 0.25;
    w2[w2_at(0, 0)] = 4.0;
    w2[w2_at(1, 0)] = 0.5;
    w2[w2_at(0, 1)] = -0.5;
    let mut p = predictor("golden-relu", &w1, &w2);
    let prediction = predicted(&mut p, &input(&[(0, 1.0), (1, 2.0)]));
    assert_eq!(prediction.raw, outputs(&[(0, 0.375)]));
    assert_eq!(prediction.values, prediction.raw);
    assert_eq!(prediction.clamped, 0);
    assert_eq!(p.clamped_predictions(), 0);
}

/// The artifact of golden 3: h[0] = 0.5 · x[0], y[6] = 0.5 · h[0], y[7] = h[0].
fn clamp_weights() -> (Vec<f32>, Vec<f32>) {
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 0.5;
    w2[w2_at(0, 6)] = 0.5;
    w2[w2_at(0, 7)] = 1.0;
    (w1, w2)
}

/// Golden 3 (Da10-9, Da10-6), an output of 1.5 clamped and counted: x[0] = 3 gives h[0] = 1.5,
/// y[6] = 0.75 and y[7] = 1.5; y[7] is clamped to 1 and the prediction counted.
#[test]
fn golden_an_output_of_1_5_is_clamped_and_counted() {
    let (w1, w2) = clamp_weights();
    let mut p = predictor("golden-clamp", &w1, &w2);
    let prediction = predicted(&mut p, &input(&[(0, 3.0)]));
    assert_eq!(prediction.raw, outputs(&[(6, 0.75), (7, 1.5)]));
    assert_eq!(prediction.values, outputs(&[(6, 0.75), (7, 1.0)]));
    assert_eq!(prediction.clamped, 1);
    assert_eq!(p.clamped_predictions(), 1);
}

/// Da10-6: the counter counts predictions with an output clamped, not outputs; an output of
/// exactly 1 or 0 is not clamped. Three predictions, two of which clamp.
#[test]
fn the_counter_counts_the_predictions_that_clamp() {
    let (w1, w2) = clamp_weights();
    let mut p = predictor("counter", &w1, &w2);
    assert_eq!(p.clamped_predictions(), 0);
    // x[0] = 3: y[7] = 1.5, one output clamped.
    assert_eq!(predicted(&mut p, &input(&[(0, 3.0)])).clamped, 1);
    // x[0] = 2: y[6] = 0.5, y[7] = 1 exactly; nothing clamped.
    let exact = predicted(&mut p, &input(&[(0, 2.0)]));
    assert_eq!(exact.raw, outputs(&[(6, 0.5), (7, 1.0)]));
    assert_eq!(exact.clamped, 0);
    // x[0] = 6: y[6] = 1.5, y[7] = 3, two outputs clamped, one prediction counted.
    let two = predicted(&mut p, &input(&[(0, 6.0)]));
    assert_eq!(two.raw, outputs(&[(6, 1.5), (7, 3.0)]));
    assert_eq!(two.values, outputs(&[(6, 1.0), (7, 1.0)]));
    assert_eq!(two.clamped, 2);
    assert_eq!(p.clamped_predictions(), 2);
    // A refused prediction is not counted.
    assert!(p.predict(FEATURES_1_VERSION, &[6.0; 31]).is_err());
    assert_eq!(p.clamped_predictions(), 2);
    // The count is the Predictor's, in memory: another loaded from the same file starts at 0.
    let path = dir().join("counter.weights");
    assert_eq!(Predictor::load(&path).unwrap().clamped_predictions(), 0);
}

/// Seeded weights in [−0.5, 0.5) and inputs in [−3, 3) from `xoshiro256**`, the dataset
/// generator's PRNG (`patchback_core::prng`).
fn seeded_weights(seed: u64) -> (Vec<f32>, Vec<f32>) {
    let mut rng = Xoshiro256StarStar::new(seed);
    let mut draw =
        |count: usize| -> Vec<f32> { (0..count).map(|_| (rng.next_f64() - 0.5) as f32).collect() };
    let w1 = draw(W1_ELEMENTS);
    let w2 = draw(W2_ELEMENTS);
    (w1, w2)
}

fn seeded_inputs(seed: u64, count: usize) -> Vec<Vec<f64>> {
    let mut rng = Xoshiro256StarStar::new(seed);
    (0..count)
        .map(|_| {
            (0..FEATURES_1_VALUE_COUNT)
                .map(|_| rng.next_f64() * 6.0 - 3.0)
                .collect()
        })
        .collect()
}

/// The forward pass in `f64`, in another loop order than the predictor's: for each output `k`,
/// for each hidden unit `j`, the pre-activation over `i`, then ReLU and the product with w2.
fn reference(w1: &[f32], w2: &[f32], x: &[f64]) -> ([f64; OUTPUTS], usize) {
    let x: Vec<f64> = x.iter().map(|&v| f64::from(v as f32)).collect();
    let mut y = [0.0f64; OUTPUTS];
    let mut cut = 0;
    for (k, yk) in y.iter_mut().enumerate() {
        for j in 0..HIDDEN {
            let pre: f64 = (0..INPUTS).map(|i| x[i] * f64::from(w1[w1_at(i, j)])).sum();
            if pre < 0.0 && k == 0 {
                cut += 1;
            }
            *yk += pre.max(0.0) * f64::from(w2[w2_at(j, k)]);
        }
    }
    (y, cut)
}

/// Da10-9: 64 seeded inputs against the `f64` reference, `|raw − ref| ≤ 1e-4 · max(1, |ref|)`;
/// the clamp of each and the counter follow `raw`. Also the wall time of one prediction
/// (Da10-15).
#[test]
fn sixty_four_seeded_inputs_match_an_f64_reference() {
    let (w1, w2) = seeded_weights(7);
    let mut p = predictor("reference", &w1, &w2);
    let inputs = seeded_inputs(8, 64);
    let mut worst = 0.0f64;
    let mut cut = 0;
    let mut counted = 0;
    let mut outside = 0;
    for (n, x) in inputs.iter().enumerate() {
        let prediction = predicted(&mut p, x);
        let (expected, cut_here) = reference(&w1, &w2, x);
        cut += cut_here;
        for (k, &wanted) in expected.iter().enumerate() {
            let raw = f64::from(prediction.raw[k]);
            let bound = 1e-4 * wanted.abs().max(1.0);
            let error = (raw - wanted).abs();
            assert!(
                error <= bound,
                "input {n} output {k}: raw {raw}, reference {wanted}, error {error}"
            );
            worst = worst.max(error / wanted.abs().max(1.0));
            assert_eq!(
                prediction.values[k].to_bits(),
                prediction.raw[k].clamp(0.0, 1.0).to_bits()
            );
        }
        let clamped = prediction
            .raw
            .iter()
            .filter(|v| **v < 0.0 || **v > 1.0)
            .count();
        assert_eq!(prediction.clamped, clamped, "input {n}");
        outside += clamped;
        counted += usize::from(clamped > 0);
    }
    assert_eq!(p.clamped_predictions(), counted as u64);
    assert!(
        cut > 0,
        "some pre-activation is negative, so ReLU is exercised"
    );
    assert!(outside > 0 && outside < 64 * OUTPUTS, "{outside}");
    say(&format!(
        "reference: 64 inputs, worst scaled error {worst:e}, {cut} negative pre-activations, {outside} outputs clamped in {counted} predictions"
    ));

    let rounds = 1000;
    let started = Instant::now();
    for n in 0..rounds {
        let _ = predicted(&mut p, &inputs[n % inputs.len()]);
    }
    say(&format!(
        "one prediction: {:?} (mean of {rounds}, {} build)",
        started.elapsed() / rounds as u32,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    ));
}

/// Da10-8 (§ 12.1): the same `Predictor` twice, and two `Predictor`s loaded from the same file,
/// give bitwise-equal `raw` over 16 seeded inputs.
#[test]
fn the_output_is_deterministic() {
    let (w1, w2) = seeded_weights(11);
    let path = write_artifact("determinism", &w1, &w2);
    let mut first = Predictor::load(&path).unwrap();
    let mut second = Predictor::load(&path).unwrap();
    let mut third = Predictor::from_artifact(WeightArtifact::read(&path).unwrap());
    assert_eq!(first.artifact(), third.artifact());
    for x in seeded_inputs(12, 16) {
        let once = predicted(&mut first, &x);
        let twice = predicted(&mut first, &x);
        let other = predicted(&mut second, &x);
        let read = predicted(&mut third, &x);
        assert_eq!(bits(&once.raw), bits(&twice.raw));
        assert_eq!(bits(&once.raw), bits(&other.raw));
        assert_eq!(bits(&once.raw), bits(&read.raw));
        assert_eq!(bits(&once.values), bits(&other.values));
        assert_eq!(once.clamped, other.clamped);
    }
    assert_eq!(
        first.clamped_predictions(),
        2 * second.clamped_predictions()
    );
}

/// Da10-6, Da10-7: `+inf` and `−inf` outputs are clamped and counted; with h[0] = 3e38 and h[1]
/// = 1, y[0] = 3e38 · 2 overflows to `+inf`, y[1] = 3e38 · (−2) to `−inf`, y[2] = 1 · 0.5 and
/// y[3] = 1 · (−0.5).
#[test]
fn infinite_outputs_are_clamped_and_counted() {
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 3e38;
    w1[w1_at(1, 1)] = 1.0;
    w2[w2_at(0, 0)] = 2.0;
    w2[w2_at(0, 1)] = -2.0;
    w2[w2_at(1, 2)] = 0.5;
    w2[w2_at(1, 3)] = -0.5;
    let mut p = predictor("infinities", &w1, &w2);
    let prediction = predicted(&mut p, &input(&[(0, 1.0), (1, 1.0)]));
    assert_eq!(
        prediction.raw,
        outputs(&[
            (0, f32::INFINITY),
            (1, f32::NEG_INFINITY),
            (2, 0.5),
            (3, -0.5)
        ])
    );
    assert_eq!(prediction.values, outputs(&[(0, 1.0), (2, 0.5)]));
    assert_eq!(prediction.clamped, 3);
    assert_eq!(p.clamped_predictions(), 1);
}

/// Da10-7: a NaN output is refused as `NonFinitePrediction`, which names no domain error, and is
/// not counted. In the second layer: y[0] = 3e38 · 2 + 3e38 · (−2) = `inf − inf`. In the first:
/// h[0]'s pre-activation is 2 · 3e38 + 2 · (−3e38) = `inf − inf`, and the NaN is not cut by ReLU
/// (it would read as 0 if it were), so it reaches every output.
#[test]
fn a_nan_output_is_refused_and_not_counted() {
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 3e38;
    w1[w1_at(1, 1)] = 3e38;
    w2[w2_at(0, 0)] = 2.0;
    w2[w2_at(1, 0)] = -2.0;
    w2[w2_at(1, 4)] = 2.0;
    let mut p = predictor("nan-second-layer", &w1, &w2);
    match p.predict(FEATURES_1_VERSION, &input(&[(0, 1.0), (1, 1.0)])) {
        Err(error @ PredictError::NonFinitePrediction { .. }) => {
            assert_eq!(error.domain_error(), None);
            let PredictError::NonFinitePrediction { index } = error else {
                unreachable!()
            };
            assert_eq!(index, 0);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(p.clamped_predictions(), 0);
    // The same artifact predicts where no `inf − inf` arises: y[4] = 3e38 · 2 = +inf, clamped.
    let prediction = predicted(&mut p, &input(&[(1, 1.0)]));
    assert_eq!(
        prediction.raw,
        outputs(&[(0, f32::NEG_INFINITY), (4, f32::INFINITY)])
    );
    assert_eq!(p.clamped_predictions(), 1);

    let (mut w1, w2) = zeros();
    w1[w1_at(0, 0)] = 3e38;
    w1[w1_at(1, 0)] = -3e38;
    let mut p = predictor("nan-first-layer", &w1, &w2);
    match p.predict(FEATURES_1_VERSION, &input(&[(0, 2.0), (1, 2.0)])) {
        Err(PredictError::NonFinitePrediction { index }) => assert_eq!(index, 0),
        other => panic!("{other:?}"),
    }
    assert_eq!(p.clamped_predictions(), 0);
}

fn refusal(p: &mut Predictor, contract: &str, x: &[f64]) -> PredictError {
    match p.predict(contract, x) {
        Err(error) => error,
        Ok(prediction) => panic!("predicted {prediction:?}"),
    }
}

/// Da10-4, Da10-10: the refusals of `predict` and their payloads, in their order (count,
/// contract, finiteness); a refusal changes nothing.
#[test]
fn predict_refuses_wrong_input() {
    let (w1, w2) = clamp_weights();
    let mut p = predictor("refusals", &w1, &w2);
    let good = input(&[(0, 3.0)]);

    for count in [0, 31, 33, 64] {
        match refusal(&mut p, FEATURES_1_VERSION, &vec![0.5; count]) {
            error @ PredictError::WrongFeatureCount { .. } => {
                assert_eq!(
                    error.domain_error(),
                    Some("patchback.model.WrongFeatureCount")
                );
                assert!(
                    matches!(error, PredictError::WrongFeatureCount { count: c } if c == count)
                );
            }
            other => panic!("{count}: {other:?}"),
        }
    }

    for contract in ["features/2", "", "features/1 ", "Features/1", "features/10"] {
        match refusal(&mut p, contract, &good) {
            error @ PredictError::FeatureContractMismatch { .. } => {
                assert_eq!(
                    error.domain_error(),
                    Some("patchback.model.FeatureContractMismatch")
                );
                let PredictError::FeatureContractMismatch { artifact, input } = error else {
                    unreachable!()
                };
                assert_eq!(
                    (artifact.as_str(), input.as_str()),
                    ("features/1", contract)
                );
            }
            other => panic!("{contract:?}: {other:?}"),
        }
    }

    for (index, value) in [
        (4, f64::NAN),
        (9, f64::INFINITY),
        (0, f64::NEG_INFINITY),
        (31, 1e39),
        (17, -1e39),
        (2, f64::MAX),
    ] {
        let mut x = good.clone();
        x[index] = value;
        match refusal(&mut p, FEATURES_1_VERSION, &x) {
            error @ PredictError::NonFiniteFeature { .. } => {
                assert_eq!(
                    error.domain_error(),
                    Some("patchback.model.NonFiniteFeature")
                );
                assert!(
                    matches!(error, PredictError::NonFiniteFeature { index: i } if i == index),
                    "{value}: {error:?}"
                );
            }
            other => panic!("{value}: {other:?}"),
        }
    }
    // The first non-finite value is named.
    let mut x = good.clone();
    x[20] = f64::NAN;
    x[5] = 1e300;
    assert!(matches!(
        refusal(&mut p, FEATURES_1_VERSION, &x),
        PredictError::NonFiniteFeature { index: 5 }
    ));
    // The largest f32 is finite and predicts (its output is clamped).
    let mut x = good.clone();
    x[0] = f64::from(f32::MAX);
    assert_eq!(predicted(&mut p, &x).clamped, 2);
    assert_eq!(p.clamped_predictions(), 1);

    // The order: count before contract before finiteness.
    let mut bad = vec![f64::NAN; 31];
    assert!(matches!(
        refusal(&mut p, "features/2", &bad),
        PredictError::WrongFeatureCount { count: 31 }
    ));
    bad.push(f64::NAN);
    assert!(matches!(
        refusal(&mut p, "features/2", &bad),
        PredictError::FeatureContractMismatch { .. }
    ));
    assert!(matches!(
        refusal(&mut p, FEATURES_1_VERSION, &bad),
        PredictError::NonFiniteFeature { index: 0 }
    ));
    assert_eq!(p.clamped_predictions(), 1, "refusals are not counted");
}

/// Da10-4: `load` refuses a missing path as `UnknownWeightArtifact`, a corrupt file as
/// `CorruptWeightArtifact`, and another I/O failure (a directory) as `Io`, no domain error.
#[test]
fn load_refuses_a_missing_a_corrupt_and_an_unreadable_artifact() {
    let missing = dir().join("missing.weights");
    let _ = fs::remove_file(&missing);
    for path in [missing.clone(), dir().join("no-such-dir").join("a.weights")] {
        match Predictor::load(&path) {
            Err(error @ PredictError::UnknownWeightArtifact { .. }) => {
                assert_eq!(
                    error.domain_error(),
                    Some("patchback.model.UnknownWeightArtifact")
                );
                let PredictError::UnknownWeightArtifact { path: at } = error else {
                    unreachable!()
                };
                assert_eq!(at, path);
            }
            other => panic!("{}: {:?}", path.display(), other.map(drop)),
        }
    }

    let directory = dir().join("a-directory.weights");
    fs::create_dir_all(&directory).unwrap();
    match Predictor::load(&directory) {
        Err(error @ PredictError::Io { .. }) => {
            assert_eq!(error.domain_error(), None);
            let PredictError::Io { path, .. } = error else {
                unreachable!()
            };
            assert_eq!(path, directory);
        }
        other => panic!("{:?}", other.map(drop)),
    }

    let (w1, w2) = clamp_weights();
    let good = artifact_bytes(&w1, &w2, FEATURES_1_VERSION);
    let start = good.len() - PAYLOAD_BYTES;
    let mut flipped = good.clone();
    flipped[start + 4 * w1_at(0, 0)] ^= 0x01;
    let mut wrong_format = good.clone();
    wrong_format[8] = b'2';
    for (name, bytes) in [
        ("flipped", flipped),
        ("truncated", good[..good.len() - 1].to_vec()),
        ("empty", Vec::new()),
        ("weights-2", wrong_format),
    ] {
        let path = dir().join(format!("corrupt-{name}.weights"));
        fs::write(&path, &bytes).unwrap();
        match Predictor::load(&path) {
            Err(error @ PredictError::CorruptWeightArtifact { .. }) => {
                assert_eq!(
                    error.domain_error(),
                    Some("patchback.model.CorruptWeightArtifact")
                );
                let PredictError::CorruptWeightArtifact { reason } = error else {
                    unreachable!()
                };
                assert!(!reason.is_empty(), "{name}");
                if name == "flipped" {
                    assert!(reason.contains("SHA-256"), "{reason}");
                }
            }
            other => panic!("{name}: {:?}", other.map(drop)),
        }
    }

    // The same bytes unflipped load, and the artifact is the reader's.
    let path = dir().join("good.weights");
    fs::write(&path, &good).unwrap();
    let p = Predictor::load(&path).unwrap();
    assert_eq!(p.artifact(), &WeightArtifact::read(&path).unwrap());
    assert_eq!(p.artifact().feature_contract(), FEATURES_1_VERSION);
    assert_eq!(p.clamped_predictions(), 0);
}

/// An artifact exported for another contract refuses a `features/1` vector and predicts one of
/// its own contract: the comparison is the artifact's string, not a constant.
#[test]
fn the_contract_compared_is_the_artifacts() {
    let (w1, w2) = clamp_weights();
    let path = dir().join("features-2.weights");
    fs::write(&path, artifact_bytes(&w1, &w2, "features/2")).unwrap();
    let mut p = Predictor::load(&path).unwrap();
    match refusal(&mut p, FEATURES_1_VERSION, &input(&[(0, 3.0)])) {
        PredictError::FeatureContractMismatch { artifact, input } => {
            assert_eq!(
                (artifact.as_str(), input.as_str()),
                ("features/2", "features/1")
            )
        }
        other => panic!("{other:?}"),
    }
    let prediction = p.predict("features/2", &input(&[(0, 3.0)])).unwrap();
    assert_eq!(prediction.clamped, 1);
}

/// The errors' messages are one line each and name what was refused.
#[test]
fn the_errors_display_one_line() {
    for error in PredictError::examples().iter().chain(&[
        PredictError::NonFinitePrediction { index: 3 },
        PredictError::Io {
            path: PathBuf::from("x"),
            error: std::io::Error::other("boom"),
        },
    ]) {
        let text = error.to_string();
        assert!(!text.is_empty() && !text.contains('\n'), "{text:?}");
    }
}
