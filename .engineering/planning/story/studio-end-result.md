---
format: aep.planning-md/3
id: story:studio-end-result
kind: story
status: draft
title: End result view
relations:
- decomposes: epic:patch-studio
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:studio-patch-graph
- depends_on: story:fit-run-artifacts
revision: 1
---
# End result view

## Outcome

The studio shows the outcome of a fit: the retained patch, its parameters against the reference,
and the reference and render spectra at an analysis FFT length. Source:
`patchback.studio.ComposeResultView`, event `ResultComposed`. A run with no retained patch or no
render has no end result; spectra are drawn only at 512, 2048 or 8192 samples.

## DESIGN.md

§ 6.3 (FFT lengths), § 9.2 step 5, § 11.

## Acceptance

- spec/scenarios/studio-end-result-composed.yaml
- spec/scenarios/studio-end-result-no-retained-patch.yaml
- spec/scenarios/studio-end-result-unsupported-size.yaml

## Numeric obligations

- Spectra only at FFT lengths 512, 2048, 8192 (§ 6.3).

## Open questions

- Where spectra are computed (Rust analysis artifacts or the browser); § 11 names no spectrum file.
  The prototype draws an analytic approximation.
