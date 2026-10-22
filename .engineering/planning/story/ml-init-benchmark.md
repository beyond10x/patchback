---
format: aep.planning-md/3
id: story:ml-init-benchmark
kind: story
status: draft
title: Default vs ML-init benchmark
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:regressor-seeded-fit
- depends_on: story:engine-baseline
revision: 1
---
# Default vs ML-init benchmark

## Outcome

Over one holdout set, the benchmark reports default, default-plus-search, ML-init and
ML-init-plus-search medians side by side, so the Milestone B exit (does the regressor reduce
matching work or improve fit) can be read off. A benchmark of the wrong kind or without a weight
artifact is refused.

## DESIGN.md

§ 9.2 (benchmark against default and raw prediction), § 12.2, § 13 Milestone B.

## Acceptance

- spec/scenarios/ml-init-benchmark-four-arms-reported.yaml
- spec/scenarios/ml-init-benchmark-kind-mismatch.yaml
- spec/scenarios/ml-init-benchmark-weight-artifact-required.yaml

## Numeric obligations

- Median normalized loss and render evaluations per arm, on one holdout set and one fixed
  configuration (§ 12.2).
- The raw-model-only result is reported (§ 12.2).

## Open questions

- § 13 B sets no numeric target for the ML-init verdict; none is recorded.
