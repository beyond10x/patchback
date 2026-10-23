---
format: aep.planning-md/3
id: story:workspace-scaffold
kind: story
status: implemented
title: 'Workspace scaffold: crates, CLI dispatch, capability list'
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- serves: vision:patchback
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/patchback-analysis
- confidence: cited
  path: crates/patchback-audio
- confidence: cited
  path: crates/patchback-cli
- confidence: cited
  path: crates/patchback-core
- confidence: cited
  path: crates/patchback-dataset
- confidence: cited
  path: crates/patchback-docs/src/main.rs
- confidence: cited
  path: crates/patchback-docs/src/reference.rs
- confidence: cited
  path: crates/patchback-docs/src/status.rs
- confidence: cited
  path: crates/patchback-dsp
- confidence: cited
  path: crates/patchback-fit
- confidence: cited
  path: crates/patchback-model
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/status.mdx
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T08:26:33Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T08:26:33Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-23T08:58:29Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
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

Cited from the merged diff `69e87fd..23caa70` (wave a1):

| path | what landed |
|---|---|
| `Cargo.toml`, `Cargo.lock` | nine workspace members, no new third-party package |
| `crates/patchback-{core,dsp,audio,analysis,dataset,model,fit}` | empty library crates |
| `crates/patchback-cli` | `patchback` binary (std-only dispatch), tests `dispatch.rs`, `dependencies.rs` (the `Cargo.lock` dependency gate), `adversary2.rs` |
| `crates/patchback-docs/src/status.rs` | seven planned Milestone A capability entries |
| `crates/patchback-docs/src/{main,reference,status}.rs` | rustfmt only; the base failed `cargo fmt --check` |
| `website/data/status.json`, `website/docs/status.mdx` | regenerated |
| `Taskfile.yml`, `AGENTS.md` | task `rust-test` (`cargo test --locked --workspace`) inside `task check` |
| `CHANGELOG.md` | Unreleased line |

The inferred scope was right on its three paths and missed the seven library crates, `Cargo.lock`,
the generated status pages, `Taskfile.yml`, `AGENTS.md` and `CHANGELOG.md`.

## Open questions

- No scenario decides the scaffold itself; the dependency-gate scenario is the only one that fits
  (no third-party crate recorded).
- Settled in wave a1: `CHANGELOG.md` already existed on `main`; this story adds its Unreleased
  line.
- Open: the workspace rule asks for clap derive on command lines, and DESIGN.md § 3 admits no
  CLI-parsing crate; the CLI parses with std only (approval-record:wave-a1). Admitting clap is a
  DESIGN.md § 3 change for the operator.
