---
format: aep.planning-md/3
id: story:dependency-gate
kind: story
status: implemented
title: Dependency gate
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: DESIGN.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/patchback-cli/tests/dependencies.rs
- confidence: cited
  path: crates/patchback-cli/tests/dependencies_adversary.rs
- confidence: cited
  path: crates/patchback-docs/src/status.rs
- confidence: cited
  path: spec/domains/dependency.yaml
- confidence: cited
  path: spec/scenarios/dependency-gate-forbidden-crate.yaml
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
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T09:26:41Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T09:26:41Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-23T11:56:33Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Dependency gate

## Outcome

A dependency audit passes a workspace with no third-party application crates, refuses any
forbidden or unreviewed crate, and admits at most one approved optional audio I/O backend.

## DESIGN.md

§ 3, § 12.2 (dependency gate), ADR-007.

## Acceptance

- spec/scenarios/dependency-gate-no-crates-passes.yaml
- spec/scenarios/dependency-gate-forbidden-crate.yaml
- spec/scenarios/dependency-gate-audio-backend.yaml

## Numeric obligations

- At most one audio I/O backend crate, behind an optional feature (§ 3).

## Open questions

- How the audit tells application-level crates from ModelSpec's or the backend's transitive
  build requirements.
- Whether a ModelSpec crate may ever appear (it runs as a subprocess, § 8.2).
- Where an approval is recorded (allow-list file, ADR, reviewer sign-off).
