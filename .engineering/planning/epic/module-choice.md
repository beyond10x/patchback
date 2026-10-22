---
format: aep.planning-md/3
id: epic:module-choice
kind: epic
status: draft
title: 'Milestone D: module choice and modulation'
relations:
- serves: vision:patchback
- depends_on: epic:phrases
- informed_by: executable-system-specification:patchback-contracts
revision: 1
---
# Milestone D: module choice and modulation

## Outcome

The system selects and drives a meaningful module combination from a bounded set of candidate
topologies, with time-varying control automation and explicit delayed feedback, instead of only
eight knobs on one fixed voice (DESIGN.md § 5.4, § 13 Milestone D).

## In scope

- Candidate topology families: Saw/Square/Sine, filter families, LFO, delay, noise (§ 5.4).
- Delayed feedback only through an explicit delay of at least one sample (§ 5.4, § 12.1).
- Control automation on stable parameter paths (§ 10).
- Topology selection by candidate rendering and ranking with a simplicity bias (§ 13 D, ADR-003, ADR-009).

## Out of scope

Unrestricted graph generation (ADR-003).
