---
title: Files
sidebar_position: 2
description: The files Patchback reads and writes - reference audio, patches, events, fit runs and the dataset.
source: "DESIGN.md § 6.1, § 7.2, § 8.2, § 11; hand-written"
---

# Files

Patchback's own formats (`.patch`, `.events` and run metadata) are small, line-oriented,
versioned text formats that Patchback parses itself. Every parser refuses unknown versions,
duplicate keys and identifiers, malformed numbers, unsupported rates and unbounded lengths.
Audio is WAV; datasets are raw little-endian `f32` matrices.

## Reference audio

| Property | Accepted |
| --- | --- |
| Container | WAV (RIFF) |
| Channels | Mono, or stereo downmixed explicitly to mono |
| Samples | PCM16 or IEEE float32; float samples must be finite |
| Sample rate | 48,000 Hz; other rates are refused, not converted |

RIFF sizes and chunks, allocation bounds, channel counts and frame limits are validated before
any sample is used.

## Patch: `.patch`

The canonical, human-readable form of a [patch](../concepts/patch.md): format version, sample
rate, topology, nodes with their parameters, and edges between ports. Parameter values are
stored in physical units, such as seconds, hertz and decibels; the regressor's normalized
values never appear in a patch file.

## Events: `.events`

A versioned list of [note and control events](../concepts/events.md), timed in samples at the
patch's sample rate.

## A fit run

`patchback fit --out results/` writes:

```text
results/
├── patch.patch           canonical, human-readable, versioned patch data
├── notes.events          versioned sample-timestamped events
├── reconstructed.wav     rendered waveform
├── report.txt            loss terms, versions, timings, warnings, confidence
└── run.meta              seed, input digests, ModelSpec version, feature spec
```

`run.meta` holds what is needed to reproduce the run: the seed, digests of the inputs, the
ModelSpec version and the feature contract.

## The dataset

`patchback generate-dataset --out data/` writes ModelSpec's dense-data layout, one directory
per split:

```text
data/train/x.f32         N × 32 f32 feature matrix, one sample per row
data/train/y.f32         N × 8  f32 normalized target matrix
data/train/order.u32     optional, checked sequence of sample indices
data/val/x.f32
data/val/y.f32
```

Each matrix is row-major little-endian `f32`. `x.f32` holds the 32 values of
[`features/1`](../concepts/listening.md#features1) per sample, `y.f32` the eight
[normalized parameters](./parameters.md) in index order.

## Exported weights

The regressor's trained weights are stored as a checksummed, versioned little-endian `f32`
artifact that also records the model version and the feature-contract version.
