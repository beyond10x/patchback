---
title: Render a patch
sidebar_position: 1
description: Validate a patch, play it with a score of note events, and get the same WAV every time.
lede: "Validate a patch, play it with a score of note events, and get the same WAV every time."
source: "DESIGN.md § 5, § 10, § 11"
---

# Render a patch

Rendering is the half of Patchback that does not guess. You give it a patch and a score; it
gives you a WAV, and the same inputs always give the same samples.

## Validate the patch

```bash
patchback patch validate patches/example.patch
```

Validation runs the identifier, parameter, port and rate, and cycle checks described under
[patches](../concepts/patch.md#from-file-to-execution-plan). A patch that fails is refused with
a diagnostic. `render` runs the same checks, so a patch that fails here never renders.

## Write the score

The score is a versioned `.events` file of [note events](../concepts/events.md): each note
has a start sample, an end sample, a MIDI note and a velocity. Times are in samples at 48 kHz,
so one second is 48,000 samples. Note-on opens the envelope and note-off starts its release;
offsets are sample-accurate.

## Render

```bash
patchback render --patch patches/example.patch --notes notes.events --out out.wav
```

The engine processes the patch in blocks, 256 frames by default, and writes `out.wav` at
48 kHz. Its output is finite for every valid patch and score. If protective clipping or
limiting had to act, the render says so instead of hiding it.

## Change something and listen

Patch parameters are stored in physical units, so editing a patch means editing numbers you
already know: an attack in seconds, a cutoff in hertz, a drive in decibels. Copy the patch,
raise the cutoff in the copy, render it and compare the two files:

```bash
patchback render --patch patches/brighter.patch --notes notes.events --out out-brighter.wav
patchback compare --reference out.wav --candidate out-brighter.wav
```

The [parameter reference](../reference/parameters.md) gives every parameter's range.
