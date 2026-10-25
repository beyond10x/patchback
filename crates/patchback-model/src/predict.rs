//! The native predictor (domain command `patchback.model.Predict`; DESIGN.md § 8.2 step 4,
//! § 8.1, ADR-006): the two dense layers and ReLU of a checked [`WeightArtifact`], run in `f32`
//! on one normalized `features/1` vector.
//!
//! Two calls (Da10-4). [`Predictor::load`] reads an artifact through [`WeightArtifact::read`]: a
//! file that does not exist is `UnknownWeightArtifact`, a file the reader refuses is
//! `CorruptWeightArtifact`, and any other I/O failure is [`PredictError::Io`].
//! [`Predictor::predict`] then refuses, in this order:
//!
//! 1. a vector of another length than [`FEATURES_1_VALUE_COUNT`] (`WrongFeatureCount`);
//! 2. a feature contract other than the artifact's, compared as strings
//!    (`FeatureContractMismatch`);
//! 3. a value that is not finite as `f64` or after the cast to `f32` (`NonFiniteFeature`).
//!
//! The spec answers `feature-contract-mismatch` before `corrupt-artifact`; here a corrupt
//! artifact never loads, so it is refused before any vector is compared.
//!
//! The input is what `NormalizationStats::apply` returns; the predictor normalizes nothing
//! (Da10-3). Each value is cast with `as f32`, the cast `x.f32` is written with. The forward pass
//! (Da10-5) is `f32` throughout, no bias, the weights row-major as exported:
//! `h[j] = relu(Σ_i x[i] · w1[i · 64 + j])` and `y[k] = Σ_j h[j] · w2[j · 8 + k]`, each sum
//! accumulated from `0.0` in ascending index order. ReLU is `0` for a negative sum and the sum
//! otherwise, so a NaN is not cut to `0` but reaches the output.
//!
//! The output (Da10-6, Da10-7) is a [`Prediction`]: `raw`, `values` clamped to [0, 1] and the
//! number of outputs that were outside it, `±inf` included. A NaN output is refused as
//! [`PredictError::NonFinitePrediction`], which is no domain error. The predictions with an
//! output clamped are counted per [`Predictor`] ([`Predictor::clamped_predictions`]), in memory;
//! nothing is written and nothing is printed: `clamped > 0` is the caller's warning.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use patchback_analysis::FEATURES_1_VALUE_COUNT;

use crate::{ReadError, W1_SHAPE, W2_SHAPE, WeightArtifact};

const INPUTS: usize = W1_SHAPE[0];
const HIDDEN: usize = W1_SHAPE[1];
const OUTPUTS: usize = W2_SHAPE[1];

// Da10-10: the vector `features/1` holds is w1's input dimension, and w1's output is w2's input.
const _: () = assert!(FEATURES_1_VALUE_COUNT == INPUTS);
const _: () = assert!(HIDDEN == W2_SHAPE[0]);

/// One prediction: the eight outputs as computed, the same clamped to [0, 1], and how many of
/// them were outside [0, 1] (`< 0` or `> 1`, `±inf` included).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prediction {
    pub raw: [f32; OUTPUTS],
    pub values: [f32; OUTPUTS],
    pub clamped: usize,
}

/// Why [`Predictor::load`] or [`Predictor::predict`] gave no prediction. Every variant but the
/// last two names a declared error of `spec/domains/model.yaml` ([`PredictError::domain_error`]).
#[derive(Debug)]
pub enum PredictError {
    /// The vector does not hold [`FEATURES_1_VALUE_COUNT`] values; the number it holds.
    WrongFeatureCount { count: usize },
    /// No file at the path given to [`Predictor::load`].
    UnknownWeightArtifact { path: PathBuf },
    /// The vector's feature contract is not the artifact's; both as given.
    FeatureContractMismatch { artifact: String, input: String },
    /// [`WeightArtifact::read`] refused the file; its reason.
    CorruptWeightArtifact { reason: String },
    /// The value at `index` is NaN or infinite, as `f64` or after the cast to `f32`.
    NonFiniteFeature { index: usize },
    /// Not a domain error: output `index` is NaN, from an `inf − inf` or an `inf · 0` in the
    /// arithmetic (a hidden value past `f32::MAX` times a zero weight). The whole prediction is
    /// refused then, outputs that are `±inf` included (Da10-7, Da10-17).
    NonFinitePrediction { index: usize },
    /// Not a domain error: the file exists and could not be read (a directory, a permission).
    Io { path: PathBuf, error: io::Error },
}

impl PredictError {
    /// The declared errors [`Predictor::load`] and [`Predictor::predict`] emit, in the order
    /// `Predict` declares them.
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
        "patchback.model.WrongFeatureCount",
        "patchback.model.UnknownWeightArtifact",
        "patchback.model.FeatureContractMismatch",
        "patchback.model.CorruptWeightArtifact",
        "patchback.model.NonFiniteFeature",
    ];

    /// The declared error this refusal is, or `None` for a failure that is not the model's.
    pub fn domain_error(&self) -> Option<&'static str> {
        Some(match self {
            PredictError::WrongFeatureCount { .. } => "patchback.model.WrongFeatureCount",
            PredictError::UnknownWeightArtifact { .. } => "patchback.model.UnknownWeightArtifact",
            PredictError::FeatureContractMismatch { .. } => {
                "patchback.model.FeatureContractMismatch"
            }
            PredictError::CorruptWeightArtifact { .. } => "patchback.model.CorruptWeightArtifact",
            PredictError::NonFiniteFeature { .. } => "patchback.model.NonFiniteFeature",
            PredictError::NonFinitePrediction { .. } | PredictError::Io { .. } => return None,
        })
    }

    /// One refusal of each declared error, in [`PredictError::DOMAIN_ERRORS`] order.
    pub fn examples() -> Vec<PredictError> {
        vec![
            PredictError::WrongFeatureCount { count: 31 },
            PredictError::UnknownWeightArtifact {
                path: PathBuf::from("missing.weights"),
            },
            PredictError::FeatureContractMismatch {
                artifact: "features/1".to_owned(),
                input: "features/2".to_owned(),
            },
            PredictError::CorruptWeightArtifact {
                reason: "the payload's SHA-256 is not the header's".to_owned(),
            },
            PredictError::NonFiniteFeature { index: 7 },
        ]
    }
}

impl fmt::Display for PredictError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PredictError::WrongFeatureCount { count } => write!(
                f,
                "the feature vector holds {count} values, not {FEATURES_1_VALUE_COUNT}"
            ),
            PredictError::UnknownWeightArtifact { path } => {
                write!(f, "no weight artifact at {}", path.display())
            }
            PredictError::FeatureContractMismatch { artifact, input } => write!(
                f,
                "the feature vector is {input:?}; the weight artifact was trained for {artifact:?}"
            ),
            PredictError::CorruptWeightArtifact { reason } => {
                write!(f, "corrupt weight artifact: {reason}")
            }
            PredictError::NonFiniteFeature { index } => {
                write!(f, "feature value {index} is not a finite f32")
            }
            PredictError::NonFinitePrediction { index } => {
                write!(f, "output {index} is NaN")
            }
            PredictError::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for PredictError {}

/// A loaded weight artifact and the count of its clamped predictions.
#[derive(Clone, Debug)]
pub struct Predictor {
    artifact: WeightArtifact,
    clamped_predictions: u64,
}

impl Predictor {
    /// Reads and verifies the artifact at `path` ([`WeightArtifact::read`]).
    pub fn load(path: &Path) -> Result<Predictor, PredictError> {
        let artifact = WeightArtifact::read(path).map_err(|error| match error {
            ReadError::Io { path, error } if error.kind() == io::ErrorKind::NotFound => {
                PredictError::UnknownWeightArtifact { path }
            }
            ReadError::Io { path, error } => PredictError::Io { path, error },
            ReadError::Corrupt { reason } => PredictError::CorruptWeightArtifact { reason },
        })?;
        Ok(Predictor::from_artifact(artifact))
    }

    /// A predictor of an artifact the caller already read.
    pub fn from_artifact(artifact: WeightArtifact) -> Predictor {
        Predictor {
            artifact,
            clamped_predictions: 0,
        }
    }

    /// The artifact whose weights are run.
    pub fn artifact(&self) -> &WeightArtifact {
        &self.artifact
    }

    /// How many predictions of this predictor had an output clamped (the domain's
    /// `clamped_predictions`); refused predictions are not counted.
    pub fn clamped_predictions(&self) -> u64 {
        self.clamped_predictions
    }

    /// `Predict`: the prediction of `x`, a normalized vector of `feature_contract`, refused in
    /// the order of the module documentation.
    pub fn predict(
        &mut self,
        feature_contract: &str,
        x: &[f64],
    ) -> Result<Prediction, PredictError> {
        if x.len() != FEATURES_1_VALUE_COUNT {
            return Err(PredictError::WrongFeatureCount { count: x.len() });
        }
        if feature_contract != self.artifact.feature_contract() {
            return Err(PredictError::FeatureContractMismatch {
                artifact: self.artifact.feature_contract().to_owned(),
                input: feature_contract.to_owned(),
            });
        }
        let mut input = [0.0f32; INPUTS];
        for (index, (cast, &value)) in input.iter_mut().zip(x).enumerate() {
            *cast = value as f32;
            if !value.is_finite() || !cast.is_finite() {
                return Err(PredictError::NonFiniteFeature { index });
            }
        }
        let raw = forward(self.artifact.w1(), self.artifact.w2(), &input);
        if let Some(index) = raw.iter().position(|value| value.is_nan()) {
            return Err(PredictError::NonFinitePrediction { index });
        }
        let values = raw.map(|value| value.clamp(0.0, 1.0));
        let clamped = raw
            .iter()
            .filter(|&&value| value < 0.0 || value > 1.0)
            .count();
        if clamped > 0 {
            self.clamped_predictions += 1;
        }
        Ok(Prediction {
            raw,
            values,
            clamped,
        })
    }
}

/// `relu(x · w1) · w2`, row-major, `f32`, each sum from `0.0` in ascending index order.
fn forward(w1: &[f32], w2: &[f32], x: &[f32; INPUTS]) -> [f32; OUTPUTS] {
    let mut h = [0.0f32; HIDDEN];
    for (j, hidden) in h.iter_mut().enumerate() {
        let mut sum = 0.0f32;
        for (i, value) in x.iter().enumerate() {
            sum += value * w1[i * HIDDEN + j];
        }
        *hidden = if sum < 0.0 { 0.0 } else { sum };
    }
    let mut y = [0.0f32; OUTPUTS];
    for (k, output) in y.iter_mut().enumerate() {
        let mut sum = 0.0f32;
        for (j, hidden) in h.iter().enumerate() {
            sum += hidden * w2[j * OUTPUTS + k];
        }
        *output = sum;
    }
    y
}
