---
format: aep.planning-md/3
id: review-result:analysis-core-adversary-3
kind: review-result
status: active
title: Adversary pass 3 on the analysis core
relations:
- reviews: story:analysis-core
revision: 1
---
# Adversary pass 3 on story:analysis-core

Reviewer: `aep:adversary` (Opus, effort high), wave a3, 2026-10-23. Subject: the implementor's
uncommitted working tree on `impl/analysis-core` after correction round 3, base
`744c25033a0feb240cdbe92ac2a22da1599d27d1`. Focus: the period refinement and the octave rule in
`pitch.rs`.

Verdict: NEEDS-CHANGE. Cases executed 289→301, red 3 (in the new
`crates/patchback-analysis/tests/adversary3.rs`). Origin: introduced 3, pre-existing 0.

```findings
- file: crates/patchback-analysis/src/pitch.rs
  line: 141
  category: property
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "A naive 10% pulse at periods 24.5-34.5 samples has its first dip at twice the period and nothing halves it: 1959.18 Hz reported as 979.59 Hz, confidence 1.000."
- file: crates/patchback-analysis/src/pitch.rs
  line: 178
  category: property
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "Alias power of a naive 10% pulse passes the -20 dB sub-harmonic test at periods 40.5-44.5, so a correct period is doubled: 1078.65 Hz reported as 539.32 Hz, confidence 1.000."
- file: crates/patchback-analysis/src/pitch.rs
  line: 76
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "A fundamental 20-30 dB under its second harmonic is reported an octave up with confidence 0.977-0.998 (55 Hz as 110.00 Hz), against the pass-2 routing."
```

Attacked without breaking: saw, square, 25% pulse and triangle at half-integer periods 24.5-959.5;
triangle on the 401-pitch grid; fundamentals 12-19 dB under; a missing fundamental; a -10 dB
sub-octave; a fifth; a 30-cent vibrato; the too-short lengths; the summed STFT bound.

## Routing (coordinator)

- All three rows: one correction round whose rule is that an octave error is never reported at a
  confidence over the threshold. Where the estimator cannot tell P from P/2 or 2P, it refuses as
  `PitchAmbiguous` (DESIGN.md § 6.2, low-confidence refusal). For a pulse narrower than 25% and a
  fundamental 20 dB or more under its second harmonic, the true pitch or `PitchAmbiguous` both
  pass; the adversary's pulse cases accept either.
- After that round, the stated accuracy domain (waveforms and harmonic balance) is written in the
  `pitch.rs` doc. A further adversary pass is not run on this unit in wave a3; Milestone A's
  references are rendered by the Patchback voice (§ 13), whose oscillator is a saw.
