---
format: aep.planning-md/3
id: story:bounded-search-fit
kind: story
status: implemented
title: Bounded derivative-free fit
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:subtractive-voice-render
- depends_on: story:audio-comparison
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: cited
  path: crates/patchback-cli/src/fit.rs
- confidence: inferred
  path: crates/patchback-cli/tests/fit.rs
- confidence: cited
  path: crates/patchback-fit
- confidence: cited
  path: crates/patchback-fit/src/error.rs
- confidence: cited
  path: crates/patchback-fit/src/lib.rs
- confidence: inferred
  path: crates/patchback-fit/src/search.rs
- confidence: inferred
  path: crates/patchback-fit/tests
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-24T20:47:32Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-10-24T20:47:32Z", actor: "human:timo", revision: 8}
- {from: "active", to: "implemented", at: "2026-10-24T22:51:31Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":2,"review_outcome":2,"verification":1}}}
---
# Bounded derivative-free fit

## Outcome

`patchback fit --input --out --max-renders` starts from a fixed safe default patch, runs seeded
bounded restarts of coordinate search within the known topology, retains only improvements, and
stops at the render bound, the wall-time bound or when improvement falls below the threshold. A fit
naming an unknown reference, note sequence, topology or analysis, or with invalid bounds, is refused
and no run starts; silence and pitch ambiguity end the run safely.

## DESIGN.md

§ 9.2, § 9.3, § 11, § 12.1 (optimization tests), § 12.3, § 15.

## Acceptance

- spec/scenarios/bounded-search-fit-retains-best.yaml
- spec/scenarios/bounded-search-fit-stops-at-render-bound.yaml
- spec/scenarios/bounded-search-fit-silence-and-pitch.yaml
- spec/scenarios/bounded-search-fit-invalid-bounds.yaml
- spec/scenarios/bounded-search-fit-unknown-references.yaml
- spec/scenarios/bounded-search-fit-ended-run-refused.yaml

## Numeric obligations

- The retained best loss never rises; a non-finite candidate never replaces it (§ 9.2, § 12.1).
- Render evaluations never exceed `max_renders` (default upper bound 2,000 per note) (§ 9.2,
  § 12.3).
- No render starts once the run's wall time has reached its bound, the first candidate excepted,
  and a render in progress is not interrupted; the recorded wall time exceeds the bound by at
  most one evaluation (§ 9.2: the budget is an offline upper bound, not a real-time guarantee;
  wave a6 decision Da6-22, replacing "wall time never exceeds its bound").
- Same seed and inputs give the same run (§ 2 invariant 5).

## Open questions

- Candidate bound read as `max_restarts`.
- Whether `max_renders` above 2,000 is allowed.
- Whether an improvement below the threshold is still retained.
- Whether the stop reason is checked against the counters.
- Whether the note sequence must match the fit's topology.
- The weight artifact's existence is not guarded for seeded fits.
