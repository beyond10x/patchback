---
title: Patches
sidebar_position: 2
description: A patch is a typed graph of DSP modules, validated and compiled before it runs. The first instrument is a fixed subtractive voice with eight parameters.
lede: "A patch is a typed graph of DSP modules, validated and compiled before it renders, and the first instrument is a fixed subtractive voice with eight parameters."
source: "DESIGN.md § 5"
---

# Patches

A patch is the instrument: a graph of DSP modules, the connections between their ports and the
values of their parameters. It holds no mutable sample state: each voice owns its phase,
filter and envelope state, so the same patch renders the same result every time.

```rust
struct Patch {
    format_version: u32,
    sample_rate_hz: u32,
    topology_id: String,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

struct Node {
    id: u32,
    kind: NodeKind,
    parameters: Vec<ParameterValue>,
}

struct Edge {
    from_node: u32,
    from_port: PortName,
    from_rate: PortRate,
    to_node: u32,
    to_port: PortName,
    to_rate: PortRate,
}

enum PortRate { Audio, Control, Event }
```

Every module declares its input and output ports, each with a rate (audio, control or event)
and a meaning. An edge connects two ports of compatible types. Parameters are addressed by
stable module and parameter identifiers, never by their position in a user interface.

## From file to execution plan

A patch is checked completely before a single sample is rendered. Compilation runs, in order:

1. identifier checks, with the agreement of the two rates an edge states;
2. parameter validation against each parameter's domain;
3. port checks against each module's port table;
4. the fixed chain: one module of each kind and every edge of the voice;
5. cycle checks;
6. resource caps;
7. one edge per input port;
8. scheduling;
9. buffer assignment;
10. the DSP execution plan.

Where a patch has several faults, the refusal is the one this order reaches first.

The renderer runs the plan. It never follows graph references inside the audio loop.
`patchback patch validate` runs the checks on their own.

## The fixed subtractive voice

The first instrument is one monophonic voice with a fixed signal chain:

```text
MIDI note/gate ---> Saw Oscillator ---> Drive ---> Lowpass Filter ---> VCA ---> Output
        |                                                    ^
        +------------------> ADSR Envelope ------------------+
```

The oscillator follows the note events; the envelope opens the VCA on note-on and releases it
on note-off. Because the topology is fixed, fitting this voice means finding eight continuous
parameters: attack, decay, sustain, release, filter cutoff, resonance, drive and output gain.
The [parameter reference](../reference/parameters.md) gives each one's range and mapping.

| Module | Behaviour |
| --- | --- |
| Saw oscillator | Phase accumulator with an anti-aliasing method implemented in Patchback, such as polyBLEP; tested at its frequency limits and for high-note aliasing |
| Drive | A deterministic, bounded nonlinearity whose output is always finite |
| Lowpass filter | A stable topology-preserving or state-variable structure; cutoff is clamped below Nyquist and resonance is bounded |
| ADSR envelope | Explicit note-on, retrigger and note-off; sample-accurate event offsets; zero and near-zero times handled safely |
| VCA and output gain | Parameter smoothing against clicks; protective clipping is reported, never used silently to hide instability |

The engine runs at 48,000 Hz and processes deterministic blocks, 256 frames by default. A
patch at another sample rate is refused, not converted.

## Beyond the fixed voice

Later instruments add modules one family at a time: square and sine oscillators, highpass and
bandpass filters, an LFO, delay, noise and FM. Ports then distinguish audio-rate from
control-rate signals, and control interpolation is explicit. Patchback chooses among a bounded
set of candidate topologies by rendering and ranking them, with a preference for the simpler
patch, before it ever generates a graph freely.

Feedback is allowed only through an explicit, stateful `Delay` node with at least one sample of
latency. A cycle without one is refused; an instantaneous cycle is never evaluated. Feedback
gain is limited, runaway energy is monitored and an unstable patch fails closed.
