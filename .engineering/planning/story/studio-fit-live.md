---
format: aep.planning-md/3
id: story:studio-fit-live
kind: story
status: draft
title: Fit evolution live
relations:
- decomposes: epic:patch-studio
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:studio-fit-replay
revision: 1
---
# Fit evolution live

## Outcome

The studio follows a running fit: frames stream in as the fit evaluates candidates, and the trace
closes with the run's end, completed or failed. No frame is accepted after close or for a trace that
was never opened. Same commands as replay with `source: Live`, actor `FitStream`, served by a
std-only Rust server over SSE on 127.0.0.1.

## DESIGN.md

§ 2 invariant 6 (no I/O in audio callbacks), § 9.2, § 12.3.

## Acceptance

- spec/scenarios/studio-fit-live-streamed.yaml
- spec/scenarios/studio-fit-live-closed-takes-no-frame.yaml
- spec/scenarios/studio-fit-live-unknown-trace.yaml

## Numeric obligations

- The replay frame rules hold live (§ 9.2).
- Streaming does not slow the fit beyond a bound stated when the story is proposed (§ 12.3).

## Open questions

- SSE path, event names, Last-Event-ID resume, bind address and back-pressure.
- The std-only server against § 3 and § 4.1 ("no HTTP backend necessary for the first release").
