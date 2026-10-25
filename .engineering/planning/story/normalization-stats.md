---
format: aep.planning-md/3
id: story:normalization-stats
kind: story
status: implemented
title: Normalization statistics from the training split
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:feature-contract
- depends_on: story:dataset-generation
- serves: vision:patchback
scope:
- confidence: cited
  path: crates/patchback-analysis
- confidence: inferred
  path: crates/patchback-analysis/src/error.rs
- confidence: inferred
  path: crates/patchback-analysis/src/lib.rs
- confidence: inferred
  path: crates/patchback-analysis/src/normalization.rs
- confidence: inferred
  path: crates/patchback-analysis/tests/domain.rs
- confidence: inferred
  path: crates/patchback-analysis/tests/normalization.rs
- confidence: inferred
  path: crates/patchback-cli/src/generate_dataset.rs
- confidence: inferred
  path: crates/patchback-dataset/src/lib.rs
- confidence: inferred
  path: crates/patchback-dataset/src/normalization.rs
- confidence: inferred
  path: crates/patchback-dataset/tests/normalization_scenarios.rs
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T01:22:04Z", actor: "human:timo", revision: 8, correlation: "wave-a8"}
- {from: "proposed", to: "active", at: "2026-10-25T01:22:04Z", actor: "human:timo", revision: 9, correlation: "wave-a8"}
- {from: "active", to: "implemented", at: "2026-10-25T02:54:30Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
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

Re-read 2026-10-25 by `story-scoper` on 0.6.0 (`main` 04515d1): each point says what the
tree or `spec/` answers and what stays open, with its citation. The wave decisions in
`approval-record:wave-a8` take the ones the implementation needs.

1. Is the validation split refused as well as the test split? — Modelled as refused: `not-training-split` fires on `role != Train`, and the train-only scenario refuses Validation. DESIGN.md names only the test set, so this stays open with the owner as OQ-18 — `spec/domains/analysis.yaml:839-841,867-873`; `spec/scenarios/normalization-stats-train-only.yaml:106-117`; `DESIGN.md:259,581`
2. Which record holds the link to the stats: dataset, train split or the stats? — Partly answered: the stats reference both the split (relation `split` via `dataset_split_id`) and the contract; Dataset and DatasetSplit carry no stats link. No scenario needs more. The dataset-side UNMAPPED stays open — `spec/domains/analysis.yaml:223-233`; `spec/domains/dataset.yaml:105-107`
3. Is `SplitRole` duplicated between analysis and dataset? — In spec, no: analysis names `patchback.dataset.SplitRole` on the entity, error and view. In Rust the only `SplitRole` is `patchback_dataset::SplitRole`, which analysis cannot import (dataset depends on analysis). Open between: A, a mirror enum in analysis that dataset converts into; B, the command implemented on the dataset side with analysis-domain error names; C, `SplitRole` moved to `patchback-core` plus a new analysis-to-core edge, which changes `Cargo.lock` — `spec/domains/analysis.yaml:219-220,447-448,1116-1117`; `crates/patchback-dataset/src/model.rs:77`; `crates/patchback-dataset/Cargo.toml:10`; `crates/patchback-analysis/Cargo.toml:10`
4. Is the role a `split_role` input or read from `DatasetSplit.role`? — Answered by spec: there is no `split_role` input. The command takes `dataset_split_id` and reads `role` from the related split (`when_related ... predicate: role != Train`; payload `related: {via: input.dataset_split_id, field: role}`). In Rust that read happens where `DatasetStore` is visible, which depends on point 3 — `spec/domains/analysis.yaml:838,847-853,867-873`; `crates/patchback-dataset/src/model.rs:967`
5. New: what does the "internal artifact" hold, and where? — Open. The entity has no means or stds field, and no file name or format. DESIGN.md § 11 lists no stats file, and the scenarios assert only identity, contract, version, split and role — `spec/domains/analysis.yaml:211-220`; `DESIGN.md:259,400-421`; `spec/scenarios/normalization-stats-train-only.yaml:144-148`
6. New: are the rows the f64 `Sample.x` in `DatasetStore` or the f32 `data/train/x.f32`? — Open. The numeric obligation "match a reference computation over the training rows" must name one, because `x.f32` is `f64 as f32` — `crates/patchback-dataset/src/model.rs:410`; `crates/patchback-dataset/src/dense.rs:8`; `DESIGN.md:281`
7. New: population (n) or sample (n − 1) standard deviation, and accumulation in f64? — Open; DESIGN.md names neither — `DESIGN.md:259`
8. New: what happens to a feature whose training std is 0, or whose mean or std is non-finite? — Open. `(x − mean) / std` would be non-finite, the model has no error for it, and invariant 4 forbids non-finite output — `spec/domains/analysis.yaml:441-466`; `DESIGN.md:60`; `crates/patchback-analysis/src/lib.rs:29-30`
9. New: is an empty train split, or one still `Open` (dense files unwritten), admitted? — Open; `ComputeNormalizationStats` has no state or row-count gate — `spec/domains/analysis.yaml:855-900`; `crates/patchback-dataset/src/model.rs:123`
10. New: does `ApplyNormalization` take the feature values, and does it check length 32 and finiteness? — Open. The model input has only the stats id and the version; `FeatureContract::check` exists for reuse — `spec/domains/analysis.yaml:909-915`; `crates/patchback-analysis/src/features.rs:247`
11. New: may the stats contract differ from the contract the dataset was generated under? — Open, and admitted today: nothing compares `NormalizationStats.feature_contract_id` with `Dataset.feature_contract_id` — `spec/domains/dataset.yaml:95-99`; `spec/domains/analysis.yaml:842-900`
12. New: does story:training-run consume normalized x? — Open. `x.f32` is written raw, and normalization before ModelSpec training is in neither story scope. If training needs normalized rows, the two stories are ordered, not parallel — `crates/patchback-dataset/src/dense.rs:8`; `DESIGN.md:259`; `aep plan artifact show story:training-run`
13. New: outcome precedence. — Answered: no-such-split is checked before no-such-contract, so the split must be resolved before the contract lookup, and a Rust API taking `Option<role>` with None for an unknown split keeps the order — `spec/domains/analysis.yaml:855-866`

## Scope

Derived 2026-10-25 by story-scoper on main 04515d1.

Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-analysis`: all three acceptance scenarios are `domain: patchback.analysis`, `ComputeNormalizationStats` is modelled at `spec/domains/analysis.yaml:842` and `ApplyNormalization` at `:904`, and DESIGN.md maps features to this crate — cited (`spec/scenarios/normalization-stats-*.yaml` lines 4/6, `spec/domains/analysis.yaml:842,904`, `DESIGN.md:127`)
- **Files (new):** `crates/patchback-analysis/src/normalization.rs`: stats over training rows, `(x − mean) / std`, the `NormalizationStats` record and its store — inferred (one module per concern, `crates/patchback-analysis/src/lib.rs:45-54`; pattern of `features.rs`)
- **Files (edit):** `crates/patchback-analysis/src/lib.rs`: `mod normalization;`, re-exports beside `:65-71`, the crate-doc list at `:22-27` — inferred (pattern of the `features` module)
- **Files (edit):** `crates/patchback-analysis/src/error.rs`: a new error type (for example `NormalizationError`) with its own `DOMAIN_ERRORS`, as `AnalysisError` (`:56`) and `FeatureError` (`:366`) have; the new errors cannot join `FeatureError`, because `tests/domain.rs:138-169` requires every `FeatureError` name to be declared by a feature-contract command, a section that ends at `ComputeNormalizationStats` (`:112-117`) — inferred (from those two tests; the type name is a guess)
- **Files (edit):** `crates/patchback-analysis/tests/domain.rs`: section and error-declaration checks for `ComputeNormalizationStats` to `ApplyNormalization` (the next command is `EstimateNotes`) — inferred (pattern of `:112-200`)
- **Files (new):** `crates/patchback-dataset/tests/normalization_scenarios.rs`: the three scenarios, step by step. Each scenario opens with `patchback.dataset.GenerateDataset` and `OpenSplit`, which only `DatasetStore` carries, and analysis cannot reach `patchback-dataset` (Safety fact). `crates/patchback-dataset/tests/scenarios.rs:1-24` already drives analysis types from the dataset side — inferred (`normalization-stats-train-only.yaml:25-66`, `crates/patchback-dataset/src/model.rs:711,727`)
- **Files (read, no edit expected):** `crates/patchback-analysis/src/features.rs`. It exists now (759 lines). `FeatureRegistry::get` (`:303`) already answers `FeatureContractNotFound`, and `FeatureContract::state` (`:224`) and `definition` (`:220`) give the Draft and version gates. The old scope listed it as an edit — inferred (read of the public API; the gates may still want a helper there)
- **Symbols:** entity `patchback.analysis.NormalizationStats` (`:203`, invariant `split_role == Train` at `:222`, relations `contract` and `split` at `:223-233`); commands `ComputeNormalizationStats` (`:842`; outcome order no-such-split, no-such-contract, not-training-split, contract-draft, version-mismatch, computed at `:855-900`) and `ApplyNormalization` (`:904`); event `NormalizationStatsComputed` (`:1021`); view `NormalizationStatsSet` (`:1102`); errors `StatsFromNonTrainingSplit` (`:441`), `DatasetSplitNotFound` (`:450`), `NormalizationStatsNotFound` (`:456`), `NormalizationContractMismatch` (`:462`); shared errors `FeatureContractNotFound` (`:405`), `FeatureContractNotPublished` (`:411`), `FeatureVersionMismatch` (`:429`) — cited (`spec/domains/analysis.yaml`)
- **Symbols (read from dataset):** `patchback_dataset::SplitRole` (`model.rs:77`), `DatasetSplit.role` (`:357`), `DatasetStore::split` (`:967`), `DatasetStore::samples_in` (`:994`), training rows `Sample.x: [f64; 32]` (`:410`) — cited (`crates/patchback-dataset/src/model.rs`)
- **Also likely:** `crates/patchback-dataset/src/lib.rs` (`:42-56`) and a new `crates/patchback-dataset/src/normalization.rs`, in place of or beside the analysis module, if the split lookup and row read go on the dataset side (open point 3) — inferred (the only crate that sees both `DatasetStore` and `FeatureRegistry`)
- **Spec:** nothing must be written for acceptance. All three scenarios are listed, and `task check` runs them on the interpreted target — cited (`spec/ess-inputs.yaml:77-79`, `Taskfile.yml:24-30`)
- **Spec (optional):** the entity has no means or stds field and no file. Recording them, or settling OQ-18 or the link question, is a spec change the story may choose (open points 1, 2, 5) — cited (`spec/domains/analysis.yaml:211-220`)
- **Documents:** coordinator-owned at integration: `CHANGELOG.md`, `crates/patchback-docs/src/status.rs:258-262` (planned row "Training-split normalization statistics"), generated `website/data/status.json` and `website/docs/status.mdx`, `website/docs/reference/*.md` — cited (`status.rs:258-262`; brief)
- **Documents (conditional):** if OQ-18 is settled, `DESIGN.md:581` and the `UNMAPPED` at `spec/domains/analysis.yaml:839-841`; if the link question is settled, `spec/domains/dataset.yaml:105-107` — cited (those lines)
- **Documents (no CLI verb):** the Outcome names no verb, and no analysis feature command has a CLI verb today, so `crates/patchback-cli/src/main.rs` is untouched — cited (`rg extract-features crates/patchback-cli/src` returns nothing)
- **Confidence:** medium: the domain, commands, errors, scenarios and crate graph are read from the tree; whether the split lookup sits in `patchback-analysis` or `patchback-dataset` is a design choice the story still leaves open — inferred (open point 3)
- **Would collide with:** no file shared with story:engine-baseline. It edits `crates/patchback-fit` and `crates/patchback-cli`; its store scope also lists `crates/patchback-dataset/src/generate.rs`, which this story does not edit — inferred (`aep plan artifact show story:engine-baseline`)
- **Would collide with:** no file shared with story:training-run under options A or B of open point 3. Neither adds a dependency edge, so `Cargo.lock` stays unchanged. Dataset-side placement edits `crates/patchback-dataset/src/lib.rs`, which training-run only reads: an additive re-export, so rebase training-run after. Option C (move `SplitRole` to `patchback-core`) edits `crates/patchback-analysis/Cargo.toml` and `Cargo.lock`, which makes `Cargo.lock` a collision with training-run — inferred (brief lists `Cargo.lock` for training-run; `Cargo.lock:132-164` records both crates)
- **Safety fact:** `patchback-dataset` depends on `patchback-analysis` (`crates/patchback-dataset/Cargo.toml:10`; imports at `src/model.rs:14-16`, `src/meta.rs:22`, `src/generate.rs:8`), and `patchback-analysis` depends only on `patchback-audio` (`crates/patchback-analysis/Cargo.toml:10`). So analysis cannot name `patchback_dataset::SplitRole` or `DatasetStore` without a cycle. The split role and the training rows must arrive as inputs, with an absent split passed through for `DatasetSplitNotFound`, which the model checks first (`spec/domains/analysis.yaml:855-859`), or the command must live on the dataset side — step 2, unproven — cited (the two manifests)
