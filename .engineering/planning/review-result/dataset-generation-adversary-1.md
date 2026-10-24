---
format: aep.planning-md/3
id: review-result:dataset-generation-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: dataset generation'
relations:
- reviews: story:dataset-generation
revision: 1
---
# Adversary pass 1: Dataset generation (story:dataset-generation)

Wave a6, unit 2. `aep:adversary` on Opus against the implementor's uncommitted change in tree
`patchback-impl-dataset-generation` (base 76bb7ec), 2026-10-24.

## Result

- verdict: red; cases executed 191 → 201 (patchback-dataset 34 → 40, patchback-cli 157 → 161),
  red 1; origin: introduced 4, pre-existing 1.
- Added `crates/patchback-dataset/tests/adversary.rs` (6 cases) and
  `crates/patchback-cli/tests/generate_dataset_adversary.rs` (4 cases); changed nothing else.
  Mutants ran in a scratch copy of the workspace.
- F1 (note): `--samples 18446744073709551600` is refused as `SplitCountNotBatchMultiple`
  naming a train count of 9223372036854775807 (saturated) where the rule gives
  15377205859844282224, a positive multiple of 16 (`generate.rs:86`).
- F2: the note's velocity 1.0 and start at sample 0 (D-a6-9) were pinned by no case; velocity
  0.5 and start 1 mutants pass the implementor's suite (`generate.rs:46`, `:224`).
- F3: the `samples.meta` row order was pinned by no case; a reversed-rows mutant passes the
  implementor's suite (`meta.rs:80`).
- F4 (judgement, NEEDS-CHANGE): `SplitRole::key` names the split directory, the `dataset.meta`
  record and the stdout key at once, so the Da6-19 rename to `val` would rename the
  `validation` record too (`model.rs:136`).
- F5 (measurement, pre-existing): `PitchAmbiguous` with confidence 0.500 is the estimator's
  undecided-subharmonic branch (`UNDECIDED_CONFIDENCE`, `crates/patchback-analysis/src/pitch.rs:251`);
  seed 42: 15 of 95 draws. For three such patches the nominal f0 returns within 0.04 Hz after
  one change: cutoff to 16 kHz (3 of 3), drive to 0 dB (2 of 3), a flat envelope (1 of 3).
  Gate length and a cutoff below the fundamental are not the cause.
- Held under attack: every `dataset.yaml` outcome present and in the model's order, no model
  guard lost; the count rule at 60000, 48, 47, 32, 20 and 0; MIDI 36..=84 inclusive; the
  MIDI-first and whole-render-span mutants are caught by the implementor's suite; diagnostic
  counts equal draws minus samples; no seed in two splits; `--seed 0` and `--seed u64::MAX`
  generate; exhaustion happens before the first write.

## Routed

- F1 → Da6-26: `--samples` admits `0..=i64::MAX` through clap; the red case is re-pinned.
- F2, F3 → closed by the adversary's cases; no code change.
- F4 → Da6-27: `SplitRole::dir()` (`val`) separate from `key()` (`validation`).
- F5 → Da6-28: recorded for the analysis owner; the measurement case stays.

Report: wave a6 scratch `dataset-generation/adversary-report-1.md`. Agent cost 205,589 tokens,
52 tool uses, 1431 s.

```findings
[
{"file":"crates/patchback-dataset/src/generate.rs","line":86,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a --samples past the i64 train range is refused as SplitCountNotBatchMultiple naming 9223372036854775807 instead of the rule's train count, which is a positive multiple of 16"},
{"file":"crates/patchback-dataset/src/generate.rs","line":46,"category":"test-coverage","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"note velocity and start (D-a6-9) were pinned by no case; velocity 0.5 and start 1 mutants pass the unit's suite (now caught by the adversary case)"},
{"file":"crates/patchback-dataset/src/meta.rs","line":80,"category":"test-coverage","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"samples.meta row order was pinned by no case; a reversed-rows mutant passes the unit's suite (now caught by the adversary case)"},
{"file":"crates/patchback-dataset/src/model.rs","line":136,"category":"judgement","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"SplitRole::key names the split directory, the dataset.meta record and the stdout key at once, so the Da6-19 rename to val also renames the validation record"},
{"file":"crates/patchback-analysis/src/pitch.rs","line":251,"category":"measurement","severity":"note","verdict":"INFEASIBLE","origin":"pre-existing","message":"PitchAmbiguous 0.500 is the estimator's undecided-subharmonic branch, resolved by opening the cutoff, removing drive or flattening the envelope; 16% of seed-42 draws become Noise diagnostics this way"}
]
```
