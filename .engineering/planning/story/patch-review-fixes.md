---
format: aep.planning-md/3
id: story:patch-review-fixes
kind: story
status: implemented
title: 'Patch review fixes: header refusal order and the fixed chain'
relations:
- decomposes: epic:sound-engine
- depends_on: story:patch-graph-validation
- serves: vision:patchback
scope:
- confidence: cited
  path: crates/patchback-core/src/patch/file.rs
- confidence: cited
  path: crates/patchback-core/src/patch/validate.rs
- confidence: cited
  path: spec/domains/patch.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T18:30:09Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-23T18:30:09Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-23T21:47:30Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Patch review fixes: header refusal order and the fixed chain

## Outcome

Two defects a whole-codebase review found at e6f67c7 (wave a3 close) are fixed:

- A `.patch` header line with an extra field and an unknown version is refused as
  `UnknownFormatVersion`, the outcome `LoadPatch` declares before `MalformedRecord`, not as
  `MalformedRecord` (`crates/patchback-core/src/patch/file.rs`).
- A patch naming topology `subtractive-mono/1` that lacks its fixed chain (no nodes, or the voice
  without its `Output` node; DESIGN.md § 5.2) is refused by validation with a new error of the
  patch domain, and `patchback patch validate` exits 1 on it. Today both validate and compile.

## DESIGN.md

§ 5.1 (validation order), § 5.2 (the fixed subtractive voice), § 11.

## Acceptance

- spec/scenarios/patch-file-format-header.yaml (extended with the extra-field, unknown-version case)
- spec/scenarios/patch-graph-validation-fixed-chain.yaml (new)

## Numeric obligations

None.

## Scope

- `crates/patchback-core/src/patch/file.rs`, `crates/patchback-core/src/patch/validate.rs`,
  `crates/patchback-core/tests/`, `crates/patchback-cli/tests/`, `spec/domains/patch.yaml`,
  `spec/scenarios/` (inferred from the review's findings).

## Open questions

- Where the fixed-chain check sits in `VALIDATE_PATCH_PRECEDENCE`. Coordinator's call: after the
  port checks and before cycles, since it reads the node kinds the identifier and port checks have
  admitted. The unit declares it in `spec/domains/patch.yaml` and names it in DESIGN.md § 5.1's
  validation order in the same change (revision 0.7).
