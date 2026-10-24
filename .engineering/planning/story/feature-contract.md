---
format: aep.planning-md/3
id: story:feature-contract
kind: story
status: implemented
title: Feature contract features/1
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:analysis-core
- serves: vision:patchback
scope:
- confidence: cited
  path: crates/patchback-analysis
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-24T18:57:06Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-24T18:57:06Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-24T20:36:41Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":2,"review_outcome":2,"verification":1}}}
---
# Feature contract features/1

## Outcome

The `features/1` contract is published and frozen: 32 values (24 log-band energies over attack,
body and release windows, then RMS, peak, centroid, rolloff, flatness, zero-crossing rate,
attack-envelope slope and normalized MIDI pitch). Extraction from an analysis yields a vector of
exactly 32 finite values; a vector of the wrong count is refused; a frozen contract is changed only
by amending it to a new version.

## DESIGN.md

§ 6.3, § 7.1, § 12.1 (feature version pinning), § 12.3, § 13 Milestone B.

## Acceptance

- spec/scenarios/feature-contract-published.yaml
- spec/scenarios/feature-contract-wrong-count.yaml
- spec/scenarios/feature-contract-frozen.yaml
- spec/scenarios/feature-contract-vector-checks.yaml
- spec/scenarios/feature-contract-extract.yaml

## Numeric obligations

- Exactly 32 values in the contract's order, each finite; a feature that cannot be computed
  reliably fails with a structured diagnostic (§ 6.3, § 12.3).
- Analysis FFT lengths are 512, 2048, 8192 at 48 kHz; changing them versions the contract (§ 6.3).

## Open questions

- Order of the 24 band energies: window-major (recommended) or band-major.
- Window bounds, amplitude reference, band edges and missing-data behaviour have no values.
- Hop/overlap convention.
- `CheckFeatureVector` and `AmendFeatureContract` were added to the model; DESIGN.md does not name
  them.
