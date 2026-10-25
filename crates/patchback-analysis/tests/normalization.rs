//! The numeric obligations of story:normalization-stats (DESIGN.md § 6.3): the means and
//! population standard deviations of the training rows against an independent loop and a
//! hand-computed case, `(x − mean) / std` per element, a constant feature applied with divisor 1,
//! statistics admitted from the training split only, no non-finite value in or out; the refusals
//! of `ComputeNormalizationStats` in the model's declared order; and the `normalization.stats`
//! text, which round-trips exactly and whose reader refuses what no writer produces.

use patchback_analysis::{
    FEATURES_1_VALUE_COUNT, FEATURES_1_VERSION, FeatureContractDefinition, FeatureContractId,
    FeatureRegistry, NORMALIZATION_STATS_FILE, NormalizationError, NormalizationStats,
    STATS_FORMAT_VERSION, STATS_HEADER, STATS_MAX_LINE_BYTES, SplitInput, StatsReason,
    compute_normalization_stats, read_stats, write_stats,
};
use patchback_core::split::SplitRole;

const N: usize = FEATURES_1_VALUE_COUNT;

/// A registry holding `features/1`, published when `publish` is set.
fn registry(publish: bool) -> (FeatureRegistry, FeatureContractId) {
    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    if publish {
        registry.publish(id).unwrap();
    }
    (registry, id)
}

/// `count` rows, row `r` feature `i` a value no simple formula relates across features, with
/// `offset` added to every value.
fn rows(count: usize, offset: f64) -> Vec<[f64; N]> {
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    (0..count)
        .map(|_| {
            let mut row = [0.0; N];
            for (i, value) in row.iter_mut().enumerate() {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let unit = (state >> 11) as f64 / (1u64 << 53) as f64;
                *value = offset + (i as f64 - 16.0) * 7.5 + unit * (1.0 + i as f64);
            }
            row
        })
        .collect()
}

fn train(rows: &[[f64; N]]) -> Option<SplitInput<'_>> {
    Some(SplitInput {
        role: SplitRole::Train,
        rows,
    })
}

fn compute(rows: &[[f64; N]]) -> NormalizationStats {
    let (registry, id) = registry(true);
    compute_normalization_stats(&registry, id, FEATURES_1_VERSION, 7, train(rows)).unwrap()
}

/// The reference: Welford's one-pass update, a different algorithm from the two passes of the
/// crate, over the same rows.
fn welford(rows: &[[f64; N]]) -> ([f64; N], [f64; N]) {
    let mut mean = [0.0; N];
    let mut m2 = [0.0; N];
    for (k, row) in rows.iter().enumerate() {
        let count = (k + 1) as f64;
        for i in 0..N {
            let delta = row[i] - mean[i];
            mean[i] += delta / count;
            m2[i] += delta * (row[i] - mean[i]);
        }
    }
    let mut std = [0.0; N];
    for i in 0..N {
        std[i] = (m2[i] / rows.len() as f64).sqrt();
    }
    (mean, std)
}

fn close(a: f64, b: f64, relative: f64) -> bool {
    a == b || (a - b).abs() <= relative * a.abs().max(b.abs())
}

#[test]
fn means_and_population_standard_deviations_match_an_independent_loop() {
    let rows = rows(257, 3.0);
    let stats = compute(&rows);
    let (mean, std) = welford(&rows);
    for i in 0..N {
        assert!(
            close(stats.means()[i], mean[i], 1e-12),
            "{i}: {} {}",
            stats.means()[i],
            mean[i]
        );
        assert!(
            close(stats.stds()[i], std[i], 1e-12),
            "{i}: {} {}",
            stats.stds()[i],
            std[i]
        );
    }
    assert_eq!(stats.rows(), 257);
    assert_eq!(stats.split_role(), SplitRole::Train);
    assert_eq!(stats.feature_contract_version(), FEATURES_1_VERSION);
    assert_eq!(stats.dataset_split_id(), 7);
}

/// Feature 0 over 1, 2, 3: mean 2, population variance (1 + 0 + 1) / 3. Feature 1 over 2, 4, 9:
/// mean 5, variance (9 + 1 + 16) / 3. The rest are 0: mean 0, standard deviation 0. The sample
/// standard deviation (divisor n − 1) would give 1 and sqrt(13) instead.
#[test]
fn three_rows_give_the_hand_computed_statistics() {
    let mut rows = [[0.0; N]; 3];
    for (k, (a, b)) in [(1.0, 2.0), (2.0, 4.0), (3.0, 9.0)].into_iter().enumerate() {
        rows[k][0] = a;
        rows[k][1] = b;
    }
    let stats = compute(&rows);
    assert_eq!(stats.means()[0], 2.0);
    assert_eq!(stats.means()[1], 5.0);
    assert_eq!(stats.stds()[0], (2.0_f64 / 3.0).sqrt());
    assert_eq!(stats.stds()[1], (26.0_f64 / 3.0).sqrt());
    for i in 2..N {
        assert_eq!((stats.means()[i], stats.stds()[i]), (0.0, 0.0), "{i}");
    }
    assert_eq!(stats.rows(), 3);
}

/// Da8-14: a split of no rows gives mean 0 and standard deviation 0 for every feature.
#[test]
fn zero_rows_give_zero_means_and_zero_standard_deviations() {
    let stats = compute(&[]);
    assert_eq!(stats.means(), &[0.0; N]);
    assert_eq!(stats.stds(), &[0.0; N]);
    assert_eq!(stats.rows(), 0);
}

#[test]
fn apply_is_x_minus_mean_over_std_element_wise() {
    let training = rows(64, -1.0);
    let stats = compute(&training);
    let x = rows(65, 0.5)[64];
    let normalized = stats.apply(FEATURES_1_VERSION, &x).unwrap();
    for i in 0..N {
        assert!(stats.stds()[i] > 0.0, "{i}");
        assert_eq!(
            normalized[i],
            (x[i] - stats.means()[i]) / stats.stds()[i],
            "{i}"
        );
    }
    // Reused unchanged: the same vector normalizes the same way twice, and the stats are as
    // they were.
    let before = stats.clone();
    assert_eq!(stats.apply(FEATURES_1_VERSION, &x).unwrap(), normalized);
    assert_eq!(stats, before);
}

/// Da8-14: a feature whose training values are all equal has standard deviation 0 and is
/// applied with divisor 1, `x − mean`, so the output stays finite.
#[test]
fn a_constant_feature_applies_with_divisor_one_and_stays_finite() {
    let mut rows = rows(20, 0.0);
    for row in &mut rows {
        row[5] = -42.25;
    }
    let stats = compute(&rows);
    assert_eq!(stats.stds()[5], 0.0);
    assert_eq!(stats.means()[5], -42.25);
    let mut x = [1.0; N];
    x[5] = 10.0;
    let normalized = stats.apply(FEATURES_1_VERSION, &x).unwrap();
    assert_eq!(normalized[5], 52.25);
    assert!(normalized.iter().all(|v| v.is_finite()), "{normalized:?}");
}

/// Da8-44: a column whose values are all one non-dyadic value (0.1 is no `f64` sum's exact
/// tenth) has that value as its mean, bit for bit, and standard deviation 0, so `apply` gives
/// +0.0 for it; the column beside it still varies and keeps the two-pass statistics.
#[test]
fn a_non_dyadic_constant_column_has_its_value_as_mean_and_std_0() {
    let mut rows = rows(3, 0.0);
    for row in &mut rows {
        row[7] = 0.1;
    }
    let stats = compute(&rows);
    assert_eq!(stats.means()[7].to_bits(), 0.1f64.to_bits(), "mean is c");
    assert_eq!(stats.stds()[7].to_bits(), 0.0f64.to_bits(), "std is +0.0");
    assert!(stats.stds()[8] > 0.0, "premise: feature 8 varies");
    let mut x = rows[1];
    x[7] = 0.1;
    let normalized = stats.apply(FEATURES_1_VERSION, &x).unwrap();
    assert_eq!(
        normalized[7].to_bits(),
        0.0f64.to_bits(),
        "+0.0, sign positive"
    );
}

/// The validation and test rows have statistics of their own, different from the training rows';
/// only the training split's are admitted (DESIGN.md § 6.3, OQ-18).
#[test]
fn only_the_training_split_is_admitted_and_the_others_differ() {
    let (registry, id) = registry(true);
    let train_rows = rows(48, 0.0);
    let validation_rows = rows(16, 2.0);
    let test_rows = rows(16, -3.0);
    let stats =
        compute_normalization_stats(&registry, id, FEATURES_1_VERSION, 1, train(&train_rows))
            .unwrap();
    for (role, held_out, split_id) in [
        (SplitRole::Validation, &validation_rows, 2),
        (SplitRole::Test, &test_rows, 3),
    ] {
        let (mean, _) = welford(held_out);
        assert!(
            (0..N).all(|i| mean[i] != stats.means()[i]),
            "{role}: the held-out means equal the training means"
        );
        let refused = compute_normalization_stats(
            &registry,
            id,
            FEATURES_1_VERSION,
            split_id,
            Some(SplitInput {
                role,
                rows: held_out,
            }),
        );
        assert_eq!(
            refused,
            Err(NormalizationError::StatsFromNonTrainingSplit { split_role: role })
        );
        assert_eq!(
            refused.unwrap_err().domain_error(),
            "patchback.analysis.StatsFromNonTrainingSplit"
        );
    }
}

#[test]
fn a_non_finite_input_to_apply_is_refused() {
    let (_, id) = registry(true);
    let stats = compute(&rows(16, 0.0));
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut x = [0.5; N];
        x[9] = bad;
        let refused = stats.apply(FEATURES_1_VERSION, &x);
        assert_eq!(
            refused,
            Err(NormalizationError::NonFiniteFeature {
                feature_contract_id: id,
                index: 9
            }),
            "{bad}"
        );
        assert_eq!(
            refused.unwrap_err().domain_error(),
            "patchback.analysis.NonFiniteFeature"
        );
    }
}

/// A finite input whose normalized value overflows (a standard deviation far below the input's
/// distance from the mean) is refused rather than returned infinite (invariant 4). The deviation
/// is 5e-161, whose square (2.5e-321, subnormal) is still above 0, so the standard deviation is
/// not 0.
#[test]
fn an_output_that_would_overflow_is_refused() {
    let mut rows = [[0.0; N]; 2];
    rows[1][3] = 1e-160;
    let stats = compute(&rows);
    assert!(stats.stds()[3] > 0.0 && stats.stds()[3] < 1e-150);
    let mut x = [0.0; N];
    x[3] = 1e300;
    assert!(matches!(
        stats.apply(FEATURES_1_VERSION, &x),
        Err(NormalizationError::NonFiniteFeature { index: 3, .. })
    ));
}

/// `NormalizationTable::normalize`, the arithmetic the dense writer uses (Da8-30), is `apply`
/// without its checks: the same values for a vector `apply` admits, bit for bit, and a NaN input
/// or an overflowing result returned as it is (NaN, +∞), each other value still normalized.
#[test]
fn normalize_is_apply_without_its_checks() {
    let stats = compute(&rows(16, 0.0));
    let x: [f64; N] = std::array::from_fn(|i| i as f64 * 0.37 - 3.0);
    let applied = stats.apply(FEATURES_1_VERSION, &x).unwrap();
    assert_eq!(
        stats.table().normalize(&x).map(f64::to_bits),
        applied.map(f64::to_bits)
    );

    let mut bad = x;
    bad[9] = f64::NAN;
    let normalized = stats.table().normalize(&bad);
    assert!(normalized[9].is_nan());
    assert_eq!(normalized[8], applied[8]);

    let mut rows = [[0.0; N]; 2];
    rows[1][3] = 1e-160;
    let tiny = compute(&rows);
    let mut far = [0.0; N];
    far[3] = 1e300;
    assert_eq!(tiny.table().normalize(&far)[3], f64::INFINITY);
}

/// ApplyNormalization's `contract-mismatch`: features of another contract version are refused,
/// before their values are read.
#[test]
fn features_of_another_contract_version_are_refused() {
    let stats = compute(&rows(16, 0.0));
    let mut x = [0.0; N];
    x[0] = f64::NAN;
    let refused = stats.apply("features/2", &x);
    assert_eq!(
        refused,
        Err(NormalizationError::NormalizationContractMismatch {
            feature_contract_version: "features/2".to_owned()
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.analysis.NormalizationContractMismatch"
    );
}

/// The outcomes of `ComputeNormalizationStats` in the model's declared order: each case holds
/// every later fault as well, so only the declared order gives the expected refusal.
#[test]
fn the_compute_refusals_come_in_the_declared_order() {
    let (published, id) = registry(true);
    let (draft, draft_id) = registry(false);
    let unknown = FeatureContractId(99);
    let rows = rows(16, 0.0);
    let validation = Some(SplitInput {
        role: SplitRole::Validation,
        rows: &rows,
    });

    assert_eq!(
        compute_normalization_stats(&draft, unknown, "features/2", 99, None),
        Err(NormalizationError::DatasetSplitNotFound {
            dataset_split_id: 99
        })
    );
    assert_eq!(
        compute_normalization_stats(&draft, unknown, "features/2", 2, validation),
        Err(NormalizationError::FeatureContractNotFound {
            feature_contract_id: unknown
        })
    );
    assert_eq!(
        compute_normalization_stats(&draft, draft_id, "features/2", 2, validation),
        Err(NormalizationError::StatsFromNonTrainingSplit {
            split_role: SplitRole::Validation
        })
    );
    assert_eq!(
        compute_normalization_stats(&draft, draft_id, "features/2", 1, train(&rows)),
        Err(NormalizationError::FeatureContractNotPublished {
            feature_contract_id: draft_id
        })
    );
    assert_eq!(
        compute_normalization_stats(&published, id, "features/2", 1, train(&rows)),
        Err(NormalizationError::FeatureVersionMismatch {
            version: "features/2".to_owned()
        })
    );
    assert!(
        compute_normalization_stats(&published, id, FEATURES_1_VERSION, 1, train(&rows)).is_ok()
    );
}

/// A training row the contract's own check refuses (`FeatureContract::check`) is refused with its
/// error, so no non-finite mean or standard deviation is computed.
#[test]
fn a_non_finite_training_row_is_refused_through_the_contract_check() {
    let (registry, id) = registry(true);
    let mut rows = rows(8, 0.0);
    rows[5][30] = f64::INFINITY;
    assert_eq!(
        compute_normalization_stats(&registry, id, FEATURES_1_VERSION, 1, train(&rows)),
        Err(NormalizationError::NonFiniteFeature {
            feature_contract_id: id,
            index: 30
        })
    );
}

/// Finite rows whose squared deviations overflow give no infinite standard deviation: refused.
#[test]
fn statistics_that_would_overflow_are_refused() {
    let (registry, id) = registry(true);
    let mut rows = [[0.0; N]; 2];
    rows[0][12] = -1.5e308;
    rows[1][12] = 1.5e308;
    assert!(matches!(
        compute_normalization_stats(&registry, id, FEATURES_1_VERSION, 1, train(&rows)),
        Err(NormalizationError::NonFiniteFeature { index: 12, .. })
    ));
}

// ---------------------------------------------------------------- normalization.stats

fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

fn join(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

#[test]
fn the_file_holds_its_records_in_order() {
    let stats = compute(&rows(3, 0.0));
    let text = write_stats(stats.table());
    assert_eq!(NORMALIZATION_STATS_FILE, "normalization.stats");
    let lines = lines(&text);
    assert_eq!(lines.len(), 5 + N);
    assert_eq!(
        lines[..5],
        [
            format!("{STATS_HEADER} {STATS_FORMAT_VERSION}"),
            "feature_contract features/1".to_owned(),
            "split train".to_owned(),
            "rows 3".to_owned(),
            "std population".to_owned(),
        ]
    );
    assert_eq!(lines[0], "patchback-normalization-stats 1");
    for (i, line) in lines[5..].iter().enumerate() {
        let fields: Vec<&str> = line.split(' ').collect();
        assert_eq!(fields.len(), 4, "{line}");
        assert_eq!(fields[0], "feature");
        assert_eq!(fields[1], i.to_string());
        assert_eq!(fields[2].parse::<f64>().unwrap(), stats.means()[i]);
        assert_eq!(fields[3].parse::<f64>().unwrap(), stats.stds()[i]);
    }
    assert!(text.ends_with('\n'));
    assert!(lines.iter().all(|line| line.len() <= STATS_MAX_LINE_BYTES));
}

/// Read back, the table is the written one bit for bit, extremes and rounding cases included.
#[test]
fn write_and_read_round_trip_exactly() {
    let mut extreme = vec![[0.0; N]; 2];
    let values = [
        0.1,
        1.0 / 3.0,
        -2.5e-308,
        5e-324,
        8.9e307,
        -0.0,
        123_456_789.123_456_78,
        f64::MIN_POSITIVE,
    ];
    // Features 8 to 15 have a spread; their squared deviations sum below f64::MAX (a sum past it
    // is refused, see statistics_that_would_overflow_are_refused).
    let spread = [
        0.1,
        1.0 / 3.0,
        -2.5e-155,
        1e-160,
        1e154,
        -0.0,
        123_456_789.123_456_78,
        7.0,
    ];
    for (i, (value, spread)) in values.iter().zip(spread).enumerate() {
        extreme[0][i] = *value;
        extreme[1][i] = *value;
        extreme[0][i + 8] = spread;
        extreme[1][i + 8] = -spread / 2.0;
    }
    for rows in [rows(3, 0.0), rows(1_000, 1e6), extreme, vec![]] {
        let stats = compute(&rows);
        let text = write_stats(stats.table());
        let read = read_stats(&text).unwrap();
        assert_eq!(&read, stats.table());
        let bits = |values: &[f64; N]| values.map(f64::to_bits);
        assert_eq!(bits(read.means()), bits(stats.means()));
        assert_eq!(bits(read.stds()), bits(stats.stds()));
        assert_eq!(write_stats(&read), text, "the text is canonical");
        assert!(lines(&text).iter().all(|l| l.len() <= STATS_MAX_LINE_BYTES));
    }
}

/// The reader takes `\r\n` and a last line with no terminator, as the other line formats do.
#[test]
fn the_reader_takes_crlf_and_a_missing_last_terminator() {
    let stats = compute(&rows(4, 0.0));
    let text = write_stats(stats.table());
    assert_eq!(
        &read_stats(&text.replace('\n', "\r\n")).unwrap(),
        stats.table()
    );
    assert_eq!(&read_stats(text.trim_end()).unwrap(), stats.table());
}

/// Each refusal names the 1-based line and its reason.
#[test]
fn the_reader_refuses_what_no_writer_produces() {
    let stats = compute(&rows(4, 0.0));
    let good = lines(&write_stats(stats.table()));
    let edit = |line: usize, with: &str| {
        let mut lines = good.clone();
        lines[line - 1] = with.to_owned();
        join(&lines)
    };
    let without = |line: usize| {
        let mut lines = good.clone();
        lines.remove(line - 1);
        join(&lines)
    };
    let feature = |i: usize| good[5 + i].clone();
    let malformed = |line| (Some(line), "malformed");
    let cases: Vec<(String, (Option<usize>, &str))> = vec![
        (
            edit(1, "patchback-normalization-stats 2"),
            (Some(1), "version"),
        ),
        (edit(1, "patchback-normalisation-stats 1"), malformed(1)),
        (edit(1, "patchback-normalization-stats"), malformed(1)),
        (edit(2, "feature_contract"), malformed(2)),
        (edit(2, "feature_contract features/1 x"), malformed(2)),
        (edit(3, "split validation"), malformed(3)),
        (edit(3, "split test"), malformed(3)),
        (edit(4, "rows -1"), (Some(4), "number")),
        (edit(4, "rows 1.5"), (Some(4), "number")),
        (edit(4, "rows 18446744073709551616"), (Some(4), "number")),
        (edit(5, "std sample"), malformed(5)),
        (edit(6, "feature 0 1.5x 0"), (Some(6), "number")),
        (edit(6, "feature 0 NaN 0"), (Some(6), "number")),
        (edit(6, "feature 0 inf 0"), (Some(6), "number")),
        (edit(6, "feature 0 0 1e999"), (Some(6), "number")),
        (edit(6, "feature 0 0 -1"), (Some(6), "negative")),
        (edit(6, "feature 0 0"), malformed(6)),
        (edit(6, "feature 0 0 0 0"), malformed(6)),
        (edit(6, "feature x 0 0"), (Some(6), "number")),
        (edit(6, "mean 0 0 0"), malformed(6)),
        (without(6 + 7), (Some(13), "out of order")),
        (edit(6 + 7, &feature(8)), (Some(13), "out of order")),
        (edit(6 + 8, &feature(7)), (Some(14), "repeated")),
        (without(6 + 31), (None, "incomplete")),
        (join(&good[..5]), (None, "incomplete")),
        (join(&good[..2]), (None, "incomplete")),
        (String::new(), (None, "incomplete")),
        (
            {
                let mut lines = good.clone();
                lines.push("feature 32 0 0".to_owned());
                join(&lines)
            },
            malformed(38),
        ),
        (
            {
                let mut lines = good.clone();
                lines.push(String::new());
                join(&lines)
            },
            malformed(38),
        ),
        (
            edit(
                6,
                &format!("feature 0 0 {}", "0".repeat(STATS_MAX_LINE_BYTES)),
            ),
            (Some(6), "longer"),
        ),
    ];
    for (text, (line, reason)) in cases {
        let error = read_stats(&text).expect_err(&text);
        assert_eq!(error.line, line, "{error} for\n{text}");
        let message = error.to_string();
        assert!(message.contains(reason), "{message:?} lacks {reason:?}");
        match (&error.reason, reason) {
            (StatsReason::UnknownFormatVersion { version: 2 }, "version")
            | (StatsReason::MalformedRecord { .. }, "malformed")
            | (StatsReason::MalformedNumber { .. }, "number")
            | (StatsReason::NegativeStd { index: 0 }, "negative")
            | (StatsReason::FeatureOutOfOrder { expected: 7, .. }, "out of order")
            | (StatsReason::RepeatedFeature { index: 7 }, "repeated")
            | (StatsReason::Incomplete { .. }, "incomplete")
            | (StatsReason::LineTooLong { .. }, "longer") => {}
            other => panic!("{other:?}"),
        }
    }
}

/// The length bound is checked before the line is split: a line one byte over is refused as too
/// long even when it would otherwise read; one at the bound is not.
#[test]
fn the_line_length_bound_is_exact() {
    let stats = compute(&rows(2, 0.0));
    let mut lines = lines(&write_stats(stats.table()));
    let line = &lines[3];
    let pad = STATS_MAX_LINE_BYTES - line.len();
    lines[3] = format!("rows {}{}", "0".repeat(pad), stats.rows());
    assert_eq!(lines[3].len(), STATS_MAX_LINE_BYTES);
    assert_eq!(&read_stats(&join(&lines)).unwrap(), stats.table());
    lines[3] = format!("rows {}{}", "0".repeat(pad + 1), stats.rows());
    let error = read_stats(&join(&lines)).unwrap_err();
    assert_eq!(error.line, Some(4));
    assert_eq!(
        error.reason,
        StatsReason::LineTooLong {
            bytes: STATS_MAX_LINE_BYTES + 1
        }
    );
}
