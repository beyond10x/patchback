---
format: aep.planning-md/3
id: story:patch-file-format
kind: story
status: draft
title: Line-oriented .patch file format
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:parameter-contracts
- depends_on: story:workspace-scaffold
scope:
- confidence: inferred
  path: crates/patchback-core
revision: 2
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
