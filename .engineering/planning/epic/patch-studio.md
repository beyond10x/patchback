---
format: aep.planning-md/3
id: epic:patch-studio
kind: epic
status: draft
title: 'Patch studio: see modules, patch graphs and how a fit evolves'
relations:
- serves: vision:patchback
- depends_on: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
revision: 1
---
# Patch Studio: see modules, patches and how a fit evolves

## Outcome

An engineer opens one local page and sees what Patchback recovered and how it got there: every
atomic module kind with its ports, rates and parameter contracts; a patch as a graph of nodes and
rate-typed edges with delayed feedback marked; a fit run replayed evaluation by evaluation (the
candidates, the loss curve, the retained best patch, the parameter trajectories, and topology
changes once Milestone D selects topologies); and the end result, the recovered patch beside the
reference and render spectra. The same view later follows a run live, and later still shows a
learned architecture changing during training (ADR-003).

## Operator intent

> "we need some UI way of rendering our different atomic modules. Yeah, because later on our
> architecture will also be learned by the network from audio samples, and yeah, we want to
> visualize this, how this is evolving, or how the end result would look like. Yeah, so I think
> that's important to have this already soon."
>
> (Timo, 2026-10-22)

## In scope

- Module catalogue view: one card per `patchback.patch.NodeKind`, with ports (direction, rate,
  meaning) and its `ParameterContract`s (index, path, mapping, physical range, default).
- Patch graph view: nodes placed by rank, ports on node edges, edges coloured and dashed by
  `PortRate`, delayed feedback (DESIGN.md § 5.4) routed visibly as a `z⁻¹` return path.
- Fit evolution view: a scrubber and play control over the evaluations of a fit run; the loss
  curve (candidate loss, best retained loss, rejected candidates with no loss); the retained or
  candidate patch drawn with its parameter values, animated between evaluations; per-parameter
  trajectories; the topology of each frame, with a marker where it changes.
- End result view: the retained patch, a parameter table of reference against recovered, and the
  reference and render spectra at an analysis FFT length of § 6.3 (512, 2048, 8192).
- Transport, in order: replay from fit-run artifacts first; live over SSE from a Rust server later.
- Wire types generated from `spec/domains/studio.yaml` with `ess generate types --target
  typescript` (`task studio-types`), with a drift check (`task studio-types-check`).
- The prototype under `studio/` (Vue 3, TypeScript, Vite, pnpm) that renders all four views from
  fixture JSON, with no server.

## Out of scope

- Editing a patch in the UI; the studio only shows patches.
- Audio playback in the browser.
- Any change to the fit, analysis or render code paths; the studio reads what they produce.
- A hosted deployment. The studio runs on `127.0.0.1`.
- Rendering a learned graph decoder's internal state (ADR-003) before Milestone D settles it.

## Acceptance

- The four views render the fixtures in `studio/fixtures/` with `task studio-dev`;
  `task studio-build` (vue-tsc, then vite build) exits 0.
- The scrubber steps through every frame of the fixture trace; the graph's parameter bars, the
  loss-curve cursor and the trajectories move with it; play animates it.
- `task studio-types-check` exits 0: the checked-in `studio/src/generated/` equals a fresh
  generation from `spec/`.
- Every story's scenarios pass against the interpreted target (`task spec-conform`).

## Specification

`spec/domains/studio.yaml` (`patchback.studio`) holds ModuleCatalogue, GraphView, EvolutionTrace,
TraceFrame and ResultView. It refers to Patch, Topology, FitRun, AudioClip and Render by identity
type only and owns none of them. Its events (`CataloguePublished`, `GraphDrawn`, `TraceOpened`,
`FrameAppended`, `TraceClosedEvent`, `ResultComposed`) carry full payloads because they are the
wire messages, for replay and later for SSE.

`ess specify validate --path spec --strict-requires`: `patchback v1 — 10 file(s), valid`.

UNMAPPED, each an open question for the store:

1. A browser UI against the dependency policy: § 3 forbids application frameworks in running code
   and § 4.1 says the first release needs no UI framework. The studio is a separate dev tool in
   TypeScript. This needs an ADR before the studio leaves prototype.
2. The trace artifact: § 11 lists no per-evaluation trace file among the fit results, so there is
   nothing yet to replay from a real run.
3. Port lists per node kind: DESIGN.md names node kinds and rates, not each kind's ports. The
   fixture catalogue's ports are illustrative.
4. Where spectra are computed for the end-result view: in the Rust analysis code and shipped as
   artifacts, or in the studio. The prototype draws an analytic approximation.
5. The live transport: SSE framing, reconnect and back-pressure from a std-only Rust server.
6. Multi-topology traces (Milestone D) and the learned-graph frame (ADR-003).
