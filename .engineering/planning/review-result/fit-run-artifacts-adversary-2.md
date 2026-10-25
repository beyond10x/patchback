---
format: aep.planning-md/3
id: review-result:fit-run-artifacts-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: fit run artifacts'
relations:
- reviews: story:fit-run-artifacts
revision: 1
---
# Adversary pass 2: fit run artifacts (story:fit-run-artifacts)

Wave a7, unit 1, the budget pass. `aep:adversary` on Opus against the corrected uncommitted
change in tree `patchback-impl-fit-run-artifacts` (base 4b99e5b), 2026-10-25.

## Result

- verdict: red; cases executed 273 → 286 (patchback-fit 81 → 92, patchback-cli 192 → 194),
  red 2; origin: introduced 2, pre-existing 0.
- Added `crates/patchback-fit/tests/results_adversary2.rs` (11 cases) and
  `crates/patchback-cli/tests/fit_artifacts_adversary2.rs` (2 cases); changed nothing else.
- The correction answered F1 to F4 of pass 1 without loss: the limiter case matches Da7-15,
  the pass 1 cases are as written and green, nothing weakened was found.
- G1 (note, INFEASIBLE): `parse_trace` accepts a `first-retained` line whose best is `-`
  (`results_adversary2.rs:398`); the grammar says `-` only while no candidate has been
  retained.
- G2 (note, INFEASIBLE): `parse_trace` accepts a numeric best on `rejected` lines before any
  candidate was retained (`results_adversary2.rs:418`).
- G3 (note, mutant): a rise tolerance of 1e-12 in `trace.rs:392` survives all 273 prior cases;
  only the new one-ulp case `results_adversary2.rs:104` kills it.
- G4 (note): the stdout `limited` row of `crates/patchback-cli/src/fit.rs:63` does not say it
  describes the uncut render; `limited true` can stand beside a `reconstructed.wav` with no
  limited sample (`fit_artifacts_adversary2.rs:127`, green).
- G5 (note, judgement): public fields of `TraceRecord` and `TraceError` carry no doc comment
  (`trace.rs:114-118`, `:193-194`).
- G6 to G9 (notes, INFEASIBLE, characterised green): a restart number that skips then falls;
  `first-retained` after the first numeric best; `improved` with a best other than its total;
  `not-improved` and `rejected` lines whose best fell. No run writes them.
- Carried F4: the field-order case stays; nothing to add.
- Mutants on a copy: warn with the cut clip's count and swap `n`/`m` are each killed by four
  cases; skip `BestLossLost` by `results.rs:614`; the 1e-12 tolerance by G3's case only.
- Docs: a real 20-render run's `report.txt`, `run.meta`, `notes.events`, `trace.events` and
  stdout match `files-doc.md` and `cli-doc.md` field for field; the examples say
  `patchback 0.6.0` where the binary writes `0.5.0` (the next release's number).

## Routed

- G1, G2 → Da7-20: both refused by `parse_trace` in correction round 2; the cases stay as
  written.
- G3 → the one-ulp case stays; nothing to add.
- G4 → correction round 2: the row says "uncut render".
- G5 → correction round 2: one doc line each.
- G6 to G9 → the owner, with the story's trace grammar (OQ-29 in DESIGN.md § 17 if the owner
  files it); the characterisation cases stay green.

Report: wave a7 scratch `fit-run-artifacts/adversary-report-2.md`. Agent cost 199,497 tokens,
70 tool uses, 1071 s.

```findings
[
{"file":"crates/patchback-fit/tests/results_adversary2.rs","line":398,"category":"contract-drift","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"parse_trace accepts a first-retained line whose best is `-`, against trace.rs's grammar (`-` while no candidate has been retained); no run writes it"},
{"file":"crates/patchback-fit/tests/results_adversary2.rs","line":418,"category":"contract-drift","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"parse_trace accepts a numeric best on rejected lines before any candidate was retained, a best loss no candidate scored; no run writes it"},
{"file":"crates/patchback-fit/src/trace.rs","line":392,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a rise tolerance of 1e-12 survived all 273 prior cases and is killed only by the new one-ulp case results_adversary2.rs:104"},
{"file":"crates/patchback-cli/src/fit.rs","line":63,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the stdout limited row omits that it describes the uncut render, so limited true can stand beside a reconstructed.wav with no limited sample; cli-doc.md states the gap, the module doc does not"},
{"file":"crates/patchback-fit/src/trace.rs","line":114,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"public fields of TraceRecord (sequence, evaluations, restart, outcome, topology) and TraceError (line, reason) have no doc comment"},
{"file":"crates/patchback-fit/tests/results_adversary2.rs","line":428,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"characterised, green: a restart number that skips and then falls is accepted"},
{"file":"crates/patchback-fit/tests/results_adversary2.rs","line":439,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"characterised, green: first-retained on a line after the first numeric best is accepted"},
{"file":"crates/patchback-fit/tests/results_adversary2.rs","line":449,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"characterised, green: improved with a best other than its total is accepted"},
{"file":"crates/patchback-fit/tests/results_adversary2.rs","line":460,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"characterised, green: not-improved and rejected lines whose best changed (fell) are accepted"},
{"file":"crates/patchback-fit/tests/results.rs","line":1,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"carried F4: the correction kept the field-order case results_adversary.rs:154 unchanged and green and added nothing, which answers it"}
]
```
