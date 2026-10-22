---
title: Patch studio
sidebar_label: Patch studio
sidebar_position: 7
description: The patch studio is a local page that shows the module catalogue, a patch as a rate-typed graph, a fit run replayed evaluation by evaluation, and the end result beside the reference.
lede: "The patch studio is a local page that shows what Patchback recovered and how it got there: the modules, the patch graph, every evaluation of a fit and the end result."
source: "epic Patch studio; DESIGN.md § 5, § 6.3, § 9.2, § 11"
---

# Patch studio

The patch studio is a page on `127.0.0.1` that draws Patchback's own data: the module kinds a patch
can use, a patch as a graph, a fit run evaluation by evaluation, and the patch the fit kept. It
only shows; it edits no patch and plays no audio.

:::info[Prototype]
The studio under `studio/` is a prototype. It renders all four views from hand-built fixture
JSON in `studio/fixtures/`, not from a Patchback fit, and it runs without a server. Following a
fit live over server-sent events is planned. See [Status](../status.mdx).
:::

![The fit evolution view at evaluation 10: transport, loss readouts, the retained patch with its knobs, the loss curve and the parameter trajectories](/img/studio/fit-evolution.png)

## Module catalogue

One card per module kind of `patchback.patch.NodeKind`. A card shows the module's ports with
their direction, rate and meaning, and its parameter contracts: index, path, mapping, physical
range and default. An engineer reads there what a module accepts and what a fit may change on
it. A kind appears once, and a port name once per kind.

DESIGN.md names the node kinds and their rates but not each kind's ports, so the catalogue's
port lists are illustrative until the design settles them.

## Patch graph

A validated patch is drawn as nodes placed by rank, with ports on the node edges and edges
coloured and dashed by port rate: audio, control and event rate each have their own colour and
line. Feedback is drawn as a delayed return path marked `z⁻¹` and routed under the graph; an
edge marked as delayed feedback must leave a Delay node (DESIGN.md § 5.4). A patch that has not
been validated is not drawn.

The view shows the fixed `subtractive-mono/1` voice and, as an example of what Milestone D
adds, a Drive into a Delay with the Delay fed back.

## Fit replay

The fit evolution view replays a finished fit run. A scrubber and a play control step through
the evaluations, and the URL parameter `?frame=N` opens a given frame. With each step:

- the loss curve moves its cursor over the candidate losses, the best retained loss and the
  rejected candidates, which carry no loss;
- the retained patch, or the candidate if asked, is drawn with its parameter values, and the
  knob of the parameter that changed is lit;
- the per-parameter trajectories move with it;
- a marker shows where the topology changes, once Milestone D selects topologies.

The trace obeys the rules of the search (DESIGN.md § 9.2): frames arrive in strictly increasing
sequence, the best retained loss never rises, a retained frame's loss is the best loss, and a
rejected frame carries no loss. The studio refuses a trace that breaks them.

DESIGN.md § 11 lists no per-evaluation trace file among a fit run's results yet, so there is
nothing to replay from a real run; the prototype replays a fixture trace.

## Live fit

The same view follows a running fit. Frames stream in as the fit evaluates candidates, and the
trace closes when the run completes or fails; no frame is accepted after the close or for a
trace that was never opened. A std-only Rust server sends the frames over server-sent events on
`127.0.0.1`.

This is planned. The event names, resume after a reconnect, back-pressure and the bound on how
much streaming may slow a fit are open questions.

## End result

The end result view shows the outcome of a fit: the retained patch, a table of each parameter in
the reference against the recovered value, and the reference and render spectra. Spectra are
drawn only at the analysis FFT lengths of DESIGN.md § 6.3: 512, 2048 and 8192 samples. A run with
no retained patch or no render has no end result.

![The end result view: the recovered patch, the harmonic spectrum of reference and render, and the parameter table](/img/studio/end-result.png)

Where the spectra are computed, in the Rust analysis and shipped as artifacts or in the studio,
is open. The prototype draws an analytic saw-through-lowpass approximation in the browser.

## Wire types

The studio's data is specified in `spec/domains/studio.yaml` (`patchback.studio`). Its events,
`CataloguePublished`, `GraphDrawn`, `TraceOpened`, `FrameAppended`, `TraceClosedEvent` and
`ResultComposed`, carry full payloads because they are the wire messages, for replay now and for
server-sent events later. The TypeScript types under `studio/src/generated/` are generated from
the specification with `task studio-types`, and `task studio-types-check` fails when they drift.

## Running it

```sh
task studio-dev     # Vite on http://127.0.0.1:3481
task studio-build   # vue-tsc, then vite build
```

The views are at `#catalogue`, `#graph`, `#evolution` and `#result`; `?graph=1` opens the
delayed-feedback patch.

## Open questions

- The studio is a TypeScript browser tool, while DESIGN.md § 3 keeps running code to Rust and
  § 4.1 needs no UI framework for the first release. An ADR settles this before the studio
  leaves prototype.
- Whether one trace spans several topologies (Milestone D), and how a learned architecture
  changing during training (ADR-003) is drawn.
