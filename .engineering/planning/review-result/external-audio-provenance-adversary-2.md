---
format: aep.planning-md/3
id: review-result:external-audio-provenance-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: external-audio-provenance'
relations:
- reviews: story:external-audio-provenance
revision: 1
---
# Adversary pass 2: story:external-audio-provenance

Wave a5, unit 2. `aep:adversary` on Opus against the corrected uncommitted change in tree
`patchback-impl-external-audio-provenance` (base c338101af4d635bdf3377e91f8a35d21a3adcdf4), after
correction round 1 (decisions D6, D7, D13), 2026-10-24.

## Result

- verdict: green; cases executed 84 → 90, red 0; origin: introduced 3, pre-existing 0, undecided 2.
- Suite (`cargo test --locked -p patchback-audio`) exit 0: adversary 9, allocation 4, domain 2,
  provenance 15, provenance_adversary 16, provenance_adversary2 6, wav 38. clippy `-D warnings`
  and fmt exit 0.
- Added `crates/patchback-audio/tests/provenance_adversary2.rs` only (6 passing guards). The
  implementor's 4 tracked files (+372/−103) are unchanged. Every mutant ran on a copy outside
  the worktree.
- Pass-1 findings: the two red cases were wrong about the order (D6) and now pass with the
  documented order; the precedence test now cites the ESS pages and pins the whole list, and
  three YAML mutants (format before rate, a new input refusal, `before_row` on too-long) each
  make it fail; the empty-string judgement is carried as D7; the fieldless-guard judgement is
  carried as INFEASIBLE.
- Mutants m1–m5 (`first` tie and order, `File::open` or a read before the guards) are killed by
  the correction's own tests. Mutants m6 (EmptyAudio judged on a second `data` chunk) and m7
  (header facts from a second `fmt `) survive the correction's suite and die only in the new
  guards.
- The "13 faults × 4 refusals = 50, 2 skipped" count holds; three faults missing from that matrix
  (a `data` chunk past the RIFF size, a `fmt ` past the RIFF size after the data, a `fmt ` cut
  after 16 bytes) are now guarded.
- Judgement: "the first `fmt ` and the first `data` chunk supply the header facts" is a reader
  convention `spec/domains/audio.yaml` does not state; it decides between `CorruptRiffChunk` and a
  predicate refusal for a file with two differing `fmt ` chunks.
- Could not break: chunk-walk termination (the cursor advances at least 8 bytes per chunk),
  allocation on duplicate chunks (none), the pipe path with a pending guard (nothing consumed), a
  mode-000 file (guard first, `ReadError::Io` only once both guards pass), WAVE_FORMAT_EXTENSIBLE
  coverage; `Truncation::Read` is not buildable as a static file.

## Coordinator's routing

- Green; no third pass (the wave protocol's budget is two).
- The judgement → decision D15: the convention stays as the reader's doc comment states it and is
  recorded as an open point for the specification's owner, beside D7; no code change.
- The guards stay.

Agent cost: 151,629 tokens, 34 tool uses, 779,428 ms. Full report:
`~/.cache/scratch/0905d614-e063-463d-a99e-28052c6bae4a/a5/external-audio-provenance/adversary-report-2.md`
(scratch, deleted 48 h after the session).

```findings
[
{"file":"crates/patchback-audio/src/read.rs","line":511,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"mutant 'EmptyAudio judged on the second data chunk' survives the correction's suite; killed only by tests/provenance_adversary2.rs:231 (passing regression guard)"},
{"file":"crates/patchback-audio/src/read.rs","line":495,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"mutant 'header facts from the second fmt chunk' survives the correction's suite; killed only by tests/provenance_adversary2.rs:167 and :264 (passing regression guards)"},
{"file":"crates/patchback-audio/src/read.rs","line":418,"category":"judgement","severity":"note","verdict":"NEEDS-CHANGE","origin":"undecided","message":"which of two fmt/data chunks supplies the header facts ('the first counts') is a reader convention the spec does not state; it decides between CorruptRiffChunk and a predicate"},
{"file":"crates/patchback-audio/src/import.rs","line":99,"category":"judgement","severity":"note","verdict":"NEEDS-CHANGE","origin":"undecided","message":"carried from pass 1 (D7): an empty provenance or licence string counts as recorded; owner decision open"},
{"file":"crates/patchback-audio/src/error.rs","line":96,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"carried from pass 1: fieldless ProvenanceMissing and LicenceMissing carry no path; the CLI cannot reach them today"}
]
```
