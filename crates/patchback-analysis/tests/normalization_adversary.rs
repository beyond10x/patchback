//! Adversary cases for story:normalization-stats (wave a8, pass 1): the statistics of a constant
//! feature whose value is not a dyadic fraction, the two-pass formula against the one-pass sum of
//! squares on rows far from 0, and the `normalization.stats` reader on integers that are not the
//! writer's canonical text.

use patchback_analysis::{
    FEATURES_1_VALUE_COUNT, FEATURES_1_VERSION, FeatureContractDefinition, FeatureRegistry,
    NormalizationStats, SplitInput, compute_normalization_stats, read_stats, write_stats,
};
use patchback_core::split::SplitRole;

const N: usize = FEATURES_1_VALUE_COUNT;

fn compute(rows: &[[f64; N]]) -> NormalizationStats {
    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    registry.publish(id).unwrap();
    compute_normalization_stats(
        &registry,
        id,
        FEATURES_1_VERSION,
        1,
        Some(SplitInput {
            role: SplitRole::Train,
            rows,
        }),
    )
    .unwrap()
}

/// Da8-14 and the test doc of `a_constant_feature_applies_with_divisor_one_and_stays_finite`: "a
/// feature whose training values are all equal has standard deviation 0 and is applied with
/// divisor 1, `x − mean`". Feature 0 is the same value in every row; the other features vary.
/// The library's mean of `n` copies of `c` is `(c + c + … + c) / n`, which is not `c` when the
/// running sum rounds (0.1 three times sums to 0.30000000000000004), so the deviations are a
/// nonzero constant, the standard deviation is that constant's magnitude and not 0, and every
/// training row normalizes to ±1 instead of 0.
#[test]
fn a_constant_feature_of_any_value_has_standard_deviation_0_and_normalizes_to_0() {
    let mut failures = Vec::new();
    for c in [0.1, 0.2, 0.7, 1.0 / 3.0, 60.0 / 127.0, -12.3] {
        for n in [3usize, 7, 32, 50_016] {
            let rows: Vec<[f64; N]> = (0..n)
                .map(|k| std::array::from_fn(|i| if i == 0 { c } else { (k + i) as f64 }))
                .collect();
            let stats = compute(&rows);
            let z = stats.apply(FEATURES_1_VERSION, &rows[0]).unwrap()[0];
            if stats.stds()[0] != 0.0 || stats.means()[0] != c || z.to_bits() != 0 {
                failures.push(format!(
                    "c {c:?} n {n}: mean {:?} std {:?} z {z:?}",
                    stats.means()[0],
                    stats.stds()[0]
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "a constant feature must have std 0 and normalize to +0.0:\n{}",
        failures.join("\n")
    );
}

/// Da8-13, two passes: rows 1e8 − 1 and 1e8 + 1 have mean 1e8 and population standard deviation
/// exactly 1 (sample: sqrt 2). The one-pass formula `sqrt(sum x² / n − mean²)` loses the 1 in the
/// rounding of `(1e8 ± 1)²` (54 significant bits); the premise below shows it does, so this case
/// is the one that fails if the computation is changed to one pass.
#[test]
fn the_two_pass_standard_deviation_is_exact_far_from_0() {
    let rows: Vec<[f64; N]> = [1e8 - 1.0, 1e8 + 1.0]
        .iter()
        .map(|&v| std::array::from_fn(|i| v + i as f64))
        .collect();
    let one_pass = {
        let n = 2.0;
        let sum: f64 = rows.iter().map(|r| r[0]).sum();
        let squares: f64 = rows.iter().map(|r| r[0] * r[0]).sum();
        let mean = sum / n;
        (squares / n - mean * mean).max(0.0).sqrt()
    };
    assert_ne!(one_pass, 1.0, "premise: the one-pass formula is not exact");
    let stats = compute(&rows);
    for i in 0..N {
        assert_eq!(stats.means()[i], 1e8 + i as f64, "{i}");
        assert_eq!(stats.stds()[i], 1.0, "{i}");
    }

    // Values near 10,000 with a spread of 0.01: the exact mean is 10000.005 and the exact
    // population std of {10000, 10000.01} is 0.005, both to within the representation of the
    // inputs; the library is within 2^-40 relative of them.
    let rows: Vec<[f64; N]> = [10_000.0, 10_000.01].iter().map(|&v| [v; N]).collect();
    let stats = compute(&rows);
    // Exact by Sterbenz's lemma: the difference of the two inputs, halved.
    let half = (10_000.01_f64 - 10_000.0) / 2.0;
    let within = |a: f64, b: f64| (a - b).abs() <= b.abs() * 2f64.powi(-40);
    let one_pass = {
        let mean = (rows[0][0] + rows[1][0]) / 2.0;
        ((rows[0][0] * rows[0][0] + rows[1][0] * rows[1][0]) / 2.0 - mean * mean)
            .max(0.0)
            .sqrt()
    };
    assert!(
        !within(one_pass, half),
        "premise: one pass is off: {one_pass:?} {half:?}"
    );
    assert!(
        within(stats.stds()[0], half),
        "{:?} {half:?}",
        stats.stds()[0]
    );
    assert!(
        within(stats.means()[0], 10_000.0 + half),
        "{:?}",
        stats.means()[0]
    );
}

/// Da8-15, "round-trip is exact": a text whose reals are the extremes the brief names (−0.0,
/// 1e-300, 5e-324, 1e21, 0.1, f64::MAX, f64::MIN_POSITIVE) and 4,000 pseudo-random finite bit
/// patterns, written in the writer's form, reads back to the same bits, and the table read back
/// writes the same text.
#[test]
fn every_finite_real_reads_back_bit_for_bit() {
    let mut values = vec![
        -0.0,
        0.0,
        1e-300,
        5e-324,
        -5e-324,
        1e21,
        1e22,
        0.1,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
        f64::EPSILON,
        123_456_789_012_345_680.0,
    ];
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    while values.len() < 4_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let v = f64::from_bits(state);
        if v.is_finite() {
            values.push(v);
        }
    }
    for chunk in values.chunks(N) {
        let mut text = String::from(
            "patchback-normalization-stats 1\nfeature_contract features/1\nsplit train\nrows 9\n\
             std population\n",
        );
        let pairs: Vec<(f64, f64)> = (0..N)
            .map(|i| {
                let mean = chunk[i % chunk.len()];
                let std = chunk[(i + 1) % chunk.len()].abs();
                (mean, std)
            })
            .collect();
        for (i, (mean, std)) in pairs.iter().enumerate() {
            text.push_str(&format!(
                "feature {i} {} {}\n",
                patchback_core::patch::format_decimal(*mean),
                patchback_core::patch::format_decimal(*std)
            ));
        }
        let read = read_stats(&text).unwrap_or_else(|error| panic!("{error}\n{text}"));
        for (i, (mean, std)) in pairs.iter().enumerate() {
            assert_eq!(
                read.means()[i].to_bits(),
                mean.to_bits(),
                "{i} mean {mean:?}"
            );
            assert_eq!(read.stds()[i].to_bits(), std.to_bits(), "{i} std {std:?}");
        }
        assert_eq!(write_stats(&read), text);
    }
}
