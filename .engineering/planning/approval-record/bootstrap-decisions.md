---
format: aep.planning-md/3
id: approval-record:bootstrap-decisions
kind: approval-record
status: draft
title: Repository bootstrap decisions D1-D5
relations:
- decides: vision:patchback
revision: 1
---
# Repository bootstrap decisions

Taken by the bootstrapping session on 2026-10-22 inside the item the orchestrator approved: "bootstrap
Patchback from design/patchback.md ... Set it up the way beyond10x/modelspec is set up ... Scope of
this item is the specification only, no implementation" (b10x-chef-orchestrator, 2026-10-22).

| ID | Decision | Reason |
| --- | --- | --- |
| D1 | `design/patchback.md` became `DESIGN.md` at the root, revision 0.2; the CLI and crates are `patchback`/`patchback-*` instead of `synthspec` | modelspec keeps its design at `DESIGN.md`; the operator named the product Patchback |
| D2 | Eight ESS domains: patch, score, audio, analysis, dataset, model, fit, dependency | one domain per design area (§ 5–§ 9, § 3); every noun in the design has one home |
| D3 | Numeric DSP properties (finite samples, golden waveforms, FFT accuracy, loss reduction, byte determinism) are stated in story bodies as numeric obligations, not in ESS | ESS scenarios observe commands and state, not sample values |
| D4 | The store pins the AEP protocol at the 0.74.0 tag commit, as modelspec does; the CI installs `aep` 0.74.0 | the installed CLI is 0.74.0; moving it is an upgrade outside this item |
| D5 | Drum and effect modules, arrangement events and the sequence-generation model stay out of the specification | DESIGN.md defines no contract for them (§ 13 E) |
