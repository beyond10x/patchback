---
format: aep.planning-md/3
id: review-result:dataset-generation-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: dataset generation'
relations:
- reviews: story:dataset-generation
revision: 1
---
# Adversary pass 2: Dataset generation (story:dataset-generation)

Wave a6, unit 2. `aep:adversary` on Opus against the corrected uncommitted change in tree
`patchback-impl-dataset-generation` (base 76bb7ec), 2026-10-25.

## Result

- verdict: green; cases executed 202 → 207, red 0; origin: introduced 3, pre-existing 1
  (F5 carried). Clippy `-D warnings` and `cargo fmt --all --check` exit 0.
- Correction round 1 verified without loss on F1 and F4: no pass 1 assertion lost; all five
  pass 1 mutants (velocity 0.5, start 1, reversed rows, MIDI first, whole-render span) still
  caught; the only test removed from the first implementor's suite is the 50,000 case Da6-18
  ordered deleted; no default changed.
- Da6-26: `--samples 9223372036854775807` is refused naming the rule's train count
  7688602929922141119; one above exits 2 with clap's range message; no overflow of the count
  rule between `MAX_SAMPLES / 2` and `MAX_SAMPLES` over 16,394 values checked against a
  differently written formula, including the first value where `samples · 4992` passes
  `i64::MAX`.
- Da6-27: no string in either crate or the scratch docs names `validation/` as a path; the
  stdout key order is unchanged and now pinned as a literal.
- A1 (low): the `--samples` ceiling was not pinned at its edge (a limit raised by 16 passed the
  suite and admitted 9223372036854775808, an endless generation); now caught by
  `adversary_one_past_max_samples_is_a_usage_error`.
- A2 (judgement, NEEDS-CHANGE, low): `--samples` up to `i64::MAX` admits batch multiples such
  as 9223372036854775792 whose generation holds every sample in memory and does not end in
  practice (20 s, 4.6 → 7.4 MB, nothing written); DESIGN.md § 12.3 bounds memory and § 7.2's
  `order.u32` indexes at most 2^32 rows.
- A3 (doc, low): the `main.rs` exit-2 row said a `--samples` "that is not an integer"; one
  above 9223372036854775807 is an integer and also exits 2.
- `SplitCounts::from_samples` saturates above `MAX_SAMPLES` only on a direct library call; its
  one caller is behind the clap range (note).

## Routed

- A1: closed by the adversary's case; no code change.
- A2 → Da6-31: the ceiling stays the integer type's limit in a6; the row limit of `order.u32`
  and the memory bound belong to story:dense-data-files, which writes the rows.
- A3: fixed at integration in `crates/patchback-cli/src/main.rs` (the exit-2 row names a
  `--samples` above 9223372036854775807).
- F5: carried (Da6-28).

Report: wave a6 scratch `dataset-generation/adversary-report-2.md`. Agent cost 139,057 tokens,
52 tool uses, 1797 s.

```findings
[
{"file":"crates/patchback-cli/src/generate_dataset.rs","line":67,"category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the --samples ceiling was not pinned at its edge; range(..=MAX_SAMPLES + 16) passed the unit's suite and admitted 9223372036854775808, which starts an endless generation; now caught by adversary_one_past_max_samples_is_a_usage_error"},
{"file":"crates/patchback-dataset/src/generate.rs","line":66,"category":"bound","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"judgement; --samples up to i64::MAX admits batch multiples such as 9223372036854775792 whose generation holds every sample in memory and never ends in practice (still running at 20 s, nothing written; max_draws saturates); DESIGN 12.3 bounds memory and 7.2 order.u32 indexes at most 2^32 rows; coordinator to choose a cap or record the type limit"},
{"file":"scratch/main-doc.patch","line":25,"category":"doc","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"main.rs exit-2 row says a --samples that is not an integer; a --samples above 9223372036854775807 is an integer and also exits 2"},
{"file":"crates/patchback-analysis/src/pitch.rs","line":251,"category":"measurement","severity":"note","verdict":"INFEASIBLE","origin":"pre-existing","message":"PitchAmbiguous 0.500 is the estimator's undecided-subharmonic branch, resolved by opening the cutoff, removing drive or flattening the envelope; 16% of seed-42 draws become Noise diagnostics this way"}
]
```
