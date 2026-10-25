---
format: aep.planning-md/3
id: review-result:training-run-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: CPU training run'
relations:
- reviews: story:training-run
revision: 1
---
# Adversary pass 1: CPU training run (story:training-run)

Wave a8, unit 3. `aep:adversary` on Opus against the uncommitted change in tree
`patchback-impl-training-run` (base d796084), 2026-10-25, before unit 2's normalized data
reached the branch (the pinned lane's NaN on raw rows was known, Da8-30, and not re-reported).

## Result

- verdict: red; cases executed 47 → 59 (patchback-model), red 4; origin: introduced 6,
  pre-existing 1.
- Added `crates/patchback-model/tests/adversary_training.rs` (12 cases); changed nothing else.
- F1 (warning, CONFIRMED): binding an unchecked source to a bound run answered
  `ModelNotChecked` through the documented `trainable()`-then-`bind` route, where the ESS
  interpreter answers `TrainingRunPastPlanning` (probe scenario through `ess verify conform
  run --target interpreted`).
- F2 (warning, CONFIRMED): a child exiting 0 with `--eval` and no `eval` line was recorded
  Failed with exit status 0, an outcome `FinishTrainingRun` does not declare (`failed: when
  exit_status != 0`).
- F3 (note, INFEASIBLE, pre-existing): `start` with `Duration::MAX` panicked on `Instant`
  overflow (`lib.rs:382`, the same expression at d796084).
- F4 (note, INFEASIBLE): `LossCurve::parse` accepted step numbers that go back.
- F5 to F7 (notes, mutants): passing the data directory as `--eval`, dropping the `OtherPin`
  refusal at `start`, and turning a spawn failure into a Failed run each survived the existing
  suite; the new cases kill them.
- Held: an unknown data split on a Planned run answers `wrong-state`, as the interpreter does;
  an unknown eval split is `EvalNotWritten`; count 0 refused by `open_split`; the pinned binary
  refuses 63 rows against 64 (`E-DATA-007`), a 15-row eval (`E-DATA-009`), and `--steps 5` on
  64 rows (`E-DATA-008`, indices 64, needed 80); `--steps 0` exits 0 with one step line; the
  timeout kills within 5 s; a SIGKILL is Failed with exit status None; 1 MiB on each pipe
  completes without deadlock; the parser reads `1e-3`, `-0.5`, `inf`, `1.`, `nan`, CRLF and
  the pinned binary's own lines.

## Routed

- F1 → Da8-39 (`bind` takes the unchecked source, state first), Da8-43 (the case calls
  `bind(&unchecked)` directly).
- F2 → Da8-40 (`finish` by exit status alone; the parse error recorded beside the state; a
  spec outcome for unusable output is the owner's).
- F3 → Da8-41 (`checked_add`). F4 → Da8-42 (`Malformed`). F5 to F7 → the cases stay.

```findings
- file: crates/patchback-model/src/training.rs
  line: 518
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: binding an unchecked source to a non-Planned run answers ModelNotChecked through the documented trainable-then-bind route, where the ESS interpreter answers TrainingRunPastPlanning
- file: crates/patchback-model/src/training.rs
  line: 650
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: a child exiting 0 with --eval and no eval line is recorded Failed with exit_status 0, an outcome FinishTrainingRun does not declare
- file: crates/patchback-model/src/lib.rs
  line: 382
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: pre-existing
  message: start with Duration::MAX panics on Instant overflow instead of running or refusing
- file: crates/patchback-model/src/training.rs
  line: 142
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: LossCurve::parse accepts step numbers that go back although its doc and the Da8-27 check assume increasing order
- file: crates/patchback-model/src/training.rs
  line: 613
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: passing the data directory as --eval survives the existing suite
- file: crates/patchback-model/src/training.rs
  line: 592
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: removing the OtherPin refusal at start survives the existing suite
- file: crates/patchback-model/src/training.rs
  line: 622
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: turning a spawn failure into a Failed run instead of leaving the run ModelBound survives the existing suite
```

Report: wave a8 scratch `training-run/adversary-report-1.md`. Agent cost 191,959 tokens,
62 tool uses, 1,055 s.
