---
format: aep.planning-md/3
id: review-result:subtractive-voice-render-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: subtractive-voice-render'
relations:
- reviews: story:subtractive-voice-render
revision: 1
---
# Adversary pass 2: story:subtractive-voice-render

Wave a5, unit 1. `aep:adversary` on Opus against the corrected uncommitted change in tree
`patchback-impl-subtractive-voice-render` (base c338101af4d635bdf3377e91f8a35d21a3adcdf4), after
correction round 1 (decisions D11, D12), 2026-10-24.

## Result

- verdict: red; cases executed 188 → 195 (cli 143 → 145, dsp 45 → 50), red 2; origin: introduced 3,
  pre-existing 0.
- Suite (`cargo test --locked -p patchback-dsp -p patchback-cli --no-fail-fast`) exit 101: new lane
  `render_adversary2` 0 passed 2 failed; every other lane ok (cli render 18, render_adversary 5,
  dependencies 39, dispatch 22; dsp adversary 4, adversary2 5, render 17, domain 3, units 21).
  fmt and clippy `-D warnings` exit 0 on the adversary's files.
- Added `crates/patchback-cli/tests/render_adversary2.rs` (2 red cases) and
  `crates/patchback-dsp/tests/adversary2.rs` (5 passing guards). The implementor's 7 tracked paths
  (+136/−19) are unchanged.
- Pass-1 findings: the scenario frame count is resolved; the `out` record no longer splits over
  two lines (resolved, with a new finding below); the zero-length note is refused and its
  assertion re-pinned exactly, not relaxed; the `RENDER_TAIL_FRAMES` doc is resolved and the tail
  is asserted to be exactly 0.0.
- The red cases: the `out` record escapes only control characters after a lossy `Path::display`,
  so the files `a\nb.wav` (a backslash) and a newline path print one record, and two non-UTF-8
  paths print one record; the doc promises the refusal's escaping, which uses `{path:?}`.
- Guards: the class test for hand-built sequences stays green under mutants dropping the lower
  velocity clamp, the MIDI 127 clamp or the stale note-off guard; the implementor's suite does not
  pin `InvalidNoteTiming`'s place after the block-size check and before the length check. Both
  are pinned by the adversary's dsp guards, which go red under the mutants.
- Could not break: refusal order and payloads, `end == start` and `end < start`, one bad note among
  good ones renders nothing, `DOMAIN_ERRORS`/`examples()`/the domain test, finiteness under NaN,
  infinite and `f32::MAX` velocities, exact silence after the release.

## Coordinator's routing

- Finding 1 → correction round 2, decision D14: the `out` value is printed with the refusal's own
  `{out:?}` quoting (a double-quoted path with `\\`, `\n`, `\"` and `\xNN` escapes), lossless and
  from one function; the module doc, `website/docs/reference/cli.md` and the two existing `out`
  tests are re-pinned to the quoted form. The coordinator verifies this correction (wave protocol:
  two adversary passes, the correction after the second is verified by the coordinator).
- Findings 2 and 3 → the guards stay in `crates/patchback-dsp/tests/adversary2.rs`.

Agent cost: 128,287 tokens, 40 tool uses, 605,422 ms. Full report:
`~/.cache/scratch/0905d614-e063-463d-a99e-28052c6bae4a/a5/subtractive-voice-render/adversary-report-2.md`
(scratch, deleted 48 h after the session).

```findings
[
{"file":"crates/patchback-cli/src/render.rs","line":292,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the out record escapes only control characters after a lossy Path::display, so a backslash path and a newline path, or two non-UTF-8 paths, print the same record, contrary to the documented 'as given, escaped as a refusal escapes it'"},
{"file":"crates/patchback-dsp/tests/render.rs","line":394,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the class test for hand-built sequences the loader refuses checks only finiteness and later silence, so dropping the lower velocity clamp, the MIDI 127 clamp or the stale note-off guard stays green; guards in adversary2.rs"},
{"file":"crates/patchback-dsp/src/render.rs","line":241,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the implementor's suite does not pin InvalidNoteTiming after the block-size check or before the length check; reordering either passes it; guard adversary2.rs:61"}
]
```
