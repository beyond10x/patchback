---
format: aep.planning-md/3
id: story:multi-stem
kind: story
status: draft
title: Multiple instrument stems
relations:
- decomposes: epic:larger-scope
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:subtractive-voice-render
revision: 2
---
# Multiple instrument stems

## Outcome

An arrangement holds several instrument stems, each a patch with its note sequence; an arrangement
that does not satisfy the stem rules is refused. Rendering and mixing follow once the mixing rule
is settled.

## DESIGN.md

§ 10 (V2, V3), § 13 Milestone E.

## Acceptance

- spec/scenarios/multi-stem-two-stems.yaml
- spec/scenarios/multi-stem-refused.yaml

## Numeric obligations

- The mix of stems is finite and deterministic (§ 2 invariants 4–5); the mixing rule is stated
  when settled.

## Open questions

- How stems are mixed (gain, pan, sum, limiting); equal lengths; a stem-count cap.
- Polyphonic stems; two stems sharing one patch.
- Arrangement events and repeated-pattern analysis have no shape.
