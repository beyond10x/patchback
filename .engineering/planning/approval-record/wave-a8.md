---
format: aep.planning-md/3
id: approval-record:wave-a8
kind: approval-record
status: approved
title: Wave a8 approved by Timo
relations:
- decides: story:engine-baseline
- decides: story:normalization-stats
- decides: story:training-run
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-25T01:21:55Z", actor: "human:timo", revision: 2, correlation: "wave-a8"}
---
# Wave a8 approved by Timo

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
follows approval-record:release-process. Wave a7 released 0.6.0 on 2026-10-25.

## The wave

- `story:engine-baseline` (`patchback-fit`, a `benchmark` verb in `patchback-cli`),
  `story:normalization-stats` (`patchback-analysis`, the `generate-dataset` verb) and
  `story:training-run` (`patchback-model`), in parallel, one Opus implementor each, then one
  Opus adversary pass, one correction round and a second adversary pass each, the correction
  answering pass 2 verified by the coordinator. `aep plan artifact waves --kind story --status
  draft` on 0.6.0 puts these three alone in wave 1 (7 draft stories, 5 waves, 28 collisions,
  none inside wave 1).
- Two `aep:story-scoper` runs re-derived the Scope sections of `story:normalization-stats` and
  `story:training-run` on 0.6.0 before selection (their 2026-10-24 sections predated the a6 and
  a7 merges); `story:engine-baseline` was scoped in a7. Each story's body carries its section.
- A coordinator scaffold commit opens `wave/a8` before dispatch (Da8-0), so the three
  implementors share no file and no `Cargo.toml`.
- Merge into `wave/a8`, fast-forward `main`, push, release 0.7.0.
- Not in the wave: `story:weight-export`, `story:native-predictor`, `story:regressor-seeded-fit`
  and `story:ml-init-benchmark` (each depends on a story of this wave or collides with one),
  `story:note-estimation` (Milestone C), the studio stories (option B since a4).

## Decisions taken by the orchestrator

- Da8-0 The coordinator writes the scaffold commit: `patchback_core::split::SplitRole`, moved
  from `crates/patchback-dataset/src/model.rs` and re-exported there at its old paths, so that
  `patchback-analysis` (which `patchback-dataset` depends on) can name the role of the split
  its statistics come from; the `patchback-core` path dependency of `patchback-analysis`; the
  `patchback-dataset` path dependency of `patchback-model`, so a training run can read a
  `DatasetSplit` and its dense files (no cycle: the dataset crate takes analysis, audio, core
  and dsp, none of which takes model); `Cargo.lock`. No behaviour changes; the four crates'
  suites and the dependency gate run on it.

Unit 1, `story:engine-baseline`:

- Da8-1 The `Benchmark` model and the medians live in `patchback-fit` (`benchmark.rs`); the verb
  that draws the holdout, runs the fits and writes the files is `patchback benchmark` in
  `patchback-cli` (`benchmark.rs`), which already depends on `patchback-dataset` and
  `patchback-fit`; `patchback-fit` does not take `patchback-dataset`. The verb's `Command` arm
  and dispatch line in `main.rs` are the unit's; the exit table is the coordinator's.
- Da8-2 The holdout: `SplitMix64::new(seed)` yields, per clip in order, a patch seed and then a
  fit seed; clip k is `PatchDraw::from_seed(patch_seed)` rendered with the run's render config
  under the generator's draw rule (a diagnostic draw is redrawn within the generator's bound).
  `--examples` is required (`example_count`, no provisional constant), `--seed` defaults to 0,
  `--topology` to `subtractive-mono/1`, the fit bounds and `--block-size` are the `fit` verb's
  flags and defaults, applied to every clip's fit.
- Da8-3 A clip the fit refuses, a run that fails (`NoValidCandidate`), a holdout that cannot
  be drawn within the bound, or a default median of 0 fail the benchmark run as a whole: exit
  1, the clip index and the error on stderr, every file the run wrote removed. The spec has no
  skipped count, and the holdout is Patchback's own synthesis, so a refusal is a defect, not
  data.
- Da8-4 The median is the lower median (the element at index (n − 1) / 2 of the sorted values).
  Reduction = (default median − search median) / default median × 100; `Met` at `>= 20`.
- Da8-5 The default arm's per-clip loss is `compare` of the default patch's render, computed by
  the verb, not read from the fit's log (`FitRun::initial_loss` is the first *valid*
  candidate's loss and differs when the default render is rejected). One test pins that it
  equals evaluation 1's `total` when that evaluation is valid.
- Da8-6 Topologies by name (`known_topology`); `TopologyNotFound` carries the name, as the fit
  does. The spec's `TopologyId` Uuid is not modelled in Rust (a6, a7 precedent).
- Da8-7 Listening examples: the first three clips (`LISTENING_EXAMPLE_COUNT = 3`, provisional),
  as `<out>/examples/<k>/reference.wav`, `default.wav`, `fit.wav`, paths relative to `<out>`.
- Da8-8 Two files, line-oriented and versioned like `run.meta`: `benchmark.txt`
  (`patchback-benchmark 1`: version, kind, topology, seed, example count, sample rate, block
  size, the four bounds, prng, the two medians, the reduction, the verdict, the median
  evaluations, wall time, the listening examples) and `clips.txt` (`patchback-benchmark-clips
  1`: one line per clip with its seeds, both losses, evaluations, restarts, stop reason). The
  `--out` rule of `fit`: any of them or `examples/` existing is refused before any work; a
  failed write removes what the run wrote.
- Da8-9 "Fixed build and device" (§ 12.2) is recorded as the version, sample rate, block size
  and bounds; no device is recorded, the code has none. For the owner.
- Da8-10 `Benchmark` is a value like `FitRun`; `BenchmarkNotFound` has no library counterpart
  (`NOT_EMITTED_BY_THE_CRATE`, as `FitRunNotFound`); the `Benchmarks` view is a function of the
  value. The five benchmark errors are `FitError` variants.
- Da8-11 Both verdicts complete the benchmark and exit 0; `RecordMlInitResult` is
  `story:ml-init-benchmark`'s.

Unit 2, `story:normalization-stats`:

- Da8-12 `ComputeNormalizationStats` is `patchback_analysis::normalization` taking the split as
  an input, `Option<SplitInput { split_id, role: SplitRole, rows }>`, `None` for a split the
  caller does not find, so the declared order holds: `DatasetSplitNotFound`,
  `FeatureContractNotFound`, `StatsFromNonTrainingSplit`, `FeatureContractNotPublished`,
  `FeatureVersionMismatch`, computed. The dataset side composes it: `DatasetStore::split` and
  `samples_in` build the input; the three scenario tests live under
  `crates/patchback-dataset/tests/` because every scenario opens with `GenerateDataset`.
- Da8-13 The rows are the split's `Sample.x` values as the store holds them (`f64`), the split
  in any state; mean and standard deviation per feature in `f64`, two passes, population
  standard deviation (divisor n). The reference computation of the test is an independent
  loop over the same rows.
- Da8-14 A feature whose standard deviation is 0 is stored with 0 and applied with divisor 1
  (the scaler convention), so the output stays finite (invariant 4); a split of 0 rows gives
  mean 0 and standard deviation 0 per feature (unreachable through the generator:
  `SplitCounts::from_samples` gives the training split at least one row for any admitted
  count; cite it). Provisional, for the owner with open points 8 and 9.
- Da8-15 The artifact is `<out>/normalization.stats` (`patchback-normalization-stats 1`):
  `feature_contract features/1`, `split train`, `rows <n>`, `std population`, then one
  `feature <i> <mean> <std>` line per feature in index order; reals as Rust prints an `f64`;
  `write_stats` and `read_stats` round-trip it; the reader refuses an unknown version, a
  malformed number, a missing or repeated feature.
- Da8-16 `patchback generate-dataset` computes the statistics of its training split under the
  dataset's contract after the dense files and writes `normalization.stats` before
  `dataset.meta`; the file joins the `--out` refusal list and the cleanup; `dataset.meta` is
  unchanged.
- Da8-17 Nothing compares the statistics' contract with the dataset's: the generator passes
  its own contract, and the library checks only what the spec declares. Open point 11 stays
  with the owner.
- Da8-18 `ApplyNormalization` is `NormalizationStats::apply(&[f64; 32]) -> Result<[f64; 32],
  _>` after the declared gates (`NormalizationContractMismatch`, `FeatureVersionMismatch`); a
  non-finite input is refused through the contract's own check and its declared error, so no
  non-finite value leaves the function.
- Da8-19 The spec is not changed in this wave: the `NormalizationStats` entity keeps no means
  or standard deviations field (open point 5), OQ-18 (validation refused as well as test) stays
  as modelled, and no `UNMAPPED` is settled. For the spec owner.

Unit 3, `story:training-run`:

- Da8-20 No `patchback` training verb in this story: the CLI reference says training uses
  ModelSpec's own CLI, the acceptance names no verb, and a verb would collide with unit 1 on
  `main.rs`. The library exposes the run; a verb is a later story's.
- Da8-21 `TrainingRun` is a value with the four commands as methods in the spec's order
  (`plan`, `bind`, `start`, `finish`), each answering its outcomes in the declared order.
  `UnknownDataSplit` is real (`DatasetStore::split` returns nothing); `UnknownPin`,
  `UnknownModelSource` and `UnknownTrainingRun` have no library counterpart (values, not
  records; the precedent is story:modelspec-pin-and-check's mapping).
- Da8-22 `Backend::Vulkan` is refused `BackendNotShipped` before anything is spawned; the pin
  file gains no `vulkan_backend_shipped` key (nobody has verified the 0.5.0 asset's features).
  Provisional, for the owner.
- Da8-23 No split-role guard: the spec marks it `UNMAPPED` and declares no error for it; the run
  records the roles it was given. Open point 1 stays with the owner.
- Da8-24 `start` takes the step count and passes `--steps`; the pinned binary's refusal of a
  run whose steps × batch size exceeds the training rows (E-DATA-008, read from ModelSpec
  0.5.0's source, unproven) is not pre-empted in Rust: it ends the run `Failed` with the
  child's stderr, and the pinned-binary test proves it with one such case against the real
  binary. The guide's and DESIGN.md's `modelspec train` command, which would exit 1 on the
  50,016-row split per that reading, is the owner's (open point 5).
- Da8-25 The loss curve is parsed from the child's stdout (`step <k> loss <v>` lines, the
  `eval loss=<v> accuracy=<v>` line) into the value; the raw stdout and stderr stay on it for
  story:weight-export; no file is written (the spec models no loss field; open point 6).
- Da8-26 `start` takes a timeout; `DEFAULT_TRAINING_TIMEOUT` is 600 s (provisional; the time of
  a 5000-step CPU run is unmeasured; the pinned test records its own).
- Da8-27 "Training loss decreases" (§ 12.2) is held by the pinned-binary test as the last
  `step` loss strictly below the first; a margin is the owner's (open point 14).
- Da8-28 `PinMismatch` compares `CheckedModel::model_spec_pin` with `ModelSpec::pin` before
  spawning; `DataNotWritten` and `EvalNotWritten` read `SplitState::Written`;
  `EvalCountNotBatchMultiple` is `is_batch_multiple(sample_count, batch_size)` with the checked
  model's batch size; the `param` lines of stdout are not parsed (story:weight-export).

Wave:

- Da8-29 The ESS pin stays at 0.68.0 (Da7-12) until the ess repository's 0.69.1 is released and
  verified; moving it is a coordinator commit, in this wave if the release lands before close.
