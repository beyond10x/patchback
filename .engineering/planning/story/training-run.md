---
format: aep.planning-md/3
id: story:training-run
kind: story
status: draft
title: CPU training run
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:modelspec-pin-and-check
- depends_on: story:dense-data-files
scope:
- confidence: inferred
  path: crates/patchback-dataset/src/lib.rs
- confidence: cited
  path: crates/patchback-model
- confidence: inferred
  path: crates/patchback-model/Cargo.toml
- confidence: cited
  path: crates/patchback-model/src/lib.rs
- confidence: inferred
  path: crates/patchback-model/tests
- confidence: inferred
  path: website/docs/guides/train-the-regressor.md
revision: 4
---
# CPU training run

## Outcome

`modelspec train --backend cpu --data <train> --eval <val>` runs as a subprocess from Rust on real
generated data for a checked model; the run records its loss curve and completes or fails. A run of
an unchecked model, on unwritten data, or on the Vulkan backend is refused.

## DESIGN.md

§ 1.1, § 7.2, § 8.1, § 8.2 step 1, § 12.2 (ModelSpec bridge gate), § 13 Milestone B.

## Acceptance

- spec/scenarios/training-run-completed.yaml
- spec/scenarios/training-run-refused-unchecked-model.yaml
- spec/scenarios/training-run-refused-unwritten-data.yaml
- spec/scenarios/training-run-failed-and-vulkan-refused.yaml

## Numeric obligations

- The `--eval` split count is a multiple of the batch size (§ 7.2).
- Training loss decreases on the real generated data (§ 12.2).
- Evaluation loss, not `accuracy`, is what is reported (§ 8.1).

## Open questions

- Whether training on a validation or test split, or evaluating on train, is refused.
- The Plan, Bind, Start, Finish command split is a modelling choice not in DESIGN.md.

## Scope

Derived 2026-10-24 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-model` — cited (crate doc `src/lib.rs:20`: "`ModelSource::trainable` is the guard a training run takes"; `CheckedModel` at `src/lib.rs:619`)
- **Files:** `crates/patchback-model/src/lib.rs` — cited (the subprocess path a `train` call extends: `run` at :361, `ModelSpec::check` at :502; the guard `ModelSource::trainable` at :710)
- **Files:** new tests under `crates/patchback-model/tests/`, including a real-binary training test beside `tests/pinned_binary.rs` — inferred, following story:modelspec-pin-and-check
- **Symbols:** `ModelSource::trainable`, `CheckedModel`, `ModelNotChecked`, `ModelSpec`, `DEFAULT_TIMEOUT`; new `TrainingRun`, `Backend`, and errors `BackendNotShipped`, `ModelNotChecked`, `UnknownModelSource`, `TrainingRunNotReady`, `DataNotWritten`, `UnknownDataSplit`, `EvalNotWritten`, `EvalCountNotBatchMultiple`, `TrainingRunFinished` — cited (`spec/domains/model.yaml:120` and the four acceptance scenarios)
- **Also likely:** `crates/patchback-model/Cargo.toml` and `Cargo.lock` — inferred: a path dependency on `patchback-dataset`, because `StartTrainingRun` takes a `patchback.dataset.DatasetSplit` that must be Written
- **Also likely:** `crates/patchback-dataset/src/lib.rs`, read only — inferred: the Written split and its dense files are story:dense-data-files (draft); the crate is a 2-line stub today
- **Spec:** nothing to write — cited: the four scenarios exist and are listed at `spec/ess-inputs.yaml:127-130`; `TrainingRun` is modelled at `spec/domains/model.yaml:120`
- **Documents:** `CHANGELOG.md`, `crates/patchback-docs/src/status.rs` (the planned(B) capability "A parameter regressor trained with ModelSpec"), generated `website/data/status.json` and `website/docs/status.mdx`, possibly `website/docs/guides/train-the-regressor.md` § Check and train — inferred, from the scope story:modelspec-pin-and-check recorded
- **Confidence:** medium — high for `patchback-model` (crate doc and spec name the training-run guard and entity); the `patchback-dataset` coupling depends on how story:dense-data-files shapes that crate, which is unwritten
- **Would collide with:** any unit on `crates/patchback-model/src/lib.rs` or on the `patchback-dataset` split and dense-file types. With the three a5 units in merge (`crates/patchback-dsp` + `crates/patchback-cli/src/render.rs`, `crates/patchback-audio`, `crates/patchback-analysis` with `src/features.rs`) there is no code overlap; it overlaps only on coordinator-owned `CHANGELOG.md`, `status.rs` and the generated pages, and on `Cargo.lock` (in the render unit scope) if the `patchback-dataset` path dependency is added — inferred
- **Safety fact:** `patchback-model` has no consumer outside its own tests (`git grep -l patchback_model -- crates` returns only `crates/patchback-model/tests/*`; no other `Cargo.toml` names the crate), so additions to it break no other crate — step 2, unproven
