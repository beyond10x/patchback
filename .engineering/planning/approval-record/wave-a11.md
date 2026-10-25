---
format: aep.planning-md/3
id: approval-record:wave-a11
kind: approval-record
status: approved
title: Wave a11 approved by Timo
relations:
- decides: story:regressor-seeded-fit
revision: 6
transitions:
- {from: "draft", to: "approved", at: "2026-10-25T06:07:04Z", actor: "human:timo", revision: 2}
---
# Wave a11 approved by Timo

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
follows approval-record:release-process. Wave a10 released 0.9.0 on 2026-10-25 (e0037a7).

## The wave

- `story:regressor-seeded-fit` (`patchback-fit`, `patchback-cli`), alone: one Opus implementor,
  then one Opus adversary pass, one correction round and a second adversary pass, the
  correction answering pass 2 verified by the coordinator. `aep plan artifact waves --kind
  story --status draft` on 0.9.0 puts it alone in wave 1: `story:ml-init-benchmark` depends on
  it.
- The story's Scope section was re-derived by `aep:story-scoper` on 2026-10-25 against the a9
  unit tree and the a10 decisions (before either had merged); its 17 open points are answered
  by Da11-2 to Da11-17 below. The 2026-10-24 section in the story body is superseded by it.
- No scaffold commit (Da11-0). `wave/a11` forks from `main` at the 0.9.0 release commit.
- Merge into `wave/a11`, fast-forward `main` after the `main` runs are green (AGENTS.md
  § Releases), push, release 0.10.0.
- Not in the wave: `story:ml-init-benchmark` (depends on this one; a12), `story:note-estimation`
  (Milestone C), the studio stories (option B since a4).

## Decisions taken by the orchestrator

Wave:

- Da11-0 No scaffold commit. The unit adds the `patchback-fit` → `patchback-model` path
  dependency itself (`crates/patchback-fit/Cargo.toml`, point 17 of the scope: no cycle,
  `patchback-model` depends only on `patchback-dataset`); `Cargo.lock` changes with it and is
  committed from the unit. The workspace `Cargo.toml` stays the coordinator's.
- Da11-1 The ESS pin: wherever it stands when the wave opens (0.68.0, or 0.69.2 if the
  coordinator commit of Da10-1 landed); the unit does not touch `spec/ess-inputs.yaml` or the
  workflows.

Unit 1, `story:regressor-seeded-fit` (the open points are the scoper's, 2026-10-25, numbered
1–17 in `a11/regressor-seeded-fit-scope/open-points.md`):

- Da11-2 The CLI verb is in scope (point 8): `patchback fit` gains `--weights <path>` (a
  `weights/1` file) and `--normalization <path>` (the dataset's `normalization.stats`), each
  requiring the other (clap `requires`); neither given is today's default fit, byte for byte.
  DESIGN.md § 11 is "Proposed CLI" and the verb already carries `--notes` and `--seed` beyond
  it (point 7), so the flags are no § 11 change; DESIGN.md is not edited. For the owner: whether
  § 11 lists them.
- Da11-3 The carrier of the normalization statistics is the `--normalization` path (point 3):
  no `weights/1` header change (an a9 format change is not this story's), no file-beside-the-
  weights convention. `run.meta` ties the run to both inputs (Da11-10). For the owner: a header
  record that names the statistics (a9 open point, Da9-9).
- Da11-4 The library entry mirrors `StartFit` (points 9, 12): a `FitRequest` (or equivalent)
  carries what `fit` takes today plus `analysis: Option<&Analysis>` and `seed:
  Option<SeedInputs>` where `SeedInputs` holds a loaded `WeightArtifact` (or `Predictor`) and a
  `NormalizationTable`; `fit` stays as it is and delegates with neither, so no existing caller
  (11 test files, `crates/patchback-cli/src/benchmark.rs:368`) changes. The library takes
  loaded inputs and does no file I/O for them.
- Da11-5 The refusals, in the spec's order (`spec/domains/fit.yaml:816-901`): the search-bound
  and note-confidence checks as today; `AnalysisRequired` when `seed` is given and `analysis`
  is not (:843); the lookups do not apply (the inputs arrive by value, as `TopologyNotFound`
  is unreachable today, point 9); `FeatureContractMismatch` (:877) when the weight artifact's
  `feature_contract()` or the table's `feature_contract_version()` is not `features/1`, checked
  before `check_reference`, so it precedes `ReferenceSilent` and `PitchAmbiguous` (point 6);
  the message names the expected contract, the one found and which input carried it. A
  statistics-versus-weights mismatch is `patchback.fit.FeatureContractMismatch`, not an
  analysis error (point 6, second half). The spec's external marker on the outcome
  (`:804-807`) stays for the owner (point 1).
- Da11-6 In the CLI (point 2), the weights and the statistics are read before the fit is
  entered, after the reference: `Predictor::load` (`UnknownWeightArtifact`,
  `CorruptWeightArtifact`, `Io`) and `read_stats` errors print as `patchback.model.<Error>:
  <message>` and `patchback.analysis.<Error>: <message>` with the refusal exit code the verb
  uses for a reference that does not read. `FitError` gains no pass-through variants: `StartFit`
  declares none, and `crates/patchback-fit/src/error.rs` is an a12 collision file. For the
  owner: a `StartFit` refusal for a missing or corrupt artifact (UNMAPPED `:808-809`).
- Da11-7 The analysis (points 1, 5): the CLI analyses the reference with `AnalysisConfig::
  default()` (the `features/1` configuration) and passes it in the request; the library uses a
  given analysis for `check_reference` and feature extraction and analyses itself only when
  none is given (the default path, unchanged). The normalization is applied by the fit from the
  `NormalizationTable`: it checks the version (Da11-5) and that every normalized value is finite
  before `predict` (`NonFiniteFeature` then cannot come from the predictor, and a non-finite
  value is refused as Da11-12 says). `crates/patchback-analysis/src/normalization.rs` is not
  changed.
- Da11-8 The note span (point 4): the features are extracted over the first note of the
  sequence, `start_sample..end_sample` clamped to the reference length, through
  `extract_features(contract, &analysis, samples, span)`; a sequence with more notes is not
  refused, the prediction comes from the first. For the owner: whether a multi-note reference
  should be refused on the seeded path (provisional).
- Da11-9 The seeded start replaces restart 0 (point 12): `Prediction.values` (each `f32 as
  f64`) is restart 0's start point in `Search::restarts`; restarts 1 and later keep
  `restart_point(seed, r, len)`. `started-seeded` evaluates no default patch; the comparison
  against the default is `story:ml-init-benchmark`'s. The outputs line up with the search
  point's indices (point 13, answered).
- Da11-10 `run.meta` (point 10): `init default` stays the default path's value; the seeded path
  writes `init regressor-prediction` (the spec's `RegressorPrediction`) and three further
  records after it, `weights sha256 <checksum of the artifact>`, `modelspec <version>
  <commit>` (from the artifact header) and `normalization sha256 <digest of the statistics
  file's bytes>`. `RUN_META_FORMAT_VERSION` stays 1: the default path's bytes do not change
  and the new records appear only on the seeded path. `RunMeta` carries them as
  `Option`s. For the owner: a format bump when the records become unconditional.
- Da11-11 `report.txt` and stdout (point 11): the hard-coded `init default` on stdout
  (`crates/patchback-cli/src/fit.rs:358`) prints the run's init; `report.txt` keeps its
  format, and both gain one line `warning clamped <n> of 8 predictions` only when the
  prediction clamped (`Prediction.clamped > 0`, DESIGN.md § 5.2's warning, Da10-6); nothing is
  written when it did not. `initial_loss` is the seeded start's loss, as it is the default
  start's today.
- Da11-12 `PredictError` in the fit (point 15): `WrongFeatureCount` and `NonFiniteFeature`
  cannot occur after Da11-7 and are mapped to the fit's `NoValidCandidate` with the predictor's
  message (precedent `RendererFault`); `NonFinitePrediction` (Rust-only, Da10-7) maps the same
  way, naming the index. `FeatureContractMismatch` from the predictor cannot occur after Da11-5
  and maps to the fit's own variant if it does.
- Da11-13 Wall time (point 16, answered): the clock starts when the fit is entered, so the
  prediction counts in `wall_time_ms`; the analysis is the CLI's and counts as it does today.
- Da11-14 Tests (point 14): `crates/patchback-fit/tests/regressor_seeded_fit_scenarios.rs`
  (one test per scenario file, every step cited); the numeric obligations (the start finite and
  in [0, 1]; the bounded-search obligations unchanged from the seeded start); `tests/domain.rs`
  accounts for `AnalysisRequired` and `FeatureContractMismatch`; the CLI tests cover both flags
  given, one given (clap usage error, exit 2), a missing weights file, a mismatched contract
  (an artifact whose header says `features/2`, which `export` allows), the seeded `run.meta`
  records and the warning line, and the default path's `run.meta` and `report.txt` byte-equal
  to 0.9.0's. The `weights/1` writer helper of a10's tests is copied into the fit's tests, not
  shared through `patchback-model` (provisional; a shared test-support writer is the owner's
  call).
- Da11-15 No spec change: the three scenarios and every outcome they reach exist
  (`spec/domains/fit.yaml:842-957`). The two UNMAPPED markers (`:804-809`) stay for the owner.
- Da11-16 `lib.rs` additive in `patchback-fit` (the request type, `SeedInputs`, the entry, the
  crate doc's § The fit); `crates/patchback-fit/src/error.rs` gains the two variants in
  `DOMAIN_ERRORS`, `domain_error` and `examples`, nothing else (a12 collision file).
- Da11-17 Documents, for the coordinator from scratch notes: CHANGELOG `Unreleased`; the
  planned row "A regressor-seeded fit" ships its seeded half (the "benchmarked against" half
  stays planned for a12); `AGENTS.md` § State; `website/docs/reference/cli.md` § patchback fit
  (the two flags); `website/docs/reference/files.md` (the `init` value and the three records);
  `website/docs/guides/fit-a-recording.md` (one paragraph: the two flags, where the two files
  come from); the `fit` usage line in `crates/patchback-cli/src/main.rs` if it lists flags.

- Da11-18 (implementor deviation from Da11-16, accepted) `FitError` gains a third variant,
  `PredictionFailed { message }`, whose `domain_error()` is `patchback.fit.NoValidCandidate`: it
  carries the predictor's message as Da11-12 asks, which the unit variant `NoValidCandidate`
  and `RendererFault`'s fixed text cannot. a12 touches `error.rs` after this wave, not beside it.
- Da11-19 (implementor deviation from Da11-10, accepted) The seeded `run.meta` record for the
  training binary is `weights_modelspec <version> <commit>`, not `modelspec …`: `run.meta`
  already carries `modelspec <version>` for the pinned binary, and keys stay unique.
- Da11-20 (implementor deviation from the brief, accepted) The seeded records (`init`,
  `weights sha256`, `weights_modelspec`, `normalization sha256`) travel in
  `FitOutcome::seeded` (`SeededStart`), not in `RunMeta`, which is unchanged; the CLI computes
  the statistics file's SHA-256 over the bytes `read_stats` parsed and passes it in
  `SeedInputs::normalization_sha256`. The weights and statistics are read after `--notes` and
  before the `--out` checks; the contract check runs after the pin check and before silence.
  `MAX_STATS_BYTES` (1 MiB) bounds the statistics read (provisional).

- Da11-21 (adversary pass 1, F1) The prediction's `features/1` vector is extracted from the
  first note's own clip, not from the whole reference: `analyze_and_extract(contract,
  &reference.samples()[..tail_end], SAMPLE_RATE_HZ, span)` with `tail_end = min(span.end_sample
  + RELEASE_OFFSET_FRAMES + RELEASE_WINDOW_FRAMES, frames)`, the dataset's own call over one
  note's render (`crates/patchback-dataset/src/generate.rs:386-391`). Measured by the adversary:
  on a two-note reference the whole-clip f0 put the second note's pitch into the first note's
  vector (index 31: 1.0 against 1.0551). The given analysis still admits the reference (step 5)
  and is still required (Da11-4); it is no longer the prediction's source. The first-note
  analysis runs inside the fit's clock. The adversary's `tests/adversary_seeded_fit.rs` and
  `tests/adversary_fit_seeded.rs` stay in the unit, as the pass-2 files will.
- Da11-22 (adversary pass 1, F2) A first note whose `end_sample` is at or past the reference's
  end has an empty release window; the seeded fit refuses `patchback.fit.NoValidCandidate` with
  the extractor's message (`FeatureNotComputable`, feature 16, "the window from sample <end> to
  <end> holds no frame centre") and does not fall back to a default start the caller did not
  ask for. The clamp of Da11-8 stays. The refusal is documented (`cli.md` § `patchback fit`,
  row 1 and the seeded paragraph; the `search.rs` module doc, step 6), and the adversary's case
  `adversary_a_note_held_to_the_reference_end_is_predicted_from` is rewritten as
  `…_is_refused`: for `end_sample` 48000 and 60000 in a 48000-frame clip the seeded fit refuses
  as above and the default fit of the same inputs runs.
- Da11-23 (adversary pass 1, F3) The `wall_time_ms` and `--max-wall-time-ms` doc rows in
  `crates/patchback-cli/src/fit.rs` and the stdout paragraph in `cli.md` say that on a seeded
  run the fit's clock holds the first-note analysis and the prediction, and that the reference
  analysis the CLI makes before the fit is outside it.

- Da11-24 (adversary pass 2, F1) `fit_request` refuses a given analysis whose frame count or
  sample rate is not the reference's, before the prediction, as `FitError::PredictionFailed`
  (`patchback.fit.NoValidCandidate`) whose message names the analysis's and the reference's
  frame counts and rates. That restores, as an explicit check, what pass 1 observed through
  `extract_features`'s `ClipMismatch` before Da11-21 stopped reading the analysis. No new
  variant (Da11-16, Da11-18); the CLI cannot reach it, since it analyses its own reference; an
  unseeded request with such an analysis is not refused: the analysis is ignored and the fit
  analyses the reference itself (the implementor's call, accepted);
  the `FitRequest.analysis` doc states the obligation.
- Da11-25 (adversary pass 2, F2) The first note's clip starts at the note's start, not at the
  reference's: `analyze_and_extract` runs over `samples[start..tail_end]` with the span
  `0..end − start` (both ends clamped as before), so a note after a lead-in gets the vector the
  dataset would compute for that note (bit-equal when the lead-in is silence). Measured by the
  adversary under Da11-21's slice from 0: a lead-in of 4,096 frames changed 11 of 32 raw values,
  one of 1,000 frames 28 of 32. `tail_end` is as Da11-21 computed it.
- Da11-26 (adversary pass 2, F3) Refusal row 7 of the `crates/patchback-cli/src/fit.rs` module
  doc names the first-note analysis refusals: the held note (Da11-22), a first-note clip that
  is silent or whose pitch is ambiguous, each `patchback.fit.NoValidCandidate` with the
  analysis crate's message. The adversary's pass-2 files stay in the unit (Da11-21).

## Provisional values

| Name | Value | Where | Why provisional |
| --- | --- | --- | --- |
| note span (Da11-8, Da11-21, Da11-25) | the first note's `start_sample..end_sample`, extracted from the reference's samples from the note's start to its release window end | fit feature extraction | DESIGN.md § 9.2 names none; training used `0..gate_samples` of one note's own clip |
| init value (Da11-10) | `init regressor-prediction` | `run.meta` | files.md documents `default` only |
| warning line (Da11-11) | `warning clamped <n> of 8 predictions` | stdout, `report.txt` | DESIGN.md § 5.2 asks for a warning, names no text |
| statistics read bound (Da11-20) | `MAX_STATS_BYTES` 1 MiB | `crates/patchback-cli/src/fit.rs` | a `features/1` statistics text is about 9.5 KB; DESIGN.md names no bound |
