---
format: aep.planning-md/3
id: story:dense-data-files
kind: story
status: draft
title: ModelSpec dense-data files
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:dataset-generation
revision: 1
---
# ModelSpec dense-data files

## Outcome

Each split is written as ModelSpec dense data: `x.f32` (N × 32) and `y.f32` (N × 8), row-major
little-endian f32, with an optional checked `order.u32`. A shape mismatch and non-finite content
are refused and nothing is written.

## DESIGN.md

§ 7.2, § 12.1 (data writer tests).

## Acceptance

- spec/scenarios/dense-data-files-written.yaml
- spec/scenarios/dense-data-files-shape-mismatch.yaml
- spec/scenarios/dense-data-files-content-refused.yaml

## Numeric obligations

- Exact byte counts: `x.f32` = N·32·4, `y.f32` = N·8·4; sample counts match between files (§ 7.2).
- Bytes are row-major little-endian f32; no NaN or Infinity (§ 7.2).

## Open questions

- What the `order.u32` check is (permutation of 0..N, indices below N, or length N).
- Writing to a split already Written has no refusal (ESS gap).
