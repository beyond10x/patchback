---
format: aep.planning-md/3
id: review-result:audio-comparison-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on audio comparison
relations:
- reviews: story:audio-comparison
revision: 1
---
# Adversary pass 1 on story:audio-comparison

Reviewer: `aep:adversary` (Opus, effort high), wave a4, 2026-10-23. Subject: the implementor's
uncommitted working tree on `impl/audio-comparison`, base `d3b61dbb4c97d18e8c6ebf5261018dc0ef1f64c0`.

Verdict: NEEDS-CHANGE, nothing found in the code. Cases executed 474→489, red 1 (in the new
`crates/patchback-fit/tests/audio_comparison_adversary.rs`; 15 cases there and in
`crates/patchback-cli/tests/compare_adversary.rs`). Origin: introduced 0, pre-existing 1.

```findings
- file: spec/scenarios/audio-comparison-length-outside-alignment.yaml
  line: 40
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: "The acceptance scenario for LengthOutsideAlignment imports two 48000-frame clips, which the implementation compares (loss 0) rather than refuses."
```

Attacked without breaking: finiteness and the [0, 1] range on 18 hostile inputs; identical clips
give 0 per component, at the 60 s bound too; lag sign and the ±4800/4801 boundary; periodic pulse
trains; silence against sound; the printed loss equals the weighted sum; CLI key order, exit
statuses, one stderr line, stereo downmix, PCM16 against float32, empty and over-long WAVs;
`cross_correlation` against a direct sum; `analyze` unchanged.

## Routing (coordinator)

- Row 1: fixed in this unit. The candidate's `frame_count` becomes 52801 (48000 + 4801); the
  adversary's case stays.
