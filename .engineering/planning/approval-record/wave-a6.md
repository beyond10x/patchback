---
format: aep.planning-md/3
id: approval-record:wave-a6
kind: approval-record
status: approved
title: Wave a6 approved by Timo
relations:
- decides: story:bounded-search-fit
- decides: story:dataset-generation
revision: 2
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
