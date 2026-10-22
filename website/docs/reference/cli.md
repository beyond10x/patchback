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

Runs the identifier, parameter, port and rate, and cycle checks on a `.patch` file. A patch
that fails is refused with a diagnostic. See [patches](../concepts/patch.md).

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
| `--reference` | The recording to match |
| `--candidate` | The rendering to judge against it |

Aligns the two in time and reports the log-spectral, envelope, transient and band-energy terms
and their weighted sum. See [the comparison](../concepts/closed-loop.md#the-comparison).

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
