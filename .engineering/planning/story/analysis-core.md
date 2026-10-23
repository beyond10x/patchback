---
format: aep.planning-md/3
id: story:analysis-core
kind: story
status: implemented
title: 'Analysis core: FFT, STFT and descriptors'
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:wav-io
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: inferred
  path: crates/patchback-analysis
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T12:38:04Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T12:38:04Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-23T18:04:23Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":3,"verification":1}}}
---
# Analysis core: FFT, STFT and descriptors

## Outcome

`patchback analyze --input --out` runs the internal radix-2 FFT, Hann-windowed STFT at the
analysis frame sizes, RMS and peak envelopes, spectral centroid, rolloff, flatness, log-band
energies, spectral flux and monophonic f0 estimation over a clip, and records the analysis. Silent
input and an ambiguous pitch are refused with a typed diagnostic; non-finite input and sizes past
the bounds are refused.

## DESIGN.md

§ 6.2, § 6.3 (FFT lengths 512, 2048, 8192), § 11, § 12.1 (FFT/STFT tests), § 12.3.

## Acceptance

- spec/scenarios/analysis-core-analyzed.yaml
- spec/scenarios/analysis-core-refused-input.yaml
- spec/scenarios/analysis-core-refused-bounds.yaml
- spec/scenarios/analysis-core-silence-and-pitch.yaml

## Numeric obligations

- FFT agrees with a direct small DFT reference; FFT then IFFT roundtrips (§ 12.1).
- A pure sine lands in its expected bin; Hann window energy is accounted for (§ 12.1).
- Descriptor values match hand-computed references on synthetic signals (§ 6.2).
- Non-finite input is detected and refused (§ 2 invariant 4).

## Open questions

- Does `analyze` re-read the WAV or take an imported clip.
- Input-duration and FFT-length bounds have no values (§ 12.3).
- Silence: refused (modelled) or completed and reported.
- f0 confidence threshold, and whether other descriptors are written when only f0 is refused.
- Hop/overlap convention (§ 6.3).
