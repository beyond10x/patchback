---
title: Note events
sidebar_position: 3
description: The patch is the instrument; note and control events are the score. Both are timed in samples, and estimated events carry their confidence.
lede: "The patch is the instrument and the events are the score: both are timed in samples, and estimated events carry their confidence."
source: "DESIGN.md § 10"
---

# Note events

A patch is an instrument, and an instrument needs a score. Patchback keeps the two apart: the
patch says how a note sounds, the events say which notes play and when.

```rust
struct NoteEvent {
    start_sample: u64,
    end_sample: u64,
    midi_note: u8,
    velocity: f32,
}

struct ControlEvent {
    sample: u64,
    parameter_path: String,
    normalized_value: f32,
}
```

Every event is timed in samples at the patch's sample rate, so a rendering is sample-accurate
and repeatable. The oscillator's pitch comes from the note events; it is not a timbre parameter
and the regressor does not learn it. A control event sets one parameter, addressed by its path,
to a normalized value at a given sample. Events are stored in a versioned `.events` file.

## Known and estimated events

For a reference Patchback rendered itself, the note events are known exactly: they are the
events it was rendered with. For external audio, Patchback estimates onsets, pitches, gate
lengths and tails from the recording, and every estimate carries a confidence. An estimate too
ambiguous to trust is refused with a diagnostic rather than guessed.

| Stage | Events |
| --- | --- |
| Single notes | Known note events of generated references |
| Phrases | Monophonic onset, pitch and gate estimates, with confidence and ambiguity diagnostics |
| Stems | One instrument stem with repeated notes, then simple polyphony |
| Arrangement | Several stems and arrangement events |

## Longer music

Patchback reconstructs sounds. It starts with isolated stems and builds up to several stems
and arrangement, not to an unconstrained stereo mix. Composing new sequences is a different
objective with a different evaluation; a sequence-generation model, if one comes, is specified
separately, and the parameter regressor does not compose music.
