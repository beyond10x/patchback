---
format: aep.planning-md/3
id: approval-record:wave-a12
kind: approval-record
status: approved
title: Wave a12 approved by Timo
relations:
- decides: story:ml-init-benchmark
- decides: story:docs-progress-and-demos
revision: 14
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

Unit 1, after adversary pass 1 (green, 0 red, 2 notes; review-result:ml-init-benchmark-adversary-1):

- Da12-28 (pass 1, note 1, closes Da12-15) The Da12-7 path stays as a guard with no end-to-end
  test: the generator admits only clips whose analysis under the same configuration
  (`contract.analysis_config()`, equal to `AnalysisConfig::default()`) succeeded, so no
  generated holdout reaches the refusal (40 of 40 clips over 5 seeds, the adversary's case
  `no_admitted_holdout_clip_reaches_the_analysis_or_seeded_fit_refusal`). The implementor says
  so in the module doc of `crates/patchback-cli/src/benchmark.rs` (correction 1, doc only).
- Da12-29 (pass 1, note 2) The adversary's case
  `the_seeded_arm_runs_with_the_clips_fit_seed_and_pairs_restart_1` stays in the suite as the
  only observation of arm 4's fit seed; the unit's own tests keep `--max-restarts 1` for wall
  time. Pass 2 runs on the corrected tree against what pass 1 did not list.

Unit 2, after the implementor's report (green, 19 → 33 cases, red 14 first):

- Da12-30 (implementor deviations) The staged voices were re-tuned to stay within the PCM16
  range and under 2 s: pluck output gain -8 dB (was -4), bass -9 dB (was -3), pad release 0.9 s
  (was 1.2) with its note ending at frame 33,600 (was 48,000); at the old gains the limiter
  clipped pluck on 9 samples and bass on 343. The lead voice is new (drive 8, cutoff 2,400 Hz,
  Q 4.5, ADSR 0.02/0.25/0.7/0.25, output gain -9 dB, MIDI 69). Accepted: Da12-25 forbids
  normalizing a rendered take, and choosing a demo voice's parameters is designing the voice.
- Da12-31 (extends Da12-19) An implemented story with no release dated on or after its
  implemented day shows "Unreleased". The progress page's sections are the milestone epics,
  then Patch studio, then "Outside the milestones"; rows are ordered by status, then
  implementation date; count lines read "N stories (…)".
- Da12-32 (the implementor's calls) `website/data/demos.json` is `patchback-demos/1` (fit
  config, generator, model, parameters, patchback version, topology; per voice the inputs with
  SHA-256, the note, the recording, the voice's parameters, both fits with start and best loss,
  evaluations, restarts, stop reason and parameters, the prediction with its clamp count);
  `models/demo/PROVENANCE` is `patchback-demo-provenance 1` key-value lines; the eight fits run
  on parallel threads; serde_json's `float_roundtrip` feature is on (no new crate); plain
  `generate`, not only `--check`, refuses a `demos.json` made from other inputs.
- Da12-33 `demos train` never ran and has no test (no ModelSpec binary on this machine, never
  downloaded); `PROVENANCE` says `retrain_reproduces_checksum not-verified`. Verifying it is the
  owner's, with a binary.
- Da12-34 (coupling) Every commit that moves a story's status or adds a CHANGELOG release
  heading regenerates `website/docs/progress.mdx` (`task docs-generate`), or `generate --check`
  fails `task check`. The wave's close and release commits do so; AGENTS.md § Commands and
  § Releases say so.
- Da12-35 The implementor ran `python3 --version` once to check whether Python was installed;
  no code ran and no file was written. Recorded as a rule deviation.

Unit 1, after adversary pass 2 (review-result:ml-init-benchmark-adversary-2; green, 402 → 411,
0 red):

- Da12-36 (correction 2, by the coordinator) The module doc of `crates/patchback-cli/src/
  benchmark.rs` claimed `wall_time_ms` is the only record that differs between two runs of the
  same flags; under a binding `--max-wall-time-ms` a clip's evaluations, losses, `fit.wav` and
  `seeded.wav` differ too (pre-existing, base `benchmark.rs:92-93`). The sentence now says so.
  No code changed, so no third pass. The three findings on the unit's own tests (a usage test
  that passes on the base binary, a modelspec record test that cannot see a mutant writing the
  pin, an `is_err()`-only refusal test) are covered by the pass-2 cases, which are committed
  with the unit; the unit's tests stay as written.
- Da12-37 (the wave's close) `epic:modelspec-regressor` (draft → proposed → active →
  implemented) and `initiative:milestone-b` (active → implemented) move in the a12 close
  commit, before the release commit, once every story of the epic is implemented and the wave
  gate is green; the release that follows delivers the last two stories. Earlier plan: after
  the release. Taken by the coordinator under the grant: one commit and one CI cycle fewer, and
  `progress.mdx` regenerates once.

Unit 2, after adversary pass 1 (review-result:docs-progress-and-demos-adversary-1; red 2 of 12,
both INFEASIBLE in reach):

- Da12-38 (correction 1) All four findings are corrected: `demos fit` records a SHA-256 of
  each take's rendered PCM16 bytes in `website/data/demos.json`, and `demos render` and
  `generate` refuse a mismatch naming the take and `task docs-demos` (the audio is bound to
  the page, not only to the inputs); CHANGELOG heading dates are compared as year, month, day
  numbers and an unreadable date is refused; the provenance test asserts the recorded
  literals; a relative `demos fit --out` resolves against `--root`. The format stays
  `patchback-demos/1`: it is unreleased. `task docs-demos` rewrites `demos.json` and
  `listen.mdx` with the digests; the fit numbers must not change.
- Da12-39 All 12 adversary cases stay in the suite (about 49 s more in the docs lane). The
  adversary offered to keep only the two red ones; the ten drift, release-rule and page
  cases are what would catch the next regression.
- Da12-40 (correction 1, the implementor's calls, accepted) The take digest is the SHA-256 of
  the WAV `data` chunk (the PCM16 sample bytes), recorded per voice under `pcm16Sha256` keyed
  by take file name; `listen.mdx` ends with one 16-row table of them. `demos render --out`
  resolves against `--root` as `demos fit --out` does; `demos train`'s paths stay relative to
  the working directory (they are not repository paths). A story whose transition time does
  not read as a date shows "—", as a missing transition does. `demos.json` grew from 14,704 to
  16,268 bytes; no fit number changed (0 lines removed, 24 added).

Unit 2, after adversary pass 2 (review-result:docs-progress-and-demos-adversary-2; red 3 of 7,
all INFEASIBLE in reach, severity note):

- Da12-41 (correction 2) The three red cases are corrected: `render_takes` recomputes the
  physical values from each recorded vector and the voice's from `voice.patch` and refuses a
  mismatch by voice and field; a versioned CHANGELOG heading whose date does not read as
  `YYYY-MM-DD` (missing, empty or malformed) is refused by version, `[Unreleased]` being the
  only undated heading; `read_day` requires a 4-digit year, 2-digit month and 2-digit day. The
  judgement finding on `main.rs:718` needs no change: the adversary's binary-level case covers
  `render --out`, and a binary-level `demos fit --out` case would need a fit. An extra key
  under `pcm16Sha256` is ignored and stays so. No third adversary pass: the correction
  recomputes and compares, and changes no recorded value; the coordinator's verification runs
  the whole docs lane, both adversary files included.
- Da12-42 (coordinator, the landing page) The site's home page
  (`website/src/theme/ProductLandingPage.tsx`) still carried its first-commit status copy:
  "Specified, not built yet", "there is no code yet", the pill "In specification · first
  milestone has no ML", Milestone A marked "first". Found by the operator's docs-site review
  of 2026-10-25 (the local build of the unit-2 tree) and by reading the page. Rewritten in the
  wave's docs commit: Milestones A and B built, phrases next, links to Progress and Listen, A
  and B marked done and C next (`data-done` styled in `landing.module.css`). Hand-written copy
  about implementation status on the home page is the coordinator's at every release, until a
  story generates it from the store as `progress.mdx` is.
- Da12-43 (correction 2, the implementor's deviation, accepted) A CHANGELOG heading's date
  needs a four-digit year; its month and day take one or two digits, because pass 1's case
  `adv_release_dates_compare_as_dates_not_strings` writes `2026-9-5` and expects `generate` to
  succeed, and an adversary case is never weakened. `26-10-30`, `02026-10-30`, `2026-010-30`
  and `2026-10-030` are refused by version. The digest check runs before the physical-value
  check, so a shifted vector is reported as the take whose samples changed and an edited
  `physical` value as the field. `voice.normalized` is not re-derived; the page shows only the
  physical values. Correction 2 green: unit tests 37, pass-1 file 12, pass-2 file 7; no fit
  rerun; `demos.json` unchanged at 16,268 bytes.
- Da12-44 (coordinator, the site review's content findings) The docs-site review of
  2026-10-25 (five haiku screeners on the local build of the unit-2 tree) confirmed six content
  defects in hand-written pages: three `patchback fit` examples without the required `--notes`
  (exit 2), "five files" for a run that writes six, a `confidence` record `report.txt` does not
  hold, analysis described as estimating onset and gate, the regressor named as the start
  without `--weights`, and "the standard library only" beside `clap`. Fixed in the wave's docs
  commit on `index.md`, `getting-started.md` and `guides/fit-a-recording.md`, with a Fixed
  section in the CHANGELOG, before the review's report reached the operator: the skill hands
  the owner its fix list only after the operator has seen the report, but a release whose
  Getting started example exits 2 is worse than an early fix, and the report keeps its
  screenshots. The visual and template findings (prose links without underline, landing-page
  small text under 4.5:1, the OSC caption overhang, the ESS graph pan boxes, the 404 page)
  wait for the operator's look and the fix list.

## Provisional values

| Name | Value | Where | Why provisional |
| --- | --- | --- | --- |
| listening examples (Da12-6) | 5 files per clip on `ml-init`, 3 on the baseline | `crates/patchback-cli/src/benchmark.rs` | DESIGN.md names no count |
| clamp record (Da12-5) | `clamped_predictions <n> of <clips>` | `benchmark.txt` | DESIGN.md § 5.2 says "measured", names no record |
| demo voices (Da12-22) | 4, one note each, under 2 s per file | `demos/` | the operator asked for "some demos" and named no count |
| demo audio format (Da12-26) | 16-bit PCM WAV, 48 kHz, mono | `website/demo-audio/` | a browser plays it; the engine writes float32 |
| demo fit wall bound (Da12-22) | `MAX_WALL_TIME_MS` 600,000 ms | `crates/patchback-docs/src/demos.rs` | the slowest fit took 98.2 s on a loaded machine; no fit may stop on wall time |
| demo data bound (Da12-26) | `MAX_DATA_BYTES` 65,536 | `crates/patchback-docs/src/demos.rs` | the story's obligation; actual 16,268 bytes |
| demo voices (Da12-30) | pluck (gain -8 dB), pad (release 0.9 s, note end 33,600), bass (gain -9 dB), lead (new) | `demos/*/voice.patch`, `note.events` | tuned so no take clips or passes 2 s |
