---
format: aep.planning-md/3
id: review-result:normalization-stats-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: normalization statistics'
relations:
- reviews: story:normalization-stats
revision: 1
---
# Adversary pass 1: normalization statistics (story:normalization-stats)

Wave a8, unit 2. `aep:adversary` on Opus against the uncommitted change in tree
`patchback-impl-normalization-stats` (base d796084), 2026-10-25, after the implementor had
applied the Da8-30 addendum (x.f32 z-scored by the writer).

## Result

- verdict: red; cases executed 433 → 436 (patchback-analysis 150 → 153), red 1; origin:
  introduced 1.
- Added `crates/patchback-analysis/tests/normalization_adversary.rs` (3 cases); changed
  nothing else.
- F1 (warning, INFEASIBLE through the generator): a training column whose values are all one
  non-dyadic value (0.1, 1/3, −12.3) got a rounding-noise standard deviation from the two-pass
  formula (1.39e-17 at n = 3 for 0.1, 1.2e-13 at n = 50,016 for −12.3), so `apply` gave ±1
  instead of +0.0; 20 of 24 (value, n) pairs failed. No generated column was constant (seed
  42, 64 samples: smallest std 9.38 in column 24).
- Green cases kept: the two-pass standard deviation is exact far from 0 (rows 1e8 ± 1 give
  std 1; the one-pass formula misses) and every finite real round-trips through
  `write_stats`/`read_stats` bit for bit (−0.0, 1e-300, ±5e-324, 1e21, f64::MAX, 4,000 random
  bit patterns).
- Judgement, Da8-17: `write_split` accepts any statistics table; detecting a wrong one would
  take two compares (`dataset_split_id` against the Train split, `feature_contract_id`
  against the dataset's). Recorded for the owner, no change.
- Judgement, the CLI test's tolerance (1e-12 × column max) would not catch the one-pass
  formula (error about 1e-11 on column 26 against 7.7e-9), which the new exact case now does.
- Held: outcome order and payloads of the three scenarios; `None` split input first; n versus
  n − 1 caught by the hand-computed case; 0 and 1 rows; NaN and inf refused in compute and
  apply; the `read_stats` refusals (BOM, CRLF, a 33rd line, a repeated or out-of-order index,
  a negative std, a non-integer `rows`); the stats-ignored, mean-over-all-splits,
  divisor-0, normalize-y, `std sample` and reversed-features mutants each caught by a named
  case (by reasoning on the suite); failed writes remove `normalization.stats`; a pre-existing
  file refused by the pre-check.

## Routed

- F1 → Da8-44: a column whose values are all bit-equal to its first is constant (mean that
  value, std 0), applied in the correction round.
- The stale `DenseFileKind::X` doc (implementor's own note) → Da8-45, same round.

Report: wave a8 scratch `normalization-stats/adversary-report-1.md`. Agent cost 147,536 tokens,
40 tool uses, 669 s.
