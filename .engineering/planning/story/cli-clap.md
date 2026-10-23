---
format: aep.planning-md/3
id: story:cli-clap
kind: story
status: implemented
title: CLI parsing with clap derive, the one reviewed CLI crate
relations:
- decomposes: epic:sound-engine
- serves: vision:patchback
- depends_on: story:workspace-scaffold
- informed_by: executable-system-specification:patchback-contracts
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: DESIGN.md
- confidence: cited
  path: crates/patchback-cli/Cargo.toml
- confidence: cited
  path: crates/patchback-cli/src/main.rs
- confidence: cited
  path: crates/patchback-cli/tests/adversary_a2.rs
- confidence: cited
  path: crates/patchback-cli/tests/dependencies.rs
- confidence: cited
  path: crates/patchback-cli/tests/dispatch.rs
- confidence: cited
  path: crates/patchback-docs/src/status.rs
- confidence: cited
  path: spec/domains/dependency.yaml
- confidence: cited
  path: spec/ess-inputs.yaml
- confidence: cited
  path: spec/scenarios/dependency-gate-cli-parser.yaml
- confidence: cited
  path: spec/scenarios/dependency-gate-forbidden-crate.yaml
- confidence: cited
  path: spec/scenarios/dependency-gate-no-crates-passes.yaml
- confidence: cited
  path: website/data/ess/patchback-dependency.domain-graph.json
- confidence: cited
  path: website/data/ess/patchback.domain-graph.json
- confidence: cited
  path: website/data/ess/patchback.ess-docs.json
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/concepts/guarantees.md
- confidence: cited
  path: website/docs/reference/ess/index.mdx
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
