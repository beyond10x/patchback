---
format: aep.planning-md/3
id: review-result:patch-review-fixes-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on the patch review fixes
relations:
- reviews: story:patch-review-fixes
revision: 1
---
# Adversary pass 1 on story:patch-review-fixes

Reviewer: `aep:adversary` (Opus, effort high), wave a4, 2026-10-23. Subject: the implementor's
uncommitted working tree on `impl/patch-review-fixes`, base `d3b61dbb4c97d18e8c6ebf5261018dc0ef1f64c0`.

Verdict: CONFIRMED, no blocker. Cases executed 451→453, red 2 (in the new
`crates/patchback-core/tests/patch_review_fixes_adversary.rs`). Origin: introduced 4, pre-existing 1.

```findings
- file: crates/patchback-core/src/patch/validate.rs
  line: 749
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "A stray lower-numbered duplicate node makes FixedChainMismatch name the wired node as extra and report chain edges the file has as missing."
- file: crates/patchback-core/src/patch/file.rs
  line: 448
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: "A version-1 header with an extra field is answered alone as MalformedRecord although LoadPatch declares unsupported-rate first."
- file: spec/domains/patch.yaml
  line: 417
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "FixedChainMismatch contradicts delayed-feedback-accepted.yaml and delayed-feedback-refused.yaml patch 143; under the full chain no delayed feedback loop can validate."
- file: spec/domains/patch.yaml
  line: 428
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "extra_nodes is declared as a list of identifiers while the code reports (id, kind) pairs."
- file: crates/patchback-core/src/patch/validate.rs
  line: 850
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The node and voice cap branches are unreachable from a loaded file, and two tests no longer test what their names say."
```

Attacked without breaking: duplicate, missing and miswired chain kinds; edges beyond the chain;
precedence pairs in both orders; header shapes; CLI exit status and one line; four mutants of
`check_chain`; the new ESS scenarios.

## Routing (coordinator)

- Row 1: pick per kind the node the chain edges use (lowest id breaks ties); an edge is missing
  only when no nodes of those kinds carry it. The adversary's case stays.
- Row 2: fixed in this unit. A version-1 header with extra fields is recorded as MalformedRecord on
  line 1, later lines are still checked, and the highest-ranked fault wins. The adversary's case stays.
- Row 3: an `UNMAPPED:` note at FixedChainMismatch: story:delayed-feedback extends the chain
  definition for a Delay feedback path before its scenarios can hold. Recorded on that story.
- Row 4: the domain declares what the code carries.
- Row 5: the cap branches stay as the § 5 caps for patches built in code; a test reaches each one
  that can be reached from code, and the two tests are renamed to what they assert.
