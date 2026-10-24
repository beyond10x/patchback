---
format: aep.planning-md/3
id: story:subtractive-voice-render
kind: story
status: implemented
title: Render the subtractive voice
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:patch-graph-validation
- depends_on: story:note-events
- depends_on: story:wav-io
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: crates/patchback-cli/Cargo.toml
- confidence: cited
  path: crates/patchback-cli/src/main.rs
- confidence: cited
  path: crates/patchback-cli/src/render.rs
- confidence: inferred
  path: crates/patchback-dsp
- confidence: cited
  path: crates/patchback-dsp/Cargo.toml
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-24T18:57:05Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-24T18:57:05Z", actor: "human:timo", revision: 9}
- {from: "active", to: "implemented", at: "2026-10-24T20:36:40Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":2,"review_outcome":2,"verification":1}}}
---
# Render the subtractive voice

## Outcome

`patchback render --patch --notes --out` runs a compiled `subtractive-mono/1` patch (polyBLEP saw,
drive, TPT/SVF lowpass, ADSR, VCA) against a note sequence and produces a 48 kHz mono render. Output
limiting is reported, never silent; a render that would emit a non-finite sample fails and leaves
nothing; an unknown or uncompiled patch, an unknown note sequence and an unsupported configuration
are refused.

## DESIGN.md

§ 2 invariants 1, 4, 5, § 5.2, § 5.3, § 11, § 12.1, § 12.2 (DSP engine gate).

## Acceptance

- spec/scenarios/subtractive-voice-render-renders.yaml
- spec/scenarios/subtractive-voice-render-limited.yaml
- spec/scenarios/subtractive-voice-render-refused-configuration.yaml
- spec/scenarios/subtractive-voice-render-refused-patch.yaml
- spec/scenarios/subtractive-voice-render-non-finite.yaml

## Numeric obligations

- Bounded input and controls give finite samples only (§ 2 invariant 4, § 12.2).
- Same patch, events, seed and configuration render bit-identical output (§ 2 invariant 5).
- Golden waveform, ADSR and filter tests pass; ADSR handles zero and near-zero times (§ 5.3,
  § 12.1, § 12.2).
- Oscillator phase is continuous; high-note aliasing with polyBLEP is measured (§ 5.3).
- Filter cutoff clamped below Nyquist, resonance bounded, filter stable; gain smoothed (§ 5.3).
- Note events take effect at their exact sample offset (§ 5.3).
- The limiter threshold is stated and every limited sample counted in the report (§ 5.3).

## Open questions

- Whether a note sequence must be written for the rendered patch (§ 10 vs § 9.2).
- Block size has default 256 and no upper bound (§ 5.3).
- A failed render leaves nothing (modelled) or a Failed render record.
- Port names per node kind (placeholders).
