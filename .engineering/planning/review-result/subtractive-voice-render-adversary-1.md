---
format: aep.planning-md/3
id: review-result:subtractive-voice-render-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: subtractive-voice-render'
relations:
- reviews: story:subtractive-voice-render
revision: 1
---
# Adversary pass 1: story:subtractive-voice-render

Wave a5, unit 1. `aep:adversary` on Opus against the implementor's uncommitted change in tree
`patchback-impl-subtractive-voice-render` (base c338101af4d635bdf3377e91f8a35d21a3adcdf4), 2026-10-24.

## Result

- verdict: red; cases executed 176 → 185 (cli 137 → 142, dsp 39 → 43), red 3; origin: introduced 3,
  pre-existing 0.
- Suite after the cases: `cargo test --locked -p patchback-dsp -p patchback-cli --no-fail-fast`
  exit 101; `render_adversary` 3 passed 2 failed; dsp `adversary` 3 passed 1 failed; every other
  lane ok (dependencies 39, cli render 17, dsp render 15, units 21). clippy and fmt exit 0.
- Added `crates/patchback-cli/tests/render_adversary.rs` and `crates/patchback-dsp/tests/adversary.rs`
  (3 red cases, 6 passing guards: refusal-order pairings, `--block-size 9223372036854775808` exits 2,
  byte-identical limited renders across processes and block sizes, `MAX_FRAMES`+1 refused, an
  18 kHz polyBLEP golden).
- The three red cases: the `renders` scenario's `frame_count` 24000 against the binary's 43200
  (note + release + tail); the `out` path printed unescaped so a newline splits the record; a
  hand-built zero-length note sounding through the tail (unreachable through the CLI, whose loader
  refuses it).
- Could not break: refusal order (guards, then not-found, then state), determinism across block
  sizes and processes, the limiter count, filter stability at 20 Hz with Q 12, the cutoff clamps
  (20 Hz and 23 520 Hz lie outside the contract's 40 Hz to 16 kHz), the debug-only seam, no
  overwrite of an existing `--out`, nothing written on refusal, the additive rule.

## Coordinator's routing

- Finding 1 → the coordinator set the scenario's `frame_count` to 43200 and recorded why in the
  scenario header (DESIGN.md 6.1 counts the release into a recording); the render-length decision
  (D1) stands.
- Finding 2 → correction round 1, decision D11: the `out` value is escaped as refusals are.
- Finding 3 → correction round 1, decision D12: `render` refuses a note with `end <= start` as
  `RenderError::InvalidNoteTiming`, mapped to `patchback.audio.NoteSequenceNotFound` (no new
  domain error).
- Finding 4 (judgement) → the `RENDER_TAIL_FRAMES` doc comment corrected.

Agent cost: 174,932 tokens, 37 tool uses, 783,172 ms. Full report:
`~/.cache/scratch/0905d614-e063-463d-a99e-28052c6bae4a/a5/subtractive-voice-render/adversary-report-1.md`
(scratch, deleted 48 h after the session).

```findings
[
{"file":"spec/scenarios/subtractive-voice-render-renders.yaml","line":128,"category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the renders scenario imports the default render's out.wav with frame_count 24000 while the binary writes 43200 frames (note plus release plus tail), so scenario and implementation disagree on the render length"},
{"file":"crates/patchback-cli/src/render.rs","line":288,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the success report prints the out path unescaped, so a path holding a newline splits the out record and breaks the documented one key-value line per record"},
{"file":"crates/patchback-dsp/src/render.rs","line":344,"category":"boundary","severity":"warning","verdict":"INFEASIBLE","origin":"introduced","message":"a hand-built zero-length note never has its gate closed because its note-off sorts before its own note-on, so the voice sustains through the tail and the render is cut off at a non-zero level"},
{"file":"crates/patchback-dsp/src/render.rs","line":19,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"RENDER_TAIL_FRAMES claims to keep the filter ring-out but the filter precedes the VCA, so the tail is always exact silence"}
]
```
