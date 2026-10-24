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
scope:
- confidence: inferred
  path: crates/patchback-cli/Cargo.toml
- confidence: inferred
  path: crates/patchback-cli/src/generate_dataset.rs
- confidence: cited
  path: crates/patchback-cli/src/main.rs
- confidence: inferred
  path: crates/patchback-cli/tests/dispatch.rs
- confidence: cited
  path: crates/patchback-dataset
- confidence: cited
  path: crates/patchback-dataset/Cargo.toml
- confidence: cited
  path: crates/patchback-dataset/src/lib.rs
- confidence: inferred
  path: crates/patchback-dataset/tests
- confidence: inferred
  path: spec/domains/dataset.yaml
revision: 4
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

## Scope

Derived 2026-10-24 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-dataset` (today a two-line doc-comment stub) — cited, DESIGN.md:128 "Deterministic examples + ModelSpec f32 files"
- **Files:** `crates/patchback-dataset/src/lib.rs`, `crates/patchback-dataset/Cargo.toml` — cited
- **Files:** `crates/patchback-cli/src/main.rs:51-104` (`Command` enum, `Command::name`, dispatch in `main`) for the `generate-dataset` verb — cited, story Outcome and `spec/domains/dataset.yaml:400-403` (wire name `generate-dataset`)
- **Files:** `crates/patchback-cli/Cargo.toml` (new `patchback-dataset` path dependency) and `Cargo.lock` — inferred, no crate depends on `patchback-dataset` today
- **Files:** new `crates/patchback-cli/src/generate_dataset.rs`, `crates/patchback-cli/tests/dispatch.rs` (one help test per verb, lines 201-227), new `crates/patchback-dataset/tests/` (domain and scenario tests) — inferred, the pattern of `analyze.rs`, `compare.rs` and `crates/patchback-fit/tests/domain.rs`
- **Symbols:** `patchback.dataset.GenerateDataset`, `OpenSplit`, `DrawPatch`, `AddSample`, `OpenDiagnosticSet`, `AddDiagnosticExample`; errors `SplitCountNotBatchMultiple`, `PatchAlreadyDrawn`, `UnknownDataset`, `UnknownPatch` — cited, the four acceptance scenarios and `spec/domains/dataset.yaml:307-637`
- **Also likely (read, not edited):** the render API of `crates/patchback-dsp` and the `features/1` extraction of `crates/patchback-analysis` — inferred, from the `depends_on` edges to story:subtractive-voice-render and story:feature-contract; neither exposes a `pub fn` on this branch yet
- **Also likely:** `crates/patchback-docs/src/status.rs:240-242` (the Dataset capability row) — cited; coordinator-owned at merge per `.engineering/waves/a5.md` (Selection)
- **Documents:** `spec/domains/dataset.yaml` only if an open question is settled in the model (PRNG name, `--samples` to split counts: the `UNMAPPED` at line 407), and DESIGN.md OQ-19 (line 582) — inferred; the four acceptance scenarios already exist in `spec/scenarios/` — cited
- **Confidence:** medium — the crate and the verb are cited by the story, DESIGN.md and the domain; the CLI module and test files are inferred from the pattern of earlier verbs
- **Would collide with:** the render unit, on `crates/patchback-cli/src/main.rs`, `crates/patchback-cli/Cargo.toml` and `Cargo.lock` (story:subtractive-voice-render scope lists all three); no overlap with the sources of `crates/patchback-dsp`, `crates/patchback-audio` or `crates/patchback-analysis`, which this story consumes and does not edit — inferred. Moot for wave a5: it `depends_on` render and feature-contract, so it cannot run beside them. Later it shares `crates/patchback-dataset` with story:dense-data-files, which depends on it
- **Safety fact:** `patchback-dataset` has no dependents (only `Cargo.toml:7` workspace membership and `crates/patchback-cli/tests/dependencies.rs:42` list it), so filling the stub breaks no caller; the dependency gate admits workspace-to-workspace path dependencies (`dependencies.rs:897-905` admits `patchback-cli` to `patchback-dsp`), so `patchback-dataset` to `patchback-dsp`/`patchback-analysis` should pass — step 2, unproven
