---
format: aep.planning-md/3
id: review-result:native-predictor-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: native predictor (wave a10, unit 1)'
relations:
- reviews: story:native-predictor
revision: 1
---
# Adversary pass 2: native predictor (story:native-predictor)

Wave a10, unit 1, after correction round 1 (Da10-16, Da10-17). `aep:adversary` on Opus against
the uncommitted change in tree `patchback-impl-native-predictor` (base 35eafa1), 2026-10-25.
Known and not re-reported: the pass-1 findings and the cases that closed them, Da10-4, Da9-2,
refusals not counted.

## Result

- verdict: green; cases executed 142 → 144 (patchback-model, both lanes), red 0; origin:
  introduced 1, pre-existing 0.
- The diff since the pass-1 snapshot is the two named hunks only: `src/predict.rs:70-72` (the
  `NonFinitePrediction` doc, Da10-17) and `tests/pinned_predict.rs:35-38` (`EQUIVALENCE` 1e-5,
  Da10-16); `predictor.rs`, `predictor_scenarios.rs` and the pass-1 file byte-identical; the
  tracked diff against 35eafa1 unchanged.
- Added `crates/patchback-model/tests/adversary_predictor_2.rs` (2 cases): `inf · 0` refuses the
  whole prediction with the lowest NaN index named and the counter unchanged, while 8 `±inf`
  outputs without a zero weight are clamped and counted; in the pinned lane, 103 measured
  defects all exceed 1e-5 (the closest, inputs at half precision, 6.47e-5), and four of them
  (weights at half precision 1.10e-4, weights at bfloat16 4.42e-4, inputs at half precision
  6.47e-5, input 12 ignored 6.77e-4) passed the old 1e-3.
- The new limit shown red: `EQUIVALENCE = 1e-9` on a scratch copy fails with "the predictor's
  mean squared error 0.9350907154992748 is not ModelSpec's eval loss 0.9350907802581787"; the
  measured relative difference 6.92541358263887e-8 is 144× under 1e-5.
- Both lanes exit 0 (144 passed; plain 8 `SKIPPED`, pinned 0); clippy and fmt exit 0; the pinned
  binary's SHA-256 matches `models/modelspec.pin`.
- F1 (note, CONFIRMED, judgement): `tests/adversary_predictor.rs:218`'s doc comment still says
  the pinned equivalence is 1e-3; the claim it makes (the order mutants pass it) holds at 1e-5
  too (at most 9.5e-8).
- Held: the relative comparison divides by ModelSpec's value (a per-row-sum reading gives 7.0, a
  halved loss 0.5; NaN fails `<=`; a separate finiteness assert); the only other copy of the
  constant (`adversary_predictor.rs:525`, 1e-3) is the must-differ threshold for the wrong
  layouts; `weights.rs:586` refuses non-finite weights, so `inf · 0` needs a hidden `inf` and a
  zero `w2` weight, as the doc says; `provisional.md`'s row says 1e-5 (its 145× is 144× from
  the precise value).
- Corrected by the adversary: pass 1 reported 6 `SKIPPED` in the plain lane, its log shows 7;
  8 now with the pass-2 pinned case.

## Routed

- F1 → Da10-19: the coordinator changed the comment to "(1e-5, Da10-16)" in the unit tree
  before its own verification; the cases are untouched.

```findings
- file: crates/patchback-model/tests/adversary_predictor.rs
  line: 218
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: The doc comment names the pinned equivalence limit as 1e-3 after Da10-16 made it 1e-5; the claim (order mutants pass it) still holds.
```
