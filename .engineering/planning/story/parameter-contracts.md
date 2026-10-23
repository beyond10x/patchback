---
format: aep.planning-md/3
id: story:parameter-contracts
kind: story
status: active
title: Parameter contracts for the subtractive-mono/1 voice
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: inferred
  path: crates/patchback-core
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T09:26:42Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T09:26:42Z", actor: "human:timo", revision: 4}
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
