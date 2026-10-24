---
format: aep.planning-md/3
id: review-result:feature-contract-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: feature-contract'
relations:
- reviews: story:feature-contract
revision: 1
---
# Adversary pass 2: story:feature-contract

Wave a5, unit 3. `aep:adversary` on Opus against the corrected uncommitted change in tree
`patchback-impl-feature-contract` (base c338101af4d635bdf3377e91f8a35d21a3adcdf4), after correction
round 1 (decisions D8, D9, D10), 2026-10-24.

## Result

- verdict: red; cases executed 122 → 128, red 1; origin: introduced 6, pre-existing 0.
- Suite (`cargo test --locked -p patchback-analysis --no-fail-fast`): every other lane ok
  (adversary 15, adversary2 11, adversary3 12, adversary_features 7, analyze 15,
  cross_correlation 8, domain 8, features 20, numeric 26); new lane `adversary2_features`
  5 passed 1 failed. clippy `-D warnings` exit 0, fmt exit 0.
- Added `crates/patchback-analysis/tests/adversary2_features.rs` only (1 red case, 5 passing
  guards). The implementor's 4 tracked files are unchanged (+557/−7).
- Pass-1 findings: F1 (define order) resolved; F2 (hop 512 → StftMissing) resolved; the three
  mutants M2, M3, M4 are caught by the pass-1 guards and still survive the unit's own suite; the
  count-mismatch message finding is resolved and its text is now pinned; the per-registry id
  judgement is carried (INFEASIBLE). The rewritten unit test `tests/features.rs:151` was neither
  relaxed nor weakened.
- The red case: a hand-built analysis whose 2048 STFT has `hop_length = usize::MAX` panics in
  `frames_in` with "attempt to add with overflow" instead of `FeatureNotComputable`; a hop of 0 is
  read too. Both reachable only through a hand-built `Analysis`; `analyze` never yields them.
- Guards: frames placed by the STFT's own hop (mutant M5 at fft/2 survives the unit suite); of two
  STFTs with one FFT length the first is read (M6 last-wins survives the unit suite); the exact
  count-mismatch message (M7 survives the unit suite).
- Judgement J1: after D9 a `features/1` contract with 32 values is defined whatever its layout; a
  reordered layout passes `check`. DESIGN.md § 6.3 fixes the layout, the model checks the count.
- Judgement J2: one published contract gives different vectors at hop 1024 and hop 512 (largest
  difference 0.802 dB at index 21). DESIGN.md § 6.3 versions FFT settings with the contract.

## Coordinator's routing

- N1 (overflow and hop 0) → correction round 2: checked arithmetic in `frames_in`; a hop of 0 or
  an overflowing centre places no frame and is refused as the existing `WindowTooShort` reason,
  so `FeatureNotComputable` at the first band index. No new error. The coordinator verifies this
  correction (wave protocol: two adversary passes, the correction after the second is verified by
  the coordinator).
- N4 → correction round 2: one doc sentence, "the first STFT of a length is read".
- N2, N3 → the guards stay in `adversary2_features.rs`.
- J1 → open point of D9 for the specification's owner: pin the `features/1` layout in
  `spec/domains/analysis.yaml` or add the layout rule as a model outcome.
- J2 → open point of D10 (OQ-15; `UNMAPPED` at `spec/domains/analysis.yaml:118`): settle the hop
  and record it in the contract.
- Carried judgement (ids per registry) → no change.

Agent cost: 104,188 tokens, 36 tool uses, 745,689 ms. Full report:
`~/.cache/scratch/0905d614-e063-463d-a99e-28052c6bae4a/a5/feature-contract/adversary-report-2.md`
(scratch, deleted 48 h after the session).

```findings
[
{"file":"crates/patchback-analysis/src/features.rs","line":640,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"extraction panics on overflow for an STFT hop near usize::MAX and reads a hop of 0 since D10 dropped the hop check; red at adversary2_features.rs:181"},
{"file":"crates/patchback-analysis/src/features.rs","line":633,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"frames placed at fft/2 instead of the STFT's own hop survives the unit's suite and pass-1's hop-512 case; guard adversary2_features.rs:113"},
{"file":"crates/patchback-analysis/src/features.rs","line":600,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"of two STFTs of one FFT length the first in the analysis is read; undocumented, and the last-wins mutant survives the unit's suite; guard adversary2_features.rs:142"},
{"file":"crates/patchback-analysis/src/error.rs","line":459,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the count-mismatch message had no test; the pre-correction message survives the unit's suite; guard adversary2_features.rs:200"},
{"file":"crates/patchback-analysis/src/features.rs","line":335,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"features/1 with count 32 is defined whatever its layout; a 40-slot layout extracts a vector its check refuses, a reordered layout passes check; DESIGN 6.3 fixes the layout, the model checks the count only"},
{"file":"crates/patchback-analysis/src/features.rs","line":600,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"one published contract gives different vectors at different hops (0.802 dB at index 21), against DESIGN 6.3's versioned FFT settings and consistent overlap"},
{"file":"crates/patchback-analysis/src/features.rs","line":348,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"carried: contract ids restart at 1 per registry, so an id from one registry resolves to a different contract in another"}
]
```
