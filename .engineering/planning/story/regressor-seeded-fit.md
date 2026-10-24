---
format: aep.planning-md/3
id: story:regressor-seeded-fit
kind: story
status: draft
title: Regressor-seeded fit
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:native-predictor
- depends_on: story:bounded-search-fit
scope:
- confidence: inferred
  path: crates/patchback-analysis/src/features.rs
- confidence: inferred
  path: crates/patchback-cli/src/analyze.rs
- confidence: inferred
  path: crates/patchback-cli/src/main.rs
- confidence: inferred
  path: crates/patchback-fit
- confidence: inferred
  path: crates/patchback-fit/Cargo.toml
- confidence: inferred
  path: crates/patchback-fit/src/error.rs
- confidence: inferred
  path: crates/patchback-fit/src/lib.rs
- confidence: inferred
  path: crates/patchback-fit/tests
- confidence: inferred
  path: crates/patchback-model/src/lib.rs
revision: 3
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
  refusal.
- The weight artifact's existence and state are not guarded.

## Scope

Derived 2026-10-24 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/patchback-fit` — inferred: the scenarios cite `patchback.fit.StartFit` and DESIGN.md § 4.1 puts candidate search in this crate, but the search does not exist yet (`story:bounded-search-fit` lands it; `patchback-fit` today holds only `loss.rs` and `error.rs`)
- **Files:** `crates/patchback-fit/src/lib.rs` and the search module `story:bounded-search-fit` adds — inferred, the start point: the fixed default patch, or the predictor output clamped to [0, 1]
- **Files:** `crates/patchback-fit/src/error.rs:34` — inferred, `AnalysisRequired` and `FeatureContractMismatch` join the fit refusals beside `CompareError::DOMAIN_ERRORS`
- **Files:** `crates/patchback-fit/Cargo.toml` — inferred, a path dependency on `patchback-model` for the predictor (today it depends on analysis and audio only)
- **Files:** `crates/patchback-cli/src/main.rs:80` (`Command::Fit`, a stub today) and the `fit` module `story:bounded-search-fit` adds — inferred, weight-artifact and analysis arguments on `patchback fit`
- **Files:** `crates/patchback-fit/tests/` — inferred, one test per acceptance scenario; `tests/domain.rs` checks each command error is declared in `spec/domains/fit.yaml`
- **Symbols:** `patchback.fit.InitSource` `RegressorPrediction` (`spec/domains/fit.yaml:62`), `patchback.fit.AnalysisRequired` (:530), `patchback.fit.FeatureContractMismatch` (:538), `StartFit` outcomes `analysis-required` (:843), `feature-contract-mismatch` (:877), `started-seeded` (:901) — cited
- **Also likely:** `crates/patchback-model/src/lib.rs` — inferred, the native predictor entry point from `story:native-predictor`, called and not changed
- **Also likely:** `crates/patchback-analysis/src/features.rs` — inferred, `features/1` extraction from `story:feature-contract`, called; changed only if the open question puts a contract version on the analysis (`spec/domains/fit.yaml:805`)
- **Also likely:** `crates/patchback-cli/src/analyze.rs` — inferred, analysis file version 1 carries no feature-contract version, which the mismatch check needs from somewhere
- **Also likely:** `Cargo.lock` — inferred, the new `patchback-fit` to `patchback-model` edge
- **Documents:** none required — cited: the three acceptance scenarios and the `StartFit` outcomes they exercise already exist in `spec/`; `CHANGELOG.md` and `crates/patchback-docs/src/status.rs` are coordinator-owned at merge (`.engineering/waves/a5.md`)
- **Confidence:** medium — the spec surface is cited, the code surface is inferred because `patchback fit` is a stub and the search, predictor and `features/1` extraction it builds on are unbuilt
- **Would collide with:** the fit search entry and refusal set in `crates/patchback-fit` and the `fit` verb in `crates/patchback-cli`, which is exactly what `story:bounded-search-fit` lands next wave (overlap; this story already depends on it, so it runs after). Against wave a5: none with provenance (`crates/patchback-audio`); render touches `crates/patchback-cli` (`render.rs`, likely `main.rs`) but this story reaches `main.rs` only after bounded-search-fit; feature contract (`crates/patchback-analysis`, `src/features.rs`) only if the analysis must carry a contract version — inferred
- **Safety fact:** every seeded outcome of `StartFit` (`analysis-required`, `feature-contract-mismatch`, `started-seeded`) is guarded on `defined(weight_artifact_id)` (`spec/domains/fit.yaml:846`, :878, :902), so a fit without a weight artifact still takes `started` with `init: DefaultPatch` (:931) — step 2, unproven; where the search stops: the `init` value on `FitStarted` and in the fit run files that `story:fit-run-artifacts` writes
