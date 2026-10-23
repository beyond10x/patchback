---
format: aep.planning-md/3
id: approval-record:wave-a2
kind: approval-record
status: draft
title: Wave a2 approved by b10x-chef-orchestrator
relations:
- decides: story:cli-clap
- decides: story:dependency-gate
- decides: story:modelspec-pin-and-check
- decides: story:parameter-contracts
- decides: story:wav-io
revision: 1
---
# Wave a2 approved by b10x-chef-orchestrator

## The stop

The stage-1 wave proposal for Milestone A wave 2.

## Who approved, in their words

Message from session `b10x-chef-orchestrator`, 2026-10-23, under the grant in the operator's
workspace instructions (class C: a wave): "Then propose and run Milestone A wave 2, the next stories
after story:workspace-scaffold, approved under the grant. Keep the build rules: one build at a time,
none under 30G free." The same message makes the clap port "the first unit of the next wave"
(`approval-record:cli-clap`).

## The wave

| order | unit | why here |
|---|---|---|
| 1 | `story:cli-clap` | first, alone: it moves `Cargo.toml`, `Cargo.lock`, the dependency domain and the `Cargo.lock` gate that `story:dependency-gate` builds on |
| 2 | `story:dependency-gate`, `story:modelspec-pin-and-check`, `story:parameter-contracts`, `story:wav-io` | in parallel, forked from `wave/a2` after unit 1 merges; `aep plan artifact waves --kind story` (aep 0.74.0) places the four in wave 2 |

- Scope: inferred from the story bodies (no scoper run). The four parallel units land in different
  crates (`patchback-model`, `patchback-core`, `patchback-audio`) and in the dependency gate. Shared
  files: `Cargo.lock` (if a unit adds an intra-workspace edge), `CHANGELOG.md`,
  `crates/patchback-docs/src/status.rs` and the generated status pages; the merges resolve those.
- Builds: through `b10x-build-slot` with `B10X_BUILD_SLOTS=1 B10X_BUILD_MIN_FREE_G=30`, so one
  build runs at a time and none starts under 30G free.

## Commits the approval authorises

Per unit, the unit commits on its `impl/<story>` branch; the merges into `wave/a2`; the opening and
closing store commits on `wave/a2`; the fast-forward of `main` to `wave/a2` and its push. All as
`b10x-bot[bot]` through `b10x-gates bot`. No tag, no version bump, no release, no next wave.
