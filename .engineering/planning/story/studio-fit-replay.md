---
format: aep.planning-md/3
id: story:studio-fit-replay
kind: story
status: active
title: Fit evolution replay
relations:
- decomposes: epic:patch-studio
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:studio-patch-graph
- depends_on: story:fit-run-artifacts
- serves: vision:patchback
scope:
- confidence: inferred
  path: crates/patchback-cli/tests/dependencies.rs
- confidence: inferred
  path: crates/patchback-studio
- confidence: cited
  path: spec/domains/studio.yaml
- confidence: cited
  path: studio/fixtures/trace.json
- confidence: inferred
  path: studio/package.json
- confidence: inferred
  path: studio/src/App.vue
- confidence: inferred
  path: studio/src/components/LossChart.vue
- confidence: inferred
  path: studio/src/components/Trajectories.vue
- confidence: cited
  path: studio/src/model.ts
- confidence: cited
  path: studio/src/views/EvolutionView.vue
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-01T15:25:04Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-11-01T15:25:05Z", actor: "human:timo", revision: 7}
---
# Fit evolution replay

## Outcome

The studio replays a finished fit run evaluation by evaluation: a scrubber and play control, the
loss curve with candidate loss and best retained loss, the retained patch with its parameters
animated between frames, per-parameter trajectories, and a marker where the topology changes.
Source: `OpenTrace`, `AppendFrame`, `CloseTrace`; events `TraceOpened`, `FrameAppended`,
`TraceClosedEvent`.

## DESIGN.md

§ 9.1 (components logged per candidate), § 9.2, § 11.

## Acceptance

- spec/scenarios/studio-fit-replay-replayed.yaml
- spec/scenarios/studio-fit-replay-best-worsened.yaml
- spec/scenarios/studio-fit-replay-out-of-order.yaml
- spec/scenarios/studio-fit-replay-inconsistent-frame.yaml
- spec/scenarios/studio-fit-replay-inconsistent-end.yaml

## Numeric obligations

- Frames arrive in strictly increasing sequence; the best retained loss never rises; a retained
  frame's loss equals the best; a rejected frame carries no loss (§ 9.2).

## Open questions

- Whether one trace spans several topologies (Milestone D).
- The learned-graph frame (ADR-003) is not modelled.

The former first question (no per-evaluation trace file) was settled by DESIGN.md 0.3 § 11 and story:fit-run-artifacts; the transport and identities are Dc1-21 to Dc1-24 in approval-record:wave-c1.

## Scope

Derived 2026-11-01 by `story-scoper` against `wave/c1` at ab1762c. Every line is **cited** (read from the story or the tree) or **inferred**. The open points it raised are answered by Dc1-20 to Dc1-27 in `approval-record:wave-c1`; the surface below is the scoper's, before those decisions added `crates/patchback-studio`.

- **Primary surface:** `studio/src` (Vue 3 + TypeScript); `studio/src/model.ts:1-12` imports the event types from `studio/src/generated/types.ts` — cited
- **Files:** `studio/src/model.ts:16,24-29,40,117-132` (fixture import, `TraceFixture`, `trace`, `checkTrace`) — cited
- **Files:** `studio/src/views/EvolutionView.vue:7-21,69-75` — cited
- **Files:** `studio/fixtures/trace.json` (20 frames, hand-built, "not the output of a Patchback fit", `:2`) — cited; removed by Dc1-23
- **Symbols (spec):** `patchback.studio.EvolutionTrace`, `TraceFrame`, `OpenTrace`, `AppendFrame`, `CloseTrace`, `TraceOpened`, `FrameAppended`, `TraceClosedEvent`, `FrameOutcome`, `TraceEnd`, `ParameterSample` (`spec/domains/studio.yaml:119-143,228-321,612-800,885-930`) — cited
- **Symbols (Rust, read):** `patchback_fit::trace::{write_trace, parse_trace, Trace, TraceRecord, TRACE_HEADER, TRACE_FORMAT_VERSION}` (`crates/patchback-fit/src/trace.rs:1-71,81-137,246-304`); `StopReason` (`crates/patchback-fit/src/run.rs:54-59`) — cited
- **Also likely:** `studio/src/components/LossChart.vue:43-44`, `Trajectories.vue`, `studio/src/App.vue:38` (badge "replay of fixture data · no server") — inferred
- **Fixture against the real trace:** different formats. The real `trace.events` has a `patchback-fit-trace 1` header, a topology name, sequences from 1, `first-retained` spellings, shortest round-trip numbers, a `<restart>` per evaluation, `completed <stop_reason>`; a failed run writes no trace (`trace.rs:3-4`, `:14-29`) — cited
- **Tests:** none for the studio; `vue-tsc --noEmit` in `build` is the only check; no workflow builds `studio/` — cited
- **Documents (coordinator):** `website/docs/concepts/studio.md:61-62` (stale), `crates/patchback-docs/src/status.rs:399-405`, `CHANGELOG.md`, `AGENTS.md:16`, `README.md:16-17` — cited
- **Confidence:** medium before Dc1-21; the transport decision brings `crates/patchback-studio`, `Cargo.toml`, the dependency gate, `Taskfile.yml` and `check.yml` into scope.
- **Would collide with:** the other studio stories (`studio/src/model.ts`, `EvolutionView.vue`, `LossChart.vue`, `Trajectories.vue`, `PatchGraph.vue`); not with unit 1 of wave c1 — cited
