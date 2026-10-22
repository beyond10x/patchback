---
format: aep.planning-md/3
id: story:delayed-feedback
kind: story
status: draft
title: Explicit delayed feedback
relations:
- decomposes: epic:module-choice
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:topology-families
revision: 1
---
# Explicit delayed feedback

## Outcome

A patch with a cycle validates only when the cycle passes through a stateful Delay node of at least
one sample; feedback gain is limited, runaway energy is monitored, and an unstable feedback path
fails closed. Any other cycle is refused.

## DESIGN.md

§ 5.4, § 12.1, § 12.3, § 13 Milestone D.

## Acceptance

- spec/scenarios/delayed-feedback-accepted.yaml
- spec/scenarios/delayed-feedback-refused.yaml

## Numeric obligations

- Delay latency ≥ 1 sample on every cycle; no instantaneous cycle is ever evaluated (§ 5.4).
- Loop gain bounded; runaway energy triggers a safe bypass or mute, with finite output (§ 5.4,
  § 12.3).

## Open questions

- The feedback gain limit and which parameter carries it.
