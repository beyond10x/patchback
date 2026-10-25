---
format: aep.planning-md/3
id: approval-record:wave-a12
kind: approval-record
status: approved
title: Wave a12 approved by Timo
relations:
- decides: story:ml-init-benchmark
- decides: story:docs-progress-and-demos
revision: 4
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
- Unit 2, added 2026-10-25 while unit 1 was under adversary review, at the operator's request
  in chat ("make sure we have our impl progress reflected in the public docs, also provide some
  demos there, eg synthesizing/reproduction samles, etc"): `story:docs-progress-and-demos`
  (`crates/patchback-docs`, `demos/`, `models/demo/`, `website/`), one Opus implementor, one
  Opus adversary pass, one correction round, verified by the coordinator. Scoped by
  `aep:story-scoper` on `wave/a12` at d99f9f5; its 8 open points are answered by Da12-18 to
  Da12-27. It is tooling, belongs to no epic, and ships in 0.11.0 with unit 1.

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

- Da12-13 (implementor deviation 1) `patchback_fit::predicted_arm` and `PredictedArmError` are
  public: the CLI's arm 3 renders and scores one arbitrary point through them, and `default_arm`
  is that function at the default point (`arm_at`). Additive to `lib.rs` (Da12-11).
- Da12-14 (implementor deviation 2) `read_bounded` and `read_seed_inputs` are duplicated from
  `crates/patchback-cli/src/fit.rs` into `benchmark.rs`, since `fit.rs` was not the unit's file.
  Accepted for the unit; moving them into a shared module of the CLI crate is a cleanup for a
  later unit (for the owner).
- Da12-15 (implementor deviation 3) The Da12-7 path (a holdout clip whose analysis is refused
  is refused as `clip <k>: …`) has no end-to-end test; the implementor could not synthesize
  such a clip. The adversary tries (a patch whose render is silent or unpitched); if none is
  reachable from the generator's parameter ranges, the path stays covered by code reading and
  the adversary says so as a judgement finding.
- Da12-16 (implementor deviation 5) The predicted arm's render is resized to the reference's
  length before scoring, as the other arms are; the implementor's first test assumed equal
  lengths and was corrected, not the code. The adversary checks that arm 3's loss equals
  `patchback compare` of its listening file against the reference.
- Da12-17 (the implementor's seven calls on open points) `MlInitResult` is a struct of the
  spec's eight inputs as public fields, `record_ml_init_result` returns `Result<(), FitError>`
  and the crate models no event type; `clips.txt` appends `raw_loss seeded_best_loss
  seeded_evaluations seeded_restarts seeded_stop_reason clamped` after the eight baseline
  columns; each clip clones the one read-once `SeedInputs`, checked once; the inputs are read
  before `Benchmark::start` (the start needs `weight_artifact_id`), the arms run 1, 2, 4, 3;
  the head records `weights`, `modelspec`, `normalization`, `feature_contract` follow `prng`;
  no zero-median refusal on `ml-init` (no reduction is computed); `--weights` or
  `--normalization` with the engine baseline exits 2 through clap's `ArgumentConflict` error
  raised by hand. Deviation 4 (the `files-doc.md` clip line shows clip 0 only) is a note for the
  coordinator's documents.

Unit 2, `story:docs-progress-and-demos` (the open points are the scoper's, 2026-10-25, numbered
1–8 in `a12/docs-demos/scoper-report.md`):

- Da12-18 Tooling acceptance (point 1): the documentation crate is tooling (AGENTS.md) with no
  ESS domain, so the story names no scenario. `task acceptance-check` accepts a story without a
  scenario when its body carries a line starting `Tooling acceptance: ` that names the Rust
  test files under `crates/patchback-docs/` holding its tests, and fails when one of those
  files does not exist. Every other story still needs a scenario. The coordinator's change, in
  the plan commit that adds the story.
- Da12-19 Release per story (point 2): the newest CHANGELOG release whose section, with its
  lines joined, names `(story <slug>)`, `(stories … <slug> …)` or `story:<slug>`; the scoper's
  unjoined search missed four wrapped mentions (`CHANGELOG.md:402`, `:469`, `:485`, `:496`).
  A story the CHANGELOG never names (six remain: wav-io, parameter-contracts,
  workspace-scaffold, cli-clap, dependency-gate, modelspec-pin-and-check) shows the first
  release dated on or after the day of its `implemented` transition, marked "by date" on the
  page. Generation never fails on it.
- Da12-20 Grouping (point 3): a story with no `decomposes` edge is listed under "Outside the
  milestones", below the milestones.
- Da12-21 Retraining (point 4): `patchback-docs demos train --modelspec <binary> --dataset
  <dir> --out <dir>` in the docs crate, running `patchback_model::TrainingRun` with the
  provenance's seed, sizes and steps; the generator never downloads ModelSpec. `PROVENANCE`
  records whether a retrain reproduced the checksum: "not verified" until someone runs it. The
  committed weights are the coordinator's 2026-10-25 demo run's, trained by a probe whose
  source is handed to the implementor.
- Da12-22 Voice count (point 5): four voices (the pluck, pad and bass of the 2026-10-25 demo
  plus one lead), one note each, under 2 s per file; the implementor measures `demos fit` in
  release mode and drops to three if the total passes 4 minutes. Fits use
  `FitConfig::default()` with `max_wall_time_ms` 600,000, so no fit stops on wall time and
  `demos.json` does not depend on the machine.
- Da12-23 Time budget (point 6): the 30 s obligation counts run time, compilation apart; the
  render step runs in debug in `pages.yml` and `task docs-generate`; the implementor measures
  and reports run time and compile time.
- Da12-24 The fourth take (point 7): the clamped prediction (`SeededStart::point()`) rendered,
  labelled "prediction", with the clamp count shown beside it.
- Da12-25 PCM16 range (point 8): demo voices are chosen so every take's render stays within
  [-1, 1]; `demo_audio_is_pcm16_48k_mono_under_two_seconds` asserts it; a voice that clips is
  replaced, not normalized.
- Da12-26 Layout (the scoper's): committed inputs `demos/<voice>/voice.patch` and
  `note.events`; committed model `models/demo/regressor.weights`, `normalization.stats` and
  `PROVENANCE`; committed numbers `website/data/demos.json`, written only by `demos fit`;
  generated pages `website/docs/progress.mdx` and `website/docs/listen.mdx`; audio rendered at
  build time into git-ignored `website/demo-audio/<voice>/{recording,default-fit,seeded-fit,prediction}.wav`,
  referenced with `require('@site/demo-audio/…')` so a missing file fails the site build. The
  pages carry titles and milestone names, no planning ids (`PRIVATE` in
  `crates/patchback-docs/src/main.rs`).
- Da12-27 Fork point: unit 2's tree forks from the plan commit that adds the story and the
  acceptance exemption, not from unit 1's merge. Unit 2 edits `Taskfile.yml` (`docs-generate`,
  new `docs-demos`); the coordinator edited `acceptance-check` in that commit; `Cargo.lock`
  gains the `patchback-docs` stanza only. The README, AGENTS.md and CHANGELOG lines are the
  coordinator's from the implementor's notes (as Da12-12).

## Provisional values

| Name | Value | Where | Why provisional |
| --- | --- | --- | --- |
| listening examples (Da12-6) | 5 files per clip on `ml-init`, 3 on the baseline | `crates/patchback-cli/src/benchmark.rs` | DESIGN.md names no count |
| clamp record (Da12-5) | `clamped_predictions <n> of <clips>` | `benchmark.txt` | DESIGN.md § 5.2 says "measured", names no record |
| demo voices (Da12-22) | 4, one note each, under 2 s per file | `demos/` | the operator asked for "some demos" and named no count |
| demo audio format (Da12-26) | 16-bit PCM WAV, 48 kHz, mono | `website/demo-audio/` | a browser plays it; the engine writes float32 |
