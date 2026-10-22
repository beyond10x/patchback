---
format: aep.planning-md/3
id: story:normalization-stats
kind: story
status: draft
title: Normalization statistics from the training split
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:feature-contract
- depends_on: story:dataset-generation
revision: 1
---
# Normalization statistics from the training split

## Outcome

Means and standard deviations of `features/1` are computed from the training split only, saved
as an internal artifact under the feature contract, and applied unchanged at inference. Statistics
from a validation or test split, or under another contract, are refused.

## DESIGN.md

§ 6.3, § 7.1.

## Acceptance

- spec/scenarios/normalization-stats-train-only.yaml
- spec/scenarios/normalization-stats-contract-gates.yaml
- spec/scenarios/normalization-stats-apply.yaml

## Numeric obligations

- Means and standard deviations match a reference computation over the training rows (§ 6.3).
- Applying the stats is `(x − mean) / std` per element, the same at training and inference (§ 6.3).

## Open questions

- Validation split refused as well as test (modelled; § 6.3 names only the test set).
- Which record holds the link to the stats: dataset, train split or the stats.
- `SplitRole` duplicated between analysis and dataset; `split_role` input vs `DatasetSplit.role`.
