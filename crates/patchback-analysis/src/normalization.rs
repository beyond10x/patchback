//! Normalization statistics of the training split (DESIGN.md § 6.3, § 7.1;
//! `spec/domains/analysis.yaml` `NormalizationStats`, `ComputeNormalizationStats`,
//! `ApplyNormalization`; story:normalization-stats) and their artifact, `normalization.stats`.
//!
//! # Computing
//!
//! [`compute_normalization_stats`] takes the split as an input, because this crate cannot see the
//! dataset store (`patchback-dataset` depends on it): the caller passes the split's role and rows
//! as a [`SplitInput`], or `None` for a split it does not find (`patchback_dataset::
//! compute_split_stats` does). It refuses in the order the model declares the outcomes
//! (Da8-12): no such split, no such contract, a split that is not the training split, a draft
//! contract, a version other than the contract's. Then every row is checked by the contract's own
//! check ([`FeatureContract::check`]), so no non-finite value enters the statistics.
//!
//! The statistics are of the rows as given (Da8-13): through `patchback-dataset`, the training
//! samples' `features/1` values as the store holds them, in `f64`, in draw order. `x.f32` holds
//! their z-scores under these statistics rounded to `f32` (Da8-30); the statistics are of the
//! `f64` values. Per feature, in `f64` and two passes, the mean `sum x / n`, then the population
//! standard deviation `sqrt(sum (x − mean)² / n)` (divisor `n`, not `n − 1`); a feature whose
//! every value is bit-equal to its first row's has that value as its mean and standard deviation
//! 0 (Da8-44), whatever that value is. For a feature that is not constant, a mean or standard
//! deviation that would not be finite (finite rows whose sums overflow) is refused
//! ([`NormalizationError::NonFiniteFeature`]).
//!
//! A split of no rows gives mean 0 and standard deviation 0 for every feature (Da8-14). The
//! generator never produces one: `SplitCounts::from_samples` gives the training split
//! `samples − 2 · held_out` rows, and `SplitCounts::check` refuses every count that is not a
//! positive multiple of 16 before anything is drawn (`crates/patchback-dataset/src/generate.rs`
//! lines 90-103 and 115-127), so an admitted generation's training split holds at least 16 rows.
//!
//! # Applying
//!
//! [`NormalizationStats::apply`] is `ApplyNormalization` over one vector, the statistics read and
//! left unchanged: features of another contract version are refused
//! ([`NormalizationError::NormalizationContractMismatch`]), then a non-finite input value, by the
//! rule of [`FeatureContract::check`]'s last step ([`NormalizationError::NonFiniteFeature`]).
//! Each value is `(x[i] − mean[i]) / std[i]`, with divisor 1 where `std[i]` is 0 (Da8-14, the
//! scaler convention); a value that would not be finite (a tiny standard deviation far from the
//! mean) is refused, so no non-finite value leaves the function (DESIGN.md § 2 invariant 4).
//! `NormalizationStatsNotFound` has no counterpart: statistics are a value, and no call can name
//! statistics that do not exist.
//!
//! # `normalization.stats`
//!
//! [`write_stats`] writes the table of the statistics ([`NormalizationTable`]) and
//! [`read_stats`] reads it back (Da8-15). UTF-8 text of lines, each ended by `\n` (the reader also
//! takes `\r\n`, and a last line with no terminator); fields are separated by one space (the
//! reader takes any run of ASCII whitespace). No comments, no blank lines. The lines, in this
//! order:
//!
//! ```text
//! patchback-normalization-stats 1     the header: the format version
//! feature_contract features/1         the version of the contract the statistics are under
//! split train                         the split they are of: always the training split
//! rows <n>                            the rows they were computed from
//! std population                      the standard deviation's divisor: n
//! feature <i> <mean> <std>            one line per feature, i from 0 to 31 in order
//! ```
//!
//! Integers are ASCII digits only. Reals are Rust's shortest round-trip text of the `f64`, the
//! plain form (`{}`) or the exponent form (`{:e}`) when that is shorter
//! (`patchback_core::patch::format_decimal`), and are read with the `.patch` grammar's finite
//! decimal (`patchback_core::patch::parse_decimal`), so a table read back is the table written,
//! bit for bit. The contract version is written as the contract's definition names it; a version
//! holding whitespace, or longer than the line bound allows, does not read back (no contract this
//! repository defines has one).
//!
//! [`read_stats`] answers the first fault, line by line, with a [`StatsFileError`] naming the
//! line: a line longer than [`STATS_MAX_LINE_BYTES`] bytes (without its terminator) is refused
//! before it is split; a header other than `patchback-normalization-stats <version>`, or a
//! version other than 1, on line 1; a record other than the one its place calls for, with the
//! wrong field count, a split other than `train` or a divisor other than `population`, or any
//! line after feature 31, is a malformed record; an integer or real that does not read is a
//! malformed number; a negative standard deviation, a feature index below the one expected
//! (repeated) or above it (a feature missing or out of order) each have their own reason; a text
//! that ends before feature 31 is incomplete.

use std::fmt::{self, Write as _};

use patchback_core::patch::{format_decimal, parse_decimal};
use patchback_core::split::SplitRole;

use crate::{
    FEATURES_1_VALUE_COUNT, FeatureContract, FeatureContractId, FeatureContractState, FeatureError,
    FeatureRegistry, NormalizationError,
};

const N: usize = FEATURES_1_VALUE_COUNT;

/// The artifact's file name under a dataset's output directory (Da8-15, Da8-16).
pub const NORMALIZATION_STATS_FILE: &str = "normalization.stats";

/// The first field of the header line.
pub const STATS_HEADER: &str = "patchback-normalization-stats";

/// The `normalization.stats` format version this module reads and writes.
pub const STATS_FORMAT_VERSION: u32 = 1;

/// The longest line, in bytes without its terminator. Provisional: a feature line with both reals
/// at their longest (24 bytes, as `-1.2345678901234567e-308`) is 61 bytes.
pub const STATS_MAX_LINE_BYTES: usize = 256;

const FEATURE_CONTRACT: &str = "feature_contract";
const SPLIT: &str = "split";
const ROWS: &str = "rows";
const STD: &str = "std";
const POPULATION: &str = "population";
const FEATURE: &str = "feature";

/// The split statistics are computed from, as its caller found it: its role and its rows, each a
/// `features/1` vector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitInput<'a> {
    pub role: SplitRole,
    pub rows: &'a [[f64; N]],
}

/// What `normalization.stats` holds: the contract version, the split's role (always
/// [`SplitRole::Train`]), the row count, and the mean and population standard deviation of each
/// feature, each finite, each standard deviation at least 0. Only [`compute_normalization_stats`]
/// and [`read_stats`] make one.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizationTable {
    feature_contract_version: String,
    split_role: SplitRole,
    rows: u64,
    means: [f64; N],
    stds: [f64; N],
}

impl NormalizationTable {
    /// The version of the contract the statistics were computed under.
    pub fn feature_contract_version(&self) -> &str {
        &self.feature_contract_version
    }

    /// The role of the split they were computed from: [`SplitRole::Train`].
    pub fn split_role(&self) -> SplitRole {
        self.split_role
    }

    /// The rows they were computed from.
    pub fn rows(&self) -> u64 {
        self.rows
    }

    pub fn means(&self) -> &[f64; N] {
        &self.means
    }

    /// The population standard deviations (divisor n).
    pub fn stds(&self) -> &[f64; N] {
        &self.stds
    }

    /// The z-scores of `x`: `(x[i] − mean[i]) / std[i]` in `f64`, divisor 1 where `std[i]` is 0
    /// (Da8-14), unchecked: a value of `x` or of the result that is not finite is returned as
    /// it is. [`NormalizationStats::apply`] checks both; the dense writer of `patchback_dataset`
    /// leaves them to its read-back (Da8-30).
    pub fn normalize(&self, x: &[f64; N]) -> [f64; N] {
        std::array::from_fn(|i| {
            let std = self.stds[i];
            let divisor = if std == 0.0 { 1.0 } else { std };
            (x[i] - self.means[i]) / divisor
        })
    }
}

/// `patchback.analysis.NormalizationStats`: the statistics of one training split under one
/// published contract, with the entity's fields (the contract, its version, the split and its
/// role) and the table of means and standard deviations the entity does not model (Da8-19).
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizationStats {
    feature_contract_id: FeatureContractId,
    dataset_split_id: u64,
    table: NormalizationTable,
}

impl NormalizationStats {
    pub fn feature_contract_id(&self) -> FeatureContractId {
        self.feature_contract_id
    }

    /// The number of the split's `patchback_dataset::DatasetSplitId`.
    pub fn dataset_split_id(&self) -> u64 {
        self.dataset_split_id
    }

    pub fn feature_contract_version(&self) -> &str {
        self.table.feature_contract_version()
    }

    /// [`SplitRole::Train`], the entity's invariant.
    pub fn split_role(&self) -> SplitRole {
        self.table.split_role()
    }

    pub fn rows(&self) -> u64 {
        self.table.rows()
    }

    pub fn means(&self) -> &[f64; N] {
        self.table.means()
    }

    pub fn stds(&self) -> &[f64; N] {
        self.table.stds()
    }

    /// The table `normalization.stats` holds.
    pub fn table(&self) -> &NormalizationTable {
        &self.table
    }

    /// `ApplyNormalization` of `x`, a vector of `feature_contract_version` (module
    /// documentation): `(x[i] − mean[i]) / std[i]`, divisor 1 where `std[i]` is 0; refused, in
    /// this order, for another version than the statistics' and for a value of `x`, or of the
    /// result, that is not finite.
    pub fn apply(
        &self,
        feature_contract_version: &str,
        x: &[f64; N],
    ) -> Result<[f64; N], NormalizationError> {
        if feature_contract_version != self.table.feature_contract_version {
            return Err(NormalizationError::NormalizationContractMismatch {
                feature_contract_version: feature_contract_version.to_string(),
            });
        }
        let non_finite = |index| NormalizationError::NonFiniteFeature {
            feature_contract_id: self.feature_contract_id,
            index,
        };
        if let Some(index) = x.iter().position(|v| !v.is_finite()) {
            return Err(non_finite(index));
        }
        let normalized = self.table.normalize(x);
        if let Some(index) = normalized.iter().position(|v| !v.is_finite()) {
            return Err(non_finite(index));
        }
        Ok(normalized)
    }
}

/// `ComputeNormalizationStats` (module documentation): the statistics of `split`, the split
/// `dataset_split_id`, under the contract `feature_contract_id` of `registry` and the version
/// `feature_contract_version`, which must be the contract's. `split` is `None` for a split the
/// caller does not find.
///
/// A column whose every value is bit-equal (`f64::to_bits`) to its first row's value has that
/// value as its mean and standard deviation 0 (Da8-44): the two-pass formula would give a
/// non-dyadic constant such as 0.1 a rounding-noise standard deviation (1e-17 and up), and
/// `apply` would then give it ±1 instead of +0.0 (Da8-14). Every other column takes the two-pass
/// mean and population standard deviation.
pub fn compute_normalization_stats(
    registry: &FeatureRegistry,
    feature_contract_id: FeatureContractId,
    feature_contract_version: &str,
    dataset_split_id: u64,
    split: Option<SplitInput<'_>>,
) -> Result<NormalizationStats, NormalizationError> {
    let Some(split) = split else {
        return Err(NormalizationError::DatasetSplitNotFound { dataset_split_id });
    };
    let contract = registry.get(feature_contract_id).map_err(|_| {
        NormalizationError::FeatureContractNotFound {
            feature_contract_id,
        }
    })?;
    if split.role != SplitRole::Train {
        return Err(NormalizationError::StatsFromNonTrainingSplit {
            split_role: split.role,
        });
    }
    if contract.state() == FeatureContractState::Draft {
        return Err(NormalizationError::FeatureContractNotPublished {
            feature_contract_id,
        });
    }
    if contract.definition().version != feature_contract_version {
        return Err(NormalizationError::FeatureVersionMismatch {
            version: feature_contract_version.to_string(),
        });
    }
    for row in split.rows {
        check_row(contract, feature_contract_version, row)?;
    }

    let mut means = [0.0; N];
    let mut stds = [0.0; N];
    if !split.rows.is_empty() {
        let n = split.rows.len() as f64;
        for row in split.rows {
            for (sum, value) in means.iter_mut().zip(row) {
                *sum += value;
            }
        }
        for mean in &mut means {
            *mean /= n;
        }
        for row in split.rows {
            for ((sum, value), mean) in stds.iter_mut().zip(row).zip(&means) {
                let deviation = value - mean;
                *sum += deviation * deviation;
            }
        }
        for std in &mut stds {
            *std = (*std / n).sqrt();
        }
        let first = split.rows[0];
        for (i, value) in first.iter().enumerate() {
            if split
                .rows
                .iter()
                .all(|row| row[i].to_bits() == value.to_bits())
            {
                means[i] = *value;
                stds[i] = 0.0;
            }
        }
    }
    let overflow = means
        .iter()
        .zip(&stds)
        .position(|(mean, std)| !mean.is_finite() || !std.is_finite());
    if let Some(index) = overflow {
        return Err(NormalizationError::NonFiniteFeature {
            feature_contract_id,
            index,
        });
    }
    Ok(NormalizationStats {
        feature_contract_id,
        dataset_split_id,
        table: NormalizationTable {
            feature_contract_version: contract.definition().version.clone(),
            split_role: split.role,
            rows: split.rows.len() as u64,
            means,
            stds,
        },
    })
}

/// A training row through the contract's own check, its refusal as a [`NormalizationError`].
fn check_row(
    contract: &FeatureContract,
    version: &str,
    row: &[f64; N],
) -> Result<(), NormalizationError> {
    contract.check(version, row).map_err(|error| match error {
        FeatureError::FeatureVersionMismatch { version } => {
            NormalizationError::FeatureVersionMismatch { version }
        }
        FeatureError::FeatureLengthMismatch { value_count } => {
            NormalizationError::FeatureLengthMismatch { value_count }
        }
        FeatureError::NonFiniteFeature {
            feature_contract_id,
            index,
        } => NormalizationError::NonFiniteFeature {
            feature_contract_id,
            index,
        },
        other => {
            unreachable!("FeatureContract::check refuses only version, length and value: {other}")
        }
    })
}

/// The `normalization.stats` text of `table` (module documentation).
pub fn write_stats(table: &NormalizationTable) -> String {
    let mut text = String::new();
    // Writing to a String cannot fail.
    let _ = writeln!(text, "{STATS_HEADER} {STATS_FORMAT_VERSION}");
    let _ = writeln!(
        text,
        "{FEATURE_CONTRACT} {}",
        table.feature_contract_version
    );
    let _ = writeln!(text, "{SPLIT} {}", table.split_role.key());
    let _ = writeln!(text, "{ROWS} {}", table.rows);
    let _ = writeln!(text, "{STD} {POPULATION}");
    for (i, (mean, std)) in table.means.iter().zip(&table.stds).enumerate() {
        let _ = writeln!(
            text,
            "{FEATURE} {i} {} {}",
            format_decimal(*mean),
            format_decimal(*std)
        );
    }
    text
}

/// Why a `normalization.stats` text was refused (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub enum StatsReason {
    /// The header names a version this module does not read.
    UnknownFormatVersion { version: u32 },
    /// A line that is not the record its place calls for; `expected` says what was.
    MalformedRecord { expected: &'static str },
    /// A token that is not an integer or a finite real of the grammar.
    MalformedNumber { token: String },
    /// The standard deviation of feature `index` is below 0.
    NegativeStd { index: usize },
    /// Feature `found` stands where feature `expected` belongs: `expected` is missing or out of
    /// order.
    FeatureOutOfOrder { expected: usize, found: usize },
    /// Feature `index` has a line already.
    RepeatedFeature { index: usize },
    /// A line longer than [`STATS_MAX_LINE_BYTES`].
    LineTooLong { bytes: usize },
    /// The text ended after `features` of the 32 feature lines.
    Incomplete { features: usize },
}

impl fmt::Display for StatsReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFormatVersion { version } => write!(
                f,
                "unknown {STATS_HEADER} version {version}; only {STATS_FORMAT_VERSION} is read"
            ),
            Self::MalformedRecord { expected } => {
                write!(f, "malformed record: expected {expected}")
            }
            Self::MalformedNumber { token } => write!(f, "{token:?} is not a number"),
            Self::NegativeStd { index } => {
                write!(f, "the standard deviation of feature {index} is negative")
            }
            Self::FeatureOutOfOrder { expected, found } => write!(
                f,
                "feature {found} is out of order: feature {expected} belongs here"
            ),
            Self::RepeatedFeature { index } => write!(f, "feature {index} is repeated"),
            Self::LineTooLong { bytes } => write!(
                f,
                "the line has {bytes} bytes, longer than the bound of {STATS_MAX_LINE_BYTES}"
            ),
            Self::Incomplete { features } => write!(
                f,
                "incomplete: the text ends after {features} of {N} feature lines"
            ),
        }
    }
}

/// A refused `normalization.stats`: the 1-based line, or none for a condition of the whole text,
/// and the reason.
#[derive(Clone, Debug, PartialEq)]
pub struct StatsFileError {
    pub line: Option<usize>,
    pub reason: StatsReason,
}

impl fmt::Display for StatsFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.reason),
            None => self.reason.fmt(f),
        }
    }
}

impl std::error::Error for StatsFileError {}

/// The lines of a text, each checked against the length bound before it is split.
struct Lines<'a> {
    lines: std::iter::Enumerate<std::str::Lines<'a>>,
    features: usize,
}

impl<'a> Lines<'a> {
    /// The next line's number and fields, or the refusal of a missing or overlong line.
    fn line(&mut self) -> Result<(usize, Vec<&'a str>), StatsFileError> {
        let Some((index, line)) = self.lines.next() else {
            return Err(StatsFileError {
                line: None,
                reason: StatsReason::Incomplete {
                    features: self.features,
                },
            });
        };
        let number = index + 1;
        if line.len() > STATS_MAX_LINE_BYTES {
            return Err(refused(
                number,
                StatsReason::LineTooLong { bytes: line.len() },
            ));
        }
        Ok((number, line.split_ascii_whitespace().collect()))
    }

    /// The next line, which must be `keyword value`; its value.
    fn record(
        &mut self,
        keyword: &str,
        expected: &'static str,
    ) -> Result<(usize, &'a str), StatsFileError> {
        let (number, fields) = self.line()?;
        match fields[..] {
            [first, value] if first == keyword => Ok((number, value)),
            _ => Err(refused(number, StatsReason::MalformedRecord { expected })),
        }
    }
}

fn refused(line: usize, reason: StatsReason) -> StatsFileError {
    StatsFileError {
        line: Some(line),
        reason,
    }
}

/// An unsigned integer of ASCII digits only.
fn integer<T: std::str::FromStr>(line: usize, token: &str) -> Result<T, StatsFileError> {
    let digits = !token.is_empty() && token.bytes().all(|b| b.is_ascii_digit());
    match token.parse() {
        Ok(value) if digits => Ok(value),
        _ => Err(refused(
            line,
            StatsReason::MalformedNumber {
                token: token.to_string(),
            },
        )),
    }
}

fn real(line: usize, token: &str) -> Result<f64, StatsFileError> {
    parse_decimal(token).ok_or_else(|| {
        refused(
            line,
            StatsReason::MalformedNumber {
                token: token.to_string(),
            },
        )
    })
}

/// Reads a `normalization.stats` text (module documentation), or the first fault in it.
pub fn read_stats(text: &str) -> Result<NormalizationTable, StatsFileError> {
    let mut lines = Lines {
        lines: text.lines().enumerate(),
        features: 0,
    };
    let (number, version) = lines.record(STATS_HEADER, "the header")?;
    let version: u32 = integer(number, version)?;
    if version != STATS_FORMAT_VERSION {
        return Err(refused(
            number,
            StatsReason::UnknownFormatVersion { version },
        ));
    }
    let (_, feature_contract_version) =
        lines.record(FEATURE_CONTRACT, "feature_contract <version>")?;
    let (number, split) = lines.record(SPLIT, "split train")?;
    if split != SplitRole::Train.key() {
        return Err(refused(
            number,
            StatsReason::MalformedRecord {
                expected: "split train",
            },
        ));
    }
    let (number, rows) = lines.record(ROWS, "rows <n>")?;
    let rows: u64 = integer(number, rows)?;
    let (number, divisor) = lines.record(STD, "std population")?;
    if divisor != POPULATION {
        return Err(refused(
            number,
            StatsReason::MalformedRecord {
                expected: "std population",
            },
        ));
    }

    let mut means = [0.0; N];
    let mut stds = [0.0; N];
    for expected in 0..N {
        lines.features = expected;
        let (number, fields) = lines.line()?;
        let [FEATURE, index, mean, std] = fields[..] else {
            return Err(refused(
                number,
                StatsReason::MalformedRecord {
                    expected: "feature <i> <mean> <std>",
                },
            ));
        };
        let index: usize = integer(number, index)?;
        if index < expected {
            return Err(refused(number, StatsReason::RepeatedFeature { index }));
        }
        if index > expected {
            return Err(refused(
                number,
                StatsReason::FeatureOutOfOrder {
                    expected,
                    found: index,
                },
            ));
        }
        means[index] = real(number, mean)?;
        stds[index] = real(number, std)?;
        if stds[index] < 0.0 {
            return Err(refused(number, StatsReason::NegativeStd { index }));
        }
    }
    if let Some((index, _)) = lines.lines.next() {
        return Err(refused(
            index + 1,
            StatsReason::MalformedRecord {
                expected: "the end of the text after feature 31",
            },
        ));
    }
    Ok(NormalizationTable {
        feature_contract_version: feature_contract_version.to_string(),
        split_role: SplitRole::Train,
        rows,
        means,
        stds,
    })
}
