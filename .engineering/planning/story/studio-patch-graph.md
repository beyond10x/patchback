---
format: aep.planning-md/3
id: story:studio-patch-graph
kind: story
status: draft
title: Patch graph view
relations:
- decomposes: epic:patch-studio
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:patch-graph-validation
revision: 1
---
# Patch graph view

## Outcome

The studio draws a validated patch as nodes placed by rank and edges coloured by port rate, with
any feedback shown as a delayed return through a Delay node, so the `subtractive-mono/1` voice and a
Milestone D feedback patch both read correctly. Source: `patchback.studio.DrawGraph`, event
`GraphDrawn`. An edge marked `delayed_feedback` must leave a Delay node; every endpoint must be a
node of the graph; a patch that has not been validated is not drawn.

## DESIGN.md

§ 5.1, § 5.2, § 5.4.

## Acceptance

- spec/scenarios/studio-patch-graph-subtractive-voice.yaml
- spec/scenarios/studio-patch-graph-delayed-feedback.yaml
- spec/scenarios/studio-patch-graph-feedback-not-at-delay.yaml
- spec/scenarios/studio-patch-graph-endpoint-missing.yaml

## Numeric obligations

None.

## Open questions

- Port lists per node kind (shared with patch-graph-validation).
