---
format: aep.planning-md/3
id: story:control-automation
kind: story
status: draft
title: Time-varying control automation
relations:
- decomposes: epic:module-choice
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:topology-families
- depends_on: story:note-events
revision: 1
---
# Time-varying control automation

## Outcome

A note sequence carries control events that set a parameter, addressed by its stable path, to a
normalized value at a sample; the renderer interpolates them explicitly. A control event naming an
unknown parameter path is refused.

## DESIGN.md

§ 5.1 (stable identifiers), § 5.4 (explicit interpolation), § 10 (ControlEvent), § 13 Milestone D.

## Acceptance

- spec/scenarios/control-automation-automated.yaml
- spec/scenarios/control-automation-unknown-path.yaml

## Numeric obligations

- Control events apply at their exact sample; interpolation is the stated rule, smoothed to avoid
  clicks (§ 5.3, § 5.4, § 10).
- Normalized values are clamped to [0, 1] per the parameter's contract (§ 5.2).

## Open questions

- Parameter path syntax.
- Whether controls share `notes.events` or have their own file.
