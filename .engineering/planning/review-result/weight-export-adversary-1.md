---
format: aep.planning-md/3
id: review-result:weight-export-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: strict weight export'
relations:
- reviews: story:weight-export
revision: 1
---
# Adversary pass 1: strict weight export (story:weight-export)

Wave a9, unit 1. `aep:adversary` on Opus against the uncommitted change in tree
`patchback-impl-weight-export` (base b20fbf0), 2026-10-25. Known and not re-reported: the
checksum covers the payload alone (Da9-2).

## Result

- verdict: green; cases executed 98 → 110 (patchback-model, both lanes), red 0; origin:
  introduced 4, pre-existing 0.
- Added `crates/patchback-model/tests/adversary_weight_export.rs` (12 cases); changed nothing
  else. Both lanes exit 0 with the cases in place; the pinned lane ran the binary (no `SKIPPED`),
  SHA-256 `171d2880…4229` as pinned.
- F1 (note, CONFIRMED, mutant): the export's header bound (`weights.rs:670`) is untested at
  exactly 1,024 bytes; a `>=` mutant survives the unit's suite. Case :803 writes a 1,024-byte
  header and reads it back, and refuses 1,025 with nothing written.
- F2 (note, CONFIRMED, mutant): the reader's `+ 1` read limit (`weights.rs:692`) is untested;
  without it an 11,265-byte file reads as the valid 11,264-byte artifact. Case :825 closes it.
- F3 (note, CONFIRMED, mutant): removing the reader's `.take(limit)` (`weights.rs:694`)
  survives the suite. Case :850 reads `/dev/zero` and gets `Corrupt` within 60 s.
- F4 (note, INFEASIBLE, judgement): `param @Other.w1` is `UnrecognizedTrainingOutput`
  (implementor decision 2 under Da9-5's fixed prefix), where `model.yaml:937`'s
  `unrecognized-parameter` outcome reads "a parameter block names something other than
  `@PatchRegressor.w1` or `@PatchRegressor.w2`". No pinned run reaches it: ModelSpec prints the
  registered model's name.
- Held: the Da9-7 order with two defects at once (transposed w1 with NaN w2; bias with short
  w2; a Running run with garbage; a Completed run with `output_error`; NaN with an unwritable
  contract); nothing written on any refusal (parent listing compared); CRLF refused at line 1;
  `3.5e38` is `NonFiniteWeight`, `+1.0`, `1.`, `.5`, `-0`, `1e-45`, `3.4028235e38` keep their
  bits; 23 grammar lines and 30 tokens each answer as the brief predicted; 16 reader edits are
  `Corrupt`, a directory and a mode-0o000 file `Io`; two exports racing for one path: one
  writes, one `AlreadyExists`; the mutants (a)–(e) of the brief are each caught by an existing
  test; dropping a `DOMAIN_ERRORS` variant is caught; the `domain.rs` restructure (58− lines)
  weakened no case (counts 14/10 → 20/16); a real run without `--eval` exports, and two exports
  are byte-identical.
- Noted, not a change: each of the 4 `weight_export_scenarios` tests runs its own 11.3 s
  generation.

## Routed

- F1 to F3 → the adversary's file stays in the unit (Da9-20).
- F4 → Da9-19: the name of an `init` or `param` line is `@<ident>.<ident>` in the grammar, and
  the names step compares the two `param` names in full, so `param @Other.w1` is
  `UnrecognizedParameter { w1_name: "@Other.w1", … }` as the spec words it; an `init` name is
  not checked (Da9-6). Supersedes the fixed prefix in Da9-5's grammar line and the
  implementor's decision 2.

```findings
- file: crates/patchback-model/src/weights.rs
  line: 670
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the export's header bound is untested at exactly 1,024 bytes, so a `>=` mutant survives the unit's suite; adversary_weight_export.rs:803 closes it
- file: crates/patchback-model/src/weights.rs
  line: 692
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the reader's `+ 1` read limit is untested, and without it a full-header artifact with one appended byte reads as valid; adversary_weight_export.rs:825 closes it
- file: crates/patchback-model/src/weights.rs
  line: 694
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: removing the reader's `.take(limit)` bound survives the unit's suite; adversary_weight_export.rs:850 closes it with a /dev/zero read
- file: crates/patchback-model/src/weights.rs
  line: 313
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: a `param @Other.w1` block is UnrecognizedTrainingOutput while model.yaml:937's unrecognized-parameter outcome words it as UnrecognizedParameter; no pinned run reaches it
```
