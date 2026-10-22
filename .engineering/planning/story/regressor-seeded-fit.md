---
format: aep.planning-md/3
id: story:regressor-seeded-fit
kind: story
status: draft
title: Regressor-seeded fit
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:native-predictor
- depends_on: story:bounded-search-fit
revision: 1
---
# Regressor-seeded fit

## Outcome

A fit can start from the native predictor's patch instead of the default: it needs an analysis of
the reference and a weight artifact under the same feature contract, and refuses a fit missing the
analysis or with a mismatched contract.

## DESIGN.md

§ 9.2 step 1, § 13 Milestone B.

## Acceptance

- spec/scenarios/regressor-seeded-fit-seeded.yaml
- spec/scenarios/regressor-seeded-fit-feature-contract-mismatch.yaml
- spec/scenarios/regressor-seeded-fit-analysis-required.yaml

## Numeric obligations

- The seeded start is the predictor's output, finite and clamped to [0, 1] (§ 8.2, § 9.2).
- The bounded-search obligations hold unchanged from the seeded start (§ 9.2).

## Open questions

- Whether an analysis is made under a feature contract; until then the mismatch is an external
  refusal.
- The weight artifact's existence and state are not guarded.
