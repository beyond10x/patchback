---
format: aep.planning-md/3
id: review-result:weight-export-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: strict weight export'
relations:
- reviews: story:weight-export
revision: 1
---
# Adversary pass 2: strict weight export (story:weight-export)

Wave a9, unit 1. A second `aep:adversary` on Opus against the uncommitted change in tree
`patchback-impl-weight-export` (base b20fbf0) after correction round 1 (Da9-19, Da9-20),
2026-10-25. Known and not re-reported: Da9-2 (payload-only checksum), the pass-1 findings and
the cases that closed them.

## Result

- verdict: green; cases executed 112 → 117 (patchback-model, both lanes), red 0; origin:
  introduced 3, pre-existing 0.
- Added `crates/patchback-model/tests/adversary_weight_export_2.rs` (5 tests, about 70 rows);
  changed nothing else. The implementor's tracked diff is byte-identical to the pass-1
  snapshot; every hunk of `weights.rs` and the two changed rows of the pass-1 file since the
  snapshot are the ones correction report 1 names.
- F1 (note, CONFIRMED, mutant): after correction 1 no case put a malformed name on an `init`
  line (the pass-1 row flipped to Exported), so skipping the form check on `init`
  (`weights.rs:318`) would survive. Case :389 closes it.
- F2 (note, CONFIRMED, mutant): no case differed from `W1_NAME`/`W2_NAME` only in case, so a
  case-insensitive comparison (`weights.rs:434`) would survive. Rows at :267 and :354 close it.
- F3 (note, CONFIRMED, mutant): no case used a non-ASCII letter, so a Unicode
  `is_alphanumeric` identifier test (`weights.rs:312`) would survive. The `@Ünicode` rows close
  it.
- Held: 15 malformed names are grammar errors at their line and 10 well-formed names are
  `UnrecognizedParameter` with the full name, on the first and the second `param` line; `init`
  names are checked for form only; the Da9-7 order with a name defect beside a shape, count or
  NaN defect; a grammar error later in the stdout wins over an earlier name error; the bias
  step of `weight-export-refused-output.yaml` still answers `UnrecognizedParameter`; the three
  mutants of the brief are each caught by an existing case; the pinned lane passes the real
  stdout whole under the new grammar; a refusal writes nothing. No docs correction needed.
- A stray `git diff` copy was written one directory above the tree for about a minute and
  deleted by the adversary.

## Routed

- F1 to F3 → the pass-2 file stays in the unit (Da9-20 extended). No correction round 2.

```findings
- file: crates/patchback-model/src/weights.rs
  line: 318
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: after correction 1 no case put a malformed name on an `init` line, so skipping the form check on `init` survived the suite; adversary_weight_export_2.rs:389 closes it
- file: crates/patchback-model/src/weights.rs
  line: 434
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: no case used a name differing from W1_NAME/W2_NAME only in case, so a case-insensitive comparison survived; rows at adversary_weight_export_2.rs:267 and :354 close it
- file: crates/patchback-model/src/weights.rs
  line: 312
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: no case used a non-ASCII letter in a name, so a Unicode `is_alphanumeric` identifier test survived; the `@Ünicode` rows in adversary_weight_export_2.rs close it
```
