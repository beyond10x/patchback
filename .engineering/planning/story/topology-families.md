---
format: aep.planning-md/3
id: story:topology-families
kind: story
status: draft
title: Admit module families to a topology
relations:
- decomposes: epic:module-choice
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:patch-graph-validation
revision: 1
---
# Admit module families to a topology

## Outcome

A topology admits one more module family at a time (Square/Sine, highpass/bandpass, LFO, delay,
noise), and its patches may then use that family's node kinds; a node kind outside the topology's
catalogue is refused.

## DESIGN.md

§ 5.4, § 13 Milestone D.

## Acceptance

- spec/scenarios/topology-families-admit-lfo.yaml
- spec/scenarios/topology-families-outside-catalogue.yaml

## Numeric obligations

- Each added family has forward/inverse mapping tests like § 5.2's, and finite output for bounded
  controls (§ 5.2, § 2 invariant 4).

## Open questions

- Admitting a family changes the topology in place or declares a new version.
- Parameter contracts of the added families (LFO rate, delay time, feedback gain, noise level).
- FM oscillator family is named without kinds.
