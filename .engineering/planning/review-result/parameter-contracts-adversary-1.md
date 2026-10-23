---
format: aep.planning-md/3
id: review-result:parameter-contracts-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on the parameter contracts
relations:
- reviews: story:parameter-contracts
revision: 1
---
# Adversary pass 1 on story:parameter-contracts

Reviewer: `aep:adversary` (Opus), wave a2, 2026-10-23. Subject: unit commit
`7e61c1cdde21b8515faeea3f26d6dc6370fb9d3c` on `impl/parameter-contracts`.

Verdict: NEEDS-CHANGE. Cases executed 72→78, red 2 (in the new
`crates/patchback-core/tests/parameter_contracts_adversary.rs`). Origin: introduced 6, pre-existing 2.

```findings
- file: crates/patchback-core/src/parameter.rs
  line: 199
  category: boundary
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "validate accepts a finite linear range whose width overflows (-1e308..1e308), so apply(0.0) yields NaN, against invariant 4; refuse when high - low is not finite"
- file: crates/patchback-core/src/parameter.rs
  line: 205
  category: boundary
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "validate accepts an exponential range whose ratio overflows (low 5e-324, high 1), so apply(0.25) yields inf and the inverse collapses to 0; refuse when high/low is not finite"
- file: spec/domains/patch.yaml
  line: 801
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: pre-existing
  message: "SetParameter declares clamped before unknown-path, so the model answers clamped for an out-of-range value on an unknown path while patchback-core refuses it with UnknownParameterPath (ESS-AUTHOR-041)"
- file: spec/scenarios/parameter-contracts-non-finite.yaml
  line: 39
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: "the acceptance scenario sends normalized_value 0.5 and expects NonFiniteParameterValue, which set(\"attack\", 0.5) returns Ok for, so it fails against any implementation target"
- file: crates/patchback-core/src/parameter.rs
  line: 93
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the domain's DeclareTopology errors carry the topology name while the Rust TopologyError variants carry path, index or reason and no topology name"
- file: crates/patchback-core/tests/parameter_contracts.rs
  line: 392
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "no existing case presents two conflicts at once, so reordering the path, index and validate checks in Topology::new stays green; the adversary ordering case holds it"
- file: crates/patchback-core/src/parameter.rs
  line: 305
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Topology::new accepts an index outside 0..n although section 5.2 defines index as a position in the normalized target vector"
- file: crates/patchback-docs/src/status.rs
  line: 582
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the page-count test derives its expected counts from the same count() the page uses, and the shipped-count check is a constant pin; generation's check_evidence still guards the docs gate"
```

## Routing (coordinator)

- Rows 1–2: correction round. `validate` refuses a contract whose width (linear) or ratio
  (exponential) is not finite; the two red cases go green.
- Row 3: correction round. The patch domain's SetParameter answers `unknown-path` before `clamped`,
  matching the code; a scenario step holds it.
- Row 4: correction round. The step keeps 0.5 only if ESS 0.68.0 cannot carry NaN (it cannot write a
  non-finite Decimal); it then carries a `Gap (ESS 0.68.0):` comment naming the stand-in value and
  the Rust test that holds the refusal.
- Row 5: correction round. The topology errors carry the topology name, as the domain's do.
- Row 6: kept. The adversary's ordering case closes it.
- Row 7: correction round, coordinator decision. Indices are exactly 0..n-1 (§ 5.2: position in the
  normalized target vector); any other set is an invalid contract, in code and in the domain.
- Row 8: kept for this unit. The three parallel units rewrote the same status tests; the merge
  keeps one form, with check_evidence as the guard.
