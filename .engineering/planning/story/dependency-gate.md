---
format: aep.planning-md/3
id: story:dependency-gate
kind: story
status: active
title: Dependency gate
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: inferred
  path: .github/workflows/check.yml
- confidence: inferred
  path: Taskfile.yml
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T09:26:41Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T09:26:41Z", actor: "human:timo", revision: 4}
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
