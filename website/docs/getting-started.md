---
title: Getting started
sidebar_position: 2
description: Render a reference from a known patch, fit it back from the audio alone, and compare the reconstruction with the reference.
---

# Getting started

The shortest way to see what Patchback does is to give it a sound whose answer you already
know. Render a reference from a patch, let Patchback fit a patch to that audio, and compare the
two renderings. Nothing in this walk-through needs a trained model: without weights, the fit
starts from the default patch.

The [status page](/docs/status) says which of these commands exist today.

## Build

Patchback is one command-line binary, `patchback`, written in Rust with the standard library
only. It lives in the `patchback-cli` crate of the
[repository](https://github.com/beyond10x/patchback) and builds with Cargo. Offline use needs
no audio library, no GPU and no Python.

## 1. Check a patch

A patch is a text file that names its modules, connects their ports and sets their parameters
in physical units. Validation checks identifiers, parameter domains, port rates and cycles
before anything renders:

```bash
patchback patch validate patches/example.patch
```

A patch that fails a check is refused with a diagnostic and never reaches the renderer. The
[patch concept](./concepts/patch.md) lists the checks in order.

## 2. Render a reference

Rendering plays the patch with a score of note events and writes a WAV at 48 kHz:

```bash
patchback render --patch patches/example.patch --notes notes.events --out reference.wav
```

`notes.events` holds note events timed in samples: start, end, MIDI note and velocity. The
same patch, events, build and configuration render the same samples every time.

## 3. Look at what Patchback hears

```bash
patchback analyze --input reference.wav --out analysis/
```

Analysis reads the WAV, refuses unsupported formats and non-finite samples, and computes
spectra at several resolutions, envelopes, onset proposals and a pitch estimate. The
[page on what Patchback hears](./concepts/listening.md) describes each.

## 4. Fit a patch to the audio

```bash
patchback fit --input reference.wav --out results/ --max-renders 2000
```

The fit renders candidate patches, compares each with the reference and keeps the best valid
one, spending at most 2,000 renders on the note. It writes five files:

```text
results/
├── patch.patch           the recovered patch
├── notes.events          the events it was played with
├── reconstructed.wav     the recovered patch's rendering
├── report.txt            loss terms, versions, timings, warnings, confidence
└── run.meta              seed, input digests, ModelSpec version, feature spec
```

## 5. Compare

```bash
patchback compare --reference reference.wav --candidate results/reconstructed.wav
```

The comparison aligns the two recordings in time and reports four loss terms: log-spectral,
envelope, transient and band energy. Lower is closer. Because several patches can sound
alike, `results/patch.patch` need not equal `patches/example.patch`; what counts is how the
two renderings compare.

## Next

- [Fit a recording](./guides/fit-a-recording.md) of a synth you did not program.
- [Train the regressor](./guides/train-the-regressor.md) so the search starts close to the
  answer.
- [Files](./reference/files.md) for what is in each file.
