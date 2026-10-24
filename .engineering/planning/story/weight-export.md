---
format: aep.planning-md/3
id: story:weight-export
kind: story
status: draft
title: Strict weight export
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:training-run
scope:
- confidence: cited
  path: crates/patchback-model
- confidence: inferred
  path: crates/patchback-model/Cargo.toml
- confidence: inferred
  path: crates/patchback-model/src/lib.rs
- confidence: inferred
  path: crates/patchback-model/src/weights.rs
- confidence: inferred
  path: crates/patchback-model/tests/weight_export.rs
revision: 4
---
# Strict weight export

## Outcome

From a completed training run, the adapter recognises only the documented `param @PatchRegressor.w1`
and `.w2` outputs of the pinned version, validates names, shapes, element counts and finiteness,
and writes a checksummed, versioned little-endian f32 weight artifact with the model and feature-spec
version. Unrecognised output and a run that did not complete are refused.

## DESIGN.md

§ 8.2 steps 2–3, § 12.1 (model bridge tests), ADR-006.

## Acceptance

- spec/scenarios/weight-export-exported.yaml
- spec/scenarios/weight-export-refused-output.yaml
- spec/scenarios/weight-export-run-not-completed.yaml

## Numeric obligations

- Exactly `[32, 64]` and `[64, 8]` weights, all finite (§ 8.2, § 12.1).
- The checksum verifies on read (§ 8.2).

## Open questions

- The checksum algorithm.
- Whether the artifact's feature contract is copied from the dataset or given at export (modelled
  as given, unchecked).

## Scope

Derived 2026-10-24 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-model` — cited, DESIGN.md:127 (§ 4.1 "ModelSpec CLI adapter + CPU weight runner") and `crates/patchback-model/src/lib.rs:1`
- **Files:** `crates/patchback-model/src/lib.rs` — inferred, the crate root (716 lines, adapter only today) that would declare and re-export the export module
- **Files:** `crates/patchback-model/src/weights.rs` (new, name guessed) — inferred
- **Files:** `crates/patchback-model/tests/weight_export.rs` (new, name guessed) — inferred
- **Symbols:** `ExportWeights` (`spec/domains/model.yaml:854`), `WeightArtifact` (`:183`), `WeightsExported` (`:1107`), errors `TrainingNotCompleted`, `UnrecognizedTrainingOutput`, `UnrecognizedParameter`, `WrongWeightShape`, `WrongElementCount`, `NonFiniteWeight` (`:374`–`:420`); none exists in `crates/` yet (`git grep`, 0 hits) — cited
- **Also likely:** `crates/patchback-model/Cargo.toml` and `Cargo.lock` — inferred, only if the artifact takes its feature-contract id from `patchback-analysis`; the crate has no `[dependencies]` today
- **Documents:** none — inferred; `spec/domains/model.yaml` and the three acceptance scenarios already exist (`590cb82`), DESIGN.md § 8.2 already describes the bridge, and § 11 lists no export command, so `crates/patchback-cli` and `crates/patchback-docs` stay untouched
- **Confidence:** high — the story, DESIGN.md § 4.1 and § 8.2 place the work in `patchback-model`; the module and test names are guesses — inferred
- **Would collide with:** no file of the three units merging now (render: `crates/patchback-dsp` + `crates/patchback-cli/src/render.rs`; provenance: `crates/patchback-audio`; feature contract: `crates/patchback-analysis/src/features.rs`); a build-order dependency on the feature-contract unit only if the artifact imports its id type from `patchback-analysis`; and any unit on `crates/patchback-model`, which includes `story:training-run` (its `depends_on`, draft, not implemented), so the two cannot share a wave — inferred
- **Safety fact:** `patchback-model` has no `[dependencies]` and no workspace crate depends on it (only `Cargo.toml:8` membership and the gate list at `crates/patchback-cli/tests/dependencies.rs:43` name it), so a new export module changes no caller outside the crate — step 2, unproven — cited
