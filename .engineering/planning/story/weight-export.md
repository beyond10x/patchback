---
format: aep.planning-md/3
id: story:weight-export
kind: story
status: draft
title: Strict weight export
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:training-run
revision: 1
---
# Strict weight export

## Outcome

From a completed training run, the adapter recognises only the documented `param @PatchRegressor.w1`
and `.w2` outputs of the pinned version, validates names, shapes, element counts and finiteness,
and writes a checksummed, versioned little-endian f32 weight artifact with the model and feature-spec
version. Unrecognised output and a run that did not complete are refused.

## DESIGN.md

§ 8.2 steps 2–3, § 12.1 (model bridge tests), ADR-006.

## Acceptance

- spec/scenarios/weight-export-exported.yaml
- spec/scenarios/weight-export-refused-output.yaml
- spec/scenarios/weight-export-run-not-completed.yaml

## Numeric obligations

- Exactly `[32, 64]` and `[64, 8]` weights, all finite (§ 8.2, § 12.1).
- The checksum verifies on read (§ 8.2).

## Open questions

- The checksum algorithm.
- Whether the artifact's feature contract is copied from the dataset or given at export (modelled
  as given, unchecked).
