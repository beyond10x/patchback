---
format: aep.planning-md/3
id: story:ml-init-benchmark
kind: story
status: draft
title: Default vs ML-init benchmark
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:regressor-seeded-fit
- depends_on: story:engine-baseline
scope:
- confidence: inferred
  path: crates/patchback-cli/src/benchmark.rs
- confidence: inferred
  path: crates/patchback-cli/src/main.rs
- confidence: inferred
  path: crates/patchback-fit
- confidence: inferred
  path: crates/patchback-fit/Cargo.toml
- confidence: inferred
  path: crates/patchback-fit/src/benchmark.rs
- confidence: inferred
  path: crates/patchback-fit/src/error.rs
- confidence: inferred
  path: crates/patchback-fit/src/lib.rs
- confidence: inferred
  path: crates/patchback-fit/tests/domain.rs
- confidence: inferred
  path: website/docs/concepts/regressor.md
- confidence: inferred
  path: website/docs/guides/train-the-regressor.md
revision: 3
---
# Default vs ML-init benchmark

## Outcome

Over one holdout set, the benchmark reports default, default-plus-search, ML-init and
ML-init-plus-search medians side by side, so the Milestone B exit (does the regressor reduce
matching work or improve fit) can be read off. A benchmark of the wrong kind or without a weight
artifact is refused.

## DESIGN.md

§ 9.2 (benchmark against default and raw prediction), § 12.2, § 13 Milestone B.

## Acceptance

- spec/scenarios/ml-init-benchmark-four-arms-reported.yaml
- spec/scenarios/ml-init-benchmark-kind-mismatch.yaml
- spec/scenarios/ml-init-benchmark-weight-artifact-required.yaml

## Numeric obligations

- Median normalized loss and render evaluations per arm, on one holdout set and one fixed
  configuration (§ 12.2).
- The raw-model-only result is reported (§ 12.2).

## Open questions

- § 13 B sets no numeric target for the ML-init verdict; none is recorded.

## Scope

Derived 2026-10-24 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-fit` — inferred: the story and its scenarios name the domain `patchback.fit` but no Rust path; DESIGN.md § 4.1 puts candidate search there, and `story:engine-baseline`, which owns the same `Benchmark` entity, is scoped there
- **Files:** `crates/patchback-fit/src/benchmark.rs` (new; no benchmark code exists today) — inferred
- **Files:** `crates/patchback-fit/src/lib.rs` (module export) and `crates/patchback-fit/src/error.rs` (the refusals `WeightArtifactRequired`, `InvalidHoldoutSize`, `BenchmarkKindMismatch`, `BenchmarkNotFound`) — inferred
- **Files:** `crates/patchback-fit/Cargo.toml` (a `patchback-model` dependency for the raw-regressor and regressor-plus-search arms, unless `story:regressor-seeded-fit` already added it) — inferred
- **Files:** `crates/patchback-fit/tests/domain.rs` (holds every fit refusal to an error declared in `spec/domains/fit.yaml`) — inferred
- **Contract, read not changed:** `spec/domains/fit.yaml:402-463` (`Benchmark`), `:1385-1435` (`StartBenchmark`), `:1524-1591` (`RecordMlInitResult`) and the three `spec/scenarios/ml-init-benchmark-*.yaml` the story lists — cited
- **Symbols:** `patchback.fit.StartBenchmark`, `RecordMlInitResult`, `RecordBaselineResult`, `BenchmarkKind` (`EngineBaseline`, `MlInit`), `MlInitCompared`, `WeightArtifactRequired`, `InvalidHoldoutSize`, `BenchmarkKindMismatch`, `BenchmarkNotFound` — cited (scenarios and `fit.yaml`; none exists in Rust yet)
- **Also likely:** `crates/patchback-cli/src/main.rs` (the `Command` enum, line 52) and a new `crates/patchback-cli/src/benchmark.rs`, only if the benchmark gets a CLI entry; DESIGN.md § 11 lists no benchmark command, so that is open — inferred
- **Also likely:** `crates/patchback-model` read only, for the native predictor; not edited — inferred
- **Documents:** `website/docs/guides/train-the-regressor.md:75` and `website/docs/concepts/regressor.md:75` describe three benchmark arms where the story reports four (default plus search is missing), so a unit closing the story corrects them — inferred
- **Documents:** `crates/patchback-docs/src/status.rs:259-264` (capability row "A regressor-seeded fit", shared with `story:regressor-seeded-fit`) and `CHANGELOG.md` `## [Unreleased]` — inferred
- **Confidence:** medium — the specification contract is cited to the line, but the story names no Rust path and none of its code exists; the crate follows DESIGN.md § 4.1 and the inferred scope of `story:engine-baseline`
- **Would collide with:** any unit on the benchmark, error and manifest surface of `crates/patchback-fit`, which this story shares with `story:engine-baseline` (one `Benchmark` type; the kind-mismatch scenario drives both record commands) — inferred
  - render (`crates/patchback-dsp`, `crates/patchback-cli/src/render.rs`): no file overlap; it meets render on `crates/patchback-cli/src/main.rs` only if it adds a CLI subcommand — inferred
  - provenance (`crates/patchback-audio`): no overlap — inferred
  - feature contract (`crates/patchback-analysis`, new `src/features.rs`): no overlap; this story consumes `features/1` only through the predictor — inferred
  - `story:bounded-search-fit` (`crates/patchback-fit`): overlaps, same crate, likely the same `lib.rs`, `error.rs` and `Cargo.toml` — inferred
- **Sequencing:** the overlap with `story:bounded-search-fit` cannot bite in the next wave: this story depends on `story:regressor-seeded-fit` (which depends on `story:bounded-search-fit` and `story:native-predictor`) and on `story:engine-baseline` — cited (store relations)
- **Safety fact:** no Rust code reads or writes a benchmark today (`git grep -n -i benchmark -- crates` returns only the prose string at `crates/patchback-docs/src/status.rs:263`), so the change is additive and breaks no caller; the one cross-story contract is the `BenchmarkKind` guard pair at `spec/domains/fit.yaml:1470-1474` and `:1558-1563` (wire values `EngineBaseline`, `MlInit`), which this story and `story:engine-baseline` must implement on one shared type — step 2, unproven
