---
format: aep.planning-md/3
id: story:native-predictor
kind: story
status: active
title: Native matmul/relu predictor
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:weight-export
- depends_on: story:normalization-stats
- serves: vision:patchback
scope:
- confidence: cited
  path: crates/patchback-docs/src/status.rs
- confidence: cited
  path: crates/patchback-model
- confidence: inferred
  path: crates/patchback-model/Cargo.toml
- confidence: inferred
  path: crates/patchback-model/src/lib.rs
- confidence: inferred
  path: crates/patchback-model/src/predict.rs
- confidence: inferred
  path: crates/patchback-model/tests/domain.rs
- confidence: inferred
  path: crates/patchback-model/tests/pinned_predict.rs
- confidence: inferred
  path: crates/patchback-model/tests/predictor.rs
- confidence: inferred
  path: crates/patchback-model/tests/predictor_scenarios.rs
- confidence: inferred
  path: website/docs/concepts/regressor.md
- confidence: inferred
  path: website/docs/guides/train-the-regressor.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T05:03:05Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-25T05:03:05Z", actor: "human:timo", revision: 9}
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

- Whether the artifact's feature contract is checked against the input in Rust only. Answered
  for wave a10 by Da10-4 (`approval-record:wave-a10`): the string comparison in
  `Predictor::predict`, with the Rust order (count, unknown, corrupt, mismatch, finiteness)
  deviating from the spec's (mismatch before corrupt) because a corrupt artifact never loads.

## Scope

Derived 2026-10-25 by `story-scoper` on `main` 081ddc1 (0.7.0), before wave a9 defined the
`weights/1` file; the lines on the weight artifact are answered by `patchback_model::WeightArtifact`
in 0.8.0. Every line is **cited** (read from the story or the tree) or **inferred** (a reading
that could be wrong).

- **Primary surface:** `crates/patchback-model` — cited (`Cargo.toml` description "The ModelSpec CLI adapter and the CPU weight runner"; `src/lib.rs:1`; DESIGN.md:129 § 4.1, DESIGN.md:336 § 8.2 step 4, ADR-006 DESIGN.md:518)
- **Files:** `crates/patchback-model/src/predict.rs` (new module; the forward pass `relu(x · w1) · w2` with no bias, clamping to [0, 1] and counting the clamps, and the refusals) — inferred, the file name is a guess; the model shape is cited from DESIGN.md:299-305
- **Files:** `crates/patchback-model/src/lib.rs` — inferred: the `mod`/`pub use` block and the numbered module doc list, which `story:weight-export` also edited
- **Files:** `crates/patchback-model/tests/predictor.rs` (new; golden matmul, determinism, clamp count, the five refusals, Predict errors declared in `spec/domains/model.yaml`) — inferred, the file name is a guess; the obligations are cited from the story and DESIGN.md:436 § 12.1
- **Symbols:** command `patchback.model.Predict` (`spec/domains/model.yaml:974-1037`; outcome order wrong-feature-count, unknown-artifact, feature-contract-mismatch, corrupt-artifact, non-finite-feature, predicted-clamped, predicted), errors `WrongFeatureCount` (:436), `UnknownWeightArtifact` (:267), `FeatureContractMismatch` (:426), `CorruptWeightArtifact` (:448), `NonFiniteFeature` (:442), event `PredictionClamped` (:1116), field `WeightArtifact.clamped_predictions` (:201), view `WeightArtifacts` (:1185) — cited; no predictor code exists in `crates/`
- **Symbols consumed:** `patchback_analysis::FEATURES_1_VALUE_COUNT` (32) and `FEATURES_1_VERSION` (`features.rs:131-134`), `NormalizationStats::apply` / `NormalizationTable::normalize` (`normalization.rs:160,219`) — cited as existing; that the predictor uses them is inferred
- **Also likely:** the weight-artifact type and reader `story:weight-export` added (`src/weights.rs`, 0.8.0) — cited; the predictor reads it and does not write it
- **Also likely:** `crates/patchback-model/Cargo.toml` + `Cargo.lock` — coordinator-owned; the `patchback-analysis` and `patchback-core` edges exist since wave a9's scaffold (Da9-0), so no change is expected
- **Also likely:** `crates/patchback-docs/src/status.rs` (the planned row "A native predictor", split from the export row in 0.8.0) and the generated `website/docs/status.mdx` — cited, coordinator-owned
- **Documents:** no spec change. Both acceptance scenarios exist and `Predict` is modelled — cited. `website/docs/concepts/regressor.md:53-68` ("From training to prediction" step 4, clamp text at :72-74) may get a line — inferred. No CLI page: DESIGN.md § 11 (:404-411) lists no `predict` verb — cited
- **Confidence:** medium — DESIGN.md and the spec place the crate and the command; every file name inside the crate is a guess
- **Would collide with:** any unit editing `crates/patchback-model/src/lib.rs` or the weight-artifact module; the planned Model row in `status.rs`; `website/docs/concepts/regressor.md` — inferred. None in wave a10 (one unit)
- **Safety fact:** the predictor is additive. The only workspace dependent of `patchback-model` is `patchback-cli`, and that uses only `ModelSpecPin` (`crates/patchback-cli/src/fit.rs:133`; `Cargo.toml:21`). A new module changes no caller — step 2, unproven

## Open points (scoper, 2026-10-25) and the decision answering each

| # | point | answered by |
| --- | --- | --- |
| 1 | the weight file format, header layout and element order | Da9-3 (`weights/1`, row-major as printed), proven against the pinned binary by Da10-9 |
| 2 | the checksum algorithm | Da9-2 (SHA-256 over the payload) |
| 3 | how the artifact records its feature contract | Da9-9 (a version string, `features <contract>`), Da10-4 |
| 4 | who normalizes the input | Da10-3 (the caller; the predictor takes `NormalizationStats::apply`'s output) |
| 5 | arithmetic precision | Da10-5 (`f32` throughout, `as f32` on input) |
| 6 | numeric tolerance and the ModelSpec comparison | Da10-9 (reference tolerance `1e-4 · max(1, |ref|)`; `eval loss` equivalence at relative `1e-3`) |
| 7 | shapes and activation | settled (`[32, 64]`, ReLU, `[64, 8]`, no bias), Da10-5 |
| 8 | clamp warning channel and counter persistence | Da10-6 (`Prediction.clamped`, in-memory `clamped_predictions()`, nothing written) |
| 9 | refusal order | Da10-4 (the Rust order; the deviation recorded) |
| 10 | what `UnknownWeightArtifact` means in Rust | Da10-4 (`io::ErrorKind::NotFound` at `load`) |
| 11 | non-finite features | Da10-4 (`NonFiniteFeature` in the predictor; the caller's normalization is not consulted) |
| 12 | non-finite output | Da10-7 (NaN refused as Rust-only `NonFinitePrediction`; `±inf` clamped and counted) |
| 13 | surface | Da10-2 (library only) |
