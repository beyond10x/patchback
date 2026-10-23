---
format: aep.planning-md/3
id: story:modelspec-pin-and-check
kind: story
status: implemented
title: Pinned ModelSpec and model check
relations:
- informed_by: executable-system-specification:patchback-contracts
- decomposes: epic:sound-engine
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/patchback-docs/src/status.rs
- confidence: cited
  path: crates/patchback-model/src/lib.rs
- confidence: cited
  path: crates/patchback-model/tests/adapter.rs
- confidence: cited
  path: crates/patchback-model/tests/adversary.rs
- confidence: cited
  path: crates/patchback-model/tests/bare_name.rs
- confidence: cited
  path: crates/patchback-model/tests/model_source.rs
- confidence: cited
  path: crates/patchback-model/tests/pinned_binary.rs
- confidence: cited
  path: models/modelspec.pin
- confidence: cited
  path: models/patch_regressor.mspec
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/status.mdx
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T09:26:42Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-23T09:26:42Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-23T11:56:33Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Pinned ModelSpec and model check

## Outcome

Patchback pins one ModelSpec release, runs `modelspec check` on `models/patch_regressor.mspec`
(the § 8.1 M0 model) as a subprocess, and records the model as checked; a model the pinned binary
rejects is recorded as rejected and cannot be trained. It runs in parallel with Milestone A
(§ 15), so it decomposes epic:sound-engine.

## DESIGN.md

§ 1.1, § 3, § 8.1, § 12.2 (ModelSpec bridge gate), § 15, ADR-001.

## Acceptance

- spec/scenarios/modelspec-pin-and-check-checked.yaml
- spec/scenarios/modelspec-pin-and-check-rejected.yaml

## Numeric obligations

- The model uses only the documented M0 operations and static shapes `[16, 32]`, `[32, 64]`,
  `[64, 8]`, `[16, 8]` (§ 8.1).

## Open questions

- What test qualifies a release as "tested" for the pin.
- A mismatch between the source pin and the run pin is external to the model.
- § 15 also wants the pinned binary to accept a minimal § 7.2 dense-data directory. No scenario
  decides that yet (for example `modelspec-pin-and-check-dense-data.yaml`); until one is authored,
  only story:training-run in Milestone B exercises dense data.
