---
format: aep.planning-md/3
id: story:note-events
kind: story
status: implemented
title: Note events and the .events file
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:patch-file-format
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: inferred
  path: crates/patchback-core
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T18:30:09Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-23T18:30:09Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-23T21:47:30Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Note events and the .events file

## Outcome

A versioned `.events` file loads into a note sequence of sample-timestamped note events at the
patch sample rate. Notes with end before start, a MIDI note outside 0–127, a velocity outside the
accepted range, overlapping notes in a monophonic sequence, an unknown version, a malformed number
or a file past the parser's length bound are refused and load no sequence.

## DESIGN.md

§ 10 (NoteEvent, V0 known events), § 11, § 12.3 (note-count bound).

## Acceptance

- spec/scenarios/note-events-loaded.yaml
- spec/scenarios/note-events-invalid-notes.yaml
- spec/scenarios/note-events-overlap-and-version.yaml
- spec/scenarios/note-events-parser-refusals.yaml

## Numeric obligations

- Event offsets are exact sample positions; rendering honours them sample-accurately (§ 5.3, § 10).
- Write then parse is identity (§ 11).

## Scope

- `crates/patchback-core` (inferred: § 4.1 names no score or events crate; the `.events` parser
  shares the line-oriented parser of the `.patch` format, which lands in `patchback-core`). Because
  it shares the crate with patch-file-format and patch-graph-validation, a wave keeps them apart.

## Open questions

- Velocity range: normalized [0, 1] (modelled) or MIDI 0–127.
- A sequence bound to one patch (modelled) versus shared by every candidate of a fit (§ 9.2).
- Whether control automation shares `notes.events` or has its own file
  (decision-blocker:control-events-file).
- The number grammar and the note-count bound have no values (§ 11, § 12.3).
- Whether note events get their own crate instead of `patchback-core` (§ 4.1 is silent).
