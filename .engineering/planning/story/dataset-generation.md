---
format: aep.planning-md/3
id: story:dataset-generation
kind: story
status: draft
title: Seeded paired dataset generation
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:subtractive-voice-render
- depends_on: story:feature-contract
revision: 1
---
# Seeded paired dataset generation

## Outcome

`patchback generate-dataset --seed --samples --out` samples valid normalized patches and one-note
programs with a std-only PRNG, renders them through the same engine, extracts `features/1` and
records `(x: [32], y: [8])` pairs in train, validation and test splits isolated by patch identity.
A split count that is not a multiple of the batch size is refused. Silence, noise and clipping
cases go into a separate diagnostic set.

## DESIGN.md

§ 2 invariants 1 and 5, § 7.1, § 11, § 12.1 (isolated splits), § 12.2 (dataset gate).

## Acceptance

- spec/scenarios/dataset-generation-seeded-splits.yaml
- spec/scenarios/dataset-generation-count-not-batch-multiple.yaml
- spec/scenarios/dataset-generation-patch-identity.yaml
- spec/scenarios/dataset-generation-diagnostic-set.yaml

## Numeric obligations

- Same seed and build give byte-identical datasets (§ 2 invariant 5, § 7.1).
- First dataset 50,016 / 4,992 / 4,992, each a multiple of batch size 16 (§ 7.1).
- Log-time and log-frequency parameters are stratified; no patch identity spans two splits (§ 7.1).
- No invalid sample enters a split (§ 12.2).

## Open questions

- PRNG algorithm and its recorded name.
- The declared training distribution (pitch and gate ranges).
- How `--samples` maps to the three split counts.
- One split per role, or several.
- Owned versus referenced children of a dataset.
- Whether the diagnostic set is written as dense data.
