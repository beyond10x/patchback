//! Parameter contracts (DESIGN.md § 2 invariant 2, § 5.1, § 5.2).
//!
//! A contract gives one parameter its index in the model's normalized target vector, its stable
//! path, its unit, its mapping from a normalized value `p` in `[0, 1]` to a physical value, the
//! physical range that mapping spans, its default and its clamping policy. Serialized values are
//! physical; model outputs are normalized (§ 5.2).
//!
//! A normalized value outside `[0, 1]` is clamped and the clamp is returned with the result
//! ([`ParameterClamped`], § 8.1 "clamp with an explicit warning"); a NaN or infinite value is
//! refused ([`SetParameterError::NonFiniteParameterValue`], § 2 invariant 4). Nothing is clamped
//! silently.
//!
//! The names follow the ESS domain `patchback.patch` (`spec/domains/patch.yaml`): the
//! `DeclareTopology` and `SetParameter` commands, their errors and the `ParameterClamped` event.

use std::collections::HashSet;
use std::fmt;

/// The unit of a parameter's physical value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterUnit {
    Seconds,
    LinearAmplitude,
    Hertz,
    Dimensionless,
    Decibels,
}

/// How a normalized value `p` maps to the physical value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterMapping {
    /// `at_zero * (at_one / at_zero)^p`; needs `at_zero > 0`.
    Exponential,
    /// `at_zero + (at_one - at_zero) * p`.
    Linear,
}

/// What happens to a normalized value outside `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClampPolicy {
    /// Clamp to `[0, 1]` and record the clamp (§ 5.2, § 8.1).
    ClampToUnitInterval,
}

/// Domain, default, unit, mapping and clamping policy of one parameter (§ 2 invariant 2).
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterContract {
    /// Position of the parameter in the model's normalized target vector (§ 5.2 "Index").
    pub index: usize,
    /// The stable identifier of the parameter within its topology (§ 5.1).
    pub path: String,
    pub unit: ParameterUnit,
    pub mapping: ParameterMapping,
    /// Physical value at `p = 0`, the lower end of the range.
    pub physical_at_zero: f64,
    /// Physical value at `p = 1`, the upper end of the range.
    pub physical_at_one: f64,
    /// The default as a normalized value in `[0, 1]`.
    pub default_normalized: f64,
    pub clamp: ClampPolicy,
}

/// Why a contract is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidContractReason {
    /// An end of the physical range is NaN or infinite.
    NonFiniteBound,
    /// The range is empty or reversed: `physical_at_one <= physical_at_zero`.
    EmptyDomain,
    /// An exponential mapping whose range does not start above zero.
    ExponentialDomainNotPositive,
    /// The default is not a normalized value in `[0, 1]` (NaN included).
    DefaultOutsideUnitInterval,
}

impl fmt::Display for InvalidContractReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFiniteBound => "an end of the physical range is not finite",
            Self::EmptyDomain => {
                "the physical range is empty: the value at 1 is not above the value at 0"
            }
            Self::ExponentialDomainNotPositive => {
                "an exponential mapping needs a physical range that starts above zero"
            }
            Self::DefaultOutsideUnitInterval => "the normalized default is not in [0, 1]",
        })
    }
}

/// A topology declaration refused (`DeclareTopology` errors of `patchback.patch`).
#[derive(Clone, Debug, PartialEq)]
pub enum TopologyError {
    InvalidParameterContract {
        path: String,
        reason: InvalidContractReason,
    },
    DuplicateParameterPath {
        path: String,
    },
    DuplicateParameterIndex {
        index: usize,
    },
}

impl fmt::Display for TopologyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameterContract { path, reason } => {
                write!(f, "invalid parameter contract {path}: {reason}")
            }
            Self::DuplicateParameterPath { path } => {
                write!(f, "parameter path {path} is declared twice")
            }
            Self::DuplicateParameterIndex { index } => {
                write!(f, "parameter index {index} is declared twice")
            }
        }
    }
}

impl std::error::Error for TopologyError {}

/// The record of a clamp: which parameter, the value given and the value used.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterClamped {
    pub path: String,
    pub submitted: f64,
    pub applied: f64,
}

/// A normalized value applied to a contract.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterSetting {
    pub path: String,
    /// The normalized value used, in `[0, 1]`.
    pub normalized: f64,
    /// The physical value the contract maps [`Self::normalized`] to.
    pub physical: f64,
    /// Present when the given value lay outside `[0, 1]`.
    pub clamped: Option<ParameterClamped>,
}

/// A normalized value refused (`SetParameter` errors of `patchback.patch`).
#[derive(Clone, Debug, PartialEq)]
pub enum SetParameterError {
    /// The path names no contract of the topology.
    UnknownParameterPath { path: String },
    /// The value is NaN or infinite.
    NonFiniteParameterValue { path: String, value: f64 },
}

impl fmt::Display for SetParameterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownParameterPath { path } => {
                write!(f, "no parameter contract has the path {path}")
            }
            Self::NonFiniteParameterValue { path, value } => {
                write!(f, "parameter {path}: the value {value} is not finite")
            }
        }
    }
}

impl std::error::Error for SetParameterError {}

/// A physical value the inverse mapping refuses.
#[derive(Clone, Debug, PartialEq)]
pub enum PhysicalValueError {
    /// The value is NaN or infinite.
    NonFinite { path: String, value: f64 },
    /// An exponential contract has no normalized value for a physical value at or below zero.
    OutsideExponentialDomain { path: String, value: f64 },
}

impl fmt::Display for PhysicalValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { path, value } => {
                write!(
                    f,
                    "parameter {path}: the physical value {value} is not finite"
                )
            }
            Self::OutsideExponentialDomain { path, value } => write!(
                f,
                "parameter {path}: the physical value {value} is not above zero, which its exponential mapping needs"
            ),
        }
    }
}

impl std::error::Error for PhysicalValueError {}

impl ParameterContract {
    /// Refuses a contract whose range is not finite, empty or reversed, whose exponential range
    /// does not start above zero, or whose default is not in `[0, 1]`.
    pub fn validate(&self) -> Result<(), TopologyError> {
        let (low, high) = (self.physical_at_zero, self.physical_at_one);
        let reason = if !low.is_finite() || !high.is_finite() {
            Some(InvalidContractReason::NonFiniteBound)
        } else if high <= low {
            Some(InvalidContractReason::EmptyDomain)
        } else if self.mapping == ParameterMapping::Exponential && low <= 0.0 {
            Some(InvalidContractReason::ExponentialDomainNotPositive)
        } else if !(0.0..=1.0).contains(&self.default_normalized) {
            Some(InvalidContractReason::DefaultOutsideUnitInterval)
        } else {
            None
        };
        match reason {
            Some(reason) => Err(TopologyError::InvalidParameterContract {
                path: self.path.clone(),
                reason,
            }),
            None => Ok(()),
        }
    }

    /// The physical value of a normalized value. The caller clamps first ([`Self::apply`]);
    /// outside `[0, 1]` the mapping extrapolates.
    pub fn to_physical(&self, normalized: f64) -> f64 {
        let (low, high) = (self.physical_at_zero, self.physical_at_one);
        match self.mapping {
            ParameterMapping::Exponential => low * (high / low).powf(normalized),
            ParameterMapping::Linear => low + (high - low) * normalized,
        }
    }

    /// The normalized value of a physical value, the inverse of [`Self::to_physical`]. A value
    /// outside the physical range gives a normalized value outside `[0, 1]`, unclamped.
    pub fn to_normalized(&self, physical: f64) -> Result<f64, PhysicalValueError> {
        if !physical.is_finite() {
            return Err(PhysicalValueError::NonFinite {
                path: self.path.clone(),
                value: physical,
            });
        }
        let (low, high) = (self.physical_at_zero, self.physical_at_one);
        Ok(match self.mapping {
            ParameterMapping::Exponential => {
                if physical <= 0.0 {
                    return Err(PhysicalValueError::OutsideExponentialDomain {
                        path: self.path.clone(),
                        value: physical,
                    });
                }
                (physical / low).ln() / (high / low).ln()
            }
            ParameterMapping::Linear => (physical - low) / (high - low),
        })
    }

    /// The physical value of the default.
    pub fn default_physical(&self) -> f64 {
        self.to_physical(self.default_normalized)
    }

    /// Applies a normalized value: refuses NaN and infinity, clamps a finite value to `[0, 1]`
    /// and records the clamp, and maps the value used to its physical value.
    pub fn apply(&self, normalized: f64) -> Result<ParameterSetting, SetParameterError> {
        if !normalized.is_finite() {
            return Err(SetParameterError::NonFiniteParameterValue {
                path: self.path.clone(),
                value: normalized,
            });
        }
        let applied = match self.clamp {
            ClampPolicy::ClampToUnitInterval => normalized.clamp(0.0, 1.0),
        };
        let clamped = (applied != normalized).then(|| ParameterClamped {
            path: self.path.clone(),
            submitted: normalized,
            applied,
        });
        Ok(ParameterSetting {
            path: self.path.clone(),
            normalized: applied,
            physical: self.to_physical(applied),
            clamped,
        })
    }
}

/// A named signal-chain topology and the contracts of its parameters, validated on construction.
#[derive(Clone, Debug, PartialEq)]
pub struct Topology {
    name: String,
    contracts: Vec<ParameterContract>,
}

/// The name of the fixed subtractive voice of § 5.2.
pub const SUBTRACTIVE_MONO: &str = "subtractive-mono/1";

/// The default of every `subtractive-mono/1` parameter, as a normalized value.
///
/// A documented placeholder pending DESIGN.md: § 5.2's default column is open for all eight
/// parameters (§ 17, OQ-1). The value is the one the ESS scenarios carry
/// (`spec/scenarios/parameter-contracts-subtractive-mono.yaml`); it is not a musical choice and
/// is replaced when the owner sets the defaults, before the Milestone A baseline is run.
pub const PLACEHOLDER_DEFAULT_NORMALIZED: f64 = 0.5;

impl Topology {
    /// Declares a topology. Refuses a repeated path, then a repeated index, then the first
    /// invalid contract, in the order of `patchback.patch.DeclareTopology`'s outcomes.
    pub fn new(
        name: impl Into<String>,
        contracts: Vec<ParameterContract>,
    ) -> Result<Self, TopologyError> {
        let mut paths = HashSet::new();
        if let Some(contract) = contracts.iter().find(|c| !paths.insert(c.path.as_str())) {
            return Err(TopologyError::DuplicateParameterPath {
                path: contract.path.clone(),
            });
        }
        let mut indices = HashSet::new();
        if let Some(contract) = contracts.iter().find(|c| !indices.insert(c.index)) {
            return Err(TopologyError::DuplicateParameterIndex {
                index: contract.index,
            });
        }
        contracts.iter().try_for_each(ParameterContract::validate)?;
        Ok(Self {
            name: name.into(),
            contracts,
        })
    }

    /// The fixed subtractive voice of § 5.2: eight contracts in the order of its table.
    ///
    /// Paths are bare names, as the ESS scenarios address them. Every default is
    /// [`PLACEHOLDER_DEFAULT_NORMALIZED`].
    pub fn subtractive_mono() -> Self {
        use ParameterMapping::{Exponential, Linear};
        use ParameterUnit::{Decibels, Dimensionless, Hertz, LinearAmplitude, Seconds};
        let rows = [
            ("attack", Seconds, Exponential, 0.001, 2.0),
            ("decay", Seconds, Exponential, 0.005, 2.0),
            ("sustain", LinearAmplitude, Linear, 0.0, 1.0),
            ("release", Seconds, Exponential, 0.005, 2.0),
            ("filter_cutoff", Hertz, Exponential, 40.0, 16000.0),
            ("resonance_q", Dimensionless, Linear, 0.5, 12.0),
            ("drive", Decibels, Linear, 0.0, 24.0),
            ("output_gain", Decibels, Linear, -24.0, 0.0),
        ];
        let contracts = rows
            .into_iter()
            .enumerate()
            .map(
                |(index, (path, unit, mapping, physical_at_zero, physical_at_one))| {
                    ParameterContract {
                        index,
                        path: path.to_owned(),
                        unit,
                        mapping,
                        physical_at_zero,
                        physical_at_one,
                        default_normalized: PLACEHOLDER_DEFAULT_NORMALIZED,
                        clamp: ClampPolicy::ClampToUnitInterval,
                    }
                },
            )
            .collect();
        match Self::new(SUBTRACTIVE_MONO, contracts) {
            Ok(topology) => topology,
            Err(error) => unreachable!("the subtractive-mono/1 contracts are valid: {error}"),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The contracts, in declaration order.
    pub fn contracts(&self) -> &[ParameterContract] {
        &self.contracts
    }

    /// The contract with this path.
    pub fn contract(&self, path: &str) -> Option<&ParameterContract> {
        self.contracts.iter().find(|c| c.path == path)
    }

    /// Applies a normalized value to the parameter at `path` ([`ParameterContract::apply`]).
    pub fn set(&self, path: &str, normalized: f64) -> Result<ParameterSetting, SetParameterError> {
        self.contract(path)
            .ok_or_else(|| SetParameterError::UnknownParameterPath {
                path: path.to_owned(),
            })?
            .apply(normalized)
    }
}
