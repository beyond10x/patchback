//! Adversary cases for story:normalization-stats (wave a8, pass 2): the constant-column rule of
//! Da8-44, which runs after the two-pass formula, pinned column by column at its boundaries:
//! constants whose sums overflow, signed zeros, a non-finite constant, and a single row.

use patchback_analysis::{
    FEATURES_1_VALUE_COUNT, FEATURES_1_VERSION, FeatureContractDefinition, FeatureContractId,
    FeatureRegistry, NormalizationError, NormalizationStats, SplitInput,
    compute_normalization_stats, read_stats, write_stats,
};
use patchback_core::split::SplitRole;

const N: usize = FEATURES_1_VALUE_COUNT;

fn try_compute(
    rows: &[[f64; N]],
) -> (
    Result<NormalizationStats, NormalizationError>,
    FeatureContractId,
) {
    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    registry.publish(id).unwrap();
    let result = compute_normalization_stats(
        &registry,
        id,
        FEATURES_1_VERSION,
        1,
        Some(SplitInput {
            role: SplitRole::Train,
            rows,
        }),
    );
    (result, id)
}

/// Three rows; every column varies (small values) unless `set` overrides it.
fn rows_with(set: impl Fn(usize, usize) -> Option<f64>) -> Vec<[f64; N]> {
    (0..3)
        .map(|k| std::array::from_fn(|i| set(k, i).unwrap_or((k * 7 + i) as f64 / 4.0)))
        .collect()
}

const PLUS_ZERO: u64 = 0;

/// Columns constant at f64::MAX, 1e200 and −f64::MAX: their sums overflow to ±inf and their
/// two-pass deviations are inf, yet the constant rule replaces both, so they are admitted with
/// the constant as mean (bit for bit) and std +0.0, and `apply` of a training row gives +0.0.
/// The same rows plus one varying column whose squared deviations overflow (column 3) are refused
/// at column 3: the constant columns' infinite intermediates fire no refusal of their own.
#[test]
fn constants_whose_sums_overflow_are_admitted_and_a_varying_overflow_is_still_refused() {
    let constants = [(0, f64::MAX), (1, 1e200), (2, -f64::MAX)];
    let rows = rows_with(|_, i| constants.iter().find(|(c, _)| *c == i).map(|(_, v)| *v));
    let (stats, _) = try_compute(&rows);
    let stats = stats.expect("constant columns are admitted (Da8-44)");
    for (i, value) in constants {
        assert_eq!(stats.means()[i].to_bits(), value.to_bits(), "{i} mean");
        assert_eq!(stats.stds()[i].to_bits(), PLUS_ZERO, "{i} std");
    }
    let z = stats.apply(FEATURES_1_VERSION, &rows[1]).unwrap();
    for (i, _) in constants {
        assert_eq!(z[i].to_bits(), PLUS_ZERO, "{i} z");
    }
    // The table with f64::MAX and −f64::MAX as means round-trips.
    let text = write_stats(stats.table());
    assert_eq!(&read_stats(&text).unwrap(), stats.table());

    let overflowing = [-1.5e308, 1.5e308, 0.0];
    let rows = rows_with(|k, i| {
        if i == 3 {
            Some(overflowing[k])
        } else {
            constants.iter().find(|(c, _)| *c == i).map(|(_, v)| *v)
        }
    });
    let (refused, id) = try_compute(&rows);
    assert_eq!(
        refused,
        Err(NormalizationError::NonFiniteFeature {
            feature_contract_id: id,
            index: 3
        })
    );
}

/// Signed zeros. Column 4 holds −0.0, +0.0, −0.0: not bit-equal, so the formula path: mean +0.0,
/// std +0.0, divisor 1, and `apply` gives `x − 0`, which is −0.0 for a −0.0 input (pinned: equal to
/// 0 by `==`, not by bits). Column 5 holds −0.0 in every row: the constant rule, mean −0.0, and
/// `apply` gives +0.0 for both zeros. Column 6 holds −0.0, −0.0, +0.0 (one +0.0 among −0.0): as
/// column 4.
#[test]
fn signed_zero_columns_take_the_formula_or_the_constant_rule_by_their_bits() {
    let rows = rows_with(|k, i| match i {
        4 => Some([-0.0, 0.0, -0.0][k]),
        5 => Some(-0.0),
        6 => Some([-0.0, -0.0, 0.0][k]),
        _ => None,
    });
    let (stats, _) = try_compute(&rows);
    let stats = stats.unwrap();
    let bits = |v: f64| v.to_bits();
    assert_eq!(
        [4, 5, 6].map(|i| (bits(stats.means()[i]), bits(stats.stds()[i]))),
        [
            (PLUS_ZERO, PLUS_ZERO),
            (bits(-0.0), PLUS_ZERO),
            (PLUS_ZERO, PLUS_ZERO)
        ]
    );
    let mut x = rows[0];
    x[4] = -0.0;
    x[5] = -0.0;
    x[6] = -0.0;
    let z = stats.apply(FEATURES_1_VERSION, &x).unwrap();
    assert_eq!(
        [4, 5, 6].map(|i| bits(z[i])),
        [bits(-0.0), PLUS_ZERO, bits(-0.0)]
    );
    x[4] = 0.0;
    x[5] = 0.0;
    x[6] = 0.0;
    let z = stats.apply(FEATURES_1_VERSION, &x).unwrap();
    assert_eq!([4, 5, 6].map(|i| bits(z[i])), [PLUS_ZERO; 3]);
    // The −0.0 mean is written as "-0" and read back with its sign.
    let read = read_stats(&write_stats(stats.table())).unwrap();
    assert_eq!(bits(read.means()[5]), bits(-0.0));
}

/// A column holding the same NaN bit pattern, or +inf, in every row is refused by the row check
/// before the constant rule could take it as constant; nothing reaches `write_stats`.
#[test]
fn a_constant_non_finite_column_is_refused_before_the_constant_rule() {
    for (index, value) in [(9, f64::NAN), (10, f64::INFINITY), (11, f64::NEG_INFINITY)] {
        let rows = rows_with(|_, i| (i == index).then_some(value));
        assert!(
            rows.iter()
                .all(|row| row[index].to_bits() == value.to_bits())
        );
        let (refused, id) = try_compute(&rows);
        assert_eq!(
            refused,
            Err(NormalizationError::NonFiniteFeature {
                feature_contract_id: id,
                index
            }),
            "{value}"
        );
    }
}

/// One row: every column is constant, so every mean is the row's value bit for bit (−0.0
/// included, which the formula's `+0.0 + −0.0` would have made +0.0) and every std +0.0;
/// `apply` of the row is +0.0 in every column.
#[test]
fn one_row_gives_its_values_as_means_and_std_0_bit_for_bit() {
    let row: [f64; N] = std::array::from_fn(|i| match i {
        0 => -0.0,
        1 => 0.1,
        2 => f64::MAX,
        3 => -5e-324,
        _ => i as f64 * -1.3,
    });
    let (stats, _) = try_compute(&[row]);
    let stats = stats.unwrap();
    assert_eq!(stats.rows(), 1);
    assert_eq!(stats.means().map(f64::to_bits), row.map(f64::to_bits));
    assert_eq!(stats.stds().map(f64::to_bits), [PLUS_ZERO; N]);
    let z = stats.apply(FEATURES_1_VERSION, &row).unwrap();
    assert_eq!(z.map(f64::to_bits), [PLUS_ZERO; N]);
}
