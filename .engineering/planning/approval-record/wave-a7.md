---
format: aep.planning-md/3
id: approval-record:wave-a7
kind: approval-record
status: approved
title: Wave a7 approved by Timo
relations:
- decides: story:fit-run-artifacts
- decides: story:dense-data-files
revision: 6
transitions:
- {from: "draft", to: "approved", at: "2026-10-24T23:09:22Z", actor: "human:timo", revision: 2}
---
# Wave a7 approved by Timo

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
follows approval-record:release-process. Wave a6 released 0.5.0 on 2026-10-25.

## The wave

- `story:fit-run-artifacts` (`patchback-fit`, the `fit` verb in `patchback-cli`) and
  `story:dense-data-files` (`patchback-dataset`, the `generate-dataset` verb), in parallel, one
  Opus implementor each, then one Opus adversary pass, one correction round and a second
  adversary pass each, the correction answering pass 2 verified by the coordinator. `aep plan
  artifact waves --kind story --status draft` on 0.5.0 puts these two alone in wave 1.
- A third Opus agent (`aep:story-scoper`, read-only) derives the Scope section of
  `story:engine-baseline`, the a8 candidate that depends on `story:fit-run-artifacts`, so a8 is
  selected on cited files.
- A coordinator scaffold commit opens `wave/a7` before dispatch (Da7-0), so the two implementors
  share no file.
- Merge into `wave/a7`, fast-forward `main`, push, release 0.6.0.
- Not in the wave: `story:engine-baseline` (depends on unit 1; collides with it on
  `crates/patchback-fit`), `story:normalization-stats` and `story:training-run` (collide with
  unit 2 on `crates/patchback-dataset`), `story:note-estimation` (Milestone C), the studio
  stories (option B since a4).

## Decisions taken by the orchestrator

- Da7-0 The coordinator writes the scaffold commit: `patchback_core::digest` (SHA-256, FIPS
  180-4, with the published reference digests as tests) so a run can record the digests of its
  inputs, and the `patchback-model` path dependency of `patchback-cli` (and `Cargo.lock`) so the
  `fit` verb can record the compiled-in ModelSpec pin. About 170 lines, no behaviour beyond the
  hash; the wave gate checks it with everything else.
- Da7-1 Unit 1 writes all six files of DESIGN.md § 11: `patch.patch`, `notes.events`,
  `reconstructed.wav`, `report.txt`, `run.meta` and `trace.events`. The story's outcome and the
  model's `FitResults` name five; `trace.events` was assigned to this story by Da6-7.
- Da7-2 `trace.events` is written from the run's evaluation log after the run ends; "written as
  the run proceeds" (§ 11) is the live studio story (`story:studio-fit-live`). A failed run
  writes nothing, as `FitResults` models it (`FailedRunHasNoPatch`; "this model writes nothing
  for a failed run"). Whether a failed run keeps a trace or writes a diagnostic report stays
  OQ-12, the owner's.
- Da7-3 `run.meta` is line-oriented and versioned like `dataset.meta` (`patchback-fit-run-meta
  1`): the `patchback` version, `init default`, `seed`, `topology subtractive-mono/1`,
  `feature_contract features/1`, `prng xoshiro256**`, `modelspec <pin version>` (the compiled-in
  `models/modelspec.pin`, passed in by the CLI), `reference sha256 <hex>` and `notes sha256
  <hex>` of the input files' bytes, the four bounds and the block size. The implementor fixes
  the record order and documents it.
- Da7-4 `report.txt` (`patchback-fit-report 1`) carries what `patchback fit` prints today
  (initial and best loss, the four components, evaluations, restarts, stop reason, wall time),
  the versions, and one `warning` line per warning (the limiter engaging on the best render is
  one). It carries no `confidence` line: DESIGN.md § 11 names one and defines it nowhere. For
  the owner, with OQ-12.
- Da7-5 `reconstructed.wav` is the clip the best patch was scored on (`FitOutcome::render`, cut
  or padded to the reference's length), float32 mono 48 kHz, so `patchback compare` of the
  reference and that file reproduces the reported best loss. `notes.events` is the canonical
  form of the input notes.
- Da7-6 The `--out` rule extends to the six files: any of them existing is refused before the
  search, nothing written; a write that fails after the search removes what the run wrote, as
  `generate-dataset` does.
- Da7-7 `FitResults` lives in `patchback-fit` beside `FitRun`; writing is refused
  `FitStillRunning` for a Running run and `FailedRunHasNoPatch` for a Failed one, in the
  model's order. `FitRunNotFound` stays a listed error with no library counterpart (the run is
  a value, not a record); the scenario mapping says so.
- Da7-8 Unit 2 adds the split state `Open → Written`, `DenseDataFile`, and the commands
  `WriteDenseFile` and `FinishSplit` to `DatasetStore` in the model's outcome order; the writer
  writes `x.f32` (N × 32), `y.f32` (N × 8) and `order.u32` (N × 1) into each split's directory,
  verifies each file by reading it back (byte count rows × columns × 4, finite values), then
  finishes the split. `patchback generate-dataset` writes them after `samples.meta` and before
  `dataset.meta`.
- Da7-9 `order.u32` is always written and holds the row indices 0 to N − 1 in draw order
  (little-endian u32); its check is: N entries, every index below N, each once. Provisional:
  what the ModelSpec loader expects of the file is not written down in this repository (the
  story's first open question stays with the owner).
- Da7-10 The row ceiling is `u32::MAX` rows a split, the most `order.u32` can index:
  `MAX_SAMPLES` becomes `u32::MAX` and `--samples` above it is a usage error (exit 2) as today.
  Every sample still sits in memory until the files are written; the memory bound DESIGN.md
  § 12.3 asks for is a streaming writer, a later story. For the owner, with Da6-31.
- Da7-11 `x` and `y` are kept as `f64` and written as `f32` by Rust's `as` (round to nearest).
  The generator cannot produce a non-finite value (the contract check refuses it), so
  `NonFiniteValue` is reached only through the model (a unit test) and the verification of a
  written file; `SampleCountMismatch` is the model's check of `rows` against the split.
- Da7-12 The ESS pin stays at 0.68.0 (wave a5 D5) until the ess repository ships a release
  without the synthesis regression.
- Da7-13 Six existing cases pinned Da6-26's ceiling (`i64::MAX`) or the pre-dense split layout, in
  files outside unit 2's assignment (`crates/patchback-dataset/tests/generate.rs`,
  `crates/patchback-dataset/tests/adversary_pass2.rs`,
  `crates/patchback-cli/tests/generate_dataset_adversary2.rs`). Da7-8 and Da7-10 supersede
  them. The coordinator applied the implementor's re-pin patch in the unit tree: the
  `adversary_pass2.rs` sweeps keep their oracle against `SplitCounts::from_samples` up to a
  local `I64_SAMPLES` (the function still takes every `u64`), one new case pins
  `MAX_SAMPLES − 15` with an exact draw bound, and the CLI cases assert exit 2 with clap's
  `0..=4294967295` range for both `4294967296` and `9223372036854775808`, and the four-file
  split layout (`order.u32`, `samples.meta`, `x.f32`, `y.f32`). No assertion was dropped; the
  `2^62 + 8` case moved from exit 1 (the rule's train count) to exit 2, which Da7-10 makes the
  first refusal it meets.
- Da7-15 (unit 1 adversary pass 1, F1) The limiter warning of `report.txt` and `FitResults.warnings`
  reads `limiter engaged on <n> of <m> rendered samples`, `n` the render report's
  `limited_samples` and `m` its `frame_count`, both of the uncut render of the best patch.
  `reconstructed.wav` stays the render cut or padded to the reference's frame count, so the
  warning says which samples it counts. Counting only inside the reference's length would need
  the limiter's sample positions, which `patchback-dsp` does not report; a later story if a
  consumer needs it.
- Da7-16 (F2, F3) `parse_trace` refuses an evaluation line whose evaluation count differs from
  its sequence, and a best-after value that rises above the previous line's or is `-` after a
  number; each is a named parser refusal. `write_trace` never writes such a line; the refusals
  hold the replay the studio reads to `TraceFrame`'s invariant ("never rises").
- Da7-17 (unit 2 adversary pass 1, F1) `ByteCountMismatch` carries the input's `byte_count`,
  `rows` and `columns`, as `spec/domains/dataset.yaml`'s payload says, in both branches of the
  check (the arithmetic and the content's length). The content's length is not part of the
  payload; the writer always passes the content's length as `byte_count`, so the CLI's refusal
  names the file's length either way.
- Da7-18 (F2) `write_split` refuses a split whose state is not `Open` before any write, as
  `DatasetError::SplitAlreadyWritten`, the same way it refuses `UnknownSplit`. The model's
  `WriteDenseFile` still admits a file on a `Written` split (the story's second open question,
  the owner's); the writer is where the CLI's behaviour is held.
- Da7-19 (F3, pre-existing) `every_error_of_the_eight_commands_is_emitted` drives each of the
  twelve domain errors through a `DatasetStore` command and asserts the error it returns, so
  it observes an emission; `DatasetError::examples()` stays for the rendering checks that read
  it.
- Da7-20 (unit 1 adversary pass 2, G1, G2) `parse_trace` refuses a best-after value of `-` on
  a line whose outcome is `first-retained`, `improved` or `not-improved` (those outcomes mean a
  candidate is retained, so a best exists), and a numeric best-after on a `rejected` line
  before the first `first-retained` line (no candidate scored it); each is a named parser
  refusal beside Da7-16's. The four other line patterns pass 2 characterised (a restart number
  that skips or falls, `first-retained` after the first numeric best, `improved` with a best
  other than its total, a `rejected` or `not-improved` line whose best fell) stay accepted:
  no run writes them, the characterisation cases pin the behaviour, and the owner decides
  with the trace grammar (DESIGN.md § 9.1). The stdout `limited` row of `patchback fit` says
  it describes the uncut render, as `report.txt`'s warning does.
