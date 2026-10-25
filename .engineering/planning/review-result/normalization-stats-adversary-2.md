---
format: aep.planning-md/3
id: review-result:normalization-stats-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: normalization statistics'
relations:
- reviews: story:normalization-stats
revision: 1
---
# Adversary pass 2: normalization statistics (story:normalization-stats)

Wave a8, unit 2, the budget pass. `aep:adversary` on Opus against the corrected uncommitted
change in tree `patchback-impl-normalization-stats` (base d796084), 2026-10-25.

## Result

- verdict: green; cases executed 437 → 442 (patchback-analysis 154 → 158, patchback-dataset
  80 → 81, patchback-cli 203), red 0; origin: introduced 1.
- Added `crates/patchback-analysis/tests/normalization_adversary2.rs` (4 cases) and
  `crates/patchback-dataset/tests/normalization_adversary.rs` (1 case); changed nothing else.
- Da8-44 holds: columns constant at `f64::MAX`, `1e200` and `−f64::MAX` (sums overflow) are
  admitted with the value as mean and std +0.0 while a varying overflowing column is still
  refused as `NonFiniteFeature`; a column of mixed signed zeros takes the formula path (mean
  +0.0, `apply(−0.0)` = −0.0, which is `== 0`); a column constant at one NaN bit pattern or
  ±inf is refused before the constant rule; one row gives its values as means bit for bit and
  std +0.0; a std-0 column writes `00 00 00 00` in every `x.f32` row and the read-back passes.
- Mutants run in a throwaway copy (each exit 101): `n − 1` and the one-pass formula killed
  by three cases each (`three_rows_give_the_hand_computed_statistics`,
  `means_and_population_standard_deviations_match_an_independent_loop`,
  `the_two_pass_standard_deviation_is_exact_far_from_0`); the constant rule removed killed by
  five.
- G1 (note, contract drift): the module doc said every overflowing sum is refused, which
  Da8-44 made untrue for a constant column.
- Notes: `generate_dataset_stats.rs` pins the literal "32" and takes 30.45 s; 48 samples
  would need that literal re-pinned (no change).

## Routed

- G1 → Da8-48: one sentence of the module doc reworded by the coordinator before the unit's
  commit; the gate ran after it (442 passed, clippy and fmt exit 0).

Report: wave a8 scratch `normalization-stats/adversary-report-2.md`. Agent cost 176,469 tokens,
14 tool uses, 736 s.
