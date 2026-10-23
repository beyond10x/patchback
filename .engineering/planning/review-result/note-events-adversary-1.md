---
format: aep.planning-md/3
id: review-result:note-events-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on note events
relations:
- reviews: story:note-events
revision: 1
---
# Adversary pass 1 on story:note-events

Reviewer: `aep:adversary` (Opus, effort high), wave a4, 2026-10-23. Subject: the implementor's
uncommitted working tree on `impl/note-events`, base `d3b61dbb4c97d18e8c6ebf5261018dc0ef1f64c0`.

Verdict: NEEDS-CHANGE. Cases executed 468→473, red 3 (in the new
`crates/patchback-core/tests/note_events_adversary.rs`); two more cases kill mutants the suite let
survive. Origin: introduced 5, pre-existing 0.

```findings
- file: crates/patchback-core/src/score.rs
  line: 379
  category: contract-drift
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "write_events accepts a patch_id or note_estimation_id ending in a newline and emits a blank line the parser refuses, against its own doc; no in-tree caller builds such an id."
- file: crates/patchback-core/src/score.rs
  line: 379
  category: acceptance
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "write_events accepts identifiers with leading or trailing space, tab or CR, and the parse returns a different id, breaking write-then-parse identity."
- file: crates/patchback-core/src/score.rs
  line: 604
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "Two overlapping notes past MAX_NOTES are refused as TooManyNotes although score.yaml ranks overlap before unbounded-length."
- file: crates/patchback-core/src/score.rs
  line: 408
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The mutant bytes.len() >= MAX_FILE_BYTES survives the implementor's suite."
- file: crates/patchback-core/src/score.rs
  line: 765
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Dropping token.len() == 36 from uuid() survives the implementor's suite."
- file: spec/domains/score.yaml
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The guards start_sample < 0 and midi_note < 0 cannot fire under the unsigned grammar (MalformedNumber), and the domain carries no marker saying so."
```

Attacked without breaking: u64 sample extremes; overlap in either file order, equal and abutting
starts; precedence tie-breaks; number grammar (leading zeros, signs, -0, nan/inf, dangling
exponents); CRLF, stray CR, trailing whitespace, BOM; velocity edges; the bound mutants the suite
already kills.

## Routing (coordinator)

- Rows 1-2: the writer checks each identifier as the parser does (`uuid()` on the raw value) and
  refuses one that would not read back; a test for each shape.
- Row 3: the domain's order holds. The parser keeps checking timing and overlap for notes past
  `MAX_NOTES` (the file bound already caps the work) and answers the highest-ranked fault.
- Rows 4-5: the cases stay. Row 6: an `UNMAPPED:`-style note at both guards in score.yaml saying the
  unsigned grammar answers negative values as MalformedNumber.
