---
format: aep.planning-md/3
id: story:topology-selection
kind: story
status: draft
title: Topology selection by candidate ranking
relations:
- decomposes: epic:module-choice
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:topology-families
- depends_on: story:bounded-search-fit
revision: 1
---
# Topology selection by candidate ranking

## Outcome

A fit renders a bounded set of candidate topologies, ranks them by loss plus a complexity penalty,
keeps a small ranked candidate set and selects the best, with the ranking visible in the report.
Selection without candidates or of a non-candidate is refused.

## DESIGN.md

§ 9.3, § 13 Milestone D, ADR-003, ADR-009.

## Acceptance

- spec/scenarios/topology-selection-ranked-with-simplicity-bias.yaml
- spec/scenarios/topology-selection-refusals.yaml

## Numeric obligations

- Score = loss + complexity penalty over nodes, edges and control routes; ranking is monotone in
  score (§ 9.3).
- Every candidate's loss is finite (§ 2 invariant 4).

## Open questions

- The penalty formula and weight.
- The candidate count bound.
- Tie-breaking between equal scores.
