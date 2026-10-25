---
format: aep.planning-md/3
id: approval-record:wave-a10
kind: approval-record
status: approved
title: 'Wave a10 approved: the native predictor'
relations:
- decides: story:native-predictor
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-25T05:03:05Z", actor: "human:timo", revision: 2}
---
# Wave a10 approved by Timo

## Who approved, in their words

Timo, 2026-10-24, in chat to this session, which he named the orchestrator for this repository:

> continue to work on patchback, you are the orchestrator and start up to 3 opus sub-agents for
> implementations, check status, use /planning

and, the same day, setting the session's goal:

> Goal set: milestone b reached

The orchestrator acts under the grant in the operator's workspace instructions (class C: a wave;
commits, pushes and merges through the bot; a release through the repository's own process; the
ESS pin). The goal keeps waves running until `epic:sound-engine` and `epic:modelspec-regressor`
are implemented and released (`initiative:milestone-b`). The release after the merged wave
follows approval-record:release-process. Wave a9 released 0.8.0 on 2026-10-25 (824d90a).

## The wave

- `story:native-predictor` (`patchback-model`), alone: one Opus implementor, then one Opus
  adversary pass, one correction round and a second adversary pass, the correction answering
  pass 2 verified by the coordinator. `aep plan artifact waves --kind story --status draft` on
  0.8.0 puts it alone in wave 1: `story:regressor-seeded-fit` depends on it, and
  `story:ml-init-benchmark` on that one.
- The story's Scope section was re-derived by `aep:story-scoper` on 0.7.0 (081ddc1), before
  wave a9 defined the `weights/1` file and `WeightArtifact`. The 13 open points under it are
  answered by Da10-2 to Da10-15 below; the file format they waited for is Da9-2, Da9-3 and
  Da9-9 (`approval-record:wave-a9`), shipped in 0.8.0.
- No scaffold commit (Da10-0). `wave/a10` forks from `main` at the 0.8.0 release commit.
- Merge into `wave/a10`, fast-forward `main` after the `main` runs are green (AGENTS.md
  § Releases), push, release 0.9.0.
- Not in the wave: `story:regressor-seeded-fit`, `story:ml-init-benchmark` (each depends on the
  one before it), `story:note-estimation` (Milestone C), the studio stories (option B since a4).

## Decisions taken by the orchestrator

Wave:

- Da10-0 No scaffold commit: `patchback-model` already depends on `patchback-analysis` and
  `patchback-core` (Da9-0), and the predictor needs no other crate. `Cargo.toml` and
  `Cargo.lock` stay the coordinator's.
- Da10-1 The ESS pin moves to 0.69.2 in a coordinator commit on `wave/a10` once the release is
  verified on patchback (470/470 locally); until then it stays at 0.68.0 (Da9-1). Either way it
  is not the unit's.

Unit 1, `story:native-predictor` (the open points are the scoper's, 2026-10-25, numbered 1–13 in
its report; the story body carries the 2026-10-24 section):

- Da10-2 Library only (points 13): `patchback_model::Predictor` and `patchback_model::Prediction`
  in a new `crates/patchback-model/src/predict.rs`; no `patchback` verb (DESIGN.md § 11 lists
  none; `TrainingRun` and `WeightArtifact` stayed library-only).
- Da10-3 The input is a `&[f64]` of normalized `features/1` values, the output of
  `NormalizationStats::apply` (points 3, 4): the caller normalizes, the predictor reads no
  `normalization.stats`. The predictor casts each value with `as f32`, the cast
  `patchback-dataset` applies when it writes `x.f32` (`crates/patchback-dataset/src/dense.rs`),
  so inference sees the bits training saw.
- Da10-4 Two calls, and the refusals in this order (points 9, 10, 11). `Predictor::load(path)`:
  a file that does not exist (`io::ErrorKind::NotFound`) is `UnknownWeightArtifact { path }`;
  another I/O failure is `PredictError::Io`; `ReadError::Corrupt` from `WeightArtifact::read` is
  `CorruptWeightArtifact { reason }`. `Predictor::predict(&mut self, feature_contract: &str,
  x: &[f64])`: `WrongFeatureCount { count }` unless `x.len() == 32`;
  `FeatureContractMismatch { artifact, input }` unless the string equals
  `artifact.feature_contract()` (Da9-9: a version string such as `features/1`, the comparison
  `NormalizationStats::apply` makes); `NonFiniteFeature { index }` when a value is not finite as
  `f64` or after the cast to `f32`; then the forward pass. Deviation from the spec's outcome
  order (feature-contract-mismatch before corrupt-artifact): a corrupt artifact never loads, so
  the comparison cannot run on it. Each scenario step exercises one refusal on its own input, so
  the scenario tests hold; the deviation is recorded in the story body.
- Da10-5 Arithmetic in `f32` throughout (point 5): `h[j] = max(0, Σ_i x[i]·w1[i·64 + j])` for
  `j` in 0..64, `i` ascending, sequential accumulation from `0.0f32`; `y[k] = Σ_j h[j]·w2[j·8 +
  k]` for `k` in 0..8, `j` ascending; no bias (DESIGN.md § 8.1, `[32, 64]` ReLU `[64, 8]`).
  Row-major as exported (Da9-3: the order `param` printed; ModelSpec's `--init` files are
  row-major little-endian). Da10-9's equivalence test proves the layout against the pinned
  binary; no other arithmetic is tuned to match it.
- Da10-6 `Prediction { raw: [f32; 8], values: [f32; 8], clamped: usize }` (point 8):
  `values[k] = raw[k].clamp(0.0, 1.0)`; `clamped` counts the outputs with `raw[k] < 0.0 ||
  raw[k] > 1.0`, `±inf` included. `Predictor::clamped_predictions() -> u64` counts the
  predictions with `clamped > 0`, the spec's `clamped_predictions {increment: 1}` per
  `predicted-clamped` outcome, in memory per `Predictor`, never written to the file (Da9-13). The
  warning is `clamped > 0` for the caller; the library prints nothing.
- Da10-7 A NaN in `raw` (reachable only through `inf − inf` in the accumulation, with weights
  near `f32::MAX`) is refused as `PredictError::NonFinitePrediction { index }` (point 12):
  Rust-only, `domain_error()` is `None`, accounted in `tests/domain.rs` the way
  `ExportError::Unwritable` is. `±inf` is clamped and counted (Da10-6). One test with crafted
  weights shows both.
- Da10-8 Determinism (§ 12.1): the same `Predictor` twice, and two `Predictor`s loaded from the
  same file, give bitwise-equal `raw` over 16 seeded inputs.
- Da10-9 Golden and equivalence tests (points 6, 7). Goldens on artifacts the test builds from
  chosen weights through a test helper that writes `weights/1` bytes (header, SHA-256 from
  `patchback_core::digest`, payload; the reader verifies them), so no training run is needed:
  three hand-computed cases with exact `f32` results (a one-hot path, a negative pre-activation
  cut by ReLU, an output of 1.5 clamped and counted), and 64 seeded inputs against an `f64`
  reference computed in another loop order (`k`-major), `|raw − ref| ≤ 1e-4 · max(1, |ref|)`.
  Equivalence in the pinned lane (`tests/pinned_predict.rs`): the `pinned_export.rs` setup with
  `--eval` over the 16-row eval split; export; `load`; predict every eval row of `x.f32`
  (`f32 as f64` is exact); the mean of the squared error of `raw` against `y.f32` over the 16 × 8
  values compared with `run.eval_loss()`, relative difference ≤ 1e-3. If ModelSpec's `eval loss`
  is defined otherwise (sum over outputs then mean over rows; the loss before the last step),
  the implementor tries those readings, asserts the one that matches and reports which; if none
  matches within 1e-2, the test asserts finiteness only, the report quotes the numbers, and the
  point goes to the owner. This is the only ModelSpec equivalence available: 0.5.0's `infer` has
  no input flag, and DESIGN.md § 1.1 excludes checkpoint use (Da9-15).
- Da10-10 `WrongFeatureCount` is checked against `patchback_analysis::FEATURES_1_VALUE_COUNT`
  (32), with a compile-time assertion that it equals `W1_SHAPE[0]`.
- Da10-11 No spec change: `Predict`, both scenarios, the errors, `PredictionClamped` and
  `clamped_predictions` exist (`spec/domains/model.yaml`). The unit does not edit
  `weights.rs`; a reader change it needs goes to `<scratch>/needs-coordinator.md`.
- Da10-12 Tests: `tests/predictor_scenarios.rs` (one test per scenario file, every step cited;
  the corrupt step flips one payload byte of a written artifact, the unknown step names a path
  that does not exist), the goldens, the reference check, the clamp, `inf` and NaN cases and
  determinism as unit tests in `predict.rs` or `tests/predictor.rs`, `tests/pinned_predict.rs`,
  and `tests/domain.rs` accounting of `Predict`.
- Da10-13 Documents, for the coordinator from scratch notes: CHANGELOG `Unreleased`; one
  sentence on step 4 of `website/docs/concepts/regressor.md` § From training to prediction; a
  new § Predict with the weights in `website/docs/guides/train-the-regressor.md` after § Hand
  the weights to Patchback; the status rows (the planned "A native predictor" row ships);
  AGENTS.md § State. No `files.md` change: the predictor writes no file.
- Da10-14 `lib.rs` additive: the module line, re-exports, the crate doc's item 8.
- Da10-15 No performance requirement; the report notes the wall time of one prediction
  (microseconds, debug and release are both fine, say which) for `story:regressor-seeded-fit`'s
  search budget.

## Provisional values

| Name | Value | Where | Why provisional |
| --- | --- | --- | --- |
| reference tolerance (Da10-9) | `1e-4 · max(1, abs(ref))` | predictor tests | DESIGN.md § 8.2 names none |
| equivalence tolerance (Da10-9) | relative `1e-3` against `eval loss` | `tests/pinned_predict.rs` | DESIGN.md § 12.2 names none |
