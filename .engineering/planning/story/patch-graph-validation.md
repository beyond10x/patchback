---
format: aep.planning-md/3
id: story:patch-graph-validation
kind: story
status: active
title: Patch graph validation and compilation
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:parameter-contracts
- depends_on: story:workspace-scaffold
- depends_on: story:patch-file-format
- serves: vision:patchback
scope:
- confidence: inferred
  path: crates/patchback-core
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T12:38:04Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-23T12:38:04Z", actor: "human:timo", revision: 5}
---
# Patch graph validation and compilation

## Outcome

A loaded patch is validated before it is compiled or rendered: identifiers are unique, every edge
endpoint is a declared node and port, connected ports share a rate, parameters satisfy their
contracts, zero-delay cycles are refused and resource caps hold. Only a validated patch compiles
to an execution plan; compiling an unvalidated patch is refused.

## DESIGN.md

§ 2 invariant 3, § 5.1 (compilation order), § 5.4, § 12.1 (graph tests), § 12.3.

## Acceptance

- spec/scenarios/patch-graph-validation-accepted.yaml
- spec/scenarios/patch-graph-validation-compile-unvalidated.yaml
- spec/scenarios/patch-graph-validation-identifiers.yaml
- spec/scenarios/patch-graph-validation-rates.yaml
- spec/scenarios/patch-graph-validation-cycle-and-caps.yaml

## Numeric obligations

- Topological sort gives a deterministic execution order; the same patch compiles to the same plan
  (§ 5.1, § 2 invariant 5).
- Cycle detection refuses every cycle; the Delay exception is story:delayed-feedback (§ 5.4).

## Open questions

- No port list per node kind in § 5.1; ports are placeholders.
- Which rates may connect: this draft admits an edge only between ports of one rate (§ 5.4).
- Resource caps on graph size and voice count have no values (§ 12.1, § 12.3).
- A parameter change returns the patch to Loaded; whether a compiled plan takes values in place is
  unsettled.
- Node/Edge as values nested in Patch versus separate entities.
