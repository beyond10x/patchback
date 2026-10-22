---
format: aep.planning-md/3
id: story:native-predictor
kind: story
status: draft
title: Native matmul/relu predictor
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:weight-export
- depends_on: story:normalization-stats
revision: 1
---
# Native matmul/relu predictor

## Outcome

A small native Rust function reads a validated weight artifact and predicts the eight normalized
parameters from a normalized `features/1` vector (two dense layers and ReLU), clamping outputs to
[0, 1] with a warning. Input that does not fit the artifact's feature contract is refused.

## DESIGN.md

§ 8.1 (clamp with warning), § 8.2 step 4, § 12.1, ADR-006.

## Acceptance

- spec/scenarios/native-predictor-predicted.yaml
- spec/scenarios/native-predictor-refused-input.yaml

## Numeric obligations

- Golden tests against independently computed reference matmul results (§ 8.2).
- Deterministic output for the same weights and input (§ 12.1).
- Outputs outside [0, 1] are clamped and counted (§ 8.1).

## Open questions

- Whether the artifact's feature contract is checked against the input in Rust only.
