---
title: Guarantees
sidebar_position: 6
description: The invariants every Patchback patch, rendering and run keeps, the limits it enforces, and the dependencies it refuses.
lede: "Every patch runs in the engine that made the training data, every output is finite, and every run is reproducible."
source: "DESIGN.md § 2, § 3, § 12.3"
---

# Guarantees

These hold for every patch, rendering and fit, whatever the milestone.

| Guarantee | What it means |
| --- | --- |
| A recovered patch runs | Every patch is rendered by the same engine that generates the training data, so what the fit returns is something the engine has already played |
| Every parameter is documented | Each has a domain, a default, a unit, a mapping and a clamping policy; see [parameters](../reference/parameters.md) |
| Validation comes first | The graph is validated before it is compiled or executed |
| Output is always finite | Non-finite input is refused or handled safely, and no rendering contains NaN or infinity |
| Runs are reproducible | The same seed, build, input and configuration give the same offline result, within the numerical contract |
| Audio callbacks stay clean | Live audio callbacks do not allocate, block, lock, read or write files, or touch the network |
| No hidden ML stack | Patchback never brings in another ML or DSP library to cover for something ModelSpec does not do |

## Limits

Patchback bounds everything it reads or computes: input duration, graph size, voice count,
note count, FFT length, memory and optimizer time. It refuses corrupt models, non-finite
features, impossible sample rates, empty audio and untrusted graphs. DSP controls are clamped
and smoothed, and an unstable feedback patch is bypassed or muted rather than left to run
away.

Every file parser (patches, events and run metadata) refuses unknown versions, duplicate keys
and identifiers, malformed numbers, unsupported rates and unbounded lengths.

## Dependencies

Patchback is Rust and the standard library. It uses no ML, tensor, DSP, FFT, maths or
serialization crates. It reads and writes WAV itself, so offline use needs no third-party
runtime crate at all. Live sound-card I/O, when it comes, uses at most one audio I/O crate
behind an optional feature. Training goes through a pinned, tested release of
[ModelSpec](https://beyond10x.github.io/modelspec/)
([GitHub](https://github.com/beyond10x/modelspec)); the application contracts are specified
with [ESS](https://beyond10x.github.io/ess/) ([GitHub](https://github.com/beyond10x/ess)),
which is a specification tool and never a runtime dependency.

## Fixtures

Every audio fixture in the repository is either synthesized by Patchback or carries a recorded
licence and provenance. Patchback is meant for recordings you are entitled to analyze.
