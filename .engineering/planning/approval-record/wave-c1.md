---
format: aep.planning-md/3
id: approval-record:wave-c1
kind: approval-record
status: approved
title: Wave c1 approved by Timo
relations:
- decides: story:note-estimation
- decides: story:studio-patch-graph
- decides: story:studio-fit-replay
revision: 6
transitions:
- {from: "draft", to: "approved", at: "2026-11-01T15:24:46Z", actor: "human:timo", revision: 2, executor: "agent:wave-c1-coordinator"}
---
# Wave c1 approved by Timo

## Who approved, in their words

Timo, 2026-11-01, in chat to this session. The session reported that the Milestone B exit is not
demonstrated (`verification-report:milestone-b-exit`) and offered option A, "Wave c1:
`note-estimation` and `studio-fit-replay`. The B result goes into the store as an open
finding", against option B, a regressor wave first. Timo answered:

> A

and, the same turn:

> you have my authority to make decissions yourself. always decide in a way that we avoid
> technical dept, we aim for the best results and highest engineering quality

The coordinator takes every decision below under that authority. The workspace rule "Deliver in
waves: one gate, one PR" (Timo, 2026-10-28) and AGENTS.md § Releases set the close: one gate on
`wave/c1`, one PR through the bot, merged to `main`, then one release.

## The wave

- Unit 1, `story:note-estimation` (`crates/patchback-analysis`, `crates/patchback-cli`,
  `spec/domains/analysis.yaml`, `spec/domains/score.yaml`, DESIGN.md § 10 and § 11). First story
  of `epic:phrases` (Milestone C).
- Unit 2, `story:studio-patch-graph` then `story:studio-fit-replay`, one implementor, in that
  order (`studio/`, a new tooling crate `crates/patchback-studio`, `spec/domains/studio.yaml`,
  DESIGN.md § 3, § 14 ADR-011 and § 16). `story:studio-fit-replay` `depends_on`
  `story:studio-patch-graph`, which is draft, so the dependency joins the unit (Dc1-20); the two
  share `studio/src/model.ts`, `studio/src/views/` and `studio/src/components/PatchGraph.vue`.
- The two units share no source file. Shared files are the coordinator's: `Cargo.toml` (workspace
  members), `Cargo.lock`, `Taskfile.yml` `check:`, `.github/workflows/check.yml`, `CHANGELOG.md`,
  `AGENTS.md`, `README.md`, `crates/patchback-docs/src/status.rs`, the generated pages and
  `website/docs/**`. DESIGN.md is edited by both units in different sections; the coordinator
  merges and sets one revision line, 0.8, at the close (Dc1-30).
- Riding along: the ESS pin moved to 0.80.0 (`approval-record:ess-pin-0-80-0`, 9f09750), and
  the Milestone B finding (`verification-report:milestone-b-exit`, `story:regressor-earns-the-exit`).
- Not in the wave: `story:phrase-fit` (depends on unit 1), `story:external-audio-acceptance`,
  the other studio stories, Milestones D and E, `story:regressor-earns-the-exit`.

## Commits the approval authorises

On `wave/c1`: the ESS pin commit, the plan commit opening the wave, one commit per unit on its
`impl/` branch plus its correction commits, the merges into `wave/c1`, the coordinator's docs
commit and the close commit. Then the push of `wave/c1`, one PR through the bot, its merge to
`main`, the release commit, the tag and its push, per AGENTS.md § Releases. All through
`b10x-gates bot`.

## Decisions taken by the coordinator

Unit 1, `story:note-estimation` (open points from the story-scoper, 2026-11-01):

- Dc1-1 Confidence per quantity, as modelled: every `EstimatedNote` carries onset, pitch, gate
  and tail confidence, each in [0, 1]. Pitch confidence is the existing `1 - d'`. The
  implementor defines the other three from the signal (onset: the flux peak's prominence; gate
  and tail: the level-crossing slope against the noise floor or an equivalent), each stated in
  the module doc and in DESIGN.md § 10. The reduction to one scalar per sequence is
  `story:phrase-fit`'s, not this story's.
- Dc1-2 Velocity is estimated. `EstimatedNote` gains `velocity` in [0, 1] (the UNMAPPED marker
  at `spec/domains/analysis.yaml:69-70` is settled): the note's peak RMS over its gate relative
  to the loudest note's, linear, the loudest note 1.0. Reason: the renderer scales the envelope
  linearly by velocity (`crates/patchback-dsp/src/render.rs:212`), so this is the inverse of
  that mapping, and a constant would throw away the phrase's dynamics before the fit sees them.
- Dc1-3 Polyphonic input is refused outright with `PolyphonicInput`, as modelled. A detector is
  added; it must refuse two simultaneous tones a minor second, a major third, a fifth and a
  tritone apart, and must not refuse a single saw, square or filtered-saw tone from the
  subtractive voice across MIDI 36 to 84. An octave is not required to be detected and the
  module doc says why.
- Dc1-4 Thresholds are named provisional constants, one per quantity, in the
  `crates/patchback-analysis/src/lib.rs` provisional-values list, set from the calibration in
  Dc1-9. The refusal `LowConfidenceNotes` fires when any note has any confidence below its
  threshold, as the guard is modelled; the UNMAPPED marker at `analysis.yaml:940` is replaced by
  the rule and the constants' names.
- Dc1-5 A note sequence written from an estimation equals its notes: one `NoteEvent` per
  `EstimatedNote`, same order, onset, gate end, MIDI pitch and velocity. The writer guarantees
  it and a Rust test holds it; the model keeps recording the link only, and the UNMAPPED marker
  at `spec/domains/score.yaml:91-93` is replaced by that statement. Tail and confidences live in
  the estimation file (Dc1-6).
- Dc1-6 A command, `patchback estimate --input <wav> --out <dir>`. It reads the WAV, runs the
  analysis itself (as `fit` does) and writes two files: `estimation.txt`, a new line format
  `patchback-note-estimation 1` (the estimation id, the clip's SHA-256, the estimator's
  configuration and constants, then one line per note with onset, gate end, tail end, MIDI
  pitch, f0 in Hz, velocity and the four confidences), and `notes.events` carrying the
  estimation id. A refusal writes nothing and exits 1 with one stderr line naming the refusal;
  for `LowConfidenceNotes` the line lists each low note's index and quantities. DESIGN.md § 11
  gains the verb and the file; `website/docs/reference/cli.md` and `files.md` are the
  coordinator's.
- Dc1-7 The estimation id is deterministic: a UUID (version 8, RFC 9562 custom) from the first
  16 bytes of the SHA-256 of the clip's samples, the format version and the configuration. Two
  runs over one clip write identical bytes (O6).
- Dc1-8 `unknown-estimation` is enforced at the binding seam. `patchback_analysis` gains a
  binding function that takes a `NoteSequence` and the estimations it may name and refuses
  `NoteEstimationNotFound`; `patchback render` and `patchback fit` take `--estimation
  <estimation.txt>`, required when the `notes.events` they read carries a `note_estimation_id`,
  and refuse a mismatch with the score domain's precedence (after `unknown-patch`, before
  `rate-mismatch`, `score.yaml:361-385`).
- Dc1-9 Numeric obligations, set now, before implementation. The reference set is seeded and
  synthesized by the subtractive voice: 32 phrases of 2 to 8 notes, MIDI 36 to 84, gaps of 0 to
  200 ms (zero gaps only with an attack of 20 ms or less), repeated same-pitch notes included,
  varied ADSR, cutoff and drive.
  - Detection: at least 95 % of notes found; at most 5 % spurious notes.
  - Onset: within 256 samples (5.3 ms at 48 kHz) of the true onset for at least 95 % of found
    notes.
  - Pitch: the exact MIDI number for at least 98 % of found notes, and f0 within 10 cents of the
    true pitch.
  - Gate end: within 1,024 samples (21.3 ms) for at least 90 %. Tail end (the release falling
    below the silence threshold): within 2,400 samples (50 ms) for at least 90 %.
  - Calibration: no quantity outside its tolerance carries a confidence at or above its
    threshold; at least 90 % of quantities inside tolerance do.
  - The tests print the measured rates. A rate that misses its bound is a red test, never a
    relaxed bound.
- Dc1-10 `estimate_f0`, its constants and `analyze` stay unchanged: they feed the frozen
  `features/1` and the demo numbers. Per-note pitch calls `estimate_f0` on segments; a segment
  too short for it is refused as `LowConfidenceNotes` with pitch confidence 0 for that note,
  never silently dropped.
- Dc1-11 The four acceptance scenarios hold as written; spec edits are additive or settle
  UNMAPPED markers. `task spec-validate`, `task spec-conform` and `task docs-generate` run after
  every spec edit.

Unit 2, `story:studio-patch-graph` and `story:studio-fit-replay` (open points from the
story-scoper, 2026-11-01):

- Dc1-20 The dependency joins the unit: the implementor finishes `story:studio-patch-graph`
  first, then `story:studio-fit-replay`, one commit each.
- Dc1-21 Rust is the only reader of Patchback's files. A new tooling crate
  `crates/patchback-studio` with a binary `patchback-studio export --run <dir> --out <file>`
  reads a fit run's `patch.patch`, `run.meta` and `trace.events` through the existing parsers
  (`patchback_fit::trace::parse_trace`, the `.patch` reader, graph validation) and writes one
  JSON document whose shape is the ESS-generated studio types (`TraceOpened`, `FrameAppended`,
  `TraceClosedEvent`, `GraphDrawn` and what the views need). The studio never parses
  `trace.events` or `.patch` itself, so neither grammar nor graph validation is duplicated in
  TypeScript. The crate is where `story:studio-fit-live`'s std-only server goes later
  (ADR-011).
- Dc1-22 The crate is tooling, admitted like `patchback-docs`: it may use crates already in
  `Cargo.lock` for serialization and argument parsing, adds no crate to the lock, and is never a
  dependency of an application crate. The dependency gate
  (`crates/patchback-cli/tests/dependencies.rs:11-14`, `:37-50`) names it; DESIGN.md § 3 and
  ADR-011 say so.
- Dc1-23 The studio loads an exported document through a file picker, and opens on a bundled
  sample: a real `patchback fit` run on a committed demo voice, exported by the crate, committed
  under `studio/fixtures/`, regenerated and compared by a drift check in `task check`. The
  hand-built `studio/fixtures/trace.json` is removed.
- Dc1-24 The spec follows the shipped trace: `TraceFrame` gains `restart`; the trace names its
  topology by name; frame sequences start at 1; a file only ever ends `Completed` with the run's
  stop reason (`Failed` stays modelled for `story:studio-fit-live`). Identities (`trace_id`,
  `fit_run_id`, `frame_id`, `patch_id`) are derived by the exporter as version-8 UUIDs from the
  SHA-256 of the run's files, so an export is byte-reproducible. The UNMAPPED markers at
  `spec/domains/studio.yaml:17-18` are settled; `:222-226` (several topologies, learned graph)
  stays open.
- Dc1-25 The studio gets a test runner and a gate. `vitest` is added (ADR-011 puts the studio
  outside the § 3 policy); `task studio-test`, `task studio-build`, `task studio-types-check`
  and the fixture drift check join `task check`; `.github/workflows/check.yml` installs Node and
  pnpm at pinned versions. Tests cover loading, the four scenarios' refusals, the replay's
  invariants and the graph's delayed-feedback routing.
- Dc1-26 Scale: a real run has up to 2,000 evaluations (`crates/patchback-fit/src/run.rs:190`).
  The loss curve and each trajectory render as one SVG path per series, the play speed scales to
  the frame count, and a test asserts the DOM node count does not grow with frames. The
  topology-change marker stays and is exercised by a fixture that is not a real run, labelled so.
- Dc1-27 The stale texts go: the stories' first open question ("§ 11 lists no per-evaluation
  trace file") and `website/docs/concepts/studio.md:61-62` (the coordinator's).

Wave:

- Dc1-30 DESIGN.md revision moves once, to 0.8, in the close commit, naming both units'
  changes.
- Dc1-31 Each unit gets two adversary passes (`aep:adversary`, Opus) with a correction round
  between, then the coordinator's verification of the last correction.
- Dc1-32 Release after the merge to `main`: 0.12.0 (new verbs and a new crate).

## Provisional values

| Name | Value | Where | Why provisional |
| --- | --- | --- | --- |
| reference set (Dc1-9) | 32 phrases, 2 to 8 notes, MIDI 36 to 84 | `crates/patchback-analysis/tests/` | DESIGN.md § 10 names no size |
| tolerances (Dc1-9) | onset 256, gate 1,024, tail 2,400 samples; pitch 10 cents | the same | set before measuring; DESIGN.md names none |
| rates (Dc1-9) | 95 % detection, 5 % spurious, 98 % pitch, 90 % gate and tail | the same | the same |
| confidence thresholds (Dc1-4) | set by calibration | `crates/patchback-analysis/src/lib.rs` | measured on the reference set only |

## Decisions taken after the adversary passes

Taken by the coordinator under Timo's standing grant of 2026-11-01 ("you have my authority to make
decissions yourself. always decide in a way that we avoid technical dept, we aim for the best
results and highest engineering quality"), after the adversary passes.

- Dc1-12 The note-estimation reference set is a distribution, not one seed
  (`review-result:note-estimation-adversary-1`: every rate 1.0000 on the pinned seed, confident
  wrong quantities on 9 of 12 other seeds). The generator draws Dc1-9's set as worded, zero-gap
  same-pitch repeats included; the committed harness measures every Dc1-9 rate over 16 seeds of
  32 phrases, the bounds hold on the pooled set and the calibration rule on every seed.
- Dc1-13 Stress shapes (long releases, short gates, low notes, quiet notes, zero-gap repeats, a
  -70 dBFS noise floor) hold calibration and polyphony, not the rates: no confident quantity
  outside tolerance and no monophonic phrase refused as `PolyphonicInput`, over 4 seeds of 32
  phrases each. Refusing a hard note as `LowConfidenceNotes` is allowed.
- Dc1-14 An estimation file is self-checking: a digest of its note lines, and a reader that
  refuses a mismatch, an unknown pitch, a MIDI number that is not nearest to its f0 and a
  confidence below its threshold. `patchback fit` refuses an estimation of another clip than its
  `--input`. The Dc1-7 id is unchanged.
- Dc1-15 Onset confidence derives from the onset's own localisation interval: the candidate
  positions whose change-point cost lies within a noise-derived margin of the minimum, widened by
  the resolution of the evidence (half the longer period for a pitch-change onset, the envelope
  step for a level onset). When its half-width reaches the 256-sample tolerance, the confidence is
  below threshold. Two stress violations left after Dc1-12..14 (a MIDI 42 pitch change, a 3 dB
  level onset) are fixed by this rule, not by a case-specific cap and not accepted as red.
- Dc1-18 Replaces Dc1-15's widening clause. Widening by half the longer period, measured, marked
  32 % of correct reference change points unconfident (pooled confident-inside 0.834 against
  0.90) and refused the shared three-note fixture phrase: it is a property of the pitch, not of
  the evidence. The χ² interval and the prominence bound stay. Pitch-change onsets are refined on
  the sample grid from the residual of the old note's periodic continuation meeting the new
  note's backward continuation, and the interval is widened only by the grid its cost was
  evaluated on. If low-note errors survive with a narrow interval, the χ² noise variance is
  recalibrated from the residual's autocorrelation. The 0.90 bound, the MIDI range and the
  fixture do not change.
- Dc1-19 After `review-result:note-estimation-adversary-2` measured pooled confident-inside at
  0.900 ± 0.004 per block of 16 unseen seeds, the committed harness measures seeds 1 to 64 and
  requires pooled confident-inside of at least 0.92, so a fresh block clears Dc1-9's 0.90 with
  margin. The Dc1-9 bound itself is unchanged.
- Dc1-20 The stress shapes grow by Gaussian noise at -70 dBFS, dark cutoffs, low sustains, short
  gaps, high drives and long decays, each measured on seeds 1 to 8 for calibration and polyphony;
  the quiet threshold derives from a robust statistic of the noise, not an assumed distribution.
- Dc1-21 The harness's tail truth is the last sample above the silence threshold, as Dc1-9 words
  it, even before the gate end.
- Dc1-22 Polyphony is decided first, on each note's steady part: no monophonic phrase is refused
  as `PolyphonicInput`, and every drawn dyad of Dc1-3's intervals is refused as
  `PolyphonicInput`.
- Dc1-23 `frame_count` is covered by the estimation file's check, and `patchback fit` compares
  every `--estimation` it reads with its `--input`.
- Dc1-31a Amends Dc1-31 for note estimation only: its defect class is overfitting, which only
  unseen data can show, so a third adversary pass re-measures on unseen seeds after correction
  round 2.
- Dc1-16 A lint-only patch to the adversary's `tests/adversary_1.rs` (a type alias, one hex
  literal regrouped to the same value) is applied, so clippy `-D warnings` passes on all targets;
  no assertion changes.
- Dc1-17 The workspace sets `[profile.test] opt-level = 2`: the note-estimation suites took 244 s
  in the unoptimised test profile and 23 s at opt-level 2 with the same results. Rust has no
  fast-math, so results do not depend on the level.
- Dc1-28 Two adversary judgement items in the studio are fixed, not deferred: the loss chart's
  maximum is one pass over the frames (no argument spread), and the export fixture is blessed only
  when `PATCHBACK_STUDIO_BLESS` equals `1`.
- Dc1-29 The studio exporter decides no refusal from an error message: the fit trace parser
  reports typed reasons (`crates/patchback-fit/src/trace.rs`) and the exporter maps those; the
  trace grammar and the `patchback fit` messages are unchanged.
