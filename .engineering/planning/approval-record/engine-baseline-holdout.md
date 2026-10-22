---
format: aep.planning-md/3
id: approval-record:engine-baseline-holdout
kind: approval-record
status: draft
title: 'Milestone A holdout: synthesized reference clips, not a dataset split'
relations:
- decides: story:engine-baseline
revision: 1
---
# Milestone A holdout: synthesized reference clips, not a dataset split

## Decision

The holdout for story:engine-baseline in Milestone A is a seeded set of synthesized reference clips
(`PatchbackSynthesized` references with their known note sequences), not a dataset split. Dataset
splits come from story:dataset-generation in Milestone B; Milestone A must not wait on Milestone B.

## Decided by

The lead coordinating the 2026-10-22 plan review fixes (finding D1 of the plan review).

## Consequence

- story:engine-baseline's body says so.
- The specification still models the holdout as a split (`spec/domains/fit.yaml` Benchmark,
  `holdout_split_id` in both engine-baseline scenarios). Remodelling it is the spec owner's work,
  recorded in the story's Open questions.
