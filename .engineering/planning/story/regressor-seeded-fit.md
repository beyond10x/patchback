---
format: aep.planning-md/3
id: story:regressor-seeded-fit
kind: story
status: active
title: Regressor-seeded fit
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:native-predictor
- depends_on: story:bounded-search-fit
- serves: vision:patchback
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/patchback-cli/src/fit.rs
- confidence: inferred
  path: crates/patchback-cli/src/main.rs
- confidence: inferred
  path: crates/patchback-cli/tests/fit.rs
- confidence: inferred
  path: crates/patchback-cli/tests/fit_results.rs
- confidence: cited
  path: crates/patchback-fit/Cargo.toml
- confidence: cited
  path: crates/patchback-fit/src/error.rs
- confidence: cited
  path: crates/patchback-fit/src/lib.rs
- confidence: inferred
  path: crates/patchback-fit/src/results.rs
- confidence: cited
  path: crates/patchback-fit/src/search.rs
- confidence: cited
  path: crates/patchback-fit/tests/domain.rs
- confidence: inferred
  path: crates/patchback-fit/tests/regressor_seeded_fit_scenarios.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T06:07:04Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-25T06:07:04Z", actor: "human:timo", revision: 9}
---
# Regressor-seeded fit

## Outcome

A fit can start from the native predictor's patch instead of the default: it needs an analysis of
the reference and a weight artifact under the same feature contract, and refuses a fit missing the
analysis or with a mismatched contract.

## DESIGN.md

§ 9.2 step 1, § 13 Milestone B.

## Acceptance

- spec/scenarios/regressor-seeded-fit-seeded.yaml
- spec/scenarios/regressor-seeded-fit-feature-contract-mismatch.yaml
- spec/scenarios/regressor-seeded-fit-analysis-required.yaml

## Numeric obligations

- The seeded start is the predictor's output, finite and clamped to [0, 1] (§ 8.2, § 9.2).
- The bounded-search obligations hold unchanged from the seeded start (§ 9.2).

## Open questions

- Whether an analysis is made under a feature contract; until then the mismatch is an external
  refusal. Wave a11 (Da11-5, Da11-7): the analysis is made with the `features/1` configuration
  and the mismatch is checked against the weight artifact's and the statistics' contract; the
  spec's external marker stays for the owner.
- The weight artifact's existence and state are not guarded. Wave a11 (Da11-6): the CLI reads
  the artifact through `Predictor::load` before the fit and prints its `patchback.model.*`
  refusal; `StartFit` still declares no such outcome (for the owner).
- Where the normalization statistics come from. Wave a11 (Da11-3): the `--normalization` path
  to the dataset's `normalization.stats`; nothing in `weights/1` names them (for the owner).

## Scope

Derived 2026-10-25 by story-scoper on main 081ddc1, against the a9 unit tree and the a10 decisions.
Every line is **cited** (read from the story, the tree, `spec/`, the a9 unit tree or the a10 decision record) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-fit/src/search.rs`. The seeded start replaces restart 0's `default_point(self.topology)` in `Search::restarts`; restarts 1 and later keep `restart_point(seed, r, len)` — cited (`crates/patchback-fit/src/search.rs:479-502`, start choice at `:491-495`)
- **Entry point today:** `patchback_fit::fit(reference, notes, topology, config)` takes no start point and no weights; 11 test files and `crates/patchback-cli/src/benchmark.rs:368` call it — cited (`crates/patchback-fit/src/search.rs:518-523`; `git grep -c 'fit(' -- crates/*/tests`)
- **Entry point to add:** a second entry beside `fit` that takes the start (default, or prediction from weights plus statistics), with `fit` delegating to it with the default start, so no existing caller changes — inferred (keeps `crates/patchback-cli/src/benchmark.rs` and every existing test untouched)
- **Where the analysis is:** the reference is analysed once, in `check_reference`, with `AnalysisConfig::default()`, and the resulting `Analysis` is dropped (`Ok(_) => Ok(())`) — cited (`crates/patchback-fit/src/search.rs:304-306`)
- **Why it can be reused:** that default config is `features/1`'s (FFT lengths 512, 2048, 8192, hop half), so returning it lets `extract_features(contract, &analysis, reference.samples(), span)` give the raw `features/1` vector without a second analysis — cited (`crates/patchback-analysis/src/analyze.rs:28-40`; `crates/patchback-analysis/src/features.rs:167-176`, `:230-241`, `:458`)
- **Files (edit):** `crates/patchback-fit/src/search.rs`: the start type, the new entry, `check_reference` returning its analysis, feature extraction over the note's span, normalization, `Predictor::predict`, the contract check before silence and pitch, and `FitOutcome` (`:188-205`) carrying the init — inferred (start site and order cited above; shape is a design reading)
- **Files (edit):** `crates/patchback-fit/src/error.rs`: `FitError` gains `AnalysisRequired` and `FeatureContractMismatch` (and `AnalysisNotFound` if the library takes an analysis by reference), each listed in `DOMAIN_ERRORS`, `domain_error` and `examples` — cited (`crates/patchback-fit/src/error.rs:127-176`, `:181-209`, `:212-229`, `:234-302`; `spec/domains/fit.yaml:508-511`, `:529-545`)
- **Files (edit):** `crates/patchback-fit/src/error.rs`: the predictor's refusals (`UnknownWeightArtifact`, `CorruptWeightArtifact`, `WrongFeatureCount`, `NonFiniteFeature`) pass through as `patchback.model.*`, the way `Compare` passes `patchback.analysis.*` — inferred (precedent at `crates/patchback-fit/src/error.rs:197-208`; `StartFit` declares no weight-artifact outcome)
- **Files (edit):** `crates/patchback-fit/src/results.rs`: `INIT_DEFAULT` is a constant that `run_meta_text` always writes; it becomes the run's init, and `RunMeta` gains the weight artifact's checksum and the statistics' digest so `run.meta` still reproduces the run — inferred (sites `crates/patchback-fit/src/results.rs:127-144`, `:268`, reproducibility rule `:75-76`)
- **Files (edit):** `crates/patchback-fit/src/lib.rs`: re-exports and the crate doc's § The fit — cited (`crates/patchback-fit/src/lib.rs:4-13`, `:110-124`)
- **Files (edit):** `crates/patchback-fit/Cargo.toml`: add `patchback-model` (today analysis, audio, core, dsp). No cycle: `patchback-model` depends only on `patchback-dataset` — cited (`crates/patchback-fit/Cargo.toml`, `crates/patchback-model/Cargo.toml` `[dependencies]`)
- **Files (edit):** `crates/patchback-fit/tests/domain.rs`: holds every `FitError` to a declared error, so it accounts for the new variants — cited (`crates/patchback-fit/tests/domain.rs:7`, `:39`)
- **Files (new):** `crates/patchback-fit/tests/regressor_seeded_fit_scenarios.rs`: one test per acceptance scenario, each step cited, plus the numeric obligations (start finite and in [0, 1]; bounded-search obligations unchanged) — inferred (naming follows `crates/patchback-model/tests/weight_export_scenarios.rs` in the a9 tree)
- **Files (edit, only if the CLI verb is in scope):** `crates/patchback-cli/src/fit.rs`: `FitArgs` gains the weights and statistics paths, `fit_and_write` reads them and calls the new entry instead of `fit` at `:285`, `RunMeta` at `:286-292`, the hard-coded `init default` in `report` at `:358`, and the module doc tables `:1-113` — inferred (sites cited, flags undecided)
- **Files (edit, only if the CLI verb is in scope):** `crates/patchback-cli/tests/fit.rs`, `crates/patchback-cli/tests/fit_results.rs`: the `init` value and new `run.meta` records — inferred (both assert today's fit output)
- **Symbols (fit spec):** `patchback.fit.InitSource` {`DefaultPatch`, `RegressorPrediction`} (`:63-65`); `FitRun` `init`, `weight_artifact_id`, `analysis_id` (`:184-189`); `FitStarted.init` (`:1611-1618`); `AnalysisNotFound` (`:508-511`), `AnalysisRequired` (`:529-535`), `FeatureContractMismatch` (`:537-545`); `StartFit` (`:765`) outcomes `analysis-required` (`:842-849`), `unknown-analysis` (`:870-874`), `feature-contract-mismatch` (`:876-884`, external), `started-seeded` (`:900-928`), `started` (`:930-957`) — cited (`spec/domains/fit.yaml`)
- **Symbols (scenarios):** `started-seeded` then `started` with `init: DefaultPatch`; `analysis-required` with no arrange and no analysis; `feature-contract-mismatch` after an `AnalyzeAudio` — cited (`spec/scenarios/regressor-seeded-fit-seeded.yaml:82-124`, `spec/scenarios/regressor-seeded-fit-analysis-required.yaml:8-26`, `spec/scenarios/regressor-seeded-fit-feature-contract-mismatch.yaml:1-4`, `:79-97`)
- **Symbols (model spec):** `patchback.model.Predict` and its `feature-contract-mismatch` predicate, `WeightArtifact.feature_contract_id`, `PredictionClamped` — cited (`spec/domains/model.yaml:974`, `:1006`, `:183`, `:197`, `:1116`)
- **Symbols (Rust, read only):** `Predictor::load(path)`, `Predictor::predict(&mut self, feature_contract: &str, x: &[f64]) -> Result<Prediction, PredictError>`, `Prediction { raw, values, clamped }`, refusals load-then-count-then-contract-then-finite — cited (wave-a10 approval Da10-2, Da10-4, Da10-6)
- **Symbols (Rust, read only):** `WeightArtifact::read`, `feature_contract()`, `checksum()` in the a9 unit; `read_stats`, `NormalizationTable::normalize`, `FEATURES_1_VERSION`, `extract_features`, `NoteSpan` on main — cited (`crates/patchback-model/src/weights.rs:686`, `:724`, `:729` in the a9 tree; `crates/patchback-analysis/src/normalization.rs:540`, `:160`; `crates/patchback-analysis/src/features.rs:131`, `:458`, `:411`)
- **Also likely:** `crates/patchback-analysis/src/normalization.rs`: a checked apply for statistics read from a file. `read_stats` returns a `NormalizationTable` whose `normalize` checks nothing; the checked `NormalizationStats::apply` can only be built by `compute_normalization_stats`. The fit could instead check the version and finiteness itself — inferred (sites `crates/patchback-analysis/src/normalization.rs:156-166`, `:219-241`, `:331`)
- **Also likely:** `Cargo.lock`, for the new `patchback-fit` to `patchback-model` edge — inferred (any new path dependency changes the lock)
- **Also likely, read and not changed:** `crates/patchback-model/src/predict.rs` (a10, not written yet) and `crates/patchback-model/src/weights.rs` (a9) — cited (Da10-2; a9 tree)
- **Spec:** nothing has to be written for the three scenarios. They and every outcome they reach exist (`spec/domains/fit.yaml:842-957`). Optional for the owner: a `StartFit` refusal for a missing or corrupt weight artifact (UNMAPPED `:808-809`, now answerable from `patchback.model.UnknownWeightArtifact` and `CorruptWeightArtifact`) and lifting the external mismatch (UNMAPPED `:804-807`) — inferred (the markers are cited; whether to act now is a reading)
- **Documents (coordinator-owned):** `CHANGELOG.md` `## [Unreleased]`; the planned row "A regressor-seeded fit" (its "benchmarked against" half belongs to `story:ml-init-benchmark`) and the generated `website/data/status.json` and `website/docs/status.mdx`; `AGENTS.md` § State; the `fit` usage and exit table in `crates/patchback-cli/src/main.rs` if the CLI gains flags — cited (`crates/patchback-docs/src/status.rs:304-310`; `crates/patchback-cli/src/main.rs:7-9`, `:20-22`)
- **Documents (coordinator-owned, already ahead of the code):** `website/docs/guides/fit-a-recording.md:39-40` and `website/docs/concepts/closed-loop.md:36-39` already say the fit starts from the prediction; `website/docs/reference/cli.md:120` (§ patchback fit) and `website/docs/reference/files.md:288`, `:304`, `:323` (the `init default` rows) change with the flags and the `init` value — cited (the files at those lines)
- **Confidence:** medium. The start site, the dropped analysis, the refusal order and every spec symbol are cited to the line on main. The predictor is a decided but unwritten a10 API, and the source of the normalization statistics and the CLI flags are undecided — inferred (judgement over the lines above)
- **Would collide with:** `story:ml-init-benchmark` (a12) on 3 files: `crates/patchback-fit/src/error.rs` (both add `FitError` variants, `DOMAIN_ERRORS`, `examples`), `crates/patchback-fit/src/lib.rs` (re-exports) and `crates/patchback-fit/tests/domain.rs`. Its `Cargo.toml` edge becomes a no-op once this story adds it. No collision on `crates/patchback-fit/src/benchmark.rs` or `crates/patchback-cli/src/benchmark.rs` while `fit`'s signature is kept. The two stories run in order through `depends_on`, so this is not a same-wave conflict — inferred (relations from `aep plan artifact show story:ml-init-benchmark`; collisions follow from the edits listed above)
- **Safety fact:** the default path stays byte-identical. With no weight artifact, restart 0 still starts at `default_point` and `run.meta` still writes `init default`, so existing fit and benchmark outputs do not change. Every seeded `StartFit` outcome is guarded on `defined(weight_artifact_id)` and `started` sets `init: DefaultPatch`. Step 2, unproven. Where a symbol search stops: `run.meta`'s `init` value is a documented file format read by `crates/patchback-cli/tests/fit_results.rs` and `website/docs/reference/files.md:304`, so a seeded value or a new record changes the format; whether `RUN_META_FORMAT_VERSION` (1) bumps is open — cited (`spec/domains/fit.yaml:845`, `:877`, `:901`, `:942`; `crates/patchback-fit/src/results.rs:121`, `:268`)
