---
format: aep.planning-md/3
id: epic:modelspec-regressor
kind: epic
status: implemented
title: 'Milestone B: native dataset and ModelSpec M0 regressor'
relations:
- serves: vision:patchback
- depends_on: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T11:49:57Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-25T11:49:57Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-25T11:49:58Z", actor: "human:timo", revision: 4}
---
# Milestone B: native dataset and ModelSpec M0 regressor

## Outcome

A ModelSpec-trained MLP predicts the eight normalized parameters from the `features/1` vector, and
seeding the search with its prediction reduces matching work or improves the audio fit against the
default-patch start (DESIGN.md § 13 Milestone B).

## In scope

- Freeze `features/1` and its normalization statistics (§ 6.3).
- Seeded paired dataset generation and ModelSpec dense-data files (§ 7.1, § 7.2).
- Pinned ModelSpec: `modelspec check` and `modelspec train --backend cpu` as a subprocess (§ 1.1, § 8.1).
- Strict weight export and the native Rust `matmul`/`relu` predictor (§ 8.2, ADR-006).
- Default vs ML-init vs ML-init-plus-search benchmark (§ 12.2).

## Out of scope

GPU training (Milestone E, ADR-010), any ML crate (§ 3).

## Depends on

Milestone A: the renderer and feature contract must be stable before data is generated at scale (§ 15).
