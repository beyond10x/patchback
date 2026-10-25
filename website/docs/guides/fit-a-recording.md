---
title: Fit a recording
sidebar_position: 2
description: Prepare a recording of one synth note, fit a patch to it, and read what the fit run writes.
lede: "Fit a patch to a recording of one synth note and read the five files the fit writes."
source: "DESIGN.md § 6.1, § 9, § 11"
---

# Fit a recording

A fit takes a recording and returns a patch, the events it plays, its rendering and a report.
This guide fits a recording you did not render yourself.

## Prepare the recording

Patchback works best on what it was designed for: one isolated, monophonic synth note, roughly
0.5 to 2 seconds long including its release, without other instruments or effects on top.

The file must be a WAV at 48 kHz, mono or stereo, with PCM16 or float32 samples. Patchback
does not resample: convert other rates before you start. Stereo is downmixed to mono
explicitly. Use only recordings you are entitled to analyze.

## Analyze first

```bash
patchback analyze --input reference.wav --out analysis/
```

Analysis estimates the note's onset, pitch and gate from the audio, each with a confidence. If
the pitch or the gate is too ambiguous to estimate, it stops with a diagnostic. A
fit on that recording would be built on a guess, so fix the recording first.

## Fit

```bash
patchback fit --input reference.wav --out results/ --max-renders 2000
```

The fit predicts a starting patch with the [regressor](../concepts/regressor.md), or uses the
default patch when no weights are present. It then runs seeded restarts and a
[coordinate search](../concepts/closed-loop.md#the-search) until 2,000 renders are spent or the
improvement stalls. `--max-renders` is the budget per note; a smaller budget finishes sooner,
a larger one searches longer.

```bash
patchback fit --input reference.wav --notes notes.events --out seeded/ \
  --weights regressor.weights --normalization normalization.stats
```

To start the search from the regressor's guess instead of the default patch, give `patchback fit`
both `--weights` and `--normalization`. The weights are the `weights/1` file exported from a
trained regressor ([train the regressor](./train-the-regressor.md)); the statistics are the
`normalization.stats` that `patchback generate-dataset` wrote for the dataset the regressor was
trained on, so the reference's features are scaled as the training rows were. Each flag needs
the other. The fit predicts a patch from the reference's first note and starts its first
restart there; later restarts are as without the flags, so the run is still deterministic for a
given seed. stdout and `run.meta` then say `init regressor-prediction`, and `run.meta` records
both files' digests. If the prediction put some of its eight values outside [0, 1], they are
clamped into range before the search starts, and stdout and `report.txt` end with
`warning clamped <n> of 8 predictions`: the regressor is unsure about this sound, and the start
is less informed than its other predictions.

## Read the results

| File | Read it for |
| --- | --- |
| `results/patch.patch` | The recovered patch, in physical units; edit it and render it again |
| `results/notes.events` | The note events the patch was played with |
| `results/reconstructed.wav` | What the recovered patch sounds like; listen to it next to the reference |
| `results/report.txt` | Each loss term, versions, timings, warnings and confidence |
| `results/run.meta` | The seed, input digests, ModelSpec version and feature contract, to reproduce the run |

Then compare the reconstruction with the reference directly:

```bash
patchback compare --reference reference.wav --candidate results/reconstructed.wav
```

## When the match is not close

The first instrument is a [fixed subtractive voice](../concepts/patch.md#the-fixed-subtractive-voice):
a saw through drive, a lowpass filter and an ADSR. A sound made with FM, noise or heavy
modulation is outside what eight knobs can reach; the report's loss terms show where the
rendering differs. Also remember that several patches can sound alike, so a close match need
not reproduce the original synth's settings.
