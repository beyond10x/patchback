---
format: aep.planning-md/3
id: story:workspace-scaffold
kind: story
status: active
title: 'Workspace scaffold: crates, CLI dispatch, capability list'
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- serves: vision:patchback
scope:
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: crates/patchback-cli/src/main.rs
- confidence: inferred
  path: crates/patchback-docs/src/status.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T08:26:33Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T08:26:33Z", actor: "human:timo", revision: 4}
---
# Workspace scaffold: crates, CLI dispatch, capability list

## Outcome

The Cargo workspace exists with the crate layout of DESIGN.md § 4.1 (`patchback-core`,
`patchback-dsp`, `patchback-audio`, `patchback-analysis`, `patchback-dataset`, `patchback-model`,
`patchback-fit`, `patchback-cli`) as empty crates, beside the existing `patchback-docs`. The root
`Cargo.toml` lists every member, `patchback-cli` has the application entry point with subcommand
dispatch stubs (`patch validate`, `render`, `analyze`, `compare`, `fit`), and
`crates/patchback-docs/src/status.rs` carries the capability list entries the later stories turn
on. The scaffold adds no third-party crate, so the dependency gate passes on it.

This story owns the shared surfaces every other Milestone A story would otherwise touch at once:
root `Cargo.toml` workspace members, the CLI entry point and the capability list.

## DESIGN.md

§ 3 (dependency policy), § 4.1 (repository structure), § 11 (CLI commands), § 15.

## Acceptance

- spec/scenarios/dependency-gate-no-crates-passes.yaml

## Numeric obligations

None. Rust tests hold that the workspace builds and that `task docs-check` stays green.

## Scope

- `Cargo.toml`, `crates/patchback-cli/src/main.rs`, `crates/patchback-docs/src/status.rs`
  (inferred from § 4.1 and AGENTS.md; nothing exists yet).

## Open questions

- No scenario decides the scaffold itself; the dependency-gate scenario is the only one that fits
  (no third-party crate recorded).
- Whether `CHANGELOG.md` is created here or at the first release.
