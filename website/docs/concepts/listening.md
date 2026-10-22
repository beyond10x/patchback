---
title: What Patchback hears
sidebar_label: Audio and features
sidebar_position: 4
description: What Patchback measures in a recording - FFT and STFT, envelopes, spectral descriptors, onsets and pitch - and the frozen 32-value feature contract features/1.
lede: "Patchback measures a recording with its own FFT and analysis code and reduces each note to the 32 values of features/1."
source: "DESIGN.md § 6"
---

# What Patchback hears

Everything Patchback knows about a recording comes from its own small DSP library, written in
Rust without an FFT or maths crate. `patchback analyze` runs it on a WAV and writes the
results to a directory.

## Input

Patchback reads mono WAV, or stereo WAV that it downmixes explicitly to mono, with PCM16 or
IEEE float32 samples at 48 kHz. The reader validates RIFF sizes and chunks, allocation bounds,
channel counts, frame limits and that every float sample is finite. Anything else is refused:
there is no silent resampling and no guessing past a corrupt chunk.

The unit of analysis is an isolated synth note, roughly 0.5 to 2 seconds including its
release.

## What is measured

- An iterative radix-2 complex FFT and its inverse, with Hann windows.
- STFT, magnitude and log magnitude at several frame sizes: 512, 2048 and 8192 samples at
  48 kHz, with one overlap and window convention.
- RMS and peak envelopes.
- Spectral centroid, rolloff, flatness and log-band energies.
- Spectral flux, for transient and onset proposals.
- Monophonic fundamental frequency, by autocorrelation or a YIN-style difference function. A
  low-confidence estimate is refused.
- Silence detection and note-gate boundaries.

A full mixed track may have no single pitch, gate or instrument these methods can identify.
Patchback reports that limit instead of hiding it.

## features/1

The [regressor](./regressor.md) sees each note as a fixed vector of 32 values, the feature
contract `features/1`:

| Values | Content |
| --- | --- |
| 24 | Energy in eight logarithmic frequency bands, computed separately over the attack, body and release windows, aligned to the note events |
| 8 | Calibrated RMS, peak, spectral centroid, rolloff, spectral flatness, zero-crossing rate, attack-envelope slope and normalized MIDI pitch |

Window bounds, FFT size, amplitude reference, band edges, normalization statistics and the
behaviour for missing data are frozen in the versioned contract before any dataset is
generated. The normalization means and standard deviations come from the training split only
and are reused unchanged at inference; no test audio influences them. A feature that cannot
be computed reliably fails with a structured diagnostic instead of an invented value. Changing
any of this means a new version of the contract.
