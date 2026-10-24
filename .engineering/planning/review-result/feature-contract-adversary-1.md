---
format: aep.planning-md/3
id: review-result:feature-contract-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: story:feature-contract (wave a5)'
summary: 'red: 115→122 cases, 2 red, 7 findings (2 routed to correction round 1, 3 mutant guards kept)'
owner: orchestrator
relations:
- reviews: story:feature-contract
revision: 1
---
# Adversary pass 1: story:feature-contract

Wave a5, unit 3. `aep:adversary` on Opus against the implementor's uncommitted change in tree
`patchback-impl-feature-contract` (base c338101af4d635bdf3377e91f8a35d21a3adcdf4), 2026-10-24.

## Result

- verdict: red; cases executed 115 → 122, red 2; origin: introduced 7, pre-existing 0.
- Package suite after the cases: `cargo test --locked -p patchback-analysis --no-fail-fast` exit 101;
  `adversary_features` 5 passed, 2 failed; every other lane ok.
- Added `crates/patchback-analysis/tests/adversary_features.rs` (7 cases: 2 red, 5 passing guards
  that catch mutants M1–M4 the unit's own suite did not).
- The two red cases: `define` refusing a taken non-`features/1` version with an inconsistent count
  as `FeatureCountMismatch` where the model says `FeatureVersionTaken` (F1); extraction refusing the
  `feature-contract-extract` scenario's hop-512 analysis as `StftMissing` (F2, infeasible through
  the CLI today).
- Mutants that survived the unit's suite and are now caught: arithmetic mean of dB for a band
  energy (M2), a window taking the frame centred on its end (M3), a silence threshold 20 dB lower (M4).

## Coordinator's routing

- F1 → correction round 1, decision D9: follow the model's outcomes and order (`wrong-count` for
  `features/1` only, then `version-taken`, then `defined`); the layout rule for other versions is
  removed from code and recorded as an open spec point.
- F2 → correction round 1, decision D10: accept an `StftAnalysis` at any hop (the hop is `UNMAPPED`).
- M2, M3, M4 → the adversary's guards stay; no code change.
- error.rs:463 message → corrected with D9; features.rs:347 (ids per registry) → no change this wave.

Agent cost: 153,232 tokens, 32 tool uses, 651,588 ms. Full report:
`~/.cache/scratch/0905d614-e063-463d-a99e-28052c6bae4a/a5/feature-contract/adversary-report-1.md`
(scratch, deleted 48 h after the session).

```findings
[
{"file":"crates/patchback-analysis/src/features.rs","line":336,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"define refuses a taken non-features/1 version with an inconsistent count as FeatureCountMismatch where the model's outcome is FeatureVersionTaken, and refuses outright layouts the model defines"},
{"file":"crates/patchback-analysis/src/features.rs","line":598,"category":"acceptance","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"extraction refuses the feature-contract-extract scenario's hop-512 analysis as StftMissing, a state no caller produces today"},
{"file":"crates/patchback-analysis/src/features.rs","line":740,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"an arithmetic mean of dB in place of the power mean survives the unit's suite"},
{"file":"crates/patchback-analysis/src/features.rs","line":638,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a window taking the frame centred on its end survives the unit's suite"},
{"file":"crates/patchback-analysis/src/features.rs","line":655,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a silence threshold 20 dB lower survives the unit's suite"},
{"file":"crates/patchback-analysis/src/error.rs","line":463,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the count-mismatch diagnostic never names the layout count it was checked against"},
{"file":"crates/patchback-analysis/src/features.rs","line":347,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"contract ids restart at 1 per registry, so an id from one registry resolves to a different contract in another"}
]
```
