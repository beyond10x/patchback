---
format: aep.planning-md/3
id: story:weight-export
kind: story
status: active
title: Strict weight export
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:training-run
- serves: vision:patchback
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
  path: crates/patchback-model/tests/domain.rs
- confidence: inferred
  path: crates/patchback-model/tests/pinned_training.rs
- confidence: inferred
  path: crates/patchback-model/tests/weight_export.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T03:48:02Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-10-25T03:48:02Z", actor: "human:timo", revision: 8}
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

Derived 2026-10-25 by `story-scoper` on `main` 081ddc1 (0.7.0). Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-model` — cited (story typed scope; DESIGN.md:129 "ModelSpec CLI adapter + CPU weight runner"; `crates/patchback-model/Cargo.toml` description)
- **Files:** `crates/patchback-model/src/weights.rs` (new, name guessed): parse the `param @PatchRegressor.w1`/`.w2` blocks of `TrainingRun::stdout()`, validate them, write and read the artifact — inferred
- **Files:** `crates/patchback-model/src/lib.rs`: a `mod`/`pub use` pair beside `mod training; pub use training::{…}` (`:31-36`) and an item 7 in the module doc list (`:3-25`) — inferred
- **Files:** `crates/patchback-model/tests/weight_export.rs` (new, name guessed): the three scenarios step by step against a stand-in binary, as `tests/training_scenarios.rs` does for training — inferred
- **Symbols (domain, to implement):** `ExportWeights` (`spec/domains/model.yaml:854-972`), `WeightArtifact` (`:183-229`), `WeightChecksum` (`:37-42`), `WeightsExported`, errors `TrainingNotCompleted`, `UnrecognizedTrainingOutput`, `UnrecognizedParameter`, `WrongWeightShape`, `WrongElementCount`, `NonFiniteWeight` (`:374-420`); none in `crates/` (`git grep`, 0 hits outside `patchback-fit`'s `WeightArtifactRequired`) — cited
- **Symbols (read, unchanged):** `TrainingRun::state`, `stdout`, `pin` (`crates/patchback-model/src/training.rs:692`, `:740`, `:701`), `TrainingRunState::Completed`; `LossCurve::parse` already skips `init` and `param` lines (`training.rs:136-137`); `pinned_training.rs:207-208` asserts that the real 0.5.0 stdout holds both `param` lines — cited
- **Stdout format:** a wave a8 probe capture of a real run holds 2 `init @PatchRegressor.wN: Tensor<f32, [r, c]> = [v, …]` lines, 4 `step` lines and 2 `param` lines of the same shape, 2,048 and 512 values (the w1 line 25,905 bytes, the w2 line 6,491) — cited
- **Also likely:** `crates/patchback-model/Cargo.toml`: a direct dependency on `patchback-core` (`digest::sha256`, `crates/patchback-core/src/digest.rs:214`) and/or `patchback-analysis` (`FEATURES_1_VERSION`, `crates/patchback-analysis/src/features.rs:131`). Today the crate lists only `patchback-dataset`, which re-exports neither. `Cargo.lock` follows; in a8 the coordinator owned both (Da8-0) — inferred
- **Also likely:** `crates/patchback-model/tests/domain.rs`: `COMMANDS` (`:37-52`) accounts for the four training commands only; `ExportWeights`' errors need the same accounting, `UnknownTrainingRun` in `NOT_EMITTED_BY_THE_CRATE` (Da8-21) — inferred
- **Also likely:** `crates/patchback-model/tests/pinned_training.rs`: export from the real pinned run under `MODELSPEC` — inferred
- **Not touched:** `crates/patchback-model/src/training.rs`: everything the export reads is already a public accessor — inferred
- **Documents:** `website/docs/reference/files.md:613-616` (an "Exported weights" stub with no format), `website/docs/guides/train-the-regressor.md:94-102`, `crates/patchback-docs/src/status.rs:289` ("no weights leave it until weight export ships") and `:297-303` (one planned row shared with the native predictor), `CHANGELOG.md`. All coordinator-owned at a8. No spec change: the three scenarios are already in `spec/ess-inputs.yaml:142-144` — inferred
- **No CLI verb:** DESIGN.md § 11 (`:404-411`) lists none, and training is a library call with no command (`train-the-regressor.md:86`), so `crates/patchback-cli` and `website/docs/reference/cli.md` stay untouched unless a verb is decided — inferred
- **Confidence:** high for the crate (the story, DESIGN.md § 4.1 and § 8.2 place it); medium for the files (module and test names guessed, the dependency edge depends on open points 1 and 4) — inferred
- **Would collide with:** any unit editing `crates/patchback-model/src/lib.rs`'s module/re-export block or doc list, `crates/patchback-model/Cargo.toml`, or `Cargo.lock` through a new edge; and the coordinator's shared docs above, whose status row is shared with the native-predictor surface — inferred
- **Safety fact:** the only workspace crate that depends on `patchback-model` is `patchback-cli` (`crates/patchback-cli/Cargo.toml:21`), and it imports only `ModelSpecPin` (`crates/patchback-cli/src/fit.rs:133`), so an additive export module changes no caller — step 2, unproven — cited

### Open points (scoper, 2026-10-25; answered by Da9-2 to Da9-18 in approval-record:wave-a9 where a decision was the coordinator's)

| # | Undecided | Source |
|---|---|---|
| 1 | Checksum algorithm, and what it covers (header and data, or data only). The tree already has a SHA-256 (`patchback_core::digest::sha256`), used for `run.meta` (`fit.rs:122`) | DESIGN.md:335, OQ-21 DESIGN.md:584, model.yaml:37 |
| 2 | Artifact format: file name, where it is written, version string, header (line-oriented text or binary), w1/w2 layout | DESIGN.md:335, :427; files.md:613-616 is an empty stub |
| 3 | What "model version" means: the ModelSpec pin (0.5.0 or its commit), the model name, a digest of the `.mspec`? The entity carries only `model_spec_pin_id`, and `TrainingRun` keeps only the source path | DESIGN.md:335; model.yaml:189-205; training.rs:706 |
| 4 | Feature contract given at export and unchecked (story open question). In Rust `FeatureContractId` is an in-memory u64, so the file would carry `features/1` | model.yaml:206-209; features.rs:189-192, :131 |
| 5 | Whether the artifact embeds or references the training split's `normalization.stats`. The predictor needs them, and 0.7.0 says they apply "the same at training and inference" | DESIGN.md:335 silent; CHANGELOG.md 0.7.0 |
| 6 | Which lines count as recognized. Real stdout has `init`, `step`, `eval` and `param` lines; read literally, the spec refuses every real run | model.yaml `unrecognized-output` (≈:944-947); DESIGN.md:334-335 |
| 7 | Which error a missing, duplicate or reordered `param` block gets; the spec has no outcome for it | model.yaml:384-420 |
| 8 | Outcome order. The spec checks shape and element count before the run checks, but shapes are parsed from the output of a completed run. a8 precedent: the run's state first. Names and shapes are spec inputs but parsed in Rust | model.yaml:887-931; training.rs:7-9 |
| 9 | Whether a `Completed` run with an `output_error`, or with a NaN loss, is exportable | Da8-33, Da8-40 (wave a8 Close); story Outcome "did not complete" |
| 10 | How "for the pinned version" is enforced: the run's pin against a compiled-in known version 0.5.0? What a pin bump does to export | DESIGN.md:334 |
| 11 | Whether ModelSpec prints f32 values that round-trip bit for bit. Unknown; the probe's 8 significant digits look like Rust's Display (inferred) | DESIGN.md:437 "deterministic output" |
| 12 | The pinned 0.5.0 already ships `train --checkpoint <PATH>` (`.msckpt`) and `infer --checkpoint` (its `--help`). DESIGN.md assumes no checkpoint interface, and its step 5 replaces the bridge once one exists and passes equivalence tests. Whether the stdout bridge is still the design needs an owner answer | DESIGN.md:26, :331, :337 |
| 13 | Who owns the reader. The story requires "the checksum verifies on read", but the spec puts `CorruptWeightArtifact` on `Predict` (the native predictor) | story Numeric obligations; model.yaml:448-452, :1013-1014; DESIGN.md:457 |
| 14 | Writer policy (create-new and remove what was written on failure, as `generate_dataset.rs:273` and `benchmark.rs:660` do) and the destination path | not in DESIGN.md |
| 15 | CLI verb or library only | DESIGN.md:404-411 has none; spec wire `export-weights` model.yaml:856 |
| 16 | `UnknownTrainingRun` cannot be reached for a run held as a value, so the second `ExportWeights` step of `weight-export-exported.yaml` has no library counterpart | Da8-21 precedent; model.yaml `unknown-run` |
| 17 | Whether `clamped_predictions` (set to 0 at export) lives in the file; if the predictor increments it, the artifact is mutable | model.yaml:201-205, :965 |
| 18 | Limits: a w1 `param` line is about 25 KB, and nothing caps the captured stdout (`lib.rs` and `training.rs` searched, no cap found) | OQ-6 DESIGN.md:563; DESIGN.md:427 "unbounded lengths" |
