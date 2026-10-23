---
title: Command line
sidebar_label: CLI
sidebar_position: 1
description: Every patchback command and its arguments.
source: "DESIGN.md § 11; hand-written until the CLI exists to generate it from"
---

# Command line

Patchback is one binary, `patchback`. Each command reads files, writes files and exits; none of
them starts a server or needs a network.

| Command | Does |
| --- | --- |
| [`patchback patch validate`](#patchback-patch-validate) | Checks a patch without rendering it |
| [`patchback render`](#patchback-render) | Plays a patch with a score into a WAV |
| [`patchback analyze`](#patchback-analyze) | Measures a recording |
| [`patchback fit`](#patchback-fit) | Recovers a patch and its events from a recording |
| [`patchback compare`](#patchback-compare) | Compares two recordings term by term |
| [`patchback generate-dataset`](#patchback-generate-dataset) | Renders the regressor's training data |

## patchback patch validate

```bash
patchback patch validate patches/example.patch
```

Loads a `.patch` file, runs the identifier, rate, parameter, port, cycle and resource-cap
checks, and compiles the patch into an execution plan. It renders nothing and writes nothing.
The order of the checks is in [the file reference](./files.md#loading-then-validating). See
[patches](../concepts/patch.md).

| Argument | Meaning |
| --- | --- |
| `<file>` | The `.patch` file to check |

A valid patch prints one line to stdout:

```text
patchback: "patches/example.patch" is valid: topology subtractive-mono/1, 7 nodes, 7 edges
```

A refused patch prints one line to stderr. The line names the error, the file, and where the
fault is: the line of the file, or the node or edge (as its `.patch` line):

```text
patchback: patch validate refused: patchback.patch.MalformedRecord: "patches/example.patch": line 5: a `node` record has 3 fields, this one has 4
patchback: patch validate refused: patchback.patch.ParameterContractViolation: "patches/example.patch": node 7: parameter output_gain is 3, outside its contract's range [-24, 0]
patchback: patch validate refused: patchback.patch.UnknownPort: "patches/example.patch": edge 6 out Audio 7 sidechain Audio: node 7 (Output) declares no input port sidechain
```

The path is quoted with control characters escaped, so a file name cannot split the line.

| Exit status | Meaning |
| --- | --- |
| 0 | The patch is valid and compiles |
| 1 | Refused: the file cannot be read, is malformed, or is a graph that fails a check |
| 2 | Usage error: no file, more than one file, or an unknown flag |

## patchback render

```bash
patchback render --patch patches/example.patch --notes notes.events --out out.wav
```

| Argument | Meaning |
| --- | --- |
| `--patch` | The `.patch` file to play |
| `--notes` | The `.events` file to play it with |
| `--out` | The WAV to write, at 48 kHz |

## patchback analyze

```bash
patchback analyze --input reference.wav --out analysis/
```

| Argument | Meaning |
| --- | --- |
| `--input` | The WAV to analyze: mono or stereo, PCM16 or float32, 48 kHz |
| `--out` | The directory for the analysis results |

See [what Patchback hears](../concepts/listening.md).

## patchback fit

```bash
patchback fit --input reference.wav --out results/ --max-renders 2000
```

| Argument | Meaning |
| --- | --- |
| `--input` | The reference WAV |
| `--out` | The directory for the [fit run's files](./files.md#a-fit-run) |
| `--max-renders` | The render budget per note; the search stops when it is spent |

See [the closed loop](../concepts/closed-loop.md).

## patchback compare

```bash
patchback compare --reference reference.wav --candidate results/reconstructed.wav
```

| Argument | Meaning |
| --- | --- |
| `--reference` | The recording to match: mono or stereo, PCM16 or float32, 48 kHz |
| `--candidate` | The rendering to judge against it, in the same format |

Aligns the candidate with the reference in time, shifting it by up to 4 800 samples (100 ms),
then reports the log-spectral, envelope, transient and band-energy terms and their weighted sum.
See [the comparison](../concepts/closed-loop.md#the-comparison). The weights are the initial
values of the design, 0.45, 0.25, 0.15 and 0.15; the command takes none. The tolerance, the
spectral floor and the term definitions are provisional and documented in the `patchback-fit`
crate.

It prints one `key value` line per value to stdout, always these keys in this order:

```text
loss 0.23438711898344936
log_spectral 0.3627212920136793
envelope 0.2500002344751215
transient 0.0576014540255528
band_energy 0.0001484056978689425
weight_log_spectral 0.45
weight_envelope 0.25
weight_transient 0.15
weight_band_energy 0.15
lag_frames 0
```

Every term is between 0 and 1, and 0 when the two recordings are the same. `lag_frames` is the
shift applied to the candidate; a positive value means the candidate was late. A refused
comparison prints one line to stderr naming the error:

```text
patchback: compare refused: patchback.fit.ClipNotFound: no clip at "missing.wav" (reference)
patchback: compare refused: patchback.fit.LengthOutsideAlignment: the reference has 48000 frames and the candidate 52801; they differ by 4801, more than the 4800 frames time alignment absorbs
patchback: compare refused: patchback.audio.UnsupportedSampleRate: candidate "candidate.wav": unsupported sample rate 44100 Hz; only 48000 Hz
```

| Exit status | Meaning |
| --- | --- |
| 0 | Compared: the values are on stdout |
| 1 | Refused: a missing or unreadable file, a WAV import refuses, lengths alignment cannot absorb, or a recording longer than the 60 s the analysis takes |
| 2 | Usage error: `--reference` or `--candidate` missing, or an argument the command does not take |

## patchback generate-dataset

```bash
patchback generate-dataset --seed 42 --samples 60000 --out data/
```

| Argument | Meaning |
| --- | --- |
| `--seed` | The generator's seed; the same seed gives the same dataset |
| `--samples` | The number of patches to render across all splits |
| `--out` | The directory for the [dataset files](./files.md#the-dataset) |

See [train the regressor](../guides/train-the-regressor.md).

## ModelSpec commands

Training uses ModelSpec's own CLI, not a `patchback` command:
`modelspec check`, `modelspec ir --stage training` and `modelspec train --backend cpu --data
--eval`. They are documented in the
[ModelSpec CLI reference](https://beyond10x.github.io/modelspec/docs/reference/cli)
([GitHub](https://github.com/beyond10x/modelspec)).
