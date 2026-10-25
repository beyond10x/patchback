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

`patchback fit --out results/` writes six files into the run's directory, each created new; none
is ever overwritten, and a fit that finds any of them already there writes nothing:

```text
results/
├── patch.patch           the best patch, canonical, human-readable, versioned
├── notes.events          the note sequence the run was driven by, canonical
├── reconstructed.wav     the clip the best patch was scored on
├── report.txt            loss terms, counts, stop reason, wall time, warnings
├── run.meta              versions, seed, input digests, ModelSpec pin, feature contract, bounds
└── trace.events          one line per render evaluation
```

`report.txt` and `run.meta` are line-oriented, versioned UTF-8 text like
[`dataset.meta`](#datasetmeta): a header line, then one record per line, a key and a value
separated by one space. Each record appears once, in the order of its table. Reals are printed as
Rust prints an `f64`: the shortest decimal that reads back to the same value, never in exponent
form. The same seed, inputs, bounds and block size write the same six files, byte for byte, except
the `wall_time_ms` record of `report.txt`, unless the wall-time bound stops the run.

### patch.patch

The best patch the run retained, in the canonical form of [`.patch`](#patch-patch). It loads and
validates like any patch: `patchback patch validate results/patch.patch`.

### notes.events

The `--notes` sequence in the canonical form of [`.events`](#events-events): the notes in start
order, one space between fields.

### reconstructed.wav

The render of the best patch, cut or padded with silence to the reference's length: the clip the
search scored. Mono, 48 kHz, 32-bit float. So
`patchback compare --reference reference.wav --candidate results/reconstructed.wav` prints the
fit's `best_loss` as its `loss`.

### report.txt

```text
patchback-fit-report 1
patchback 0.6.0
initial_loss 0.37590542047749115
best_loss 0
log_spectral 0
envelope 0
transient 0
band_energy 0
evaluations 12
restarts 1
stop_reason RenderBudgetExhausted
wall_time_ms 2132
```

| Record | Value |
| --- | --- |
| `patchback-fit-report` | the format version, `1`; first line |
| `patchback` | the Patchback version that wrote it |
| `initial_loss` | the loss of the run's first valid candidate, the default patch's when it rendered |
| `best_loss` | the loss of the best patch, the one written |
| `log_spectral`, `envelope`, `transient`, `band_energy` | the best patch's four loss terms |
| `evaluations` | the render evaluations counted, valid and rejected |
| `restarts` | the restarts that began |
| `stop_reason` | `RenderBudgetExhausted`, `TimeBudgetExhausted`, `RestartsExhausted` or `ImprovementBelowThreshold` |
| `wall_time_ms` | the run's wall time in milliseconds, the reference analysis included; the one record that differs between two runs of the same inputs |
| `warning` | zero or more, last, one per warning: `warning limiter engaged on <n> of <m> rendered samples` when the protective limiter changed `<n>` of the `<m>` samples of the best patch's whole render (`reconstructed.wav` is that render cut or padded to the reference's length, so it can hold fewer of them) |

There is no `confidence` record: the design names one and does not define it yet.

### run.meta

```text
patchback-fit-run-meta 1
patchback 0.6.0
init default
seed 0
topology subtractive-mono/1
feature_contract features/1
prng xoshiro256**
modelspec 0.5.0
reference sha256 45b93160d895363ef050bb4d4e1d35196a06a69892041677532d81e1a6161192
notes sha256 8db54832289b7c7c00738b1f8a87bd19a7ad32ff97592307aa77d65c7bf68ed5
max_renders 12
max_wall_time_ms 60000
max_restarts 1
improvement_threshold 0.0001
block_size 256
```

| Record | Value |
| --- | --- |
| `patchback-fit-run-meta` | the format version, `1`; first line |
| `patchback` | the Patchback version that wrote it |
| `init` | `default`: the run started from the default patch |
| `seed` | the `--seed` |
| `topology` | `subtractive-mono/1` |
| `feature_contract` | `features/1` |
| `prng` | the generator of the restart points, `xoshiro256**` |
| `modelspec` | the version of the ModelSpec release the build is pinned to (`models/modelspec.pin`) |
| `reference` | `sha256` and the 64 lower-case hexadecimal digits of the SHA-256 of the `--input` file's bytes |
| `notes` | the same for the `--notes` file |
| `max_renders`, `max_wall_time_ms`, `max_restarts`, `improvement_threshold` | the run's bounds, as given |
| `block_size` | the `--block-size` |

### trace.events

```text
patchback-fit-trace 1
topology subtractive-mono/1 attack decay sustain release filter_cutoff resonance_q drive output_gain
evaluation 1 1 0 first-retained subtractive-mono/1 0.48410502977419295 0.12391674674059834 0.5346748721716756 0.3125182637880222 0.37590542047749115 0.37590542047749115 0.5 0.5 0.5 0.5 0.5 0.5 0.5 0.5
evaluation 2 2 0 not-improved subtractive-mono/1 0.5266998323119957 0.30277470621039193 0.5183991466580976 0.31215091418501223 0.4372911102194625 0.37590542047749115 0.75 0.5 0.5 0.5 0.5 0.5 0.5 0.5
completed RenderBudgetExhausted
```

The per-candidate log of the run, written once the run has completed. After the header line,
`patchback-fit-trace 1`:

| Line | Fields |
| --- | --- |
| `topology` | the topology, then its [parameter paths](./parameters.md) in index order |
| `evaluation` | one per render evaluation, in order: the sequence number from 1; the run's evaluation count after it, always equal to the sequence number; the restart, from 0; the outcome, `first-retained`, `improved`, `not-improved` or `rejected`; the topology; the `log_spectral`, `envelope`, `transient` and `band_energy` terms and the weighted loss, or a single `-` for a rejected candidate; the best loss after it, or `-` until a candidate is retained (a number on a `first-retained`, `improved` or `not-improved` line, `-` on a `rejected` line before the first `first-retained` one), never above the line before and never `-` again once a number; the candidate's normalized value for each path of the `topology` line |
| `completed` | last: the stop reason |

Reals are written in the shortest form that reads back to the same value, in exponent form when
that is shorter. A reader refuses a version other than 1, a number that does not read, a line
longer than 4 096 bytes, a line out of place, an evaluation count other than its line's sequence
number, a best loss above the line before or `-` after a number, a `-` best loss on a
`first-retained`, `improved` or `not-improved` line, a numeric best loss on a `rejected` line
before the first `first-retained` line, and a file without its `completed` line. A fit that fails
writes no trace; whether it should keep one is open.

## The dataset

`patchback generate-dataset --out data/` writes `data/dataset.meta` and one directory per split,
`data/train/`, `data/val/` and `data/test/`, each holding `samples.meta` and the split's
[dense files](#dense-files). `dataset.meta` and `samples.meta` are line-oriented, versioned UTF-8
text like `.patch`: a header line, then one record per line, with fields separated by one space.

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

Each split directory also holds three binary files in ModelSpec's dense-data layout. Row `i` of
each file is the `i`th sample of the split, in the order of `samples.meta`:

```text
data/train/x.f32         N × 32 f32 feature matrix, one sample per row
data/train/y.f32         N × 8  f32 normalized target matrix
data/train/order.u32     N u32 sample indices
data/val/x.f32
data/val/y.f32
data/val/order.u32
data/test/x.f32
data/test/y.f32
data/test/order.u32
```

| File | One row | Bytes for N samples | N = 16 | N = 4,992 |
| --- | --- | --- | --- | --- |
| `x.f32` | the 32 values of [`features/1`](../concepts/listening.md#features1) | N × 32 × 4 | 2,048 | 638,976 |
| `y.f32` | the eight [normalized parameters](./parameters.md), in index order | N × 8 × 4 | 512 | 159,744 |
| `order.u32` | the row's index | N × 4 | 64 | 19,968 |

The matrices are row-major little-endian `f32`. Each value is the generator's `f64` rounded to
the nearest `f32`. `order.u32` holds the row indices 0 to N − 1 in order, as little-endian `u32`.
The check that `order.u32` gets is provisional until ModelSpec's expectation is known: N entries,
each below N, each appearing once.

`patchback generate-dataset` reads each file back after writing it. A file is refused, and
nothing is left written, when its byte count is not rows × columns × 4, when a value of `x.f32`
or `y.f32` is NaN or infinite, or when `order.u32` fails its check. A split admits at most
`u32::MAX` (4,294,967,295) rows, so that its row count is a `u32`; `--samples`, which bounds all
splits together, stops at the same number.

## Exported weights

The regressor's trained weights are stored as a checksummed, versioned little-endian `f32`
artifact that also records the model version and the feature-contract version.
