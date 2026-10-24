---
format: aep.planning-md/3
id: review-result:external-audio-provenance-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: external-audio-provenance'
relations:
- reviews: story:external-audio-provenance
revision: 1
---
# Adversary pass 1: story:external-audio-provenance

Wave a5, unit 2. `aep:adversary` on Opus against the implementor's uncommitted change in tree
`patchback-impl-external-audio-provenance` (base c338101af4d635bdf3377e91f8a35d21a3adcdf4), 2026-10-24.

## Result

- verdict: red; cases executed 67 → 71, red 2; origin: introduced 1, pre-existing 1.
- Suite (`cargo test --locked -p patchback-audio --no-fail-fast`): adversary 9 ok, allocation 4 ok,
  domain 2 ok, provenance 14 ok, provenance_adversary 2 passed 2 failed, wav 38 ok. clippy
  `--tests -D warnings` exit 0, fmt exit 0.
- Added `crates/patchback-audio/tests/provenance_adversary.rs` only (2 red cases, 2 passing guards:
  a stereo external file is downmixed and keeps provenance and licence; a sparse file one frame
  past `MAX_FRAMES` answers the guard on file and pipe and `InputTooLong` once both guards hold).
- The two red cases: an external import of a file with a corrupt `fmt`/`data` and an unsupported
  rate answers `ProvenanceMissing`/`LicenceMissing`; a plain decode of the same file answers
  `UnsupportedSampleRate`. The adversary expected `CorruptRiffChunk` in both from `before_row`.
- Judgement: an empty provenance or licence string counts as recorded; the precedence test encodes
  an evaluation order no ESS source confirms; fieldless `ProvenanceMissing`/`LicenceMissing` carry
  no path and the CLI cannot reach them.
- Mutants traced and caught by the existing 14 tests: guard order swapped, `ranked` arms, pipe
  path ignoring a pending guard, `imported` dropped, precedence default.

## Coordinator's routing

- The ESS evaluation order is documented (`ess` repository,
  `website/docs/guides/specify/guards-and-predicates.md` § "Which branch answers";
  `website/docs/reference/diagnostics.md` ESS-AUTHOR-041): input refusals first in declaration
  order, then existence, then held state, then accepting and external branches in declaration
  order; `before_row` externals sit between the input refusals and existence. Both the
  implementor's order and the adversary's expectation were wrong.
- Findings 1 and 2 → correction round 1, decision D6: `IMPORT_WAV_PRECEDENCE` follows the documented
  order (ProvenanceMissing, LicenceMissing, UnsupportedSampleRate, UnsupportedSampleFormat,
  UnsupportedChannelCount, EmptyAudio, TruncatedRiff, CorruptRiffChunk, [RenderNotFound never
  emitted], InputTooLong, NonFiniteSample); guards answered before bytes are read or the file is
  opened; plain `decode`/`read_wav` take the same order from the rate on; the only behaviour
  change against the base is for files with two faults, recorded under CHANGELOG `### Changed`.
  The adversary's two cases are rewritten to the documented order and two-fault cases are added
  for every adjacent pair.
- Finding 3 → decision D7: an empty string counts as given, as the model's `defined()` says;
  recorded as an open point for the specification's owner, no code change.
- Finding 4 → closed by D6: the precedence test now derives the order from `spec/domains/audio.yaml`
  with the ESS rule and cites the ESS pages.
- Finding 5 → no change: fieldless errors stay (first-round decision).

Agent cost: 95,349 tokens, 31 tool uses, 367,151 ms. Full report:
`~/.cache/scratch/0905d614-e063-463d-a99e-28052c6bae4a/a5/external-audio-provenance/adversary-report-1.md`
(scratch, deleted 48 h after the session).

```findings
[
{"file":"crates/patchback-audio/src/read.rs","line":485,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"external import of a file with a corrupt fmt/data chunk and an unsupported rate answers ProvenanceMissing or LicenceMissing where the adversary expected CorruptRiffChunk; the implementation's precedence order is not the documented ESS order"},
{"file":"crates/patchback-audio/src/read.rs","line":496,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"plain decode and read_wav of a file with a corrupt fmt/data chunk and an unsupported rate answer UnsupportedSampleRate; check_fmt's doc claims the domain's order but the order is not derived from the ESS rule"},
{"file":"crates/patchback-audio/src/import.rs","line":96,"category":"judgement","severity":"note","verdict":"NEEDS-CHANGE","origin":"undecided","message":"an empty provenance or licence string counts as recorded; the model's defined() agrees, so the specification's owner must decide"},
{"file":"crates/patchback-audio/tests/provenance.rs","line":311,"category":"judgement","severity":"note","verdict":"NEEDS-CHANGE","origin":"undecided","message":"the precedence test checks the YAML against an assumed ESS evaluation order that no ESS source the adversary found confirms"},
{"file":"crates/patchback-audio/src/error.rs","line":90,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"fieldless ProvenanceMissing and LicenceMissing carry no path and the CLI cannot reach them today"}
]
```
