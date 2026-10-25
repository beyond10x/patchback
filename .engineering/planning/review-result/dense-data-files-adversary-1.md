---
format: aep.planning-md/3
id: review-result:dense-data-files-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: ModelSpec dense-data files'
relations:
- reviews: story:dense-data-files
revision: 1
---
# Adversary pass 1: ModelSpec dense-data files (story:dense-data-files)

Wave a7, unit 2. `aep:adversary` on Opus against the implementor's uncommitted change in tree
`patchback-impl-dense-data-files` (base 4b99e5b, with the coordinator's Da7-13 re-pin applied),
2026-10-25.

## Result

- verdict: red; cases executed 245 → 255 (patchback-dataset 60 → 68, patchback-cli 185 → 187),
  red 2; origin: introduced 2, pre-existing 1.
- Added `crates/patchback-dataset/tests/dense_adversary.rs` (8 cases) and
  `crates/patchback-cli/tests/generate_dataset_dense_adversary.rs` (2 cases); changed nothing
  else.
- F1 (warning, INFEASIBLE): `write_dense_file` with content shorter than the input's
  `byte_count` reports the content's length in `ByteCountMismatch`, where the domain's payload
  is `input.byte_count` (`model.rs:897`); `dense_scenarios.rs:361` pins the drift. The writer
  always passes `byte_count = content.len()`, so the CLI never takes the branch.
- F2 (note, INFEASIBLE): `write_split` on a split already `Written` writes and records three
  files before `FinishSplit` returns `SplitAlreadyWritten`, whose summary says nothing changed
  (`dense.rs:128`). The CLI calls it once per fresh split.
- F3 (note, CONFIRMED, pre-existing): `every_error_of_the_eight_commands_is_emitted`
  (`tests/domain.rs:84`) reads `DatasetError::examples()`, a hand-built list, so it observes no
  command emitting anything; same body at base, renamed from "six".
- Held under attack: all 31 combinations of the five `WriteDenseFile` refusals in the model's
  order; the f32 edges by an independent reader (subnormals, -0.0, the value just above
  `f32::MAX` admitted, the rounding midpoint refused as infinite); hand-built `order.u32`
  (reversed permutation admitted, N±1 entries and a non-multiple of 4 as `ByteCountMismatch`, a
  duplicate as `InvalidOrder`); a `write` closure that writes nothing, twice, or elsewhere; the
  CLI pre-check on an existing or dangling dense path; determinism over all 13 files; the
  additive rule; `columns()` from the contracts; the Da7-13 re-pins.

## Routed

- F1 → Da7-17: the error carries the input's `byte_count`, as the domain's payload says;
  `the_content_itself_is_verified` is re-pinned in correction round 1.
- F2 → Da7-18: `write_split` refuses a split that is not `Open` before any write, as
  `SplitAlreadyWritten`.
- F3 → Da7-19: the test drives each of the twelve errors through the library's commands.

Report: wave a7 scratch `dense-data-files/adversary-report-1.md`. Agent cost 155,466 tokens,
37 tool uses, 1259 s.

```findings
[
{"file":"crates/patchback-dataset/src/model.rs","line":897,"category":"contract-drift","severity":"warning","verdict":"INFEASIBLE","origin":"introduced","message":"ByteCountMismatch for content shorter than the input byte count reports the content length, where spec/domains/dataset.yaml's payload is input.byte_count, and dense_scenarios.rs:361 pins the drift"},
{"file":"crates/patchback-dataset/src/dense.rs","line":128,"category":"concurrency","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"write_split of a Written split writes and records three files before returning SplitAlreadyWritten, whose domain summary says nothing changed"},
{"file":"crates/patchback-dataset/tests/domain.rs","line":84,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"every_error_of_the_eight_commands_is_emitted checks a hand-built examples() list, so it observes no command emitting anything"}
]
```
