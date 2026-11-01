---
format: aep.planning-md/3
id: story:regressor-earns-the-exit
kind: story
status: draft
title: The regressor earns the Milestone B exit
relations:
- serves: vision:patchback
- informed_by: verification-report:milestone-b-exit
- depends_on: story:ml-init-benchmark
revision: 1
---
# The regressor earns the Milestone B exit

## Outcome

`patchback benchmark --kind ml-init` over a seeded holdout shows the regressor-seeded search
needing fewer evaluations than the search from the default patch, or ending at a lower loss, by a
margin larger than the clip-to-clip spread, on the same command that recorded
`verification-report:milestone-b-exit` (24 clips, seed 7, the default fit configuration).

## Why

DESIGN.md § 13 Milestone B's exit is "a ModelSpec-trained regressor genuinely reduces matching
work or improves audio fit". The 2026-11-01 run (`verification-report:milestone-b-exit`) measured
a median of 366 evaluations against 381 and a median loss of 0.060 against 0.068, with the
regressor's start ending lower on 11 of 24 clips. That is not a reduction anybody can tell from
noise.

## DESIGN.md

§ 7 (dataset), § 8 (ModelSpec M0), § 9.2 (search), § 13 Milestone B exit.

## Acceptance

- spec/scenarios/ml-init-benchmark-four-arms-reported.yaml

## Numeric obligations

- The margin and the statistic (a paired per-clip comparison, a median difference with its
  spread) are stated when the story is proposed, before any change is measured.
- The baseline is the 2026-11-01 run's `clips.txt`, recorded in
  `verification-report:milestone-b-exit`.

## Candidate directions (none chosen)

- The search spends the prediction's head start in its first restart: start the search with a
  smaller step around the predicted point, or use the prediction's per-parameter error on the
  validation split as the initial step.
- 5 of 24 predictions clamped: the dataset's parameter distribution against the holdout's.
- The demo model is 50,016 training rows and 3,126 steps (`models/demo/PROVENANCE`); a larger
  dataset or model.

## Open questions

- Whether the exit is judged on evaluations, on final loss, or on either.
