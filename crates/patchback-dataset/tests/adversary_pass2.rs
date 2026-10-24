//! Adversary cases for story:dataset-generation (wave a6, pass 2): the split-count rule over the
//! upper half of the range `--samples` admits (Da6-26), where `samples · 4992` no longer fits an
//! `i64`, checked against an oracle that does not repeat the implementation's formula, and the
//! largest batch-multiple count the CLI admits.

use std::path::PathBuf;

use patchback_dataset::{Generator, MAX_DRAWS_PER_SAMPLE, MAX_SAMPLES, SplitCounts};

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

/// Da6-26: up to `MAX_SAMPLES` the counts are the rule's. Every value here has `samples · 4992`
/// past `i64::MAX` (an `i64` or wrapping `u64` computation would be wrong or panic), from the
/// first such value through `MAX_SAMPLES / 2` to `MAX_SAMPLES`, plus a strided sweep of the upper
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
    let half = MAX_SAMPLES / 2;
    let mut values = vec![
        first_past_i64,
        first_past_i64 + 15,
        half,
        half + 1,
        1 << 62,
        (1 << 62) + 8,
        MAX_SAMPLES - 16,
        MAX_SAMPLES - 15,
        MAX_SAMPLES - 1,
        MAX_SAMPLES,
    ];
    let step = (MAX_SAMPLES - half) / 4_095;
    for i in 0..4_096u64 {
        let n = half + i * step;
        values.extend([n, n.saturating_sub(1), n + 1, n - n % 16]);
    }
    for samples in values {
        let samples = samples.min(MAX_SAMPLES);
        assert!(
            i64::try_from(samples).unwrap().checked_mul(4_992).is_none(),
            "{samples}: the premise, samples · 4992 passes i64::MAX"
        );
        assert_rule(samples);
    }
}

/// `--samples 9223372036854775792` (`i64::MAX − 15`, a batch multiple): every split opens, the
/// counts are the rule's, and the draw bound saturates at `u64::MAX` (4 · samples passes it), so
/// a generation of this size is admitted and nothing bounds it short of the samples themselves.
/// A characterisation of the current behaviour (pass-2 finding on the admitted range), not a
/// specification.
#[test]
fn adversary_characterise_the_largest_batch_multiple_is_admitted_with_a_saturated_draw_bound() {
    let samples = MAX_SAMPLES - 15;
    assert_eq!(samples, 9_223_372_036_854_775_792);
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
