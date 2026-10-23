---
format: aep.planning-md/3
id: story:parameter-contracts
kind: story
status: implemented
title: Parameter contracts for the subtractive-mono/1 voice
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: cited
  path: .engineering/planning/approval-record/non-finite-seam.md
- confidence: cited
  path: .engineering/planning/story/parameter-contracts.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/patchback-core/src/lib.rs
- confidence: cited
  path: crates/patchback-core/src/parameter.rs
- confidence: cited
  path: crates/patchback-core/tests/parameter_contracts.rs
- confidence: cited
  path: crates/patchback-core/tests/parameter_contracts_adversary.rs
- confidence: cited
  path: crates/patchback-docs/src/status.rs
- confidence: cited
  path: spec/domains/patch.yaml
- confidence: cited
  path: spec/ess-inputs.yaml
- confidence: cited
  path: spec/scenarios/parameter-contracts-clamp.yaml
- confidence: cited
  path: spec/scenarios/parameter-contracts-invalid-contract.yaml
- confidence: cited
  path: spec/scenarios/parameter-contracts-unknown-path.yaml
- confidence: cited
  path: website/data/ess/patchback.ess-docs.json
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference/ess/patchback-analysis.md
- confidence: cited
  path: website/docs/reference/ess/patchback-audio.md
- confidence: cited
  path: website/docs/reference/ess/patchback-dataset.md
- confidence: cited
  path: website/docs/reference/ess/patchback-dependency.md
- confidence: cited
  path: website/docs/reference/ess/patchback-fit.md
- confidence: cited
  path: website/docs/reference/ess/patchback-model.md
- confidence: cited
  path: website/docs/reference/ess/patchback-patch.md
- confidence: cited
  path: website/docs/reference/ess/patchback-score.md
- confidence: cited
  path: website/docs/reference/ess/patchback-studio.md
- confidence: cited
  path: website/docs/status.mdx
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T09:26:42Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T09:26:42Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-23T11:56:33Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Parameter contracts for the subtractive-mono/1 voice

## Outcome

The `subtractive-mono/1` topology is declared with its eight parameter contracts (attack, decay,
sustain, release, filter cutoff, resonance, drive, output gain), each with index, stable path,
normalized-to-physical mapping, physical range and default. A normalized value outside [0, 1] is
clamped and the clamp recorded; a non-finite value and an invalid contract are refused.

## DESIGN.md

§ 2 invariant 2, § 5.1, § 5.2, § 8.1 (clamp with warning).

## Acceptance

- spec/scenarios/parameter-contracts-subtractive-mono.yaml
- spec/scenarios/parameter-contracts-invalid-contract.yaml
- spec/scenarios/parameter-contracts-clamp.yaml
- spec/scenarios/parameter-contracts-unknown-path.yaml

## Numeric obligations

- Forward and inverse mapping of each of the eight contracts roundtrips within f32 tolerance across
  [0, 1], and the endpoints hit the § 5.2 ranges: attack 1 ms–2 s, decay and release 5 ms–2 s,
  sustain linear, cutoff 40 Hz–16 kHz, Q 0.5–12, drive 0–24 dB, gain -24–0 dB (§ 5.2).
- `p` is clamped to [0, 1]; NaN and Infinity are rejected (§ 5.2, § 2 invariant 4). ESS Decimal
  and Binary64 hold finite values only (https://github.com/beyond10x/ess/issues/676, by design),
  so this refusal sits at the decoder seam in `patchback-core` and is held by Rust tests
  (`a_non_finite_value_is_refused_naming_the_parameter`,
  `a_range_whose_width_or_ratio_overflows_is_refused`); no scenario stands in for it.
  Orchestrator, 2026-10-23: "Approved for wave a2: refuse NaN/inf at your decoder/adapter seam and
  keep the Rust tests there."
- Serialized values are physical units, model outputs normalized units (§ 5.2).

## Open questions

- Parameter path syntax: bare (`attack`) or module-qualified (`adsr.attack`).
- § 2 requires a documented default per parameter and § 5.2 gives none; scenarios carry
  placeholders.
- A FM oscillator family is named in § 5.4 without kinds; not declared.
