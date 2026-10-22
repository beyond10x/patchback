---
format: aep.planning-md/3
id: story:audio-comparison
kind: story
status: draft
title: Audio comparison loss
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:analysis-core
- depends_on: story:workspace-scaffold
scope:
- confidence: inferred
  path: crates/patchback-fit
revision: 3
---
# Audio comparison loss

## Outcome

`patchback compare --reference --candidate` aligns a candidate render with the reference and
records the weighted loss with its four components (log-spectral, envelope, transient, band
energy). Clips at different sample rates and lengths outside what alignment can absorb are refused.

## DESIGN.md

§ 9.1, § 11, § 12.1.

## Acceptance

- spec/scenarios/audio-comparison-records-components.yaml
- spec/scenarios/audio-comparison-sample-rate-mismatch.yaml
- spec/scenarios/audio-comparison-length-outside-alignment.yaml
- spec/scenarios/audio-comparison-unknown-reference.yaml

## Numeric obligations

- L = 0.45 log-spectral + 0.25 envelope + 0.15 transient + 0.15 band energy, components logged per
  candidate (§ 9.1).
- Log-spectral term is multi-resolution over `log(epsilon + |STFT|)`; alignment precedes the loss
  (§ 9.1).
- Identical clips give loss 0; every component is finite (§ 2 invariant 4).

## Open questions

- Constraints on the loss weights (non-negative, summing to 1, or free).
- Length tolerance alignment can absorb.
- The sample-rate refusal is unreachable while every clip is 48 kHz: keep or drop.
