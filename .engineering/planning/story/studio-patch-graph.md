---
format: aep.planning-md/3
id: story:studio-patch-graph
kind: story
status: active
title: Patch graph view
relations:
- decomposes: epic:patch-studio
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:patch-graph-validation
- serves: vision:patchback
scope:
- confidence: inferred
  path: crates/patchback-studio
- confidence: cited
  path: spec/domains/studio.yaml
- confidence: cited
  path: studio/src/components/PatchGraph.vue
- confidence: cited
  path: studio/src/model.ts
- confidence: cited
  path: studio/src/views/GraphView.vue
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-01T15:25:04Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-01T15:25:04Z", actor: "human:timo", revision: 6}
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

## Scope

Derived 2026-11-01 by the wave c1 coordinator from the `story:studio-fit-replay` scoper's report and the tree; no scoper ran on this story alone. Every line is **cited** or **inferred**. Decisions Dc1-20 to Dc1-27 in `approval-record:wave-c1` apply.

- **Primary surface:** `studio/src/components/PatchGraph.vue`, `studio/src/views/GraphView.vue`, `studio/src/model.ts` — cited (the prototype draws `studio/fixtures/graph-subtractive-mono-1.json` and `graph-delayed-feedback.json`)
- **Shared with `story:studio-fit-replay`:** `EvolutionView.vue:22` draws `graphs[0]` through `PatchGraph.vue`, which `GraphView` and `ResultView` also use — cited (scoper of `story:studio-fit-replay`, point 5)
- **Rust, read:** the `.patch` reader and graph validation in `crates/patchback-core/src/patch/` (`validate.rs`), reached through `crates/patchback-studio` (Dc1-21) — inferred
- **Contract:** `patchback.studio.DrawGraph`, event `GraphDrawn` in `spec/domains/studio.yaml`; the four `spec/scenarios/studio-patch-graph-*.yaml` — cited
- **Confidence:** low to medium: no scoper read this story's surface on its own.
- **Would collide with:** `story:studio-fit-replay` (same unit, Dc1-20), `story:studio-end-result`, `story:studio-module-catalogue` — inferred
