---
format: aep.planning-md/3
id: story:patch-file-format
kind: story
status: implemented
title: Line-oriented .patch file format
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:parameter-contracts
- depends_on: story:workspace-scaffold
- serves: vision:patchback
scope:
- confidence: inferred
  path: crates/patchback-core
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T12:38:04Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T12:38:04Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-23T18:04:22Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Line-oriented .patch file format

## Outcome

`patchback patch validate` and every command that reads a patch parse a versioned, line-oriented
`.patch` file into a loaded patch, and the writer emits the canonical form in physical units. The
parser refuses an unknown format version, duplicate keys or IDs, malformed numbers, unsupported
rates and unbounded lengths, and loads nothing.

## DESIGN.md

§ 5.1, § 5.2 (physical units on disk), § 11, § 12.3.

## Acceptance

- spec/scenarios/patch-file-format-header.yaml
- spec/scenarios/patch-file-format-keys.yaml
- spec/scenarios/patch-file-format-parser.yaml

## Numeric obligations

- Write then parse is identity on the patch, parameter values included (§ 11).
- Numbers parse only as finite decimals; any other token is refused (§ 11, § 2 invariant 4).

## Open questions

- The number grammar and the length bounds (line length, node count, file size) have no values
  (§ 11, § 12.3).
- Parameter path syntax (shared with parameter-contracts).
