---
format: aep.planning-md/3
id: story:studio-fit-replay
kind: story
status: draft
title: Fit evolution replay
relations:
- decomposes: epic:patch-studio
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:studio-patch-graph
- depends_on: story:fit-run-artifacts
revision: 1
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

## Numeric obligations

- Frames arrive in strictly increasing sequence; the best retained loss never rises; a retained
  frame's loss equals the best; a rejected frame carries no loss (§ 9.2).

## Open questions

- § 11 lists no per-evaluation trace file; replay reads fixture JSON until one is settled.
- Whether one trace spans several topologies (Milestone D).
- The learned-graph frame (ADR-003) is not modelled.
