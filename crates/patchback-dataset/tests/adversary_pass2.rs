//! Adversary cases for story:dataset-generation (wave a6, pass 2): the split-count rule over the
//! upper half of `0..=i64::MAX` (the range `--samples` admitted under Da6-26), where
//! `samples · 4992` no longer fits an `i64`, checked against an oracle that does not repeat the
//! implementation's formula, and the largest batch-multiple count the CLI admits. Re-pinned to
//! Da7-10 (story:dense-data-files): `--samples` now admits `0..=u32::MAX` ([`MAX_SAMPLES`]);
//! `SplitCounts::from_samples` still takes every `u64`, so the rule is still checked up to the
//! old ceiling [`I64_SAMPLES`].

use std::path::PathBuf;

use patchback_dataset::{Generator, MAX_DRAWS_PER_SAMPLE, MAX_SAMPLES, SplitCounts};

/// The ceiling of Da6-26, `i64::MAX`, past every count `--samples` admits today.
const I64_SAMPLES: u64 = i64::MAX as u64;

/// D-a6-10 restated without the implementation's division chain: 4992 / 60000 / 16 is 13 / 2500
/// a batch, so the held-out count is `16 · max(1, k)` with `k · 2500 ≤ 13 · samples <
/// (k + 1) · 2500`. Returns `(train, held_out)` as `i128`.
fn oracle(samples: u64) -> (i128, i128) {
    let thirteen_n = 13 * u128::from(samples);
    let k = thirteen_n / 2_500;
    assert!(k * 2_500 <= thirteen_n && thirteen_n < (k + 1) * 2_500);
    let held_out = 16 * i128::try_from(k.max(1)).unwrap();
    (i128::from(samples) - 2 * held_out, held_out)
}

fn assert_rule(samples: u64) {
    let counts = SplitCounts::from_samples(samples);
    let (train, held_out) = oracle(samples);
    assert_eq!(
        (
            i128::from(counts.train),
            i128::from(counts.validation),
            i128::from(counts.test)
        ),
        (train, held_out, held_out),
        "--samples {samples}"
    );
    assert!(
        counts.train < i64::MAX,
        "--samples {samples}: no saturation"
    );
    assert_eq!(counts.total(), samples, "--samples {samples}");
    assert_eq!(
        counts.check().is_ok(),
        samples.is_multiple_of(16),
        "--samples {samples}: train is a positive batch multiple exactly when samples is"
    );
}

/// Da6-26: up to `I64_SAMPLES` the counts are the rule's. Every value here has `samples · 4992`
/// past `i64::MAX` (an `i64` or wrapping `u64` computation would be wrong or panic), from the
/// first such value through `I64_SAMPLES / 2` to `I64_SAMPLES`, plus a strided sweep of the upper
/// half and its neighbours.
#[test]
fn adversary_the_count_rule_is_exact_where_samples_times_4992_passes_i64() {
    let first_past_i64 = (i64::MAX / 4_992 + 1) as u64;
    assert!(
        i64::try_from(first_past_i64 - 1)
            .unwrap()
            .checked_mul(4_992)
            .is_some()
    );
    let half = I64_SAMPLES / 2;
    let mut values = vec![
        first_past_i64,
        first_past_i64 + 15,
        half,
        half + 1,
        1 << 62,
        (1 << 62) + 8,
        I64_SAMPLES - 16,
        I64_SAMPLES - 15,
        I64_SAMPLES - 1,
        I64_SAMPLES,
    ];
    let step = (I64_SAMPLES - half) / 4_095;
    for i in 0..4_096u64 {
        let n = half + i * step;
        values.extend([n, n.saturating_sub(1), n + 1, n - n % 16]);
    }
    for samples in values {
        let samples = samples.min(I64_SAMPLES);
        assert!(
            i64::try_from(samples).unwrap().checked_mul(4_992).is_none(),
            "{samples}: the premise, samples · 4992 passes i64::MAX"
        );
        assert_rule(samples);
    }
}

/// `9223372036854775792` (`i64::MAX − 15`, a batch multiple, the largest `--samples` admitted
/// under Da6-26 and refused as a usage error since Da7-10): every split opens, the counts are the
/// rule's, and the draw bound saturates at `u64::MAX` (4 · samples passes it). A characterisation
/// of `Generator` beyond the CLI's range, not a specification.
#[test]
fn adversary_characterise_the_largest_i64_batch_multiple_has_a_saturated_draw_bound() {
    let samples = I64_SAMPLES - 15;
    assert_eq!(samples, 9_223_372_036_854_775_792);
    assert!(samples > MAX_SAMPLES, "past the CLI's range (Da7-10)");
    assert_eq!(samples % 16, 0);
    let counts = SplitCounts::from_samples(samples);
    let (train, held_out) = oracle(samples);
    assert_eq!(i128::from(counts.train), train);
    assert_eq!(i128::from(counts.validation), held_out);
    assert_eq!(counts.train % 16, 0);
    assert_eq!(counts.check(), Ok(()));
    let generator = Generator::new(42, counts, PathBuf::from("data/"));
    assert!(samples.checked_mul(MAX_DRAWS_PER_SAMPLE).is_none());
    assert_eq!(generator.max_draws, u64::MAX);
}

/// `--samples 4294967280` (`u32::MAX − 15`, the largest batch multiple the CLI admits since
/// Da7-10): every split opens, the counts are the rule's, every split fits `order.u32`, and the
/// draw bound is 4 · samples exactly, no longer saturated. A characterisation, not a
/// specification.
#[test]
fn adversary_characterise_the_largest_admitted_batch_multiple_has_an_exact_draw_bound() {
    let samples = MAX_SAMPLES - 15;
    assert_eq!(samples, 4_294_967_280);
    assert_eq!(samples % 16, 0);
    let counts = SplitCounts::from_samples(samples);
    let (train, held_out) = oracle(samples);
    assert_eq!(i128::from(counts.train), train);
    assert_eq!(i128::from(counts.validation), held_out);
    assert!(u32::try_from(counts.train).is_ok());
    assert_eq!(counts.train % 16, 0);
    assert_eq!(counts.check(), Ok(()));
    assert_eq!(counts.total(), samples);
    let generator = Generator::new(42, counts, PathBuf::from("data/"));
    assert_eq!(generator.max_draws, samples * MAX_DRAWS_PER_SAMPLE);
}
