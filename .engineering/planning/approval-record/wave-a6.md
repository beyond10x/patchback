---
format: aep.planning-md/3
id: approval-record:wave-a6
kind: approval-record
status: approved
title: Wave a6 approved by Timo
relations:
- decides: story:bounded-search-fit
- decides: story:dataset-generation
revision: 9
transitions:
- {from: "draft", to: "approved", at: "2026-10-24T20:46:43Z", actor: "human:timo", revision: 2}
---
# Wave a6 approved by Timo

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
follows approval-record:release-process.

## The wave

- `story:bounded-search-fit` (`patchback-fit`, the `fit` verb in `patchback-cli`) and
  `story:dataset-generation` (`patchback-dataset`, the `generate-dataset` verb), in parallel,
  one Opus implementor each, then one Opus adversary pass, one correction round and a second
  adversary pass each, the correction answering pass 2 verified by the coordinator.
- A third Opus agent (`ess:author`) remodels `Benchmark.holdout` in `spec/domains/fit.yaml` per
  `approval-record:engine-baseline-holdout`, so `story:engine-baseline` becomes ready for a7.
- A coordinator scaffold commit opens `wave/a6` before dispatch (decision Da6-0), so the two
  implementors share no file.
- The ESS pin stays at 0.68.0 (wave a5 decision D5) until the ess repository ships a release
  without the synthesis regression.
- Merge into `wave/a6`, fast-forward `main`, push, release 0.5.0.
- Not in the wave: `story:fit-run-artifacts` and `story:dense-data-files` (a7, each depends on a
  unit of this wave), `story:engine-baseline` (needs the remodel), `story:note-estimation`
  (Milestone C), the studio stories (option B since a4).

## Decisions taken by the orchestrator

- Da6-0 The coordinator writes the scaffold commit (`patchback_core::prng` xoshiro256** and
  SplitMix64 with reference-vector tests; `fit.rs` and `generate_dataset.rs` argument stubs
  exiting 3; the `main.rs` dispatch arms; the path dependencies and `Cargo.lock`; `dispatch.rs`
  parse-only assertions). A deviation from "every change is an implementor's": about 120 lines
  of clap wiring and a 60-line PRNG, with no behaviour beyond argument parsing and the PRNG, so
  the two units can run in one wave. The wave gate checks it with everything else.
- Da6-1 `patchback fit` takes `--notes` (known note events; estimation is Milestone C) and the
  bound flags `--max-renders` (2000), `--max-wall-time-ms` (60000), `--max-restarts` (4),
  `--improvement-threshold` (0.0001) and `--seed` (0) beyond DESIGN.md § 11's three; the
  defaults are provisional.
- Da6-2 Coordinate search: one parameter at a time over the normalized vector, step 0.25 halving
  per sweep; the implementor fixes and documents the convergence rule.
- Da6-3 The PRNG is `patchback_core::prng` (xoshiro256**); the fit run records nothing about it
  yet (`run.meta` is `story:fit-run-artifacts`).
- Da6-4 Before the search the reference is analysed: `SilentInput` becomes `ReferenceSilent`; a
  pitch refusal (`PitchAmbiguous`, `ClipTooShortForPitch`, `PitchOutOfRange`) becomes
  `PitchAmbiguous`; in the model these externals follow the input refusals and the existence
  checks.
- Da6-5 `FitRun` mirrors the model's commands and outcomes; the search drives it.
- Da6-6 A generated patch that `validate` refuses is a `RendererFault` failure; a render refusal
  or a non-finite loss is a rejected candidate, counted, never retained. An improvement below the
  threshold is retained when strictly lower; the threshold only ends a sweep (OQ-14 open).
  `max_renders` above 2000 is allowed; the model refuses only `<= 0`.
- Da6-7 a6 writes `<out>/patch.patch` only; the other run artifacts are
  `story:fit-run-artifacts`, and `FitOutcome` is shaped so that story changes no search code.
- Da6-8 Dataset PRNG xoshiro256**, recorded as `PrngAlgorithm("xoshiro256**")`; patch seeds
  from SplitMix64 of the dataset seed; parameters and the note from the patch's own stream, in
  the order parameters (index order), MIDI, gate.
- Da6-9 Training distribution: MIDI 36 to 84, gate 0.25 to 1.5 s log-uniform, velocity 1.0,
  note at sample 0. Provisional; OQ-19 stays open in DESIGN.md.
- Da6-10 `--samples` gives the split counts: validation = test = max(16, floor(N · 4992 / 60000
  / 16) · 16), training the rest (60 000 gives 50 016 / 4 992 / 4 992, DESIGN.md § 7.1).
- Da6-11 One note per patch in a6; the sample count equals the patch count; splits are
  isolated by patch seed.
- Da6-12 Diagnostic kinds: `Clipping` when the render was limited, `Silence` for a silence
  refusal, `Noise` otherwise. A diagnostic example is never a sample.
- Da6-13 a6 writes `dataset.meta` and `<role>/samples.meta` only; dense files are
  `story:dense-data-files`.
- Da6-14 `features/1` comes from a registry the generator creates and publishes; the vector is
  checked with `FeatureContract::check` before it becomes a sample. Owned versus referenced
  children, and whether the diagnostic set becomes dense data, stay `UNMAPPED` in
  `spec/domains/dataset.yaml`.
- Da6-15 `Benchmark.topology_id` (`references` `patchback.patch.Topology`) replaces
  `holdout_split_id` and the `holdout` relation; `StartBenchmark` gains the existence refusal
  `unknown-topology` (`patchback.fit.TopologyNotFound`) after its input refusals; the holdout is
  defined by `seed` and `example_count`; scenarios updated; `patchback.dataset` untouched.
  `approval-record:engine-baseline-holdout` moves to approved when the remodel merges.
- Da6-16 The adversary's three scenarios on the holdout remodel are adopted and renamed to the
  `engine-baseline-*.yaml` convention: they pin `topology_id` in `BenchmarkStarted`,
  `WeightArtifactRequired`, the stored `Benchmark` and the `Benchmarks` view, and the refusal
  order `invalid-holdout-size` before `weight-artifact-required`; four `fit.yaml` mutants that
  passed the author's 398 scenarios are refused with them (unit 3 adversary pass 1, F1 and F2).
- Da6-17 The `unknown-topology` acceptance step of `story:engine-baseline` gets its own
  scenario `engine-baseline-unknown-topology.yaml` instead of a third step in
  `story:ml-init-benchmark`'s `ml-init-benchmark-weight-artifact-required.yaml` (F3). The
  holdout's generator algorithm, note distribution and sample rate (F4) and a zero-parameter
  topology as holdout topology (F5) stay with the owner under OQ-28.
- Da6-18 `spec/scenarios/dataset-generation-count-not-batch-multiple.yaml` refused 50,000 at
  batch size 16, which is 16 × 3,125; the model's own rule and DESIGN.md § 7.1 admit it, and
  the interpreted target passed only because the outcome is `external`. The coordinator changes
  the refused count to 50,008 on the integration branch (unit 2 implementor's patch). Spec
  defect, not a behaviour change.
- Da6-19 The validation split's directory is `val/`, as DESIGN.md § 7.2 and
  `website/docs/reference/files.md` name it (`data/val/x.f32`), not the brief's `validation/`;
  the correction round changes `SplitRole::key`. `story:dense-data-files` writes into it.
- Da6-20 Running out of draws (at most 4 per sample, a provisional constant) is
  `GenerateError::Exhausted` and the CLI line `generation exhausted: …`; the model has no such
  error. Open point for the specification's owner beside wave a5's D7. Also for the owner: about
  half of the drawn patches become diagnostics (seed 42: 95 draws for 48 samples, 23 of them
  `PitchAmbiguous`, 20 limited renders), so the training `y` is not uniform at high drive and
  output gain; measured by the implementor, cause not yet characterised.
- Da6-21 Pass 2's scenario `engine-baseline-unknown-topology-ml-init.yaml` is adopted: the
  `unknown-topology` refusal through kind `MlInitBenchmark` had no scenario, and a mutant that
  hard-codes the `BenchmarkStarted` payload's kind passed every earlier suite. The remodel merges
  as c2ece3f with 403 conformance scenarios; `approval-record:engine-baseline-holdout` moves to
  approved (Da6-15), and `story:engine-baseline`'s acceptance list names the eight
  `engine-baseline-*` scenarios. DESIGN.md OQ-28 stays the owner's.
- Da6-22 The wall-time bound of a fit means that no render starts once the wall time since
  `fit` was entered has reached `max_wall_time_ms`; the first candidate (the default point) is
  always evaluated, so a time bound stops a run instead of failing it, and a render in progress
  is not interrupted. The recorded `wall_time_ms` is the whole run's, analysis included, and
  may exceed the bound by at most one evaluation. `story:bounded-search-fit`'s obligation "wall
  time never exceeds its bound" is amended to this rule (unit 1 adversary pass 1, F1): the model
  bounds `evaluations` and not `wall_time_ms`, and DESIGN.md § 9.2 calls the budget an offline
  upper bound, not a real-time guarantee.
- Da6-23 The fit's stop rule is the implementor's D2: `ImprovementBelowThreshold` ends the
  restart loop early when a restart r >= 1 lowered the run's best by less than the threshold;
  `RestartsExhausted` is the loop completing. With `max_restarts` 1 a natural end is always
  `RestartsExhausted` (F3). The unit brief's sentence is superseded.
- Da6-24 Restart r >= 1 seeds its PRNG with the r-th output of `SplitMix64::new(seed)`, not
  `seed ^ r`, which collides across (seed, r) pairs (F6).
- Da6-25 `patchback fit` refuses an `--out` that exists and is not a directory before the
  search, at the existing-patch step of its fault order (F5). F2 (a restart counted that rendered
  nothing) is fixed in correction round 1; F4's constant-pin test is deleted.
- Da6-26 `patchback generate-dataset --samples` admits `0..=9223372036854775807` through clap
  (a larger value is a usage error, exit 2), so the split count rule never saturates and a
  `SplitCountNotBatchMultiple` refusal names the train count the rule gives (unit 2 adversary
  pass 1, F1: 18446744073709551600 was refused naming 9223372036854775807).
- Da6-27 `SplitRole` separates the directory (`train`, `val`, `test`; Da6-19) from the record
  and stdout key (`train`, `validation`, `test`, as the unit brief specifies them) (F4). Only
  the directory changes.
- Da6-28 The pitch estimator's undecided-subharmonic branch (`UNDECIDED_CONFIDENCE` 0.5,
  `crates/patchback-analysis/src/pitch.rs:251`) turns 15 of 95 seed-42 draws into `Noise`
  diagnostics although their pitch is definite: the nominal f0 returns within 0.04 Hz after
  opening the cutoff to 16 kHz (3 of 3 probed), removing drive (2 of 3) or flattening the
  envelope (1 of 3) (F5, adversary measurement). Pre-existing, the analysis owner's; it
  resolves the "cause not yet characterised" of Da6-20. Not changed in a6.
- Da6-29 The fit's clock stays at `fit` entry (Da6-22), and the overshoot sentence is
  corrected: the recorded `wall_time_ms` may exceed `max_wall_time_ms` by the reference
  analysis plus one evaluation (unit 1 adversary pass 2, N1: with a 1 ms bound, 566 to 795 ms
  recorded against a longest evaluation of 399 to 489 ms and an analysis of 157 to 169 ms).
- Da6-30 `patchback fit` checks `--out` with `fs::symlink_metadata`: a symlink that does not
  resolve to a directory is refused before the search as "is not a directory" (N2: a dangling
  symlink let the whole search run before `create_dir_all` refused it).
- Da6-31 `patchback generate-dataset --samples` keeps the integer type's limit
  (9223372036854775807) as its only ceiling in a6: a batch multiple near it (unit 2 adversary
  pass 2, A2: 9223372036854775792) runs without end holding every sample in memory. The row
  limit of `order.u32` (DESIGN.md § 7.2, at most 2^32 rows a split) and the memory bound of
  § 12.3 are `story:dense-data-files`', which writes the rows; its body records the open
  point. A3 (the `main.rs` exit-2 row) is fixed at integration.
