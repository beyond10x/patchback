---
format: aep.planning-md/3
id: story:note-estimation
kind: story
status: draft
title: Monophonic note estimation
relations:
- decomposes: epic:phrases
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:analysis-core
- depends_on: story:note-events
revision: 1
---
# Monophonic note estimation

## Outcome

For external monophonic audio, Patchback estimates note onsets, pitches, gate lengths and tails
with confidence scores, refuses low-confidence and polyphonic input with a diagnostic, and writes
the estimate as a note sequence that records the estimation it came from; a sequence naming an
unknown estimation is refused.

## DESIGN.md

§ 6.1, § 6.2, § 10 (V1), § 13 Milestone C.

## Acceptance

- spec/scenarios/note-estimation-estimated.yaml
- spec/scenarios/note-estimation-low-confidence.yaml
- spec/scenarios/note-estimation-polyphonic.yaml
- spec/scenarios/note-events-from-estimation.yaml

## Numeric obligations

- Onset, pitch, gate and tail accuracy against synthesized references with known events, within
  bounds stated when the story is proposed (§ 6.2, § 10).
- Confidence is calibrated against those references (§ 10).

## Open questions

- Confidence per note or per estimated quantity (modelled per quantity).
- Velocity is not estimated.
- Polyphonic input refused outright (modelled) or estimated as far as possible.
- No note confidence threshold.
- Must a sequence written from an estimation equal its notes.
