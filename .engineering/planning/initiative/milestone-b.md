---
format: aep.planning-md/3
id: initiative:milestone-b
kind: initiative
status: active
title: 'Goal: Milestone B reached'
summary: 'Timo, 2026-10-24: Goal set: milestone b reached. Reached when epic:sound-engine and epic:modelspec-regressor are implemented and released.'
owner: timo
relations:
- serves: vision:patchback
- depends_on: epic:sound-engine
- depends_on: epic:modelspec-regressor
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-24T19:45:23Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-24T19:45:23Z", actor: "human:timo", revision: 3}
---
# Goal: Milestone B reached

## Who set it, in their words

Timo, 2026-10-24, in chat to the orchestrator session for this repository, after the a5 roadmap:

> Goal set: milestone b reached

## What "reached" means

DESIGN.md § 13, Milestone B exit: "a ModelSpec-trained regressor genuinely reduces matching work
or improves audio fit." Reached when `epic:sound-engine` (Milestone A, its exit: reliable
executable patches, defensible audio comparison, deterministic test fixtures) and
`epic:modelspec-regressor` (Milestone B) are implemented and released from `main`.

## The path, as waves

| wave | stories | why in this order |
|---|---|---|
| a5 (running) | subtractive-voice-render, external-audio-provenance, feature-contract | the renderer the fit needs; `features/1` the dataset needs |
| a6 | bounded-search-fit, fit-run-artifacts | both land on `patchback-fit`; serial units or one unit |
| a7 | engine-baseline | blocked until the `Benchmark.holdout` remodel in `spec/domains/fit.yaml` is settled |
| b1 | dataset-generation, dense-data-files, normalization-stats | the data path; depend on render and `features/1` |
| b2 | training-run, weight-export, native-predictor | the model path; depend on b1 |
| b3 | ml-init-benchmark, regressor-seeded-fit | the comparison that decides the exit; depend on b2 and a6 |

Each wave gets its own `approval-record:wave-<name>` quoting this goal, its own `wave/<name>`
branch and a release from `main` after the merge (AGENTS.md § Releases). Wave composition is
re-read from `aep plan artifact waves` before each wave; this table is the plan at the time the
goal was set, not a commitment to the grouping.

## Out of the goal

`epic:phrases`, `epic:patch-studio`, `epic:module-choice` and `epic:larger-scope`. They are not
blocked by this goal and may be scheduled beside it when they share no files with a running wave.
