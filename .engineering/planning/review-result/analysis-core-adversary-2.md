---
format: aep.planning-md/3
id: review-result:analysis-core-adversary-2
kind: review-result
status: active
title: Adversary pass 2 on the analysis core
relations:
- reviews: story:analysis-core
revision: 1
---
# Adversary pass 2 on story:analysis-core

Reviewer: `aep:adversary` (Opus, effort high), wave a3, 2026-10-23. Subject: the implementor's
uncommitted working tree on `impl/analysis-core` after correction round 2, base
`744c25033a0feb240cdbe92ac2a22da1599d27d1`.

Verdict: NEEDS-CHANGE. Cases executed 269→283, red 6 (in the new
`crates/patchback-analysis/tests/adversary2.rs`; `crates/patchback-cli/tests/adversary_cli2.rs`
is green). One more case kills a mutant the suite let survive. Origin: introduced 9,
pre-existing 0.

```findings
- file: crates/patchback-analysis/src/pitch.rs
  line: 33
  category: property
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "The 5-cent naive-saw claim fails on a 401-pitch grid: 1823.81 Hz is reported as 1830.86 Hz (+6.7 cents) and 1945.43 Hz as 1938.27 Hz (-6.4 cents); squares reach -6.0 cents."
- file: crates/patchback-analysis/src/pitch.rs
  line: 120
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "Pitches 1 to 5 cents below 50 Hz are refused as PitchOutOfRange because the lag search ends at 960, contradicting F0_RANGE_TOLERANCE_CENTS, which the top edge honours."
- file: crates/patchback-analysis/src/pitch.rs
  line: 67
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "ClipTooShortForPitch reports required_frame_count 2074 for a 2500-sample clip; the spec says too short to search the whole lag range (3009)."
- file: crates/patchback-analysis/src/pitch.rs
  line: 105
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "A 220 Hz tone with its fundamental 12 dB below the second harmonic is reported as 440.05 Hz with confidence 0.885 (DESIGN.md section 6.2, surfaced not guessed)."
- file: crates/patchback-analysis/src/descriptors.rs
  line: 268
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "Normalising before squaring underflows a band 200 orders of magnitude below the peak to the -120 dB floor; analyze never produces such a spectrum."
- file: crates/patchback-analysis/src/stft.rs
  line: 58
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Changing the magnitude-bound check from > to >= survives the implementor's suite and pass 1."
- file: spec/domains/analysis.yaml
  line: 307
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "FrameCountOverBound has fields fft_lengths and stft_frame_count in the spec, fft_length and frame_count in Rust."
- file: crates/patchback-analysis/src/analyze.rs
  line: 518
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "The magnitude bound is checked per STFT, so an AnalysisConfig with N entries is bounded at N times MAX_STFT_VALUES."
- file: crates/patchback-analysis/src/pitch.rs
  line: 105
  category: property
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Square and 25% pulse waves are off by up to 6.2 cents above 1.5 kHz."
```

## Routing (coordinator)

- Accuracy above 1.5 kHz (rows 1, 9): correction round. Hold 5 cents on the dense grid for saw,
  square and pulse by a documented refinement; if no std-only method holds it, state a
  frequency-dependent tolerance in the `pitch.rs` doc, and the dense-grid tests hold
  that stated tolerance.
- Low edge (row 2): the lag search covers the tolerance band at both edges.
- Too-short refusal (row 3): `required_frame_count` is the length that searches the whole lag
  range, as the spec says.
- Weak fundamental (row 4): report the true period or refuse as ambiguous; never the octave above
  at a confidence over the threshold.
- Row 5: no change; the band function documents its dynamic range.
- Rows 6: the case stays. Row 7: the spec and Rust use the same field names. Row 8: the bound is
  checked over the whole config before any allocation.
