---
format: aep.planning-md/3
id: story:fit-run-artifacts
kind: story
status: implemented
title: Fit run artifacts
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:bounded-search-fit
- depends_on: story:patch-file-format
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: inferred
  path: crates/patchback-fit
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-24T23:09:44Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-24T23:09:44Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-25T00:49:21Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
# Fit run artifacts

## Outcome

A completed fit writes `results/` with `patch.patch`, `notes.events`, `reconstructed.wav`,
`report.txt` and `run.meta`. A failed run and a run with no valid candidate write no patch; a run
still running cannot write results.

## DESIGN.md

§ 9.2 step 5, § 11, § 15.

## Acceptance

- spec/scenarios/fit-run-artifacts-completed-writes-results.yaml
- spec/scenarios/fit-run-artifacts-failed-run-writes-no-patch.yaml
- spec/scenarios/fit-run-artifacts-no-valid-candidate.yaml
- spec/scenarios/fit-run-artifacts-running-run-refused.yaml

## Numeric obligations

- `report.txt` carries loss terms, versions, timings, warnings and confidence; `run.meta` carries
  seed, input digests, ModelSpec version and feature spec (§ 11).
- The written `patch.patch` parses back to the retained patch; `reconstructed.wav` is the best
  render (§ 11).

## Scope

- `crates/patchback-fit` (inferred: § 4.1 names no results-writer crate; the writer serializes the
  fit's retained candidate, so it sits with the fit rather than in `patchback-cli`).

## Open questions

- May a run write results more than once.
- Does a failed run write a diagnostic `report.txt` without a patch.
- The full set of failure reasons.
- Nothing checks that the named WAV is the best render's.
