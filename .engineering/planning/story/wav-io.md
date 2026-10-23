---
format: aep.planning-md/3
id: story:wav-io
kind: story
status: active
title: std-only WAV import and export
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: inferred
  path: crates/patchback-audio
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T09:26:43Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T09:26:43Z", actor: "human:timo", revision: 4}
---
# std-only WAV import and export

## Outcome

Patchback reads a mono or stereo RIFF/WAVE file with PCM16 or IEEE float32 samples at 48 kHz into
an audio clip, downmixing stereo to mono and recording that it did, and writes a clip back out as a
WAV file. Unsupported headers, corrupt or truncated chunks, non-finite float samples and files past
the frame bound are refused with a typed reason and import nothing. No crate is used.

## DESIGN.md

§ 3 (std-only WAV), § 5.3 (48 kHz, no silent conversion), § 6.1, § 11, § 12.1, § 12.3, ADR-007.

## Acceptance

- spec/scenarios/wav-io-import-mono.yaml
- spec/scenarios/wav-io-stereo-downmix.yaml
- spec/scenarios/wav-io-unsupported-header.yaml
- spec/scenarios/wav-io-corrupt-input.yaml
- spec/scenarios/wav-io-length-bounds.yaml
- spec/scenarios/wav-io-export.yaml

## Numeric obligations

Held by Rust tests, not by the scenarios:

- PCM16 and float32 roundtrip is byte-exact on export and re-import (§ 12.1).
- Stereo downmix follows one documented formula, applied per frame (§ 6.1).
- RIFF and chunk sizes are checked before allocation; a size that overflows or exceeds the
  allocation bound is refused, never allocated (§ 6.1, § 12.1).
- A NaN or Infinity float sample is refused (§ 2 invariant 4, § 12.1).

## Open questions

- The frame/duration bound has no value (§ 6.1, § 12.3); `too-long` stays an external refusal.
- "Downmixed explicitly": operator opt-in or always-recorded downmix (modelled as recorded).
- WAVE_FORMAT_EXTENSIBLE wrapping PCM16/float32 is refused as unsupported; not stated in DESIGN.md.
- PCM16 export of samples outside [-1, 1]: refuse, or clip and report.
