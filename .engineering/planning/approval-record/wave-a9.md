---
format: aep.planning-md/3
id: approval-record:wave-a9
kind: approval-record
status: approved
title: Wave a9 approved by Timo
relations:
- decides: story:weight-export
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-25T03:47:53Z", actor: "human:timo", revision: 2}
---
# Wave a9 approved by Timo

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
follows approval-record:release-process. Wave a8 released 0.7.0 on 2026-10-25 (081ddc1).

## The wave

- `story:weight-export` (`patchback-model`), alone: one Opus implementor, then one Opus
  adversary pass, one correction round and a second adversary pass, the correction answering
  pass 2 verified by the coordinator. `aep plan artifact waves --kind story --status draft` on
  0.7.0 puts it alone in wave 1 (4 draft stories of `epic:modelspec-regressor`, 4 waves, each
  depending on the one before: weight-export, native-predictor, regressor-seeded-fit,
  ml-init-benchmark).
- Two `aep:story-scoper` runs on 0.7.0 re-derived the Scope sections of `story:weight-export`
  and `story:native-predictor` (their 2026-10-24 sections predated the a8 merge). The
  native-predictor scope says the predictor cannot be built before the weight file's byte layout
  and checksum exist, which this wave defines; it follows in a10.
- A coordinator scaffold commit opens `wave/a9` before dispatch (Da9-0).
- Merge into `wave/a9`, fast-forward `main` after the `main` runs are green (AGENTS.md § Releases),
  push, release 0.8.0.
- Not in the wave: `story:native-predictor`, `story:regressor-seeded-fit`,
  `story:ml-init-benchmark` (each depends on the one before it), `story:note-estimation`
  (Milestone C), the studio stories (option B since a4).

## Decisions taken by the orchestrator

Wave:

- Da9-0 The coordinator writes the scaffold commit: the `patchback-core` path dependency of
  `patchback-model` (for `patchback_core::digest::sha256`, Da9-2) and its `patchback-analysis`
  path dependency (for `FEATURES_1_VERSION` and `FEATURES_1_VALUE_COUNT`, Da9-9); `Cargo.lock`.
  No cycle: analysis takes core and dsp, dataset takes analysis, audio, core and dsp, none of
  them takes model. No behaviour change; the model crate's suite and the dependency gate run on
  it. `Cargo.toml` and `Cargo.lock` stay the coordinator's.
- Da9-1 The ESS pin stays at 0.68.0 (Da8-29) until the ess synthesis fix for the second external
  outcome (ess PR 690) is released and verified on patchback; moving it is a coordinator commit.

Unit 1, `story:weight-export` (the open points are the scoper's, 2026-10-25, in the story's body):

- Da9-2 The checksum is SHA-256 (`patchback_core::digest::sha256`, the digest `run.meta` already
  uses) over the payload bytes alone, written as 64 lowercase hexadecimal digits in the header.
  A header field is checked by the parser, so the checksum need not cover it. Provisional
  (OQ-21, the spec's `UNMAPPED` on `WeightChecksum`); for the owner.
- Da9-3 The artifact is one file, format `weights/1`: a text header of fixed lines in fixed
  order, each `<key> <value…>` terminated by `\n`, then one empty line, then the payload of
  2,560 little-endian `f32` (w1's 2,048 values, then w2's 512, each in the order the `param`
  line printed them, 10,240 bytes). The header lines: `weights/1`; `model PatchRegressor`;
  `modelspec <version> <commit>` (the run's pin); `features <feature-contract version>`;
  `w1 32 64`; `w2 64 8`; `sha256 <hex>`. The recommended file name is `regressor.weights`
  (provisional); the path is the caller's. The element order is "as printed": whether that is
  row-major is a property of ModelSpec 0.5.0 and is proven by `story:native-predictor`'s
  equivalence test, not here.
- Da9-4 Values are parsed with `f32::from_str`; a token that does not parse is
  `UnrecognizedTrainingOutput`. The pinned test proves the round trip on the real binary: every
  parsed value re-printed with `{}` equals its token (ModelSpec is Rust and prints the shortest
  representation); if a value does not round-trip, the report says so and the unit keeps the
  parsed `f32`.
- Da9-5 "The documented output of the pinned version" is the whole stdout: every line is one of
  `init @PatchRegressor.<name>: Tensor<f32, [<r>, <c>]> = [<values>]`, `step <k> loss <v>`,
  `eval loss=<v> accuracy=<v>` and `param @PatchRegressor.<name>: Tensor<f32, [<r>, <c>]> =
  [<values>]`; a line outside that grammar, including an empty line, is
  `UnrecognizedTrainingOutput`. Exactly two `param` lines are expected; fewer or more is
  `UnrecognizedTrainingOutput`. The pinned test proves the real 0.5.0 stdout passes.
- Da9-6 The first `param` line is w1 and the second is w2. Their names are compared with
  `@PatchRegressor.w1` and `@PatchRegressor.w2`; a difference (a bias, a swap) is
  `UnrecognizedParameter { w1_name, w2_name }` carrying the names as printed. The declared
  `[r, c]` of each line is its shape (`WrongWeightShape` against [32, 64] and [64, 8], both
  tensors in one error as the spec declares it); the number of values is its element count
  (`WrongElementCount` against 2,048 and 512); then every value finite (`NonFiniteWeight`).
- Da9-7 Outcome order in Rust: the run's state first (`TrainingNotCompleted` unless
  `Completed`, Da8-39 precedent: there is no output to read otherwise), then the grammar
  (`UnrecognizedTrainingOutput`), the names (`UnrecognizedParameter`), the shapes
  (`WrongWeightShape`), the counts (`WrongElementCount`), finiteness (`NonFiniteWeight`), then
  the write. The spec lists `wrong-shape` and `wrong-element-count` first because its shapes
  and counts are caller inputs; in Rust they are constants (`W1_SHAPE`, `W2_SHAPE`,
  `W1_ELEMENTS`, `W2_ELEMENTS`) and read from the output, so the scenario steps that pass a
  transposed w1 or a short w2 as inputs map to a stdout printing that tensor. The `unknown-run`
  step has no library counterpart (Da8-21): a `TrainingRun` is a value.
- Da9-8 Only `TrainingRunState::Completed` gates the export. A completed run with an
  `output_error` on its loss curve is read like any other: its malformed line fails Da9-5. A
  NaN loss with finite weights exports (Da8-33: the loss is the training's verdict, not the
  export's); NaN weights are `NonFiniteWeight`.
- Da9-9 The artifact records the feature-contract version the caller gives (`&str`, unchecked,
  as the spec models the id "given at export and not checked against the dataset"); the
  constant to give is `patchback_analysis::FEATURES_1_VERSION`. The artifact carries no
  normalization statistics; the carrier of the training split's `normalization.stats` to
  inference is `story:native-predictor`'s and the owner's (open point 5).
- Da9-10 Library only, no `patchback` verb (Da8-20; DESIGN.md § 11 lists none). The surface:
  `WeightArtifact::export(run: &TrainingRun, feature_contract: &str, path: &Path) ->
  Result<WeightArtifact, ExportError>` (`ExportWeights`; `WeightsExported` is the `Ok`) and
  `WeightArtifact::read(path: &Path) -> Result<WeightArtifact, ReadError>`, which verifies the
  header grammar, the version line, the payload length and the checksum; any failure is
  `ReadError::Corrupt { reason }`, whose domain error is `CorruptWeightArtifact` (the story's
  "the checksum verifies on read"; the spec declares the error on `Predict`, which will call
  this reader; `tests/domain.rs` accounts for it as emitted by the reader). `WeightArtifact`
  holds `w1: Vec<f32>` (2,048), `w2: Vec<f32>` (512), the model name, the pin version and
  commit, the feature-contract version and the checksum.
- Da9-11 The writer is create-new: an existing file is `ExportError::Io` (not a domain error;
  the spec has no outcome for it), nothing is written before every check of Da9-7 has passed,
  and a write that fails removes the file it created (`generate_dataset.rs:273`,
  `benchmark.rs:660` precedent). The reader bounds the header at `MAX_HEADER_BYTES = 1024`
  (provisional) and the payload at exactly 10,240 bytes; past either is `Corrupt`.
- Da9-12 No cap on the captured stdout is added (a8 added none; a w1 line is 25,905 bytes, the
  whole stdout of a real run about 70 KB). The parser collects at most one value past the
  expected count before refusing. OQ-6 stays with the owner.
- Da9-13 `clamped_predictions` is not in the file: the artifact is immutable once written, and
  the counter is a runtime figure of the predictor (`story:native-predictor`, open point 17).
- Da9-14 `model_spec_pin_id` of the spec is the run's pin (`TrainingRun::pin`), recorded as
  version and commit; "for the pinned version" means that pin, with no compiled-in version
  check (the run was pinned at `plan`).
- Da9-15 The stdout bridge of DESIGN.md § 8.2 steps 2-3 is built as written. ModelSpec 0.5.0's
  `train --checkpoint` and `infer --checkpoint` (its `--help`, read by the scoper) are DESIGN.md
  § 8.2 step 5's replacement, which needs an equivalence test and the owner's word (DESIGN.md
  § 1.1 says checkpoint and inference are not an assumed capability). For the owner.
- Da9-16 The header's `model` line carries the parameter prefix as printed (`PatchRegressor`);
  no digest of the `.mspec` source is recorded (the run keeps the source path; a digest is the
  owner's if wanted).
- Da9-17 Tests: the three scenarios step by step in
  `crates/patchback-model/tests/weight_export_scenarios.rs` against stand-in scripts whose
  stdout is chosen in the test (the `training_scenarios.rs` pattern: a real small generation
  for the Written split, exit status and output from the script), one test per scenario file,
  each step cited; the parser and the reader on fixed strings and bytes (every refusal, a
  flipped payload byte, a flipped header byte, a truncated payload, `weights/2`); the pinned
  lane in `tests/pinned_training.rs` or a new `tests/pinned_export.rs`: export from the real
  completed run, read back, the values equal the parsed ones, the round trip of Da9-4.
- Da9-18 `tests/domain.rs` gains `ExportWeights` to its command table and accounts for every
  error it declares, `UnknownTrainingRun` as not emitted (Da8-21), and `CorruptWeightArtifact`
  as emitted by the reader for `Predict`.

Correction round 1 (after adversary pass 1, `review-result:weight-export-adversary-1`):

- Da9-19 The name of an `init` or `param` line is `@<ident>.<ident>` in the grammar (each part
  one or more ASCII letters, digits or `_`); the names step compares the first `param` name with
  `@PatchRegressor.w1` and the second with `@PatchRegressor.w2` in full, so `param @Other.w1` is
  `UnrecognizedParameter { w1_name: "@Other.w1", … }` as `model.yaml`'s `unrecognized-parameter`
  outcome words it; an `init` name is not checked (Da9-6). Supersedes the fixed prefix in Da9-5's
  grammar line and the implementor's decision 2 (adversary F4).
- Da9-20 The adversary's cases stay in the unit (`tests/adversary_weight_export.rs`): three of
  them are the only tests of the export's header bound at 1,024 bytes, the reader's read limit
  and its `.take(limit)` guard (adversary F1 to F3).
