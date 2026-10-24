---
format: aep.planning-md/3
id: review-result:fit-run-artifacts-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: fit run artifacts'
relations:
- reviews: story:fit-run-artifacts
revision: 1
---
# Adversary pass 1: fit run artifacts (story:fit-run-artifacts)

Wave a7, unit 1. `aep:adversary` on Opus against the implementor's uncommitted change in tree
`patchback-impl-fit-run-artifacts` (base 4b99e5b), 2026-10-25.

## Result

- verdict: red; cases executed 260 → 272 (patchback-fit 72 → 80, patchback-cli 188 → 192),
  red 3; origin: introduced 3, pre-existing 0.
- Added `crates/patchback-fit/tests/results_adversary.rs` and
  `crates/patchback-cli/tests/fit_artifacts_adversary.rs`; changed nothing else (it overwrote
  `crates/patchback-cli/tests/fit_adversary.rs` once and restored it byte for byte).
- F1 (warning, CONFIRMED): the limiter warning counts the clipped samples of the whole uncut
  render (`limiter engaged on 6242 samples`, render 33,600 frames) while `reconstructed.wav`
  holds the reference's 6,000 frames, 2,163 of them at the threshold (`results.rs:215`).
- F2 (note, INFEASIBLE): `parse_trace` accepts an evaluation count that differs from the
  sequence (`evaluation 2 9 …`); no run writes such a line (`trace.rs:305`).
- F3 (note, INFEASIBLE): `parse_trace` accepts a best loss that rises, against `TraceFrame`'s
  "never rises" (`trace.rs:355`).
- F4 (note, judgement): the implementor's trace tests only round-trip through
  `parse_trace(write_trace(..))`; `results_adversary.rs:154` now pins the exact field order.
- Held under attack: decimal round trip bit for bit (subnormal, 1e-300, 0.1, 1/3, 17 digits,
  largest finite, -0.0, a rejected evaluation); refusals of a differing topology, a duplicate
  sequence, an extra field; a failed fourth write removes only the three files it wrote; the
  `ulimit` write failure is the limit's (control run succeeds); `--block-size` 64 vs 4096
  differ only in `wall_time_ms` and `block_size`; trailing WAV bytes are accepted and digested;
  `--out` as a dangling link or a link to a file refused before the search; refusal order of
  `WriteFitResults`; the additive rule (two `FitError` variants added, nothing changed).

## Routed

- F1 → Da7-15: the warning names both counts, `limiter engaged on <n> of <m> rendered samples`;
  the adversary's case is re-pinned to it in correction round 1.
- F2, F3 → Da7-16: `parse_trace` refuses both; the cases stay as written.
- F4 → the order case stays; nothing to add.

Report: wave a7 scratch `fit-run-artifacts/adversary-report-1.md`. Agent cost 156,569 tokens,
38 tool uses, 821 s.

```findings
[
{"file":"crates/patchback-fit/src/results.rs","line":215,"category":"acceptance","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the limiter warning counts clipped samples of the whole uncut render (6242, render 33600 frames) while reconstructed.wav holds the reference's 6000 frames, 2163 of them at the threshold"},
{"file":"crates/patchback-fit/src/trace.rs","line":305,"category":"contract-drift","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"parse_trace accepts an evaluation count that differs from the sequence (evaluation 2 9 ...), a line no run writes"},
{"file":"crates/patchback-fit/src/trace.rs","line":355,"category":"contract-drift","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"parse_trace accepts a best loss that rises from 0.1 to 0.5, against TraceFrame's never-rises rule"},
{"file":"crates/patchback-fit/tests/results.rs","line":1,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the trace tests only round-trip through parse_trace(write_trace(..)), so a writer and parser that both swapped total and best would pass; results_adversary.rs:154 pins the exact field order"}
]
```
