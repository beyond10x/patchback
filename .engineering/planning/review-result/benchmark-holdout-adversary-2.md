---
format: aep.planning-md/3
id: review-result:benchmark-holdout-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: Benchmark holdout remodel (wave a6)'
summary: 'green: 402→403 scenarios, 0 red; correction 1 verified without loss; 1 pre-existing payload-kind gap closed by an adopted scenario; F4 F5 carried to the owner'
relations:
- reviews: story:engine-baseline
revision: 1
---
# Adversary pass 2: Benchmark holdout remodel (approval-record:engine-baseline-holdout)

Wave a6, unit 3, specification only. `aep:adversary` on Opus against the corrected uncommitted
change in tree `patchback-spec-benchmark-holdout` (base 76bb7ec), 2026-10-24.

## Result

- verdict: green; scenarios executed 402 → 403, red 0 on the tree (1 red against a scratch
  mutant); origin: introduced 2 (F4, F5 carried), pre-existing 1.
- Correction round 1 verified without loss: each renamed scenario differs from its pass 1 copy
  in the `scenario:` id only; pass 1's four mutants are still refused (ESS-AUTHOR-041, 042);
  `engine-baseline-unknown-topology.yaml` is pass 1's third step unchanged;
  `ml-init-benchmark-weight-artifact-required.yaml` differs from the base in the two
  `holdout_split_id` → `topology_id` lines only.
- All 130 scenario ids equal their file names and each file is registered once in
  `spec/ess-inputs.yaml`; `task acceptance-check` exit 0.
- New: the `unknown-topology` refusal through kind `MlInitBenchmark` was covered by no scenario
  (the synthesized one sends `EngineBaseline`); `engine-baseline-unknown-topology-ml-init.yaml`
  added and adopted (Da6-21). It fails a mutant that hard-codes the `BenchmarkStarted` payload's
  kind, which the 402 scenarios (and the base's 397) passed.
- `task spec-validate` 130 scenarios valid; `task spec-conform` 403 passed, 0 failed; the 92
  synthesize refusals unchanged.

## Routed

- The pre-existing payload-kind gap: closed by adopting the scenario; no model change.
- F4, F5: carried; the owner's under OQ-28.
- Noted for the coordinator: `engine-baseline-target-met-at-boundary.yaml` was named by no
  story; `story:engine-baseline`'s acceptance list now names it with the five new files.

Report: wave a6 scratch `benchmark-holdout/adversary-report-2.md`. Agent cost 93,663 tokens,
48 tool uses, 533 s.

```findings
[
{"file":"spec/domains/fit.yaml","line":1439,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"hard-coding the BenchmarkStarted payload to kind EngineBaseline passes 402/402 (and 397/397 at base); engine-baseline-unknown-topology-ml-init.yaml fails it"},
{"file":"spec/domains/fit.yaml","line":400,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the holdout triple records no generator algorithm, note distribution or sample rate that DESIGN.md 267 and 452 ask to fix"},
{"file":"spec/domains/fit.yaml","line":1419,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a zero-parameter topology is accepted as a holdout topology, making every reference the default patch"}
]
```
