---
format: aep.planning-md/3
id: review-result:benchmark-holdout-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: Benchmark holdout remodel (wave a6)'
summary: 'green: 398→401 scenarios, 0 red, 5 findings (3 scenarios adopted, F3 to correction round 1, F4 F5 to the owner)'
relations:
- reviews: story:engine-baseline
revision: 1
---
# Adversary pass 1: Benchmark holdout remodel (approval-record:engine-baseline-holdout)

Wave a6, unit 3, specification only. `aep:adversary` on Opus against the author's uncommitted
change in tree `patchback-spec-benchmark-holdout` (base 76bb7ec), 2026-10-24.

## Result

- verdict: green; scenarios executed 398 → 401, red 0; origin: introduced 3, pre-existing 2.
- `task spec-validate` valid (128 scenarios), `task spec-conform` 401 passed, 0 failed, 0 error,
  0 unsupported; the 92 synthesize refusals unchanged from the base; `unknown-topology` gets a
  synthesized scenario that passes.
- Added three scenarios (`adversary-benchmark-{refusal-precedence,topology-recorded,view-topology}.yaml`,
  registered in `spec/ess-inputs.yaml`) that pin `topology_id` in `BenchmarkStarted`,
  `WeightArtifactRequired`, the stored `Benchmark` and the `Benchmarks` view, and the order
  `invalid-holdout-size` before `weight-artifact-required`. Four mutants of `fit.yaml` that pass
  the author's 398 are refused with them.
- Held under attack: ESS refuses a declared order that disagrees with evaluation; a different
  declared topology does not satisfy the existence check; no field or relation names a dataset
  split; `patchback.dataset` untouched; `--strict-requires` valid; payloads changed in the field
  name only; holdout sizes -1, 0 and 1 give the declared outcomes.
- Generated pages (`website/docs/reference/ess/patchback-fit.md`, `website/data/ess/*.json`) still
  carry `holdout_split_id`; the coordinator regenerates them at integration.

## Routed

- F1, F2: closed by adopting the three scenarios (Da6-16, renamed to `engine-baseline-*.yaml`).
- F3: correction round 1 moves the `unknown-topology` step into
  `engine-baseline-unknown-topology.yaml` (Da6-17).
- F4, F5: the owner's, under OQ-28; recorded, not changed.

Report: wave a6 scratch `benchmark-holdout/adversary-report-1.md`. Agent cost 112,802 tokens,
46 tool uses, 484 s.

```findings
[
{"file":"spec/scenarios/engine-baseline-target-met.yaml","line":34,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"no authored scenario pins topology_id in BenchmarkStarted, WeightArtifactRequired or the Benchmark record, so three {generated: true} mutants pass 398/398"},
{"file":"spec/domains/fit.yaml","line":1404,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"swapping invalid-holdout-size and weight-artifact-required passes every authored scenario"},
{"file":"spec/scenarios/ml-init-benchmark-weight-artifact-required.yaml","line":37,"category":"judgement","severity":"note","verdict":"NEEDS-CHANGE","origin":"introduced","message":"story:engine-baseline's unknown-topology acceptance step sits in story:ml-init-benchmark's weight-artifact scenario file"},
{"file":"spec/domains/fit.yaml","line":400,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the holdout triple records no generator algorithm, note distribution or sample rate that DESIGN.md 267 and 452 ask to fix"},
{"file":"spec/domains/fit.yaml","line":1419,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a zero-parameter topology is accepted as a holdout topology, making every reference the default patch"}
]
```
