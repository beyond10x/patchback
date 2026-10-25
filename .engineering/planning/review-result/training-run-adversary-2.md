---
format: aep.planning-md/3
id: review-result:training-run-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: CPU training run'
relations:
- reviews: story:training-run
revision: 1
---
# Adversary pass 2: CPU training run (story:training-run)

Wave a8, unit 3, the budget pass. `aep:adversary` on Opus against the corrected uncommitted
change in tree `patchback-impl-training-run` on merge 054b03f (unit 2's normalized dense
files), 2026-10-25.

## Result

- verdict: green; cases executed 60 → 70 (patchback-model), red 0, in both lanes; origin:
  introduced 2, pre-existing 1.
- Added `crates/patchback-model/tests/adversary_training_2.rs` (10 cases); changed nothing
  else.
- Da8-39 to Da8-42 hold: the full (run state, source state, pin) matrix refuses in the order
  state → `ModelNotChecked` → `PinMismatch` with the value unchanged; `bind(&source)` and
  `bind(&checked)` agree; exit 0 with a malformed line, a repeated step, only an `eval` line
  or `eval loss=nan` is Completed with the matching `output_error`; exit 1 after a clean curve
  is Failed with stdout and stderr kept; a signal is Failed with exit status None;
  `Duration::MAX` with a 1 s child completes; a repeated step is refused and the gap (0, 4)
  parses.
- The Da8-27 loss-decrease check of the pinned test goes red for stand-ins printing
  `1 → 1`, `1 → 2`, `1 → NaN` and an `eval` line only (`NoStepLine`), each exit 101.
- End to end through the binaries: `patchback generate-dataset --seed 42 --samples 96`
  (64/16/16 rows, `normalization.stats` written, exit 0), `modelspec check` exit 0,
  `modelspec train … --steps 4` exit 0 with `step 0 loss 1.0717294`, `step 4 loss
  0.8882302`, `eval loss=0.9350907802581787 accuracy=0.0625`.
- G1 (warning, pre-existing): the guide's `modelspec train` command without `--steps`
  (`website/docs/guides/train-the-regressor.md:54`, DESIGN.md:324) exits 1 with `E-DATA-008`
  on the generated data (indices 64, needed 80,000) and on a 50,016-row split (needed 80,000).
- G2 (note, mutant): accepting a repeated step number survived the 60-case suite; the two
  new parser cases catch it.
- G3 (note): the implementor's scratch guide wording said the loss curve is empty for every
  output error; a `NoEvalLine` run keeps its curve.
- The pinned lane with the pinned binary: 70 passed, 0 failed. Wall time: `pinned_training`
  22.43 s (22.39 s generation), `training_scenarios` about 11.6 s.

## Routed

- G1 → Da8-49: the guide written at integration documents `--steps` with steps × 16 ≤ the
  training rows; DESIGN.md:324 is the owner's.
- G2 → the cases stay. G3 → the guide wording at integration says which output errors leave
  the curve empty.

```findings
- file: website/docs/guides/train-the-regressor.md
  line: 54
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: the guide's modelspec train command without --steps exits 1 with E-DATA-008 at the documented 50,016-row split and on CLI-generated data
- file: crates/patchback-model/src/training.rs
  line: 160
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: accepting a repeated step number survives the 60-case suite; the two pass-2 parser cases catch it
- file: scratch/guide-doc.md
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the proposed guide wording says the loss curve is empty for every output error, but a NoEvalLine run keeps its curve
```

Report: wave a8 scratch `training-run/adversary-report-2.md`. Agent cost 254,152 tokens by the
harness count (the agent states about 70,000 for this pass), 24 tool uses, 780 s.
