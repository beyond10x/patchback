---
format: aep.planning-md/3
id: approval-record:wave-a12
kind: approval-record
status: approved
title: Wave a12 approved by Timo
relations:
- decides: story:ml-init-benchmark
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-25T08:28:53Z", actor: "human:timo", revision: 2}
---
# Wave a12 approved by Timo

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
follows approval-record:release-process. Wave a11 released 0.10.0 on 2026-10-25 (ef1838a).

## The wave

- `story:ml-init-benchmark` (`patchback-fit`, `patchback-cli`), alone: one Opus implementor,
  then one Opus adversary pass, one correction round and a second adversary pass, the
  correction answering pass 2 verified by the coordinator. It is the last story of
  `epic:modelspec-regressor`; every story of `epic:sound-engine` is implemented. After the
  release both epics and `initiative:milestone-b` move to implemented in the store.
- The story's Scope section was re-derived by `aep:story-scoper` on 2026-10-25 against
  `wave/a11` at 2ef9cd0, before a11's seeded fit was in code; its 12 open points are answered
  by Da12-1 to Da12-12 below. The 2026-10-24 section is superseded (it predates
  `story:engine-baseline`'s benchmark code).
- No scaffold commit (Da12-0). `wave/a12` forks from `main` at the 0.10.0 release commit.
- Merge into `wave/a12`, fast-forward `main` after the `main` runs are green (AGENTS.md
  § Releases), push, release 0.11.0.
- Not in the wave: `story:note-estimation` and the other Milestone C–E stories, the studio
  stories (option B since a4).

## Decisions taken by the orchestrator

Wave:

- Da12-0 No scaffold commit; no `Cargo.toml` change: `patchback-cli` already depends on
  `patchback-model` and `patchback-analysis`, and `patchback-fit` on `patchback-model` since
  a11 (Da11-0). `Cargo.lock` unchanged.
- Da12-1 The ESS pin: wherever it stands when the wave opens; not the unit's.

Unit 1, `story:ml-init-benchmark` (the open points are the scoper's, 2026-10-25, numbered 1–12
in `a12/ml-init-benchmark-scope/open-points.md`):

- Da12-2 Kind selection (point 1): `patchback benchmark --kind engine-baseline|ml-init`,
  default `engine-baseline`, so every existing invocation is unchanged byte for byte;
  `--weights <path>` and `--normalization <path>` as `patchback fit` has them (Da11-2), each
  requiring the other. `--kind ml-init` without `--weights` reaches `Benchmark::start`, which
  refuses `WeightArtifactRequired` (`crates/patchback-fit/src/benchmark.rs:220-224`), so the
  scenario `ml-init-benchmark-weight-artifact-required.yaml` has a CLI path. `--weights` with
  `--kind engine-baseline` is a clap conflict (usage exit code).
- Da12-3 Inputs read once (points 6, 7): after the arguments and before the holdout is drawn,
  the CLI reads the artifact (`Predictor::load`) and the statistics (`read_stats`) and checks
  both contracts against `features/1` the way a11's fit does (through a11's public check if it
  exposes one, otherwise the two comparisons repeated); a refusal prints as `patchback fit`
  prints it (Da11-6) and no clip is rendered. `weight_artifact_id` is `sha256:<checksum hex>`
  of the artifact (the spec's `WeightArtifactId`).
- Da12-4 The four arms per clip (points 2, 3): arm 1 `default_arm` and arm 2 `fit` as today;
  arm 3, the raw prediction, renders the predicted point once through the `default_arm`
  pattern generalised to any point (`predicted_arm`), the point being a11's seeded start
  computed for the clip (a11's entry exposes it, or the CLI calls `Predictor::predict` on the
  clip's normalized `features/1` vector the way a11's fit does); arm 4 is a11's seeded fit
  (`FitRequest` with `seed`) with the clip's `fit_seed`, so restarts 1 and later are paired
  with arm 2's. A predicted point whose render is rejected refuses the clip as Da8-3 does.
- Da12-5 Records (points 4, 8, 9): kind-dependent at format version 1, as `run.meta` is
  (Da11-10). On `ml-init`, `benchmark.txt` gains `raw_regressor_median_loss`,
  `regressor_search_median_loss`, `regressor_search_median_evaluations`, `weights sha256
  <hex>`, `modelspec <version> <commit>`, `normalization sha256 <hex>`, `feature_contract
  features/1` and `clamped_predictions <n> of <clips>` (the clips whose prediction clamped,
  `Prediction.clamped > 0`; DESIGN.md § 5.2 "measured"); no `target_verdict` and no reduction
  (`fit.yaml:1528-1529`). `clips.txt` gains the raw loss, the seeded best loss, evaluations,
  restarts, stop reason and clamp count per clip on `ml-init` only. The baseline files are
  byte-identical to 0.10.0's (`crates/patchback-cli/tests/benchmark.rs` `BENCHMARK_KEYS` and
  the byte test hold unchanged). The spec is not changed (point 9: the two search arms'
  evaluations are what `RecordMlInitResult` carries).
- Da12-6 Listening examples (point 5): five files per clip on `ml-init` (`predicted.wav` and
  `seeded.wav` beside the three), three on the baseline; `EXAMPLE_FILES` becomes
  kind-dependent; `LISTENING_EXAMPLE_COUNT` stays the baseline's 3 (provisional).
- Da12-7 A holdout clip the analysis refuses is refused as Da8-3 refuses a clip (point 10).
- Da12-8 `Benchmark::record_ml_init_result(MlInitResult)` beside `record_baseline_result`,
  the order `kind-mismatch` then `wrong-state` (`fit.yaml:1564-1597`); the four ML-init fields
  set; `MlInitResult` exported from `lib.rs` (additive); `error.rs` doc comments only (every
  benchmark error exists); `tests/domain.rs` gains the `RecordMlInitResult` section;
  `BenchmarkNotFound` stays in `NOT_EMITTED_BY_THE_CRATE` (point 12, Da8-10).
- Da12-9 No numeric target and no verdict (point 11): the Milestone B exit reading is the
  owner's, from the records.
- Da12-10 Tests: `crates/patchback-fit/tests/ml_init_benchmark.rs` (one test per scenario,
  following `tests/engine_baseline.rs`), `crates/patchback-cli/tests/ml_init_benchmark.rs`
  (the CLI end to end on a synthesized holdout with a `weights/1` artifact and a
  `normalization.stats` text the test writes; the kind flag and its refusals; the records
  byte-exact; the baseline bytes unchanged), `tests/domain.rs`.
- Da12-11 `lib.rs` additive; `crates/patchback-cli/src/main.rs` synopsis, exit table and the
  verb's one-line help.
- Da12-12 Documents, for the coordinator from scratch notes: CHANGELOG `Unreleased`; the
  planned row "Default vs ML-init benchmark" (added by a11, Da11-17) ships with a CLI test as
  evidence; `website/docs/reference/cli.md` § patchback benchmark; `files.md` § A benchmark
  run; `train-the-regressor.md` and `concepts/regressor.md` (four arms, not three);
  `AGENTS.md` § State.

## Provisional values

| Name | Value | Where | Why provisional |
| --- | --- | --- | --- |
| listening examples (Da12-6) | 5 files per clip on `ml-init`, 3 on the baseline | `crates/patchback-cli/src/benchmark.rs` | DESIGN.md names no count |
| clamp record (Da12-5) | `clamped_predictions <n> of <clips>` | `benchmark.txt` | DESIGN.md § 5.2 says "measured", names no record |
