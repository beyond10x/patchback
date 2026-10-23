---
format: aep.planning-md/3
id: approval-record:wave-a3
kind: approval-record
status: draft
title: Wave a3 approved by b10x-chef-orchestrator
relations:
- decides: story:patch-file-format
- decides: story:analysis-core
- decides: story:patch-graph-validation
revision: 1
---
# Wave a3 approved by b10x-chef-orchestrator

## Who approved, in their words

Message from session `b10x-chef-orchestrator`, 2026-10-23, under the grant in the operator's
workspace instructions (class C: a wave; commits, merges and a release through the bot). It quotes
the operator: "adopt both, and then trigger them to do some work" and "they are also meant to
release often". The message reads: "Next: take the next wave from patchback's store, the stories
that implement the CLI verbs (`patch validate` first), and run it. You don't need to propose it to
me first ... Release after each wave that changes behaviour."

## The wave

- Stage 1, in parallel: `story:patch-file-format` (`patchback-core`) and `story:analysis-core`
  (`patchback-analysis`, the `analyze` verb). No shared file except `CHANGELOG.md` and
  `crates/patchback-docs/src/status.rs`.
- Stage 2, after patch-file-format merges: `story:patch-graph-validation` (`patchback-core`, the
  `patch validate` verb).
- `story:note-events` shares `patchback-core` with both patch stories and waits for wave a4.

## Coordinator decisions on the stories' open questions

Taken by the coordinator, reversible, and not DESIGN.md changes:

- Where `spec/` already models an answer (bare parameter paths, the § 5.1 port table, one rate per
  edge, silence refused), the code follows the model and the `UNMAPPED:` marker stays.
- A bound DESIGN.md gives no value for (number grammar, line, file and graph sizes, input duration,
  FFT length) is a named constant with a documented provisional value, as wav-io's frame bound is.
  The values are listed in the wave page and stay open questions in DESIGN.md § 17.
- Release: 0.2.0 after the wave merges and `main` is green.
