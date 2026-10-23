---
format: aep.planning-md/3
id: review-result:cli-clap-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on the clap port
relations:
- reviews: story:cli-clap
revision: 1
---
# Adversary pass 1 on story:cli-clap

Reviewer: `aep:adversary` (Opus), wave a2, 2026-10-23. Subject: unit commit
`6d4fccd04742d6871e4ebce0e20aefd544409da5` on `impl/cli-clap`.

Verdict: NEEDS-CHANGE. Cases executed 58→62, red 4 (two in
`crates/patchback-cli/tests/dependencies.rs`, two in the new
`crates/patchback-cli/tests/adversary_a2.rs`). Origin: introduced 5, pre-existing 1.

```findings
- file: crates/patchback-cli/tests/dependencies.rs
  line: 155
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: The Cargo.lock gate identifies the reviewed clap closure by name and source only, so clap 5.0.0 or a second syn 1.0.109 entering the closure is admitted and the exactness test stays green.
- file: spec/scenarios/dependency-gate-no-crates-passes.yaml
  line: 1
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: The scenario quotes "built with zero third-party runtime crates", which DESIGN.md 0.4 no longer says.
- file: spec/domains/dependency.yaml
  line: 266
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: The ConcludeDependencyAudit summary still says a passed audit means nothing beyond one audio backend, ignoring the admitted clap, and the generated docs publish it.
- file: spec/scenarios/dependency-gate-forbidden-crate.yaml
  line: 3
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: The header says every category but audio I/O is refused, but CliParsing is now admitted.
- file: spec/scenarios/dependency-gate-cli-parser.yaml
  line: 1
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: 'The quote "CLI parsing: clap (derive), ..." is not verbatim DESIGN.md text, which is a table row.'
- file: spec/domains/dependency.yaml
  line: 191
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: pre-existing
  message: A non-clap CliParsing crate recorded into a passed audit answers ForbiddenCrate rather than AuditConcluded, because ESS 0.68.0 answers input-based refusals before state-based ones, as forbidden-crate already did at base.
```

## Routing (coordinator)

- Row 1: correction round. Coordinator decision on the adversary's question: the reviewed closure
  is name and version, because DESIGN.md 0.4 § 3 says "exactly the reviewed packages".
  `CLAP_CLOSURE` records `(name, version)` pairs as `Cargo.lock` holds them at the unit commit; a
  different version of any of them, a second version beside a reviewed one, or a clap version
  outside the list fails the gate until the list is reviewed. The adversary's two cases stay and
  must go green.
- Rows 2–5: correction round. The four texts are rewritten to match DESIGN.md 0.4 (clap and its
  reviewed closure admitted beside at most one audio backend); quotes are DESIGN.md text verbatim.
  `tests/adversary_a2.rs` stays and must go green.
- Row 6: kept. Pre-existing ordering of ESS 0.68.0 refusals; the forbidden-crate scenarios already
  rely on it, and no Patchback contract depends on the other order.
