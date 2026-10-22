---
format: aep.planning-md/3
id: story:phrase-fit
kind: story
status: draft
title: Phrase fit with estimated notes
relations:
- decomposes: epic:phrases
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:note-estimation
- depends_on: story:fit-run-artifacts
revision: 1
---
# Phrase fit with estimated notes

## Outcome

A short monophonic bass or lead phrase fits to one editable patch driven by an estimated note
sequence; the report carries the estimation's confidence, and a phrase fit whose estimated notes
carry no confidence is refused.

## DESIGN.md

§ 9.2, § 10 (V1, V2), § 11 (report confidence), § 13 Milestone C.

## Acceptance

- spec/scenarios/phrase-fit-estimated-notes-carry-confidence.yaml
- spec/scenarios/phrase-fit-confidence-missing.yaml

## Numeric obligations

- The bounded-search obligations hold for a phrase; the render budget per phrase is stated
  (§ 9.2, § 10).
- Confidence is reported in `report.txt` (§ 11).

## Open questions

- Must a Phrase fit use an Estimated note sequence, and may a SingleNote fit.
- Whether a phrase budget other than 2,000 renders per note is refused.
