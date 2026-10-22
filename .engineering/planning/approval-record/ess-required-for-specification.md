---
format: aep.planning-md/3
id: approval-record:ess-required-for-specification
kind: approval-record
status: draft
title: ESS is required for the specification, never at runtime
relations:
- decides: vision:patchback
- decides: executable-system-specification:patchback-contracts
revision: 1
---
# ESS is required for the specification

## Decision

ESS specifies Patchback's application contracts in `spec/` and `task check` validates them and runs
their conformance suite. ESS is never a runtime, ML or executable dependency of Patchback.
DESIGN.md ADR-008 and § 1 were changed to say so (revision 0.2).

## Authority

b10x-chef-orchestrator, 2026-10-22, under the grant in the workspace instructions: "Note the
design's 'ESS is not a required dependency' conflicts with the workspace rule 'ESS drives every
product repository': the workspace rule wins for the specification; record that as a decision in
the store."

## What it supersedes

DESIGN.md revision 0.1, ADR-008: ESS "optional".
