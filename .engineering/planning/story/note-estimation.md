---
format: aep.planning-md/3
id: story:note-estimation
kind: story
status: active
title: Monophonic note estimation
relations:
- decomposes: epic:phrases
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:analysis-core
- depends_on: story:note-events
- serves: vision:patchback
scope:
- confidence: cited
  path: DESIGN.md
- confidence: inferred
  path: crates/patchback-analysis/Cargo.toml
- confidence: inferred
  path: crates/patchback-analysis/src/error.rs
- confidence: inferred
  path: crates/patchback-analysis/src/estimate.rs
- confidence: cited
  path: crates/patchback-analysis/src/lib.rs
- confidence: cited
  path: crates/patchback-analysis/tests/domain.rs
- confidence: inferred
  path: crates/patchback-analysis/tests/estimate.rs
- confidence: inferred
  path: crates/patchback-cli/src/estimate.rs
- confidence: inferred
  path: crates/patchback-cli/src/fit.rs
- confidence: inferred
  path: crates/patchback-cli/src/main.rs
- confidence: inferred
  path: crates/patchback-cli/src/render.rs
- confidence: inferred
  path: crates/patchback-cli/tests/dispatch.rs
- confidence: inferred
  path: crates/patchback-cli/tests/estimate.rs
- confidence: cited
  path: spec/domains/analysis.yaml
- confidence: cited
  path: spec/domains/score.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-01T15:25:04Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-11-01T15:25:04Z", actor: "human:timo", revision: 8}
---
# Monophonic note estimation

## Outcome

For external monophonic audio, Patchback estimates note onsets, pitches, gate lengths and tails
with confidence scores, refuses low-confidence and polyphonic input with a diagnostic, and writes
the estimate as a note sequence that records the estimation it came from; a sequence naming an
unknown estimation is refused.

## DESIGN.md

§ 6.1, § 6.2, § 10 (V1), § 13 Milestone C.

## Acceptance

- spec/scenarios/note-estimation-estimated.yaml
- spec/scenarios/note-estimation-low-confidence.yaml
- spec/scenarios/note-estimation-polyphonic.yaml
- spec/scenarios/note-events-from-estimation.yaml

## Numeric obligations

Set by Dc1-9 in approval-record:wave-c1, before implementation: 32 seeded synthesized phrases of 2 to 8 notes, MIDI 36 to 84; detection at least 95 %, spurious at most 5 %; onset within 256 samples for 95 %; exact MIDI pitch for 98 % and f0 within 10 cents; gate end within 1,024 samples and tail end within 2,400 samples for 90 %; no quantity outside tolerance at or above its confidence threshold, at least 90 % inside tolerance at or above it.

## Open questions

Answered by Dc1-1 to Dc1-11 in approval-record:wave-c1: per-quantity confidence; velocity estimated (relative peak level); polyphony refused with a detector; one provisional threshold per quantity; a sequence written from an estimation equals its notes; `patchback estimate` writes `estimation.txt` and `notes.events`; deterministic estimation id; `--estimation` binds sequences in `render` and `fit`.

## Scope

Derived 2026-11-01 by `story-scoper` against `wave/c1` at ab1762c (release 0.11.0). Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong). The open points it raised are answered by Dc1-1 to Dc1-11 in `approval-record:wave-c1`.

- **Primary surface:** `crates/patchback-analysis`. Its manifest describes it as "pitch, onset and feature extraction" (`Cargo.toml:3`); the estimator builds on `estimate_f0` (`src/pitch.rs:128-134`), per-frame `spectral_flux`, `rms` and `peak` in `FrameDescriptors` (`src/analyze.rs:44-56`) and `SILENCE_THRESHOLD_DBFS` (`src/lib.rs:121-124`). No onset picking, segmentation, gate-end or tail-end code exists — cited
- **Files to add:** `crates/patchback-analysis/src/estimate.rs` (`EstimatedNote`, `NoteEstimation`, `estimate_notes`, the provisional constants) — inferred
- **Files to edit:** `crates/patchback-analysis/src/lib.rs:50-86` (module, re-exports) and `:37-48` (provisional values) — cited for the pattern, inferred for the edit
- **Files to edit:** `crates/patchback-analysis/src/error.rs`, a new `EstimationError` with `DOMAIN_ERRORS`, `domain_error`, `examples`, after `AnalysisError` (`:14-72`); `PolyphonicInput`, `LowConfidenceNotes { notes }` (`spec/domains/analysis.yaml:468-486`) — inferred
- **Files to edit:** `crates/patchback-analysis/tests/domain.rs`, a section from `EstimateNotes` to `\nevents:\n` (the normalization section ends at the `EstimateNotes` marker, `:256-262`, `:311`) — cited
- **Files to add:** `crates/patchback-analysis/tests/estimate.rs` (scenarios and the Dc1-9 numeric obligations) — inferred
- **Also likely:** `crates/patchback-analysis/Cargo.toml` `[dev-dependencies] patchback-dsp` to render reference phrases; no cycle; the gate admits `patchback-*` path crates (`crates/patchback-cli/tests/dependencies.rs:442-507`) — inferred
- **Files to add/edit (Dc1-6, Dc1-8):** `crates/patchback-cli/src/estimate.rs`, `crates/patchback-cli/src/main.rs:1-28`, `:55-114`, `crates/patchback-cli/tests/dispatch.rs:9-17` (`TOP_LEVEL`), `crates/patchback-cli/tests/estimate.rs`, and the `--estimation` flag in `crates/patchback-cli/src/render.rs` and `src/fit.rs` — inferred
- **Binding seam (Dc1-8):** the parser leaves `note_estimation_id` to "the caller that binds the sequence" (`crates/patchback-core/src/score.rs:35-38`, `:111-125`); no Rust type names `NoteEstimationNotFound` today — cited for the gap
- **Not edited:** the `notes.events` grammar and writer, which already carry `note_estimation_id` (`crates/patchback-core/src/score.rs:18`, `:85-86`, `:388-391`) — cited
- **Contract:** `spec/domains/analysis.yaml:64-91` (`EstimatedNote`), `:239-264` (`NoteEstimation`), `:468-486` (errors), `:935-975` (`EstimateNotes`), `:1030-1035` (`NotesEstimated`), `:1122-1130`; `spec/domains/score.yaml:76-93`, `:225-229`, `:369-376`; edits per Dc1-2, Dc1-4, Dc1-5 — cited
- **Documents (coordinator):** DESIGN.md § 10, § 11 (`:402-411`), revision line; `crates/patchback-docs/src/status.rs:343-349`; `website/docs/concepts/events.md:34-44`, `guides/fit-a-recording.md:30-32`, `reference/cli.md`, `reference/files.md:155-227`; `CHANGELOG.md`, `AGENTS.md:16` — cited
- **Confidence:** medium. The spec contract and the analysis primitives are cited to the line; the Rust landing follows the Dc1 decisions, not a source.
- **Would collide with:** any unit on `crates/patchback-analysis/src/lib.rs`, `src/error.rs`, `tests/domain.rs`, `crates/patchback-cli/src/main.rs`, `src/render.rs`, `src/fit.rs` or `tests/dispatch.rs`. Unit 2 of wave c1 touches none of them — inferred
- **Safety fact:** `estimate_f0` and its constants feed `analyze` (`src/analyze.rs:151`), the fit (`patchback-fit/src/search.rs:476`), the frozen `features/1` (`features.rs:723-735`) and `website/data/demos.json`; Dc1-10 keeps them unchanged — cited
