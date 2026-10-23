---
format: aep.planning-md/3
id: approval-record:wave-a1
kind: approval-record
status: draft
title: Wave a1 approved by b10x-chef-orchestrator
relations:
- decides: story:workspace-scaffold
revision: 1
---
# Wave a1 approved by b10x-chef-orchestrator

## The stop

The stage-1 wave proposal for Milestone A wave 1 (`story:workspace-scaffold`).

## Who approved, in their words

Message from session `b10x-chef-orchestrator`, 2026-10-23, under the grant in the operator's
workspace instructions (class C: a wave), citing Timo on 2026-10-23: "keep patchback running
(bypass permissions)". The message reads: "propose and run patchback Milestone A wave 1 starting
with story:workspace-scaffold. Treat this message as the wave approval and record it as an
approval-record in the patchback store."

## The wave

- Unit: `story:workspace-scaffold`, serving `epic:sound-engine` (Milestone A).
- N is 1. `aep plan artifact waves --kind story` (aep 0.74.0) puts `story:workspace-scaffold`
  alone in wave 1; the four stories of wave 2 depend on the root `Cargo.toml` and the CLI entry
  point this story owns.
- Collisions the verb reported, all inside `crates/patchback-fit` and outside this wave:
  audio-comparison/bounded-search-fit, audio-comparison/engine-baseline,
  audio-comparison/fit-run-artifacts, bounded-search-fit/engine-baseline,
  bounded-search-fit/fit-run-artifacts, engine-baseline/fit-run-artifacts. Unassessed: none.
- Scope: inferred (`Cargo.toml`, `crates/patchback-cli/src/main.rs`,
  `crates/patchback-docs/src/status.rs`).

## Commits the approval authorises

One unit commit series on `impl/workspace-scaffold` (implementor and adversary cases), the merge
into `wave/a1`, the opening and closing store commits on `wave/a1`, and the fast-forward of
`main` to `wave/a1` with its push, all as `b10x-bot[bot]` through `b10x-gates bot`. No tag, no
version bump, no release, no next wave.

## Decision taken inside the item

The CLI entry point parses its arguments with `std` only, not clap. Reason: the story's outcome
says "The scaffold adds no third-party crate", and DESIGN.md § 3 lists no CLI-parsing category
for the application. The workspace instruction "Running code is Rust ... with clap derive for
command lines" points the other way; admitting clap to the application is a DESIGN.md § 3 change
and is left to the orchestrator and the operator. The docs tool `patchback-docs` keeps clap.
