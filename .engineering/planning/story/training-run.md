---
format: aep.planning-md/3
id: story:training-run
kind: story
status: implemented
title: CPU training run
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:modelspec-pin-and-check
- depends_on: story:dense-data-files
- serves: vision:patchback
scope:
- confidence: cited
  path: crates/patchback-model
- confidence: cited
  path: crates/patchback-model/src/lib.rs
- confidence: inferred
  path: crates/patchback-model/src/training.rs
- confidence: inferred
  path: crates/patchback-model/tests
- confidence: inferred
  path: crates/patchback-model/tests/pinned_training.rs
- confidence: inferred
  path: crates/patchback-model/tests/training_scenarios.rs
- confidence: inferred
  path: website/docs/guides/train-the-regressor.md
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T01:22:05Z", actor: "human:timo", revision: 8, correlation: "wave-a8"}
- {from: "proposed", to: "active", at: "2026-10-25T01:22:05Z", actor: "human:timo", revision: 9, correlation: "wave-a8"}
- {from: "active", to: "implemented", at: "2026-10-25T03:13:46Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
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

Re-read 2026-10-25 by `story-scoper` on 0.6.0 (`main` 04515d1): each point says what the
tree or `spec/` answers and what stays open, with its citation. The wave decisions in
`approval-record:wave-a8` take the ones the implementation needs.

1. (story) Is training on a validation or test split, or evaluating on the train split, refused? — Open. The spec still marks it UNMAPPED with no role guard; the tree makes a guard cheap, since `DatasetSplit.role` is public — `spec/domains/model.yaml:162-164`, `crates/patchback-dataset/src/model.rs:354-361`
2. (story) The Plan, Bind, Start, Finish split is a modelling choice not in DESIGN.md. — Answered by the spec: ESS 0.67.0 arranges one kind of related row per command, so each preflight step (backend against pin, model source, data) is its own command. Open: whether Rust exposes four calls or one preflight that reports the same errors in that order — `spec/domains/model.yaml:611-613`
3. How does `patchback-model` reach a Written split and its dense files without a crate cycle? — Answered: add the path dependency `patchback-model -> patchback-dataset`. Dataset takes analysis, audio, core and dsp, none of which takes model; only patchback-cli names model; the dependency gate admits patchback-* path crates. Alternative: model stays dependency-free and the caller hands in directories and counts, which moves `DataNotWritten` and `EvalCountNotBatchMultiple` into the caller. Pick one — `crates/patchback-dataset/Cargo.toml:16-20`, `crates/patchback-model/Cargo.toml:1-7`, `crates/patchback-cli/Cargo.toml:18,21`, `crates/patchback-cli/tests/dependencies.rs:506`
4. What does the pinned `modelspec train` need on disk? — Answered from the 0.5.0 source, binary not run: the `.mspec` path, `--backend cpu`, `--data <dir>` holding `x.f32` and `y.f32` (named after `input x` and `target y`), with an optional `order.u32`; `--eval <dir>` laid out the same, its count a multiple of B = 16 (E-DATA-009), its `order.u32` not read. The split directory is `<out>/train` or `<out>/val` — modelspec tag 0.5.0 `website/docs/reference/cli.md:98-110` and `crates/modelspec-runtime/src/train/data.rs:11-17,306`; `models/patch_regressor.mspec:13-14`; `crates/patchback-dataset/src/model.rs:108`
5. (new) The default run cannot pass on Patchback data: 0.5.0 refuses an `order.u32` with fewer than steps × B indices (E-DATA-008), Patchback writes exactly N indices, and 5000 × 16 = 80,000 > 50,016. The guide and DESIGN.md command would exit 1. — Open, owner decision. Options: pass `--steps` with steps × 16 <= N; write a longer `order.u32` (changes story:dense-data-files contract); or train from a directory without `order.u32`. Prove it with the real binary first — modelspec tag 0.5.0 `crates/modelspec-runtime/src/train/data.rs:242-283`; `crates/patchback-dataset/src/dense.rs:179`; `models/patch_regressor.mspec:17`; `website/docs/guides/train-the-regressor.md:53-54`; `DESIGN.md:323-324`
6. (new) Where does the loss curve come from, and where is it recorded? — Source answered: stdout `step <k> loss <v>` at step 0, every 50 steps and the last step, then `eval loss=<v> accuracy=<v>` with `--eval`; stdout is written once, after the run. Open: where Patchback records it (a struct only, or a file in a run directory); the spec models no loss field — modelspec tag 0.5.0 `crates/modelspec-cli/src/train.rs:7-12,573-600`; `spec/domains/model.yaml:120-160`
7. (new) What timeout bounds a training run? — Open. `DEFAULT_TIMEOUT` is 60 s, sized for `check` (the doc says "well under a second"); the time of 5000 CPU steps on 0.5.0 is not measured — `crates/patchback-model/src/lib.rs:49-52`
8. (new) How is Vulkan refused when the pin carries no `vulkan_backend_shipped`? — Open. The spec refuses it from that pin field; the Rust `ModelSpecPin` has version, commit and sha256 only, and the pin file was deliberately left without the key because nobody knows whether the 0.5.0 Linux asset was built with the vulkan feature. Simplest reading (inferred): refuse `Backend::Vulkan` with `BackendNotShipped` before spawning, with no pin-file change — `spec/domains/model.yaml:67,636-645`; `crates/patchback-model/src/lib.rs:62-66`; `.engineering/planning/review-result/modelspec-pin-and-check-adversary-1.md:76-79`
9. (new) `UnknownPin`, `UnknownModelSource`, `UnknownTrainingRun` and `UnknownDataSplit` are store lookups, and `patchback-model` has no store. — Open. The unchecked-model scenario (unknown-source step) and the unwritten-data scenario (unknown-data step) need one: add a `ModelStore` like `DatasetStore`, or keep those steps domain-only as story:modelspec-pin-and-check did — `.engineering/planning/review-result/modelspec-pin-and-check-adversary-1.md:76-78`; `crates/patchback-dataset/src/model.rs:673`; `spec/scenarios/training-run-refused-unchecked-model.yaml:87-95`; `spec/scenarios/training-run-refused-unwritten-data.yaml:113-122`
10. (new) Can `PinMismatch` be enforced in Rust, given the spec makes it external? — Answered: yes, by comparing `CheckedModel::model_spec_pin` with `ModelSpec::pin` before spawning — `spec/domains/model.yaml:707-709`; `crates/patchback-model/src/lib.rs:445-449,483,637`
11. (new) How are `EvalNotWritten` and `EvalCountNotBatchMultiple` decided, given they are external in the spec? — Answered: `SplitState::Written` on the eval split, and `is_batch_multiple(sample_count, batch_size)` with the batch size of the `CheckedModel` — `spec/domains/model.yaml:773-774`; `crates/patchback-dataset/src/model.rs:127,666`; `crates/patchback-model/src/lib.rs:632`
12. (new) Does a `patchback` training command land? — Open. The spec names one, the CLI reference says training uses ModelSpec own CLI, and the acceptance needs none. If yes, `crates/patchback-cli/src/main.rs` becomes a collision with story:engine-baseline — `spec/domains/model.yaml:611`; `website/docs/reference/cli.md:332-337`
13. (new) What size of real generated data does the pinned-binary test use? — Open. `Generator::new` takes per-split counts that must be multiples of 16; with `order.u32` present the test also needs steps × 16 <= N_train (point 5); generation cost per sample is not measured — `crates/patchback-dataset/src/generate.rs:29,421`
14. (new) How is "training loss decreases" (§ 12.2) decided? — Partly answered: compare the first and last `step` lines of one run. Open: the threshold (strictly lower, or by a margin) — `DESIGN.md:447`; modelspec tag 0.5.0 `crates/modelspec-cli/src/train.rs:593`
15. (new) Should the `param @PatchRegressor.w1` and `.w2` lines of the same stdout be parsed? — Answered: no. Their parsing and `TrainingNotCompleted` belong to story:weight-export; this story keeps the raw stdout available to it — `spec/domains/model.yaml:374-383`; `aep plan artifact list` (story:weight-export, draft)

## Scope

Derived 2026-10-25 by story-scoper on main 04515d1.

Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-model` — cited (crate doc `crates/patchback-model/src/lib.rs:20-21`: "`ModelSource::trainable` is the guard a training run takes"; `CheckedModel` at :619, `ModelSource::trainable` at :710)
- **Files (edit):** `crates/patchback-model/src/lib.rs` — cited (the subprocess path a `train` call reuses: private `run` at :361 bounds the child by a timeout and drains both pipes; `ModelSpec` at :423 holds binary, pin and timeout; `ModelSpec::check` at :502 is the pattern for a `train` method; the `open_with_timeout` doc at :445-449 assigns `PinMismatch` to the training run)
- **Files (edit):** `crates/patchback-model/Cargo.toml` — cited (no `[dependencies]` today, lines 1-7; adding `patchback-dataset = { path = "../patchback-dataset" }` is the edge that lets `StartTrainingRun` read a `patchback.dataset.DatasetSplit`, `spec/domains/model.yaml:135-160,735-806`)
- **Files (edit):** `Cargo.lock` — inferred (the `patchback-model` package at `Cargo.lock:197-198` has no `dependencies` list; the new path edge adds one)
- **Files (new):** `crates/patchback-model/src/training.rs` — inferred (lib.rs is one 716-line module; `TrainingRun`, `Backend`, the run errors and the stdout parser fit a module of their own; keeping them in lib.rs is equally valid)
- **Files (new):** `crates/patchback-model/tests/training_scenarios.rs` — inferred (the four scenarios step by step, the pattern of `crates/patchback-dataset/tests/dense_scenarios.rs:1-5`)
- **Files (new):** `crates/patchback-model/tests/pinned_training.rs` — inferred (the pinned binary on a small real generation, skipped with a `SKIPPED` line unless `PATCHBACK_REQUIRE_MODELSPEC` is set, the pattern of `crates/patchback-model/tests/pinned_binary.rs:1-5`; CI installs the checksummed binary and sets the variable at `.github/workflows/check.yml:76-103`)
- **Symbols (model, existing):** `ModelSpec`, `ModelSpecPin`, `ModelSpecError`, `ModelSource`, `ModelSourceState`, `CheckedModel`, `ModelNotChecked`, `DEFAULT_TIMEOUT` — cited (`crates/patchback-model/src/lib.rs:52,62,176,423,550,559,600,619`)
- **Symbols (model, new from spec):** entity `patchback.model.TrainingRun` (Planned, ModelBound, Running, Completed, Failed), enum `Backend` (Cpu, Vulkan); commands `PlanTrainingRun`, `BindModelSource`, `StartTrainingRun`, `FinishTrainingRun`; errors `UnknownPin`, `BackendNotShipped`, `GpuEquivalenceNotRecorded`, `UnknownTrainingRun`, `UnknownModelSource`, `ModelNotChecked`, `PinMismatch`, `TrainingRunPastPlanning`, `UnknownDataSplit`, `DataNotWritten`, `EvalNotWritten`, `EvalCountNotBatchMultiple`, `TrainingRunNotReady`, `TrainingRunFinished` — cited (`spec/domains/model.yaml:46,120-181,614,673,735,807` and the four `spec/scenarios/training-run-*.yaml`)
- **Symbols (dataset, read only):** `DatasetStore::split` (:967), `DatasetSplit` with `role`, `sample_count`, `state` (:354-361), `SplitState::Written` (:127), `DatasetStore::dense_files_of` (:951), `DenseDataFile.path` (:367-375), `DenseFileKind` (:151), `SplitRole::dir` (:108), `is_batch_multiple` (:666), all in `crates/patchback-dataset/src/model.rs`; `Generator` and `Generated` (`src/generate.rs:412,521`), `dense::write_split` (`src/dense.rs:130`) — cited (all `pub`, re-exported at `crates/patchback-dataset/src/lib.rs:42-58`)
- **Also likely:** `crates/patchback-dataset/src/*`, read only, no edit — inferred (every accessor the run needs is public; a split directory is the parent of its `x.f32` `DenseDataFile.path`, which `write_split` builds as `dir.join(kind.file_name())` at `crates/patchback-dataset/src/dense.rs:153`)
- **Also likely:** `crates/patchback-cli/src/main.rs` and a new `crates/patchback-cli/src/train.rs`, only if a `patchback` training command is decided — inferred (`spec/domains/model.yaml:611` says "The `patchback` training command performs its preflight", while `website/docs/reference/cli.md:334` says "Training uses ModelSpec own CLI, not a `patchback` command"; the story Outcome and the four scenarios name no command)
- **Spec:** nothing must be written for the four scenarios — cited (all four exist; `TrainingRun` and its four commands are modelled at `spec/domains/model.yaml:120-181,614-853`). Not modelled, and held only by Rust tests per AGENTS.md § Planning and specification: the loss curve the Outcome names, the evaluation loss and the loss decrease. The split-role guard stays UNMAPPED (`spec/domains/model.yaml:162-164`) — cited
- **Documents:** `website/docs/guides/train-the-regressor.md:44-60` (§ Check and train; its command at :53-54 runs the default 5000 steps on a directory whose `order.u32` has 50,016 entries, which the pinned binary refuses, see Safety fact) and `DESIGN.md:323-324` (same command; a change there bumps the revision line, AGENTS.md § Rules) — inferred. Coordinator-owned at integration: `CHANGELOG.md`, `crates/patchback-docs/src/status.rs:276-282` (planned(B) "A parameter regressor trained with ModelSpec"), generated `website/data/status.json` and `website/docs/status.mdx`, `website/docs/reference/cli.md:332-337` if a `patchback` command lands — inferred
- **Confidence:** medium — high for `crates/patchback-model` (crate doc, spec entity and the existing subprocess path name it); medium because two shape choices are open: the run reads the `DatasetStore` (new crate edge, `Cargo.lock`) or takes directories from a caller, and a `patchback` command lands or not — inferred
- **Would collide with:** no certain shared file with the other two wave-a8 candidates; two conditional ones. (1) `Cargo.lock` with story:engine-baseline, only if its harness lands in `patchback-fit` and takes `patchback-dataset` (its own open question, `aep plan artifact show story:engine-baseline` Scope "Where the harness lives"). (2) `crates/patchback-cli/src/main.rs` (command enum and dispatch, beyond the exit table) with story:engine-baseline, only if this story adds a `patchback` training command. story:normalization-stats shares no file; it has a read coupling with this story on `crates/patchback-dataset` (`DatasetStore`, `DatasetSplit`, `SplitRole`) if it edits those types. Neither candidate touches `crates/patchback-model` — inferred (the candidate lists in the wave-a8 brief)
- **Safety fact:** the edge `patchback-model -> patchback-dataset` makes no cycle: `patchback-dataset` takes analysis, audio, core and dsp (`crates/patchback-dataset/Cargo.toml:16-20`); analysis takes audio, dsp takes audio and core, audio and core take nothing (their `Cargo.toml` files); only `crates/patchback-cli/Cargo.toml:21` names `patchback-model`; the dependency gate admits any `patchback-*` path crate (`crates/patchback-cli/tests/dependencies.rs:506`) and refuses only `modelspec*` packages (:133) — cited (read, not run), step 2, unproven. Second fact, to prove at step 4 before the real-data test is written: the pinned 0.5.0 binary refuses `--data` whose `order.u32` holds fewer than steps × B indices (E-DATA-008, modelspec tag 0.5.0 = pin commit 7c45c1a, `crates/modelspec-runtime/src/train/data.rs:242-283`); Patchback writes `order.u32` with exactly N entries (`crates/patchback-dataset/src/dense.rs:179`), so the default 5000 steps × 16 = 80,000 on the 50,016-row train split exits 1 unless the run passes `--steps` with steps × 16 <= N (`--steps` ships in 0.5.0, modelspec tag 0.5.0 `website/docs/reference/cli.md:109`) — cited (read, not run), step 2, unproven
