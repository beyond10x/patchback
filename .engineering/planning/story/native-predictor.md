---
format: aep.planning-md/3
id: story:native-predictor
kind: story
status: draft
title: Native matmul/relu predictor
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:weight-export
- depends_on: story:normalization-stats
scope:
- confidence: cited
  path: crates/patchback-model
- confidence: inferred
  path: crates/patchback-model/Cargo.toml
- confidence: inferred
  path: crates/patchback-model/src/lib.rs
- confidence: inferred
  path: crates/patchback-model/src/predict.rs
- confidence: inferred
  path: crates/patchback-model/tests/predictor.rs
- confidence: inferred
  path: website/docs/concepts/regressor.md
revision: 4
---
# Native matmul/relu predictor

## Outcome

A small native Rust function reads a validated weight artifact and predicts the eight normalized
parameters from a normalized `features/1` vector (two dense layers and ReLU), clamping outputs to
[0, 1] with a warning. Input that does not fit the artifact's feature contract is refused.

## DESIGN.md

§ 8.1 (clamp with warning), § 8.2 step 4, § 12.1, ADR-006.

## Acceptance

- spec/scenarios/native-predictor-predicted.yaml
- spec/scenarios/native-predictor-refused-input.yaml

## Numeric obligations

- Golden tests against independently computed reference matmul results (§ 8.2).
- Deterministic output for the same weights and input (§ 12.1).
- Outputs outside [0, 1] are clamped and counted (§ 8.1).

## Open questions

- Whether the artifact's feature contract is checked against the input in Rust only.

## Scope

Derived 2026-10-24 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-model` — cited (its `Cargo.toml` description "The ModelSpec CLI adapter and the CPU weight runner"; `src/lib.rs:1` names the native CPU weight runner; DESIGN.md § 8.2 step 4, ADR-006)
- **Files:** `crates/patchback-model/src/predict.rs` (new module) — inferred, the file name is a guess
- **Files:** `crates/patchback-model/src/lib.rs` — inferred, the crate is one 716-line file today, so the module declaration and re-export land here
- **Files:** `crates/patchback-model/tests/predictor.rs` (new; golden matmul, determinism, clamp count, refusals) — inferred, file name guessed; the obligations themselves are cited from the story
- **Symbols:** domain command `patchback.model.Predict` (outcomes predicted, predicted-clamped), errors `UnknownWeightArtifact`, `FeatureContractMismatch`, `WrongFeatureCount`, `CorruptWeightArtifact`, `NonFiniteFeature`, event `PredictionClamped`, field `WeightArtifact.clamped_predictions` — cited (`spec/domains/model.yaml:974-1036`, `:436`, `:201`, `:1116`; both scenarios). No Rust predictor code exists yet (`git grep` over `crates/`)
- **Also likely:** the weight-artifact reader written by `story:weight-export` (depends_on), presumably also in `crates/patchback-model` — inferred; the predictor reads that artifact, so it extends that code
- **Also likely:** `crates/patchback-model/Cargo.toml` and `Cargo.lock` — inferred, only if the predictor takes the `features/1` type or constants from `patchback-analysis` or the normalization stats from `patchback-dataset`; today `patchback-model` has no `[dependencies]`
- **Also likely:** `crates/patchback-docs/src/status.rs:252-258` (row "Checked weight export and a native predictor", shared with `story:weight-export`) and the generated `website/docs/status.mdx` — cited, coordinator-owned shared files per `.engineering/waves/a5.md` Selection
- **Documents:** no spec change: both scenarios are already in `spec/ess-inputs.yaml:70-71` and `Predict` is modelled — cited. `website/docs/concepts/regressor.md` (#from-training-to-prediction, linked from the status row) may get a line — inferred
- **Confidence:** medium — the crate is cited by its own description and DESIGN.md; every file name inside it is guessed, and whether it takes a dependency on `patchback-analysis` is an open design choice
- **Would collide with:** any unit touching `crates/patchback-model`, above all `story:weight-export`, which owns the artifact format this reads (serialise them; it is a depends_on anyway). No overlap with the three units merging now: render (`patchback-dsp`, `patchback-cli/src/render.rs`), provenance (`patchback-audio`) and feature contract (`patchback-analysis`, new `src/features.rs`) are all outside `patchback-model`. The only contact is read-only: if the predictor imports `features/1` from `patchback-analysis/src/features.rs`, it must start after that unit merges, and the new dependency edge edits `Cargo.lock` (which unit 1 also edits). Plus the coordinator shared files every unit touches (`status.rs`, `CHANGELOG.md`, generated pages) — inferred
- **Safety fact:** `patchback-model` has no dependents and no dependencies in the workspace (`git grep` over `*/Cargo.toml` and `crates/*/src/*.rs` finds it outside its own crate only in the evidence string at `status.rs:226`; `Cargo.lock:182-184` lists no deps), so an additive predictor module breaks no other crate — step 2, unproven
