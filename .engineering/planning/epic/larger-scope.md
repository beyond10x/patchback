---
format: aep.planning-md/3
id: epic:larger-scope
kind: epic
status: draft
title: 'Milestone E: larger musical scope'
relations:
- serves: vision:patchback
- depends_on: epic:module-choice
- depends_on: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
revision: 2
---
# Milestone E: larger musical scope

## Outcome

Several instrument stems are reconstructed as patches plus events, and supported training moves to
the ModelSpec GPU backend once upstream ships and verifies it (DESIGN.md § 13 Milestone E, ADR-010).

## In scope

- Multiple monophonic stems, each with its own patch and event sequence (§ 10 V3).
- GPU training adoption gated on the pinned ModelSpec release and CPU/GPU equivalence (§ 1.1, ADR-010).

## Out of scope, recorded for later specification

- Drum and effect modules, repeated-pattern analysis, arrangement events (§ 13 E): no section of
  DESIGN.md defines their contracts yet.
- An optional sequence-generation model, which DESIGN.md § 13 E says is a separate specification.
- Full-mix source separation (§ 13 E exit).
- Polyphonic stems and simple polyphony (DESIGN.md § 10 V2, § 13 E): no contract yet.
