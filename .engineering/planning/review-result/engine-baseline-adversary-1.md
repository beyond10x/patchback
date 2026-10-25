---
format: aep.planning-md/3
id: review-result:engine-baseline-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: engine baseline benchmark'
relations:
- reviews: story:engine-baseline
revision: 1
---
# Adversary pass 1: engine baseline benchmark (story:engine-baseline)

Wave a8, unit 1. `aep:adversary` on Opus against the uncommitted change in tree
`patchback-impl-engine-baseline` (base d796084), 2026-10-25.

## Result

- verdict: red; cases executed 321 → 334 (patchback-fit 108 → 114, patchback-cli 213 → 220),
  red 2; origin: introduced 3.
- Added `crates/patchback-fit/tests/engine_baseline_adversary.rs` and
  `crates/patchback-cli/tests/benchmark_adversary.rs`; changed nothing else.
- F1 (note, INFEASIBLE): the target verdict is decided on the rounded `f64` reduction, so 325
  of 6000 constructed pairs near 20 % disagree with exact arithmetic (default 0.001, search
  0.0008 gives 20.0 and Met); no real pair of medians has been observed there, and the
  boundary scenario pins the comparison, not the formula.
- F2 (note, INFEASIBLE): `record_baseline_result` completes a benchmark as NotMet with a NaN
  reduction, which the model types `Decimal`; the CLI refuses a default median of 0 before it.
- F3 (note, judgement): `the_seeds_follow_the_documented_stream` searched the stream with
  `position`, so it accepted any number of skipped outputs and did not pin the derivation.
- Held: refused records change nothing; median edges; `DefaultArmError` reachability; the
  seeds of `clips.txt` equal the documented stream with a seed whose first draw is diagnostic;
  `compare` reproduces the best and default losses bit for bit for clips 0 to 2; an exhausted
  holdout exits 1 with nothing written; `--block-size` changes no sample; `examples` as a
  file, an empty directory, a nested `0/` or a dangling link refused; a failed write keeps a
  pre-existing `--out` and an unrelated file in it; the refusal order and the exit-2 thresholds;
  every admitted holdout clip passes the fit's reference analysis (200 of 200 at `--seed 0`).

## Routed

- F1 → Da8-35 (verdict on the recorded value, documented; the case re-pinned).
- F2 → Da8-36 (precondition documented, no new refusal; a spec outcome is the owner's).
- F3 → Da8-37 (exact recomputation). The suite-time note → Da8-38 (`--max-renders 2`).

```findings
- file: crates/patchback-fit/src/benchmark.rs
  line: 370
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: the target verdict is decided on the rounded f64 reduction, so pairs within one ulp of 20 % disagree with exact arithmetic, and the boundary scenario pins only the comparison
- file: crates/patchback-fit/src/benchmark.rs
  line: 256
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: record_baseline_result completes the benchmark as NotMet with a NaN reduction, which the model types as Decimal
- file: crates/patchback-cli/tests/benchmark.rs
  line: 300
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the_seeds_follow_the_documented_stream accepts arbitrary skipped stream outputs, so it does not pin the documented derivation
```

Report: wave a8 scratch `engine-baseline/adversary-report-1.md`. Agent cost 190,152 tokens,
51 tool uses, 1,431 s.
