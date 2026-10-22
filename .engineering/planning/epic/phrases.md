---
format: aep.planning-md/3
id: epic:phrases
kind: epic
status: draft
title: 'Milestone C: from notes to phrases'
relations:
- serves: vision:patchback
- depends_on: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
revision: 1
---
# Milestone C: from notes to phrases

## Outcome

An isolated monophonic bass or lead phrase goes in; an estimated note sequence, one editable patch
and an audible reconstruction come out, with timing and pitch uncertainty reported explicitly
(DESIGN.md § 10, § 13 Milestone C).

## In scope

- Onset, pitch, gate-length and tail estimation with confidence (§ 10).
- Phrase-level fit: one patch for the whole phrase (§ 13 C).
- Acceptance on at least one independently recorded, provenance-recorded note or phrase (§ 12.2 External audio, § 12.3).

## Out of scope

Polyphony, stems, topology choice.
