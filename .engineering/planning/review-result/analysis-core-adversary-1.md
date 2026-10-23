---
format: aep.planning-md/3
id: review-result:analysis-core-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on the analysis core
relations:
- reviews: story:analysis-core
revision: 1
---
# Adversary pass 1 on story:analysis-core

Reviewer: `aep:adversary` (Opus, effort high), wave a3, 2026-10-23. Subject: the implementor's
uncommitted working tree on `impl/analysis-core`, base `744c25033a0feb240cdbe92ac2a22da1599d27d1`.

Verdict: NEEDS-CHANGE. Cases executed 244→263, red 8 (in the new
`crates/patchback-analysis/tests/adversary.rs` and `crates/patchback-cli/tests/adversary_cli.rs`);
5 more cases are green but kill mutants the unit's own suite let survive. Origin: introduced 14,
pre-existing 0.

```findings
- file: crates/patchback-analysis/src/descriptors.rs
  line: 123
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "log_band_energies floors the spectrum scale at -120 dB before adding band power, so quiet frames report band energies up to 33 dB too high, including the last frame of default analyze runs."
- file: crates/patchback-analysis/src/pitch.rs
  line: 71
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "A pitch above F0_MAX_HZ is reported one or more octaves down with confidence up to 1.0 instead of refused (C7 2093 Hz -> 2042.6 Hz, 3000 Hz -> 1500.7 Hz)."
- file: crates/patchback-analysis/src/pitch.rs
  line: 73
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "A pitch just below F0_MIN_HZ is pinned to the max_lag edge and reported as 49.97 Hz with confidence above 0.97 (48 Hz, 49 Hz)."
- file: crates/patchback-analysis/tests/numeric.rs
  category: property
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The stated 5-cent saw tolerance holds at the four tested frequencies but not across the range; a 1588 Hz naive saw is 5.9 cents off."
- file: crates/patchback-analysis/src/pitch.rs
  line: 45
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Any clip shorter than 3009 samples is refused as PitchAmbiguous with confidence 0 even when cleanly pitched, so the diagnostic names the wrong cause."
- file: crates/patchback-analysis/src/descriptors.rs
  line: 64
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "spectral_centroid and spectral_rolloff return infinity for a finite bin_hz of f64::MAX, against the crate's finite-or-refused contract; analyze never passes such a bin width."
- file: crates/patchback-cli/src/analyze.rs
  line: 69
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "A path containing a newline splits the documented one-line refusal over two stderr lines."
- file: crates/patchback-analysis/src/stft.rs
  line: 35
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Reading every STFT frame from sample 0 survives the implementor's suite."
- file: crates/patchback-analysis/src/analyze.rs
  line: 166
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Computing flux against no previous spectrum for every frame survives the implementor's suite."
- file: crates/patchback-analysis/src/analyze.rs
  line: 118
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Refusing a clip of exactly MAX_DURATION_FRAMES survives the implementor's suite."
- file: crates/patchback-analysis/src/fft.rs
  line: 17
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Refusing an FFT of exactly MAX_FFT_LENGTH survives the implementor's suite."
- file: crates/patchback-cli/src/analyze.rs
  line: 182
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Swapping the rms and peak columns of the analysis file survives the implementor's suite."
- file: crates/patchback-analysis/src/lib.rs
  line: 10
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The crate doc says Hann energy is 3n/8 for n >= 2, which is false at n = 2 (energy 1, not 0.75)."
- file: crates/patchback-analysis/src/analyze.rs
  line: 98
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "The public AnalysisConfig has no hop or frame-count bound, so hop 1 with FFT 65536 on a 60 s clip asks for about 755 GB; only the default config is reachable from the CLI."
```

## Routing (coordinator)

- Band energies: correction round; the scale is not floored before the band power is added.
- Pitch outside the search range (above and below): refused, never reported at an edge or an octave
  away (DESIGN.md § 6.2, "surfaced rather than guessed away"). A new error `PitchOutOfRange` in
  `spec/domains/analysis.yaml`.
- A clip shorter than the f0 window: its own error `ClipTooShortForPitch` in the domain, not
  `PitchAmbiguous`.
- Saw tolerance: tested across 50–2000 Hz; the stated tolerance is the one the test holds.
- Non-finite descriptor result: a typed refusal. One-line refusal: the path is escaped.
- `AnalysisConfig`: a named frame-count bound, checked before allocation.
- Crate doc: n >= 3. The adversary's cases stay in the suite and hold the mutants.
