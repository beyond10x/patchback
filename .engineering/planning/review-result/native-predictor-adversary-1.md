---
format: aep.planning-md/3
id: review-result:native-predictor-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: native predictor (wave a10, unit 1)'
relations:
- reviews: story:native-predictor
revision: 1
---
# Adversary pass 1: native predictor (story:native-predictor)

Wave a10, unit 1. `aep:adversary` on Opus against the uncommitted change in tree
`patchback-impl-native-predictor` (base 35eafa1), 2026-10-25. Known and not re-reported: the
Rust refusal order (Da10-4), the payload-only checksum (Da9-2), refusals not counted.

## Result

- verdict: green; cases executed 132 → 142 (patchback-model, both lanes), red 0; origin:
  introduced 3, pre-existing 0.
- Added `crates/patchback-model/tests/adversary_predictor.rs` (10 cases); changed nothing
  else. Both lanes exit 0 with the cases in place; the pinned lane ran the binary (no
  `SKIPPED`), SHA-256 `171d2880…4229` as pinned.
- F1 (warning, CONFIRMED, mutant): no implementor case pins how the sums add up. Summing in
  `f64`, in descending order or pairwise passes the three goldens (exact in every order), the
  `f64` reference check (1e-4) and the pinned equivalence (at most 9.5e-8 against 1e-3). The
  adversary's cancellation cases (:220 first layer, :247 second layer; inputs where `1e8 + 1`
  rounds in `f32`) pin the Da10-5 order.
- F2 (note, CONFIRMED, judgement): the equivalence limit 1e-3 (`tests/pinned_predict.rs:37`)
  is 14,000 times the measured 6.9e-8, while a transposed `w1` misses it by a factor of 5.9
  only (5.87e-3) and misaligned rows by 7.9 (7.85e-3). The adversary measured: `f64` sums
  7.6e-8, descending 5.5e-9, pairwise 9.5e-8, transposed `w2` 5.99e-2, all-zero outputs 0.62,
  `x` read one value late 3.97e-2.
- F3 (note, INFEASIBLE, judgement): the `NonFinitePrediction` doc names only `inf − inf`;
  `inf · 0` also yields a NaN and the whole prediction is then refused, including outputs
  Da10-7 would clamp. Nothing reaches a hidden value above 3.4e38.
- Held: a transposed `w1` or `w2` is caught by the implementor's one-hot golden; the clamp
  mutants (`<=`/`>=`, counting per output, counting every prediction, no clamp) are caught;
  `-0.0` cannot reach ReLU (every sum starts at `+0.0`); the Da10-4 order with two defects at
  once; a dropped `DOMAIN_ERRORS` variant fails two checks; the 19 removed `domain.rs` lines are
  rewording and raised counts; `lib.rs` additive, `weights.rs`, `training.rs`, `Cargo.*`
  unchanged; `raw` bit-equal across `load` and `from_artifact`; `x.f32` read row-major with
  32 columns; exact 1.0 not counted, `1 + ε` on 8 outputs counts 8 and the counter once;
  subnormals accepted as 0, `f32::MAX + 2^103` refused as non-finite; dangling symlink
  `Unknown`, NUL in the path, a path under a file and mode 0o000 `Io`.
- Noted: the adversary's pinned case runs its own 23 s generation.

## Routed

- F1 → the adversary's file stays in the unit (Da10-18).
- F2 → Da10-16: `EQUIVALENCE` tightened to 1e-5 (margin 145× over the measured 6.9e-8; a
  transposed `w1` misses it by 587×); the provisional value updated.
- F3 → Da10-17: the doc comment names `inf · 0` beside `inf − inf`; behaviour unchanged.

```findings
- file: crates/patchback-model/src/predict.rs
  line: 238
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: Summing in f64, in descending order or pairwise survives every implementor case; only the adversary cancellation cases at adversary_predictor.rs:220 and :247 pin the Da10-5 order.
- file: crates/patchback-model/tests/pinned_predict.rs
  line: 37
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: The 1e-3 equivalence limit is 14,000x the measured 6.9e-8, while a transposed w1 is only 5.9x the limit (5.87e-3) and misaligned rows 7.9x.
- file: crates/patchback-model/src/predict.rs
  line: 70
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: The NonFinitePrediction doc names only inf minus inf, but inf times a zero weight also refuses the whole prediction, including outputs Da10-7 would clamp; nothing found reaches a hidden value above 3.4e38.
```
