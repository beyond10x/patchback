---
format: aep.planning-md/3
id: story:external-audio-acceptance
kind: story
status: draft
title: External recording acceptance
relations:
- decomposes: epic:phrases
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:external-audio-provenance
- depends_on: story:note-estimation
- depends_on: story:fit-run-artifacts
revision: 3
---
# External recording acceptance

## Outcome

At least one independently recorded, isolated monophonic synth note with recorded provenance
produces a valid editable patch, a reconstructed WAV and a diagnostic report. Provenance refusal is
story:external-audio-provenance.

## DESIGN.md

§ 6.1, § 12.2 (external audio gate), § 12.3, § 13 Milestone C.

## Acceptance

- spec/scenarios/external-audio-acceptance-recording-with-provenance.yaml

## Numeric obligations

- The produced patch validates and parses back; the reconstructed WAV is finite (§ 12.2).

## Open questions

- Whether provenance is a `when_related` guard on the clip rather than an external refusal.
- Whether provenance applies to every external import or only fixtures.
- The depends_on story:note-estimation exists for the estimated-notes path (§ 6.1), but the
  passing scenario uses `note_source: Known`. A scenario with `note_source: Estimated` is still to
  be authored.
- The import refusal for a clip without provenance is decided by story:external-audio-provenance.
