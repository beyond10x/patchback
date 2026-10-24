---
format: aep.planning-md/3
id: story:dense-data-files
kind: story
status: draft
title: ModelSpec dense-data files
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:dataset-generation
scope:
- confidence: cited
  path: crates/patchback-dataset
- confidence: inferred
  path: crates/patchback-dataset/Cargo.toml
- confidence: inferred
  path: crates/patchback-dataset/src/dense.rs
- confidence: cited
  path: crates/patchback-dataset/src/lib.rs
- confidence: inferred
  path: spec/domains/dataset.yaml
revision: 5
---
# ModelSpec dense-data files

## Outcome

Each split is written as ModelSpec dense data: `x.f32` (N × 32) and `y.f32` (N × 8), row-major
little-endian f32, with an optional checked `order.u32`. A shape mismatch and non-finite content
are refused and nothing is written.

## DESIGN.md

§ 7.2, § 12.1 (data writer tests).

## Acceptance

- spec/scenarios/dense-data-files-written.yaml
- spec/scenarios/dense-data-files-shape-mismatch.yaml
- spec/scenarios/dense-data-files-content-refused.yaml

## Numeric obligations

- Exact byte counts: `x.f32` = N·32·4, `y.f32` = N·8·4; sample counts match between files (§ 7.2).
- Bytes are row-major little-endian f32; no NaN or Infinity (§ 7.2).

## Open questions

- What the `order.u32` check is (permutation of 0..N, indices below N, or length N).
- Writing to a split already Written has no refusal (ESS gap).
- The row ceiling: `order.u32` indexes at most 2^32 rows a split, and DESIGN.md § 12.3 bounds
  memory, while `patchback generate-dataset --samples` admits up to 9223372036854775807 and
  holds every sample in memory until the files are written (wave a6, Da6-31; unit 2 adversary
  pass 2, A2). This story decides the ceiling and where it is refused.

## Scope

Derived 2026-10-24 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-dataset` — cited (DESIGN.md:128 "Deterministic examples + ModelSpec f32 files"; `crates/patchback-dataset/src/lib.rs:1` names "the ModelSpec dense-data f32 files")
- **Files:** `crates/patchback-dataset/src/lib.rs` — cited (a doc comment only today; the crate has no code and no dependencies)
- **Symbols:** ESS `patchback.dataset.WriteDenseFile`, `FinishSplit`, `DenseFileKind` (X, Y, Order), `DenseDataFile`; refusals `WrongColumnCount`, `SampleCountMismatch`, `ByteCountMismatch`, `NonFiniteValue`, `MissingDenseFile`, `UnknownSplit`, `SplitAlreadyWritten` — cited (`spec/domains/dataset.yaml:58,256,638,727` and the three `spec/scenarios/dense-data-files-*.yaml`)
- **Also likely:** `crates/patchback-dataset/src/dense.rs` — inferred, a new module for the writer and its verification
- **Also likely:** `crates/patchback-dataset/Cargo.toml` and the `patchback-dataset` entry at `Cargo.lock:158` — inferred, only if the 32-column width is taken from the feature contract in `patchback-analysis` rather than a local constant
- **Also likely:** `spec/domains/dataset.yaml:277` and the generated `website/docs/reference/ess/patchback-dataset.md` — inferred, only if the `order.u32` open question is settled in the specification (today UNMAPPED, no refusal modelled)
- **Documents:** none required; the domain and all three acceptance scenarios are already in the tree and run interpreted (`Taskfile.yml:24,30`), so no Rust conformance binding is needed — cited
- **Confidence:** high — DESIGN.md § 4 layout and the crate doc comment place the writer, and the acceptance names three scenarios that exist
- **Would collide with:** any unit touching `crates/patchback-dataset` or `spec/domains/dataset.yaml`, notably story:dataset-generation, which calls this writer and owns the `generate-dataset` wiring in `crates/patchback-cli`. No file overlap with the three units now merging: render (`crates/patchback-dsp`, `crates/patchback-cli/src/render.rs`), provenance (`crates/patchback-audio`), feature contract (`crates/patchback-analysis`, `src/features.rs`); one semantic coupling, the x.f32 width of 32 is the `features/1` length the feature contract defines — inferred (the three impl branches had no commits past `wave/a5` when read)
- **Safety fact:** `patchback-dataset` has no dependants and no dependencies (its `Cargo.toml` has no `[dependencies]`, `crates/patchback-cli/Cargo.toml` does not take it, and the only code naming it is the allow-list at `crates/patchback-cli/tests/dependencies.rs:42`), so adding the writer changes no caller — step 2, unproven
