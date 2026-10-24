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

Loads a `.patch` file, runs the identifier, rate, parameter, port, fixed-chain, cycle and
resource-cap checks, and compiles the patch into an execution plan. It renders nothing and writes nothing.
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
patchback: patch validate refused: patchback.patch.FixedChainMismatch: "patches/example.patch": the patch is not the fixed chain of subtractive-mono/1: missing nodes Output; missing edges Vca out -> Output in
```

A refusal of the whole patch, such as the fixed chain, names no line, node or edge.

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
| `--patch` | The `.patch` file to play; it is loaded, validated and compiled first |
| `--notes` | The `.events` file to play it with, at 48 kHz |
| `--out` | The mono 48 kHz float32 WAV to write; an existing file is never overwritten |
| `--block-size` | Frames per processing block, a positive integer; 256 by default. It changes how the work is cut, never the samples |

The render lasts until the last note ends, then for the envelope's release, then for a 100 ms
tail. It prints one `key value` line per value to stdout, always these keys in this order; the
`out` value is the path quoted and escaped as a refusal quotes a path (`\n`, `\\`, `\xNN` for a
byte outside UTF-8), so one record is one line whatever the path holds:

```text
out "out.wav"
sample_rate_hz 48000
block_size_frames 256
frame_count 43200
limited false
limited_samples 0
```

`limited true` means the protective output limiter set samples above full scale to full scale;
`limited_samples` counts them. A refused render writes nothing and prints one line to stderr
naming the error:

```text
patchback: render refused: patchback.audio.InvalidBlockSize: block size 0 frames; a block is a positive number of frames
patchback: render refused: patchback.audio.PatchNotFound: "missing.patch": No such file or directory (os error 2)
```

| Exit status | Meaning |
| --- | --- |
| 0 | Rendered and written |
| 1 | Refused: a rate other than 48 kHz, a block size of zero or less, a patch or score that cannot be read or loaded, a patch that fails validation, a render past the length bound, a non-finite sample, or an output that exists or cannot be written |
| 2 | Usage error: a missing `--patch`, `--notes` or `--out`, a `--block-size` that is not an integer, or an unknown flag |

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
patchback fit --input reference.wav --notes notes.events --out results/ --max-renders 2000
```

| Argument | Meaning |
| --- | --- |
| `--input` | The reference WAV: mono or stereo, PCM16 or float32, 48 kHz |
| `--notes` | The `.events` file of the reference's known notes, at 48 kHz |
| `--out` | The directory for the [fit run's files](./files.md#a-fit-run); created when absent |
| `--max-renders` | The most render evaluations; 2000 by default. The search stops when it is spent |
| `--max-wall-time-ms` | The wall time in milliseconds after which no further render starts (the first render always runs); 60000 by default |
| `--max-restarts` | The restarts of the coordinate search; 4 by default |
| `--improvement-threshold` | A restart that lowers the best loss by less than this ends the search; a finite, non-negative loss, 0.0001 by default |
| `--seed` | The seed of the restarts' starting points, 0 to 2^64 - 1; 0 by default |
| `--block-size` | Frames per render block, a positive integer; 256 by default |

The fit starts from the default patch, then restarts from seeded points; each restart moves one
normalized parameter at a time by a step that starts at 0.25 and halves, and keeps a candidate
only when its loss is strictly lower. No render starts once the wall time has reached
`--max-wall-time-ms`, except the first, which always runs, so a time bound stops a fit instead of
failing it. A render in progress is not interrupted, and the clock starts before the reference
is analysed, so `wall_time_ms` may exceed `--max-wall-time-ms` by the reference analysis plus
one evaluation. The same seed and inputs write the same `patch.patch`.

It writes `patch.patch`, the best patch as canonical text, and prints one `key value` line per
value to stdout, always these keys in this order; `out` is the directory quoted and escaped as a
refusal quotes a path:

```text
out "results/"
init default
evaluations 40
restarts 1
initial_loss 0.37590542047749115
best_loss 0
log_spectral 0
envelope 0
transient 0
band_energy 0
stop_reason RenderBudgetExhausted
wall_time_ms 6856
limited false
```

`initial_loss` is the loss of the default patch, `best_loss` and the four components those of
the patch written. `stop_reason` is `RenderBudgetExhausted`, `TimeBudgetExhausted`,
`RestartsExhausted` or `ImprovementBelowThreshold`. `wall_time_ms` is the run's wall time in
milliseconds, the reference analysis included; it may exceed `--max-wall-time-ms` by the
reference analysis plus one evaluation. `limited true` means the output limiter changed a sample
of the best patch's render.
A refused or failed fit writes nothing, not even the directory, and prints one line to stderr
naming the error:

```text
patchback: fit refused: patchback.fit.InvalidSearchBound: the RenderEvaluations bound is 0; a fit runs only within positive bounds
patchback: fit refused: patchback.fit.ClipNotFound: no clip at "missing.wav": No such file or directory (os error 2)
patchback: fit refused: cannot write "results": is not a directory
patchback: fit refused: cannot write "results/patch.patch": already exists; it is never overwritten
```

They are answered in this order: the bounds, `--input`, `--notes`, `--out` (an `--out` that exists
and is not a directory, a symlink that does not resolve to a directory included, or lies beneath
such a file, then an existing `patch.patch`, the file unchanged in both cases), the reference's
silence and pitch, the search.

| Exit status | Meaning |
| --- | --- |
| 0 | Fitted, and `patch.patch` written |
| 1 | Refused: a bound of 0 (`InvalidSearchBound`), a reference that cannot be read or imported (`ClipNotFound`), a note sequence that cannot be read or parsed (`NoteSequenceNotFound`), a silent reference (`ReferenceSilent`), a reference whose pitch cannot be estimated (`PitchAmbiguous`), a run that found no valid candidate (`NoValidCandidate`), an `--out` that is not a directory, or a `patch.patch` that exists or cannot be written |
| 2 | Usage error: a missing `--input`, `--notes` or `--out`, a bound or seed that is not a non-negative integer, a threshold that is not a finite non-negative decimal, a `--block-size` below 1, or an unknown flag |

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
| `--seed` | The generator's seed; the same seed and build give byte-identical files |
| `--samples` | The number of samples across all splits, from 0 to 9223372036854775807 |
| `--out` | The directory for the [dataset files](./files.md#the-dataset); created when absent |

The samples are split by patch: validation and test each get
`max(16, floor(samples × 4992 / 60000 / 16) × 16)` samples and train the rest, so
`--samples 60000` gives 50,016 / 4,992 / 4,992 and `--samples 48` gives 16 / 16 / 16. Every split
must be a positive multiple of the batch size 16.

Each patch seed comes from the dataset seed. Each seed draws eight normalized parameters,
uniform, a MIDI note from 36 to 84 and a gate from 0.25 to 1.5 s, log-uniform, played at full
velocity from the first sample. The patch is rendered with the same engine as `patchback render`
and described with [`features/1`](../concepts/listening.md#features1). A render that the output
limiter changed, a silent render, and a render whose features cannot be computed (for example an
ambiguous pitch) go to a separate diagnostic set, and the next seed is drawn. A patch is in one
split only. The training distribution is provisional.

The command prints one `key value` line per value to stdout: `out`, the directory quoted as
`patchback render` quotes its output, then the counts that `dataset.meta` records:

```text
out "data/"
train 50016
validation 4992
test 4992
diagnostic_silence 4120
diagnostic_noise 28750
diagnostic_clipping 25000
```

The diagnostic counts above are illustrative. About half of the drawn patches went to the
diagnostic set in the 48- and 480-sample runs measured so far.

A refusal prints one line to stderr and leaves nothing written:

```text
patchback: generate-dataset refused: patchback.dataset.SplitCountNotBatchMultiple: the Train split of 15 samples is not a positive multiple of the batch size 16
patchback: generate-dataset refused: cannot write "data/dataset.meta": already exists; it is never overwritten
```

| Exit status | Meaning |
| --- | --- |
| 0 | Generated and written |
| 1 | Refused: a split count that is not a positive multiple of 16, an existing `dataset.meta` or `samples.meta`, an output that cannot be written, or a generation that ran out of draws (more than 4 per sample) before the splits were full |
| 2 | Usage error: a missing `--seed`, `--samples` or `--out`, a `--seed` or `--samples` that is not an unsigned integer, a `--samples` above 9223372036854775807, or an unknown flag |

See [train the regressor](../guides/train-the-regressor.md).

## ModelSpec commands

Training uses ModelSpec's own CLI, not a `patchback` command:
`modelspec check`, `modelspec ir --stage training` and `modelspec train --backend cpu --data
--eval`. They are documented in the
[ModelSpec CLI reference](https://beyond10x.github.io/modelspec/docs/reference/cli)
([GitHub](https://github.com/beyond10x/modelspec)).
