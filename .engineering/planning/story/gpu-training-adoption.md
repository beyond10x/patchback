---
format: aep.planning-md/3
id: story:gpu-training-adoption
kind: story
status: draft
title: GPU training adoption gate
relations:
- decomposes: epic:larger-scope
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:training-run
revision: 1
---
# GPU training adoption gate

## Outcome

Training on ModelSpec's GPU backend is allowed only once the backend is documented as shipped
upstream and CPU/GPU equivalence tests pass; until then a GPU run is refused as not shipped or
awaiting equivalence.

## DESIGN.md

§ 1.1, § 3, § 13 Milestone E, ADR-010.

## Acceptance

- spec/scenarios/gpu-training-adoption-not-shipped.yaml
- spec/scenarios/gpu-training-adoption-awaiting-equivalence.yaml
- spec/scenarios/gpu-training-adoption-allowed.yaml

## Numeric obligations

- CPU/GPU equivalence within a tolerance ADR-010 does not yet state; set before adoption.

## Open questions

- The equivalence tolerance.
