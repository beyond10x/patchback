//! Adversary pass 1 on story:native-predictor (wave a10): cases the implementor's suite does not
//! state. Each artifact is written here from chosen weights (the `weights/1` bytes of
//! `website/docs/reference/files.md` § Exported weights), never downloaded.
//!
//! The forward-pass mutants (Da10-5) are expressed inside the cases, as `mutant_forward`, so the
//! file under attack is never touched: each case asserts the predictor's literal, hand-computed
//! result and that every named mutant computes another value on the same input.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
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

fn dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-adversary-predictor");
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_artifact(name: &str, w1: &[f32], w2: &[f32]) -> PathBuf {
    let path = dir().join(format!("{name}.weights"));
    fs::write(&path, artifact_bytes(w1, w2, FEATURES_1_VERSION)).unwrap();
    path
}

fn predictor(name: &str, w1: &[f32], w2: &[f32]) -> Predictor {
    Predictor::load(&write_artifact(name, w1, w2)).unwrap_or_else(|e| panic!("{e}"))
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

/// The forward pass as Da10-5 states it, and the mutants a suite has to tell apart from it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mutant {
    /// Da10-5: `f32`, each sum from `0.0` in ascending index order, row-major.
    Spec,
    /// Each sum accumulated in `f64` and cast to `f32` at the end.
    F64,
    /// Each sum in descending index order.
    Descending,
    /// Each sum as a pairwise tree over the products.
    Pairwise,
    /// w1 read as `w1[j · 32 + i]`.
    TransposedW1,
    /// w2 read as `w2[k · 64 + j]`.
    TransposedW2,
}

/// The accumulation mutants, which no tolerance-based check of this suite tells apart.
const ORDER_MUTANTS: [Mutant; 3] = [Mutant::F64, Mutant::Descending, Mutant::Pairwise];

const MUTANTS: [Mutant; 5] = [
    Mutant::F64,
    Mutant::Descending,
    Mutant::Pairwise,
    Mutant::TransposedW1,
    Mutant::TransposedW2,
];

fn pairwise(products: &[f32]) -> f32 {
    match products.len() {
        0 => 0.0,
        1 => products[0],
        n => pairwise(&products[..n / 2]) + pairwise(&products[n / 2..]),
    }
}

fn layer_sum(mutant: Mutant, products: &[f32], wide: &[f64]) -> f32 {
    match mutant {
        Mutant::F64 => wide.iter().fold(0.0f64, |sum, v| sum + v) as f32,
        Mutant::Descending => products.iter().rev().fold(0.0f32, |sum, v| sum + v),
        Mutant::Pairwise => pairwise(products),
        _ => products.iter().fold(0.0f32, |sum, v| sum + v),
    }
}

fn mutant_forward(mutant: Mutant, w1: &[f32], w2: &[f32], x: &[f32]) -> [f32; OUTPUTS] {
    let w1_index = |i: usize, j: usize| match mutant {
        Mutant::TransposedW1 => j * INPUTS + i,
        _ => w1_at(i, j),
    };
    let w2_index = |j: usize, k: usize| match mutant {
        Mutant::TransposedW2 => k * HIDDEN + j,
        _ => w2_at(j, k),
    };
    let mut h = [0.0f32; HIDDEN];
    for (j, hidden) in h.iter_mut().enumerate() {
        let products: Vec<f32> = (0..INPUTS).map(|i| x[i] * w1[w1_index(i, j)]).collect();
        let wide: Vec<f64> = (0..INPUTS)
            .map(|i| f64::from(x[i]) * f64::from(w1[w1_index(i, j)]))
            .collect();
        let sum = layer_sum(mutant, &products, &wide);
        *hidden = if sum < 0.0 { 0.0 } else { sum };
    }
    let mut y = [0.0f32; OUTPUTS];
    for (k, output) in y.iter_mut().enumerate() {
        let products: Vec<f32> = (0..HIDDEN).map(|j| h[j] * w2[w2_index(j, k)]).collect();
        let wide: Vec<f64> = (0..HIDDEN)
            .map(|j| f64::from(h[j]) * f64::from(w2[w2_index(j, k)]))
            .collect();
        *output = layer_sum(mutant, &products, &wide);
    }
    y
}

fn as_f32(x: &[f64]) -> Vec<f32> {
    x.iter().map(|&v| v as f32).collect()
}

/// Asserts the predictor's `raw` for each input is the literal `expected`, that the Da10-5
/// statement computes the same, and that each mutant computes another `raw` on at least one input.
fn assert_pins_every_mutant(
    p: &mut Predictor,
    w1: &[f32],
    w2: &[f32],
    cases: &[(Vec<f64>, [f32; OUTPUTS])],
    mutants: &[Mutant],
) {
    for (n, (x, expected)) in cases.iter().enumerate() {
        let prediction = predicted(p, x);
        assert_eq!(
            bits(&prediction.raw),
            bits(expected),
            "input {n}: raw {:?}, expected {expected:?}",
            prediction.raw
        );
        assert_eq!(
            bits(&mutant_forward(Mutant::Spec, w1, w2, &as_f32(x))),
            bits(expected),
            "input {n}: the Da10-5 statement"
        );
    }
    for &mutant in mutants {
        let results: Vec<[f32; OUTPUTS]> = cases
            .iter()
            .map(|(x, _)| mutant_forward(mutant, w1, w2, &as_f32(x)))
            .collect();
        assert!(
            cases
                .iter()
                .zip(&results)
                .any(|((_, expected), got)| bits(expected) != bits(got)),
            "{mutant:?} survives: {results:?}"
        );
    }
}

/// Da10-5, first layer: `h[0] = x[0] + x[1] + x[2] + x[3]` (w1[i, 0] = 1, w2[0, 0] = 1), with
/// 1e8 + 1 = 1e8 in `f32` (the spacing at 1e8 is 8). Ascending sequential `f32` gives 0, 1, 0, 1;
/// `f64` gives 1, 1, 1, 2; descending 0, 0, 1, 0; pairwise 0, 1, 0, 0. The `f64` reference check of
/// `predictor.rs` (1e-4) and the pinned equivalence (1e-5, Da10-16) accept the `f64` and the order mutants.
#[test]
fn the_first_layer_sums_in_f32_from_index_0_upward() {
    let (mut w1, mut w2) = zeros();
    for i in 0..4 {
        w1[w1_at(i, 0)] = 1.0;
    }
    w2[w2_at(0, 0)] = 1.0;
    let mut p = predictor("first-layer-order", &w1, &w2);
    let cases = [
        (input(&[(0, 1e8), (1, 1.0), (2, -1e8)]), outputs(&[])),
        (
            input(&[(0, 1e8), (1, -1e8), (2, 1.0)]),
            outputs(&[(0, 1.0)]),
        ),
        (input(&[(0, 1.0), (1, 1e8), (2, -1e8)]), outputs(&[])),
        (
            input(&[(0, 1e8), (1, 1.0), (2, -1e8), (3, 1.0)]),
            outputs(&[(0, 1.0)]),
        ),
    ];
    assert_pins_every_mutant(&mut p, &w1, &w2, &cases, &ORDER_MUTANTS);
    assert_eq!(p.clamped_predictions(), 0);
}

/// Da10-5, second layer: h = (1e8, 1, 1e8, 1, 0, …) through w1, then
/// `y[0] = h[0] − h[2] + h[1] + h[3]` ordered as w2[0..4, 0] = (1, 1, −1, 1): products
/// (1e8, 1, −1e8, 1). Ascending sequential `f32` gives 1; `f64` 2; descending 0; pairwise 0.
#[test]
fn the_second_layer_sums_in_f32_from_index_0_upward() {
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 1.0;
    w1[w1_at(1, 1)] = 1.0;
    w1[w1_at(0, 2)] = 1.0;
    w1[w1_at(1, 3)] = 1.0;
    for (j, w) in [1.0, 1.0, -1.0, 1.0].into_iter().enumerate() {
        w2[w2_at(j, 0)] = w;
    }
    let mut p = predictor("second-layer-order", &w1, &w2);
    let cases = [(input(&[(0, 1e8), (1, 1.0)]), outputs(&[(0, 1.0)]))];
    assert_pins_every_mutant(&mut p, &w1, &w2, &cases, &ORDER_MUTANTS);
}

/// Da10-5 row-major, asymmetric: every weight distinct, the result computed by hand.
/// x = (1, 2, 0, …); w1[0, 0] = 0.5, w1[1, 0] = 0.25, w1[0, 1] = 2, w1[1, 1] = −0.5,
/// w1[0, 2] = 0.125, w1[1, 2] = 4, w1[2, 0] = 8 (x[2] = 0, so only a misread reaches it):
/// h = (1, 1, 8.125). w2[0, 0] = 0.5, w2[1, 0] = 0.25, w2[2, 0] = 0.0625, w2[0, 1] = 0.125:
/// y[0] = 0.5 + 0.25 + 0.5078125 = 1.2578125, y[1] = 0.125.
#[test]
fn an_asymmetric_row_major_layout_computed_by_hand() {
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 0.5;
    w1[w1_at(1, 0)] = 0.25;
    w1[w1_at(0, 1)] = 2.0;
    w1[w1_at(1, 1)] = -0.5;
    w1[w1_at(0, 2)] = 0.125;
    w1[w1_at(1, 2)] = 4.0;
    w1[w1_at(2, 0)] = 8.0;
    w2[w2_at(0, 0)] = 0.5;
    w2[w2_at(1, 0)] = 0.25;
    w2[w2_at(2, 0)] = 0.0625;
    w2[w2_at(0, 1)] = 0.125;
    let mut p = predictor("asymmetric", &w1, &w2);
    // h[0] = 1 · 0.5 + 2 · 0.25 = 1; h[1] = 1 · 2 + 2 · (−0.5) = 1; h[2] = 0.125 + 8 = 8.125.
    let x = input(&[(0, 1.0), (1, 2.0)]);
    let expected = outputs(&[(0, 1.2578125), (1, 0.125)]);
    let prediction = predicted(&mut p, &x);
    assert_eq!(prediction.raw, expected);
    assert_eq!(prediction.values, outputs(&[(0, 1.0), (1, 0.125)]));
    assert_eq!(prediction.clamped, 1);
    assert_pins_every_mutant(
        &mut p,
        &w1,
        &w2,
        &[(x, expected)],
        &[Mutant::TransposedW1, Mutant::TransposedW2],
    );
    assert_eq!(p.clamped_predictions(), 2);
}

/// Da10-6 boundaries of the clamp: exactly 0 and 1 are not counted, `1 + ε` and the smallest
/// negative subnormal are; all eight outside counts 8 and the counter once.
#[test]
fn the_clamp_boundaries() {
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 1.0;
    for k in 0..OUTPUTS {
        w2[w2_at(0, k)] = 1.0;
    }
    let mut p = predictor("clamp-boundaries", &w1, &w2);
    let exactly_one = predicted(&mut p, &input(&[(0, 1.0)]));
    assert_eq!(exactly_one.raw, [1.0; OUTPUTS]);
    assert_eq!(exactly_one.clamped, 0);
    let above = predicted(&mut p, &input(&[(0, f64::from(1.0f32 + f32::EPSILON))]));
    assert_eq!(above.raw, [1.0 + f32::EPSILON; OUTPUTS]);
    assert_eq!(above.values, [1.0; OUTPUTS]);
    assert_eq!(above.clamped, 8);
    assert_eq!(p.clamped_predictions(), 1, "one prediction, eight outputs");

    // All 32 values 0, and all 32 values −0.0: every output is +0.0 (sums start at +0.0), none
    // clamped.
    for x in [vec![0.0; 32], vec![-0.0; 32]] {
        let zero = predicted(&mut p, &x);
        assert_eq!(bits(&zero.raw), vec![0u32; OUTPUTS]);
        assert_eq!(bits(&zero.values), vec![0u32; OUTPUTS]);
        assert_eq!(zero.clamped, 0);
    }
    assert_eq!(p.clamped_predictions(), 1);

    // The smallest negative f32 as an output: h[0] = x[0] is the smallest positive subnormal,
    // y[3] = h[0] · (−1).
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 1.0;
    w2[w2_at(0, 3)] = -1.0;
    let mut q = predictor("clamp-tiny-negative", &w1, &w2);
    let tiny = f64::from(f32::from_bits(1));
    let below = predicted(&mut q, &input(&[(0, tiny)]));
    assert_eq!(below.raw[3].to_bits(), (-f32::from_bits(1)).to_bits());
    assert_eq!(below.values[3].to_bits(), 0.0f32.to_bits());
    assert_eq!(below.clamped, 1);
    assert_eq!(q.clamped_predictions(), 1);
}

/// Da10-3, Da10-4: the cast's edges. A subnormal `f64` that becomes 0 in `f32`, `f64::MIN_POSITIVE`
/// and a value that rounds down to `f32::MAX` are accepted; the first `f64` that rounds up to
/// `inf` is `NonFiniteFeature`; two non-finite values name the first, at 0 and at 31.
#[test]
fn the_cast_edges() {
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 1.0;
    w2[w2_at(0, 0)] = 1.0;
    let mut p = predictor("cast-edges", &w1, &w2);
    for value in [1e-310, f64::MIN_POSITIVE, -1e-310] {
        let prediction = predicted(&mut p, &input(&[(0, value)]));
        assert_eq!(bits(&prediction.raw), vec![0u32; OUTPUTS], "{value:e}");
    }
    let max = f64::from(f32::MAX);
    let half_ulp = 2f64.powi(103);
    let below_tie = predicted(&mut p, &input(&[(0, max + half_ulp / 2.0)]));
    assert_eq!(below_tie.raw[0], f32::MAX);
    assert_eq!(below_tie.clamped, 1);
    for (index, value) in [(0, max + half_ulp), (31, -(max + half_ulp))] {
        assert!(matches!(
            p.predict(FEATURES_1_VERSION, &input(&[(index, value)])),
            Err(PredictError::NonFiniteFeature { index: i }) if i == index
        ));
    }
    let mut x = input(&[]);
    x[0] = f64::NAN;
    x[31] = f64::INFINITY;
    assert!(matches!(
        p.predict(FEATURES_1_VERSION, &x),
        Err(PredictError::NonFiniteFeature { index: 0 })
    ));
    let mut x = input(&[]);
    x[31] = f64::NAN;
    assert!(matches!(
        p.predict(FEATURES_1_VERSION, &x),
        Err(PredictError::NonFiniteFeature { index: 31 })
    ));
    assert_eq!(p.clamped_predictions(), 1);
}

/// Da10-4, two defects at once: a missing file and 31 values is `UnknownWeightArtifact` from
/// `load` (the count is never compared); a 33-value NaN vector of another contract is the count.
#[test]
fn two_defects_at_once() {
    let missing = dir().join("two-defects-missing.weights");
    let _ = fs::remove_file(&missing);
    match Predictor::load(&missing) {
        Err(PredictError::UnknownWeightArtifact { path }) => assert_eq!(path, missing),
        other => panic!("{:?}", other.map(drop)),
    }
    let (w1, w2) = zeros();
    let mut p = predictor("two-defects", &w1, &w2);
    assert!(matches!(
        p.predict("features/2", &[f64::NAN; 33]),
        Err(PredictError::WrongFeatureCount { count: 33 })
    ));
    assert!(matches!(
        p.predict("features/2", &[f64::NAN; 32]),
        Err(PredictError::FeatureContractMismatch { .. })
    ));
}

/// Da10-4: the feature contract is compared whole; a 1,000-byte contract and a prefix of the
/// artifact's are refused and carried back as given.
#[test]
fn the_contract_is_compared_whole() {
    let (w1, w2) = zeros();
    let mut p = predictor("contract-whole", &w1, &w2);
    let long = "f".repeat(1000);
    for contract in [
        "features/",
        "features",
        "features/1\0",
        "features/1\n",
        long.as_str(),
    ] {
        match p.predict(contract, &input(&[])) {
            Err(PredictError::FeatureContractMismatch { artifact, input }) => {
                assert_eq!(
                    (artifact.as_str(), input.as_str()),
                    ("features/1", contract)
                );
            }
            other => panic!("{contract:?}: {other:?}"),
        }
    }
}

/// Da10-4 `load` on the file system's edges: a dangling symlink is `UnknownWeightArtifact`, a
/// symlink to a valid file loads, an unreadable file and a path with a NUL are `Io` (no domain
/// error), a path under a regular file is `Io`.
#[test]
fn load_on_the_file_systems_edges() {
    let (w1, w2) = zeros();
    let good = write_artifact("edges-good", &w1, &w2);

    let dangling = dir().join("edges-dangling.weights");
    let _ = fs::remove_file(&dangling);
    std::os::unix::fs::symlink(dir().join("edges-nowhere.weights"), &dangling).unwrap();
    match Predictor::load(&dangling) {
        Err(error @ PredictError::UnknownWeightArtifact { .. }) => {
            assert_eq!(
                error.domain_error(),
                Some("patchback.model.UnknownWeightArtifact")
            );
        }
        other => panic!("{:?}", other.map(drop)),
    }

    let link = dir().join("edges-link.weights");
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(&good, &link).unwrap();
    assert_eq!(
        Predictor::load(&link).unwrap().artifact(),
        &WeightArtifact::read(&good).unwrap()
    );

    let nul = PathBuf::from("edges\0nul.weights");
    match Predictor::load(&nul) {
        Err(error @ PredictError::Io { .. }) => assert_eq!(error.domain_error(), None),
        other => panic!("{:?}", other.map(drop)),
    }

    let under_a_file = good.join("x.weights");
    match Predictor::load(&under_a_file) {
        Err(error @ PredictError::Io { .. }) => assert_eq!(error.domain_error(), None),
        other => panic!("{:?}", other.map(drop)),
    }

    let locked = write_artifact("edges-locked", &w1, &w2);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let readable = fs::read(&locked).is_ok();
    let result = Predictor::load(&locked);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
    if readable {
        say("load_on_the_file_systems_edges: mode 0o000 is readable here (root?); not asserted");
    } else {
        match result {
            Err(PredictError::Io { path, error }) => {
                assert_eq!(path, locked);
                assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
            }
            other => panic!("{:?}", other.map(drop)),
        }
    }
}

/// Da10-6: the counter is the `Predictor`'s; two from the same file count apart, `from_artifact`
/// starts at 0, a refused `NonFinitePrediction` whose other outputs are infinite is not counted,
/// and a clone carries the count it was cloned with.
#[test]
fn the_counter_is_the_predictors() {
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 1.0;
    w2[w2_at(0, 0)] = 2.0;
    let path = write_artifact("counter-apart", &w1, &w2);
    let mut a = Predictor::load(&path).unwrap();
    let mut b = Predictor::load(&path).unwrap();
    assert_eq!(predicted(&mut a, &input(&[(0, 1.0)])).clamped, 1);
    assert_eq!(predicted(&mut a, &input(&[(0, 1.0)])).clamped, 1);
    assert_eq!(predicted(&mut b, &input(&[(0, 1.0)])).clamped, 1);
    assert_eq!((a.clamped_predictions(), b.clamped_predictions()), (2, 1));
    let c = a.clone();
    assert_eq!(c.clamped_predictions(), 2);
    let d = Predictor::from_artifact(a.artifact().clone());
    assert_eq!(d.clamped_predictions(), 0);

    // h[0] overflows to +inf (x[0] = 3e38, w1 = 2); y[0] = inf · 2 = inf, y[1] = inf · 0 = NaN.
    let (mut w1, mut w2) = zeros();
    w1[w1_at(0, 0)] = 2.0;
    w2[w2_at(0, 0)] = 2.0;
    let mut e = predictor("counter-zero-times-inf", &w1, &w2);
    assert!(matches!(
        e.predict(FEATURES_1_VERSION, &input(&[(0, 3e38)])),
        Err(PredictError::NonFinitePrediction { index: 1 })
    ));
    assert_eq!(e.clamped_predictions(), 0);
}

// The pinned lane: the equivalence against ModelSpec's `eval loss` must tell the layout apart.

const TRAIN_ROWS: i64 = 64;
const EVAL_ROWS: usize = 16;
const STEPS: u32 = 4;
const EQUIVALENCE: f64 = 1e-3;

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

#[test]
fn the_pinned_equivalence_tells_the_layout_apart() {
    let test = "the_pinned_equivalence_tells_the_layout_apart";
    let pin = ModelSpecPin::repository().unwrap();
    let Some(binary) = binary(test, &pin) else {
        return;
    };
    let modelspec = ModelSpec::open(&binary, &pin).unwrap_or_else(|e| panic!("{e}"));
    let model =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec");
    let mut source = ModelSource::register(&model, "PatchRegressor", 16, &pin);
    assert_eq!(source.check(&modelspec).unwrap(), ModelSourceState::Checked);

    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-adversary-pinned");
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

    let mut native = Vec::new();
    for row in &rows {
        let values: Vec<f64> = row.iter().map(|&v| f64::from(v)).collect();
        let prediction = predicted(&mut predictor, &values);
        assert_eq!(
            bits(&prediction.raw),
            bits(&mutant_forward(Mutant::Spec, &w1, &w2, row)),
            "the predictor is the Da10-5 statement, bit for bit"
        );
        native.push(prediction.raw);
    }
    let native_mse = mse(&native, &y);
    let mut report = vec![format!(
        "native {native_mse} (relative {:e})",
        relative(native_mse, eval_loss)
    )];
    assert!(relative(native_mse, eval_loss) <= EQUIVALENCE);

    let mut survivors = Vec::new();
    let mut judge = |name: String, value: f64, must_differ: bool| {
        let r = relative(value, eval_loss);
        report.push(format!("{name} {value} (relative {r:e})"));
        if must_differ && r <= EQUIVALENCE {
            survivors.push(format!("{name}: relative {r:e} <= {EQUIVALENCE:e}"));
        }
    };
    for mutant in MUTANTS {
        let predictions: Vec<[f32; OUTPUTS]> = rows
            .iter()
            .map(|row| mutant_forward(mutant, &w1, &w2, row))
            .collect();
        let layout = matches!(mutant, Mutant::TransposedW1 | Mutant::TransposedW2);
        judge(format!("{mutant:?}"), mse(&predictions, &y), layout);
    }
    judge(
        "zero output".to_owned(),
        mse(&[[0.0; OUTPUTS]; EVAL_ROWS], &y),
        true,
    );
    let shifted: Vec<[f32; OUTPUTS]> = (0..EVAL_ROWS)
        .map(|r| {
            let mut row: Vec<f32> = x[r * INPUTS + 1..].iter().copied().take(INPUTS).collect();
            row.resize(INPUTS, 0.0);
            mutant_forward(Mutant::Spec, &w1, &w2, &row)
        })
        .collect();
    judge("x read one value late".to_owned(), mse(&shifted, &y), true);
    let misaligned: Vec<[f32; OUTPUTS]> = (0..EVAL_ROWS)
        .map(|r| native[(r + 1) % EVAL_ROWS])
        .collect();
    judge(
        "x row r against y row r + 1".to_owned(),
        mse(&misaligned, &y),
        true,
    );
    say(&format!(
        "{test}: eval loss {eval_loss}; {}",
        report.join("; ")
    ));
    assert!(
        survivors.is_empty(),
        "the pinned equivalence accepts: {survivors:?}"
    );
}
