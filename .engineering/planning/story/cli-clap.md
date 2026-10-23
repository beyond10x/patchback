---
format: aep.planning-md/3
id: story:cli-clap
kind: story
status: active
title: CLI parsing with clap derive, the one reviewed CLI crate
relations:
- decomposes: epic:sound-engine
- serves: vision:patchback
- depends_on: story:workspace-scaffold
- informed_by: executable-system-specification:patchback-contracts
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: DESIGN.md
- confidence: inferred
  path: crates/patchback-cli
- confidence: inferred
  path: spec/domains/dependency.yaml
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T09:26:41Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T09:26:41Z", actor: "human:timo", revision: 4}
---
# CLI parsing with clap derive, the one reviewed CLI crate

## Outcome

`clap` with its `derive` feature is the single reviewed command-line parsing dependency of the
application. DESIGN.md § 3 names the category in one row ("CLI parsing: clap (derive), required by
the workspace rule") and its revision line moves. The `patchback-dependency` domain
(`spec/domains/dependency.yaml`) gains the category: an approved `clap` is admitted, any other
CLI-parsing crate and an unapproved `clap` are refused. The `Cargo.lock` gate
(`crates/patchback-cli/tests/dependencies.rs`) admits `clap` and exactly the packages its resolved
closure brings in, and refuses everything else as before. `patchback-cli` parses with clap derive
and keeps the dispatch and exit codes of `story:workspace-scaffold`, except where clap's own
conventions differ, which the CLI's doc comment then documents.

## DESIGN.md

§ 3 (dependency policy), § 11 (CLI commands), § 12.2 (dependency gate). Workspace rule "Running
code is Rust ... with clap derive for command lines".

## Acceptance

- spec/scenarios/dependency-gate-cli-parser.yaml

## Numeric obligations

None. Rust tests hold the CLI's exit codes and the `Cargo.lock` gate.

## Scope

- `DESIGN.md`, `spec/domains/dependency.yaml`, `spec/scenarios/dependency-gate-cli-parser.yaml`,
  `Cargo.toml`, `Cargo.lock`, `crates/patchback-cli` (inferred).

## Open questions

None. Decided by `approval-record:cli-clap`.
