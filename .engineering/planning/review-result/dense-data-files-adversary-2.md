---
format: aep.planning-md/3
id: review-result:dense-data-files-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: ModelSpec dense-data files'
relations:
- reviews: story:dense-data-files
revision: 1
---
# Adversary pass 2: ModelSpec dense-data files (story:dense-data-files)

Wave a7, unit 2, the budget pass. `aep:adversary` on Opus against the corrected uncommitted
change in tree `patchback-impl-dense-data-files` (base 4b99e5b), 2026-10-25.

## Result

- verdict: green; cases executed 255 → 262 (patchback-dataset 68 → 75, patchback-cli 187),
  red 0; origin: introduced 3, pre-existing 1.
- Added `crates/patchback-dataset/tests/dense_adversary_pass2.rs` (7 cases); changed nothing
  else.
- The correction answered F1 to F3 of pass 1 without loss: the re-pin keeps every other
  assertion, the pass 1 files predate the correction and both red cases are green, the Da7-13
  re-pins are byte-identical to the implementor's patched copies.
- G1 (note, mutant): dropping the `Order` exemption of the non-finite check (`model.rs:903`)
  left all 255 earlier cases green; the new case at `dense_adversary_pass2.rs:261` kills it.
  Unreachable today (`check_order` refuses an entry ≥ N first).
- G2 (note, mutant): a wrapping `i64` byte-count product (`model.rs:889`) admits 2^61 rows with
  `byte_count` 0; only the new case at `:227` kills it.
- G3 (note, judgement): the code's and the scratch docs' wording "`u32::MAX`, the most rows
  `order.u32` indexes" is off by one (a file of N entries each below N holds 2^32 rows); the
  bound is the `u32` row count at `dense.rs:147`, and the CLI cap applies to `--samples` as a
  whole, not to one split.
- Carried F3: Da7-19's rewritten test drives 17 real calls; mutants dropping
  `WrongColumnCount` or `MissingDenseFile` turn it red; it pins per-command name sets, not
  order or payloads, which `dense_scenarios.rs:239` and `dense_adversary.rs:175` pin.
- Mutants on a copy: M3 (drop the `state != Open` check) killed only by adversary cases; M4
  (content length in the second branch) killed by four cases; M5 (`SplitAlreadyWritten`
  before `UnknownSplit`) equivalent, an unknown split has no state.
- Held: content longer than the byte count, a consistent byte count with a wrong shape,
  negative rows or columns, rows past `u32::MAX` (2^32, 2^61, `i64::MAX − 15`) all refused
  with the input's values and no panic; `write_split` on a `Written` split returns
  `SplitAlreadyWritten { state: Written }` with 0 writes and 3 records; after a partial
  failure a second `write_split` fails `AlreadyExists` on `x.f32` and the split stays `Open`
  (nothing reaches it: the CLI calls `write_split` once per split); `TooManyRows` is dead
  from the CLI and the library; a 48-sample run gives 2,048 / 512 / 64 bytes a split,
  byte-identical across two processes over all 13 files; the refusal texts and the exit
  table match the scratch docs.

## Routed

- G1, G2 → the new cases stay; no code change.
- G3 → the coordinator reworded the four doc comments (`dense.rs:25`, `:54`,
  `generate.rs:64`, `cli/generate_dataset.rs:12`) before the unit's commit and carries the
  wording into the published docs at integration.
- Carried F3 → answered by Da7-19.

Report: wave a7 scratch `dense-data-files/adversary-report-2.md`. Agent cost 189,939 tokens,
45 tool uses, 1000 s.

```findings
[
{"file":"crates/patchback-dataset/src/model.rs","line":903,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"removing the Order exemption from the non-finite check left all 255 earlier cases green; only the new dense_adversary_pass2.rs:261 kills it"},
{"file":"crates/patchback-dataset/src/model.rs","line":889,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a wrapping i64 byte-count product admits 2^61 rows with byte_count 0 and left all 255 earlier cases green; only the new dense_adversary_pass2.rs:227 kills it"},
{"file":"crates/patchback-dataset/src/dense.rs","line":25,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the code and the scratch docs say u32::MAX is the most rows order.u32 indexes, but a u32 file of N entries each below N holds 2^32 rows; the bound is the u32 row count at dense.rs:147, and the CLI cap applies to the total, not to one split"},
{"file":"crates/patchback-dataset/tests/domain.rs","line":84,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"carried F3, answered by Da7-19, which drives 17 real calls so that mutants dropping WrongColumnCount or MissingDenseFile turn it red; it pins per-command name sets, not order or payloads, which other cases pin"}
]
```
