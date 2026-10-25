---
format: aep.planning-md/3
id: review-result:engine-baseline-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: engine baseline benchmark'
relations:
- reviews: story:engine-baseline
revision: 1
---
# Adversary pass 2: engine baseline benchmark (story:engine-baseline)

Wave a8, unit 1, the budget pass. `aep:adversary` on Opus against the corrected uncommitted
change in tree `patchback-impl-engine-baseline` (base d796084), 2026-10-25.

## Result

- verdict: green; cases executed 334 → 338 (patchback-fit 114 → 116, patchback-cli 220 → 222),
  red 0; origin: introduced 3.
- Added two cases to each of `engine_baseline_adversary.rs` and `benchmark_adversary.rs`;
  changed nothing else.
- Da8-35 to Da8-38 hold: the 6000 boundary pairs tell `>` from `>=` and four reorderings of
  the formula apart; a default median of `+0.0` records `−inf` and NotMet, `−0.0` records
  `+inf` and Met, both Completed as documented; the rewritten seed test catches an extra
  skipped output and a reused fit seed; the 2-render determinism case still compares every
  file the run writes.
- G1 (warning, mutant): no CLI case had run a restart past restart 0 (`--max-restarts 1`
  everywhere), so the fit seed passed to the fit was unpinned; the new case
  `the_fit_is_seeded_with_the_recorded_fit_seed` pins it.
- G2 (note, judgement): the implementor's `draws <= 2 × MAX_DRAWS_PER_SAMPLE` assertion read
  only the test's own stream walk and could not fail for a verb defect; the new case
  `the_draw_bound_admits_exactly_four_draws_a_clip` pins the verb's bound at both edges.
- G3 (note, judgement, INFEASIBLE today): the holdout drew patch seeds from the same
  `SplitMix64(seed)` stream as `generate-dataset` and the fit's restarts, so with equal seeds
  holdout clip 0 was training sample 0 and a fit seeded with `--seed` started at clip 0's own
  patch (loss 0.0); it matters once an ML-init benchmark exists.
- Held: the doc sites of Da8-35 match the code; the CLI's refusal of a default median of 0 is
  exact and not pinnable through the binary (no seed gives a default loss of 0).

## Routed

- G1 → the case stays, no change. G2 → Da8-47 (the assertion dropped).
- G3 → Da8-46 (`SplitMix64::new(seed ^ HOLDOUT_DOMAIN)`, taken before 0.7.0 ships
  `clips.txt`); the fit-seed case re-pinned at `--seed 83` (Da8-50) because at most seeds
  restart 1 loses to restart 0.

```findings
- file: crates/patchback-cli/src/benchmark.rs
  line: 362
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: no CLI test ran a restart past restart 0, so the fit seed passed to fit was unpinned until the pass-2 case the_fit_is_seeded_with_the_recorded_fit_seed
- file: crates/patchback-cli/tests/benchmark.rs
  line: 340
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the draws bound assertion reads only the test's own stream walk and cannot fail for a defect in the verb
- file: crates/patchback-cli/src/benchmark.rs
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: holdout patch seeds come from the same SplitMix64(seed) stream as generate-dataset and the fit's restarts, so equal seeds overlap the training draws and a fit seeded with --seed starts at clip 0's patch; this matters once an ML-init benchmark exists
```

Report: wave a8 scratch `engine-baseline/adversary-report-2.md`. Agent cost 244,566 tokens,
35 tool uses, 1,154 s.
