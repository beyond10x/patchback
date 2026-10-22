---
title: Parameters
sidebar_position: 3
description: The eight parameters of the fixed subtractive voice, their normalized and physical ranges, and the mapping between them.
source: "DESIGN.md § 5.2; hand-written"
---

# Parameters

The [fixed subtractive voice](../concepts/patch.md#the-fixed-subtractive-voice) has eight
continuous parameters. The regressor and the search work with a normalized value `p` in
`[0, 1]`; the patch file stores the physical value.

| Index | Parameter | Physical value | Range |
| --- | --- | --- | --- |
| 0 | Attack | `0.001 * 2000^p` seconds | 1 ms to 2 s |
| 1 | Decay | `0.005 * 400^p` seconds | 5 ms to 2 s |
| 2 | Sustain | `p`, linear amplitude | 0 to 1 |
| 3 | Release | `0.005 * 400^p` seconds | 5 ms to 2 s |
| 4 | Filter cutoff | `40 * 400^p` Hz | 40 Hz to 16 kHz |
| 5 | Resonance Q | `0.5 + 11.5 * p` | 0.5 to 12 |
| 6 | Drive | `24 * p` dB, before the filter | 0 to 24 dB |
| 7 | Output gain | `-24 + 24 * p` dB | -24 to 0 dB |

## Rules

- `p` is clamped to `[0, 1]`; NaN and infinity are refused.
- Every mapping has a tested inverse, from physical value back to `p`.
- Patch files hold physical units; model inputs and outputs hold normalized units.
- A regressor output outside `[0, 1]` is clamped with a warning.
- Note pitch is not a parameter. It comes from the [note events](../concepts/events.md).

The index is the column of the parameter in the dataset's `y.f32` and in the regressor's
output.
