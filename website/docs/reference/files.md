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

A file past the byte bound, or with a fault on a first line that does not read as version 1, is
refused for that fault alone. The first line's own faults rank in the same order, so
`patchback-patch 2 extra` is refused as an unknown format version, not as a malformed record. A
first line that reads as version 1 with a field too many, `patchback-patch 1 extra`, is a
malformed record ranked with the faults of the later lines: with `sample_rate_hz 44100` on line
2, the file is refused for the sample rate.

A file can be well formed and still be a graph that does not run.
[`patchback patch validate`](./cli.md#patchback-patch-validate) also checks the graph, and
refuses with the first failing check, in this order:

1. the topology is known, and every node's kind is in its catalogue
2. every edge names declared nodes
3. both ends of every edge state one rate
4. every node carries exactly its kind's parameters, each inside its contract's range
5. every edge names ports its node kinds declare, at the rates they declare
6. the patch is the fixed chain of its topology: one node of each of the seven kinds below and
   every edge of the chain (`FixedChainMismatch`, naming the missing kinds, the extra nodes and
   the missing edges)
7. the graph has no cycle
8. at most `MAX_GRAPH_NODES` (256) nodes, `MAX_GRAPH_EDGES` (1024) edges and `MAX_VOICES` (1)
   `NoteInput` nodes; all three caps are provisional. After check 6 a file can exceed none of
   them: the chain has seven nodes and one `NoteInput`, and a file holds at most 1024 edges
9. every input port takes at most one edge

The fixed chain of `subtractive-mono/1`:

```text
NoteInput pitch -> SawOscillator pitch
NoteInput gate  -> AdsrEnvelope gate
SawOscillator out -> Drive in
Drive out         -> LowpassFilter in
LowpassFilter out -> Vca in
AdsrEnvelope env  -> Vca gain
Vca out           -> Output in
```

The chain feeds every input port, so an edge beyond it closes a cycle or feeds a taken input,
and checks 7 and 9 refuse it.

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

`notes.events` is a versioned list of [note events](../concepts/events.md), timed in samples at
the patch's sample rate. It holds notes only; control automation gets its own file,
`controls.events`.

```text
patchback-notes 1
sample_rate_hz 48000
patch_id 7f0c0000-0000-4000-8000-000000000151
note 0 24000 45 0.8
note 24000 48000 52 0.6
```

### Records

The file is UTF-8 text, one record per line, fields separated by whitespace. There are no
comments and no blank lines. After the header, records may come in any order.

| Record | Fields | Occurs |
| --- | --- | --- |
| `patchback-notes` | the format version, `1` | first line, once |
| `sample_rate_hz` | the sample rate the notes are timed at | once |
| `patch_id` | the patch the sequence was written for, a lower-case UUID | once |
| `note_estimation_id` | the note estimation it was written from, a lower-case UUID | at most once |
| `note` | start sample, end sample, MIDI note, velocity | once per note |

Sample positions, the MIDI note, the version and the rate are unsigned decimal integers, so a
negative value is a malformed number. A velocity is a finite decimal of the
[`.patch` grammar](#numbers), read as a 32-bit float.

A note ends after it starts, its MIDI note is 0 to 127, and its velocity is between 0 and 1.
The sequence is monophonic: no note starts while another sounds, and no two notes start on the
same sample. A note may start on the sample the previous one ends.

The parser reads the sample rate as written. Whether it matches the rate of the patch that
renders the sequence, and whether that patch exists, is checked when the sequence is bound to
its patch.

### Bounds

| Constant | Value | Bounds |
| --- | --- | --- |
| `MAX_LINE_BYTES` | 256 | bytes in one line |
| `MAX_FILE_BYTES` | 8,388,608 | bytes in the file |
| `MAX_NOTES` | 65,536 | `note` records |

All three are provisional.

### Refusals

A refused file loads no sequence. A file with several faults is refused with the one that ranks
first, and a fault on a line names that line:

1. unknown format version
2. a note that does not end after it starts
3. a MIDI note above 127
4. a velocity outside 0 to 1
5. a note overlapping a note on an earlier line
6. duplicate key
7. malformed record
8. malformed number
9. a bound exceeded

A file past the byte bound, or with a fault on its first line, is refused for that fault alone.

### Canonical form

The writer emits the header, `sample_rate_hz`, `patch_id`, `note_estimation_id` if there is
one, then the notes in start order. A velocity is written as the shortest text that reads back
to the same value. Writing and then reading gives the same sequence bit for bit, and sample
positions are exact up to the largest 64-bit value.

## A fit run

`patchback fit --out results/` writes `patch.patch` today; the other four files are story
fit-run-artifacts. The run's directory:

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

`patchback generate-dataset --out data/` writes `data/dataset.meta` and one directory per split,
`data/train/`, `data/val/` and `data/test/`, each holding `samples.meta`. Both files are
line-oriented, versioned UTF-8 text like `.patch`: a header line, then one record per line, with
fields separated by one space.

### `dataset.meta`

```text
patchback-dataset-meta 1
seed 42
prng xoshiro256**
batch_size 16
feature_contract features/1
topology subtractive-mono/1
train 16
validation 16
test 16
diagnostic_silence 4
diagnostic_noise 23
diagnostic_clipping 20
```

| Record | Value |
| --- | --- |
| `patchback-dataset-meta` | the format version, `1`; first line |
| `seed` | the `--seed` the dataset was generated from |
| `prng` | the generator, `xoshiro256**` |
| `batch_size` | `16`; every split count is a multiple of it |
| `feature_contract` | `features/1` |
| `topology` | `subtractive-mono/1` |
| `train`, `validation`, `test` | the split's sample count; the validation split's directory is `val/` |
| `diagnostic_silence`, `diagnostic_noise`, `diagnostic_clipping` | the patches set aside in the diagnostic set, by kind |

The records appear once each, in this order.

### `samples.meta`

```text
patchback-dataset-samples 1
13679457532755275413 80 49509
6349198060258255764 58 31590
```

After the header line, `patchback-dataset-samples 1`, there is one line per sample in the order
the samples were drawn: the patch seed, the MIDI note and the gate in samples at 48 kHz, all
unsigned decimal integers. Line `i + 1` describes row `i` of the split's dense files.

### Dense files

The dense files are not written yet (story dense-data-files). They will follow ModelSpec's
dense-data layout, one directory per split:

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
