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
scope:
- confidence: cited
  path: crates/patchback-analysis
- confidence: inferred
  path: crates/patchback-analysis/src/error.rs
- confidence: inferred
  path: crates/patchback-analysis/src/features.rs
- confidence: inferred
  path: crates/patchback-analysis/src/lib.rs
- confidence: inferred
  path: crates/patchback-analysis/src/normalization.rs
- confidence: inferred
  path: crates/patchback-analysis/tests/domain.rs
- confidence: inferred
  path: crates/patchback-dataset
revision: 4
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

## Scope

Derived 2026-10-24 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-analysis` — cited, all three acceptance scenarios are `domain: patchback.analysis` and the commands live in `spec/domains/analysis.yaml:842` (`ComputeNormalizationStats`) and `:904` (`ApplyNormalization`); DESIGN.md:127 maps features to this crate
- **Files:** new `crates/patchback-analysis/src/normalization.rs` (stats computation over training rows, `(x − mean) / std` apply), `crates/patchback-analysis/src/lib.rs` (module and re-exports), `crates/patchback-analysis/src/error.rs` (new variants plus `DOMAIN_ERRORS` at line 53), `crates/patchback-analysis/tests/domain.rs` (error/command section checks) — inferred, the pattern of the existing modules and of story:feature-contract
- **Symbols:** `NormalizationStats` (`analysis.yaml:203`), `ComputeNormalizationStats`, `ApplyNormalization`, `NormalizationStatsSet` (`:1102`); errors `StatsFromNonTrainingSplit` (`:441`), `DatasetSplitNotFound` (`:450`), `NormalizationStatsNotFound` (`:456`), `NormalizationContractMismatch` (`:462`), and the shared `FeatureContractNotFound` (`:405`), `FeatureContractNotPublished` (`:411`), `FeatureVersionMismatch` (`:429`) — cited
- **Also likely:** `crates/patchback-analysis/src/features.rs` (the `FeatureContract` type, version and published state this story gates on; created by the feature-contract unit, absent on this branch) — inferred, from `depends_on story:feature-contract` and the contract-gates scenario
- **Also likely:** `crates/patchback-dataset` (the split role comes from `DatasetSplit.role`, `spec/domains/dataset.yaml:113-125`; the training rows are `data/train/x.f32`, DESIGN.md:283) — inferred; whether the role is passed into analysis or the computation sits in the dataset crate is the story open question "`split_role` input vs `DatasetSplit.role`"
- **Documents:** none required, the entity, both commands, the errors and the three scenarios already exist in `spec/` — cited; `spec/domains/analysis.yaml:838-841`, `spec/domains/dataset.yaml:105-107` and DESIGN.md OQ-18 (line 581) change only if an open question is settled. `CHANGELOG.md` and `crates/patchback-docs/src/status.rs:235` (planned Analysis row) are coordinator-owned at merge — cited, `.engineering/waves/a5.md` Selection
- **Confidence:** medium — the domain, commands and errors are cited by the spec; file placement is inferred, and the crate that reads the split role is an open question of the story itself
- **Would collide with:** `crates/patchback-analysis` — `error.rs` (`AnalysisError`, `DOMAIN_ERRORS`), `lib.rs` exports, `tests/domain.rs` and `src/features.rs`: this overlaps the feature-contract unit merging now, and the three contract errors are shared between the two. No overlap with the render unit (`crates/patchback-dsp`, `crates/patchback-cli/src/render.rs`) or the provenance unit (`crates/patchback-audio`): analysis compiles against `patchback-audio` (`crates/patchback-analysis/Cargo.toml`) but this story edits none of its files — inferred. Later: any unit on `crates/patchback-dataset` (story:dataset-generation, story:dense-data-files) if the computation lands there. Moot for wave a5: it `depends_on` feature-contract (active) and dataset-generation (draft)
- **Safety fact:** `patchback-analysis` depends only on `patchback-audio` (`crates/patchback-analysis/Cargo.toml`), and `patchback-dataset` is expected to depend on `patchback-analysis` to extract `features/1`, so analysis cannot read `DatasetSplit` without a crate cycle: the role must arrive as an input, or the computation must live in `patchback-dataset` — step 2, unproven
