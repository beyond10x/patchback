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

```text
patchback-patch 1
sample_rate_hz 48000
topology_id subtractive-mono/1
node 4 LowpassFilter
param 4 filter_cutoff 1200
param 4 resonance_q 0.7
edge 3 out Audio 4 in Audio
```

### Records

The file is UTF-8 text, one record per line, fields separated by whitespace. There are no
comments and no blank lines.

| Record | Fields | Occurs |
| --- | --- | --- |
| `patchback-patch` | the format version, `1` | first line, once |
| `sample_rate_hz` | `48000`; no other rate is read | once |
| `topology_id` | the topology, `subtractive-mono/1` | once |
| `node` | identifier, kind (`NoteInput`, `SawOscillator`, `Drive`, `LowpassFilter`, `AdsrEnvelope`, `Vca`, `Output`, or a module family's kind) | once per node |
| `param` | node identifier, parameter name, value | after its node's line, once per parameter |
| `edge` | from node, output port, rate, to node, input port, rate | once per edge |

Identifiers and the version are unsigned decimal integers. Names are lower-case snake_case. A
rate is `Audio`, `Control` or `Event`. A parameter's name is the path of its
[contract](./parameters.md), such as `filter_cutoff`.

### Numbers

A value is a finite decimal: an optional sign, digits, an optional `.` and digits, and an
optional exponent (`e` or `E`, optional sign, digits). `nan`, `inf`, hexadecimal, `.5`, `5.` and
a value that overflows to infinity are refused. The canonical writer prints the shortest text
that reads back to the same value.

### Bounds

| Constant | Value | Bounds |
| --- | --- | --- |
| `MAX_LINE_BYTES` | 256 | bytes in one line |
| `MAX_FILE_BYTES` | 1,048,576 | bytes in the file |
| `MAX_NODES` | 256 | `node` records |
| `MAX_EDGES` | 1024 | `edge` records |

All four are provisional. A file past one is refused before the excess is stored.

### Loading, then validating

`topology_id` holds the topology's name, such as `subtractive-mono/1`. A name no topology
carries is an unknown topology, and so is a UUID.

Loading a file refuses it with the fault that ranks first. A fault in the file's form names the
line it is on:

1. unknown format version
2. unsupported sample rate
3. duplicate node identifier
4. duplicate key or parameter
5. unknown topology
6. a node kind outside the topology's catalogue
7. malformed record
8. malformed number
9. a bound exceeded

A file past the byte bound, or with a fault on its first line, is refused for that fault alone.

A file can be well formed and still be a graph that does not run.
[`patchback patch validate`](./cli.md#patchback-patch-validate) also checks the graph, and
refuses with the first failing check, in this order:

1. the topology is known, and every node's kind is in its catalogue
2. every edge names declared nodes
3. both ends of every edge state one rate
4. every node carries exactly its kind's parameters, each inside its contract's range
5. every edge names ports its node kinds declare, at the rates they declare
6. the graph has no cycle
7. at most `MAX_GRAPH_NODES` (256) nodes, `MAX_GRAPH_EDGES` (1024) edges and `MAX_VOICES` (1)
   `NoteInput` nodes; all three caps are provisional
8. every input port takes at most one edge

Only a validated patch is compiled into an execution plan.

The ports each kind declares:

| Kind | Inputs | Outputs |
| --- | --- | --- |
| `NoteInput` | none | `pitch` Control, `gate` Event |
| `SawOscillator` | `pitch` Control | `out` Audio |
| `Drive` | `in` Audio | `out` Audio |
| `LowpassFilter` | `in` Audio | `out` Audio |
| `AdsrEnvelope` | `gate` Event | `env` Control |
| `Vca` | `in` Audio, `gain` Control | `out` Audio |
| `Output` | `in` Audio | none |

An input port takes at most one edge: a duplicated edge, or a second source into one input, is
refused. An output port feeds any number of edges.

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
