---
format: aep.planning-md/3
id: story:ml-init-benchmark
kind: story
status: active
title: Default vs ML-init benchmark
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:regressor-seeded-fit
- depends_on: story:engine-baseline
- serves: vision:patchback
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/patchback-cli/src/benchmark.rs
- confidence: cited
  path: crates/patchback-cli/src/main.rs
- confidence: inferred
  path: crates/patchback-cli/tests/ml_init_benchmark.rs
- confidence: cited
  path: crates/patchback-docs/src/status.rs
- confidence: cited
  path: crates/patchback-fit/src/benchmark.rs
- confidence: inferred
  path: crates/patchback-fit/src/error.rs
- confidence: inferred
  path: crates/patchback-fit/src/lib.rs
- confidence: cited
  path: crates/patchback-fit/tests/domain.rs
- confidence: inferred
  path: crates/patchback-fit/tests/ml_init_benchmark.rs
- confidence: cited
  path: website/docs/concepts/regressor.md
- confidence: cited
  path: website/docs/guides/train-the-regressor.md
- confidence: cited
  path: website/docs/reference/cli.md
- confidence: cited
  path: website/docs/reference/files.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T08:28:54Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-25T08:28:54Z", actor: "human:timo", revision: 9}
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

Derived 2026-10-25 by `story-scoper` against `wave/a11` at 2ef9cd0 (0.9.0 plus the a11 plan opening), before a11's seeded fit is in code. It supersedes the 2026-10-24 section. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-cli/src/benchmark.rs`. The verb hard-codes `BenchmarkKind::EngineBaseline` (:320-326) and `kind engine-baseline` (:510), and runs two arms per clip, `default_arm` (:366) and `fit` (:368) — cited
- **Entry points:** `patchback benchmark` (`crates/patchback-cli/src/main.rs:103-104`, `:162`), `Benchmark::start` (`crates/patchback-fit/src/benchmark.rs:210-248`, which already refuses `MlInit` without a weight artifact at :220-224), and a new `Benchmark::record_ml_init_result` beside `record_baseline_result` (:264-284) — cited
- **Files to edit:** `crates/patchback-fit/src/benchmark.rs`. The four ML-init fields and their getters exist and are never set (:178-183, :335-358); the module doc defers `RecordMlInitResult` to this story (:12-14). Add a `MlInitResult` input. The order is `kind-mismatch` then `wrong-state`, as `fit.yaml:1564-1597` gives it — cited
- **Files to edit:** `crates/patchback-cli/src/benchmark.rs` — cited, as above. A kind selector, `--weights` and `--normalization` (mirroring Da11-2), arms 3 and 4 per clip, ML-init records in `benchmark.txt`/`clips.txt`, more listening files (`EXAMPLE_FILES` is `[&str; 3]` at :179) — inferred
- **Files to edit:** `crates/patchback-fit/tests/domain.rs:226-276`, a third section from `RecordMlInitResult` to `events:` (`BenchmarkNotFound`, `BenchmarkKindMismatch`, `BenchmarkStateConflict`) — cited
- **Files to edit:** `crates/patchback-cli/src/main.rs:10-12, :20-21, :103` (the module doc's synopsis and exit-code table, and the verb's one-line help "against the default patch") — cited
- **Files to edit:** `crates/patchback-fit/src/lib.rs:103-107` (export `MlInitResult`) — inferred
- **Also likely:** `crates/patchback-fit/src/error.rs:170-175`, doc comments only. All four benchmark errors already exist (:164-175, `DOMAIN_ERRORS` :191-194), so no variant is added — inferred
- **Files to add:** `crates/patchback-fit/tests/ml_init_benchmark.rs`, one test per scenario, following `tests/engine_baseline.rs` — inferred
- **Files to add:** `crates/patchback-cli/tests/ml_init_benchmark.rs`. The CLI end to end needs a `weights/1` and a `normalization.stats` fixture; none is checked in, and a10/a11 tests write them through helpers — inferred
- **Not edited:** `crates/patchback-cli/tests/benchmark.rs`, whose `BENCHMARK_KEYS` (:121-141) and byte-equality test (:539) must keep passing unchanged. Also `crates/patchback-cli/Cargo.toml`, which already takes `patchback-model` and `patchback-analysis`, and `crates/patchback-fit/Cargo.toml`, which gets `patchback-model` from a11 (Da11-0) — cited
- **Contract, read not changed:** `spec/domains/fit.yaml:400-462` (`Benchmark`), `:1384-1441` (`StartBenchmark`), `:1525-1597` (`RecordMlInitResult`; "no verdict is recorded for an ML-init benchmark", :1528-1529), `:1710-1721` (`MlInitCompared`), `:631-663` (errors) and the three `spec/scenarios/ml-init-benchmark-*.yaml` — cited
- **Symbols, spec:** `patchback.fit.RecordMlInitResult`, `MlInitCompared`, `BenchmarkKind` (`EngineBaseline`, `MlInit`), `WeightArtifactRequired`, `InvalidHoldoutSize`, `BenchmarkKindMismatch`, `BenchmarkNotFound`, `BenchmarkStateConflict` — cited
- **Symbols, Rust today:** `Benchmark`, `BenchmarkKind`, `BaselineResult`, `default_arm`, `median`, `median_count`, `fit`, `FitOutcome::log`, `Evaluation::{normalized,total}` (`crates/patchback-fit/src/search.rs:168-205`), `Predictor::load` (`crates/patchback-model/src/predict.rs:161`), `Prediction::clamped` (:53), `read_stats` (`crates/patchback-analysis/src/normalization.rs:540`), `NormalizationTable` (:123) — cited
- **Symbols, a11 (Da11-4, not in code yet):** `FitRequest` ("or equivalent"), `SeedInputs` (a loaded `WeightArtifact` or `Predictor` plus a `NormalizationTable`), the request fields `analysis: Option<&Analysis>` and `seed: Option<SeedInputs>`, and an unnamed new fit entry; `fit` stays and delegates. `run.meta` gains `init regressor-prediction`, `weights sha256`, `modelspec`, `normalization sha256` (Da11-10) — cited (approval-record:wave-a11)
- **Documents the coordinator owns:** `crates/patchback-docs/src/status.rs:319-325`, planned row "A regressor-seeded fit", split by a11 (Da11-17); its generated `website/data/status.json` and `website/docs/status.mdx` follow — cited
- **Documents the coordinator owns:** `website/docs/reference/cli.md:268-360` (§ patchback benchmark), `website/docs/reference/files.md:361-480` (§ A benchmark run), `website/docs/guides/train-the-regressor.md:172-174` and `website/docs/concepts/regressor.md:77-78` (both say three arms; the story has four) — cited
- **Documents the coordinator owns:** `CHANGELOG.md` `## [Unreleased]`, `AGENTS.md:9` (§ State: "the engine baseline benchmark") — inferred
- **Confidence:** high for the library half: the fields, errors and guard order are in code and spec to the line. Medium for the CLI half: it rests on a11's entry, which is decided but not coded, and on design choices no source makes (open points 1-7).
- **Would collide with:** any unit on `crates/patchback-cli/src/benchmark.rs`, `crates/patchback-fit/src/benchmark.rs`, `crates/patchback-fit/src/lib.rs`, `crates/patchback-fit/tests/domain.rs` or `crates/patchback-cli/src/main.rs`. With a11 (`story:regressor-seeded-fit`) it shares `lib.rs`, `tests/domain.rs` and `main.rs` (Da11-16, Da11-17), and the coordinator's `status.rs`, `cli.md`, `files.md`, `CHANGELOG.md` and `AGENTS.md`. The `depends_on` edge already orders them. Does not touch `crates/patchback-cli/src/fit.rs`, `results.rs`, `search.rs` or `crates/patchback-fit/Cargo.toml` — inferred
- **Safety fact:** the engine-baseline path stays byte-identical. Da11-4 keeps `fit` as it is, so the call at `crates/patchback-cli/src/benchmark.rs:368` compiles and behaves unchanged. The ML-init records must appear only when the ML-init kind is selected, or `BENCHMARK_KEYS` (`crates/patchback-cli/tests/benchmark.rs:121-141`) and the same-flags byte test (:539) break, along with the format documented at `files.md:427-480` — step 2, unproven
