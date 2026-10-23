---
format: aep.planning-md/3
id: review-result:dependency-gate-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on the dependency gate
relations:
- reviews: story:dependency-gate
revision: 1
---
# Adversary pass 1 on story:dependency-gate

Reviewer: `aep:adversary` (Opus), wave a2, 2026-10-23. Subject: unit commit
`f69534cc4d13f4f16e0c15d19e7e6ed8f1914487` on `impl/dependency-gate`.

Verdict: CONFIRMED. Cases executed 76→84, red 5 (in the new
`crates/patchback-cli/tests/dependencies_adversary.rs`). Origin: introduced 7. Rows 1–3 are
unreachable today (`AUDIO_BACKENDS` is empty) and open with the first backend entry.

```findings
- file: crates/patchback-cli/tests/dependencies.rs
  line: 1588
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "approval_problems accepts any record that mentions the crate in backticks, including one that declines it"
- file: crates/patchback-cli/tests/dependencies.rs
  line: 1586
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "approval_problems ignores the record status, so an archived, rejected or superseded record still approves a backend"
- file: crates/patchback-cli/tests/dependencies.rs
  line: 405
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "always_enabled skips dev-dependency feature requests, so a dev-dependency can turn the backend on in every test build and the optional rule still passes"
- file: crates/patchback-cli/tests/dependencies.rs
  line: 485
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the modelspec prefix check is case-sensitive, so a ModelSpec crate is refused with the generic registry reason instead of the ModelSpec reason"
- file: crates/patchback-cli/tests/dependencies.rs
  line: 1397
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "a high surrogate followed by a non-low escape makes parse_json panic with subtract overflow instead of returning an error"
- file: AGENTS.md
  line: 64
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the unit edited AGENTS.md, which its brief lists as not its file and asks to propose in the report instead"
- file: spec/domains/dependency.yaml
  line: 9
  category: contract-drift
  severity: note
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "the domain records OQ-22 as settled and adds a ModelSpec ban while DESIGN.md:583 still lists OQ-22 open and section 3 is unchanged"
```

## Routing (coordinator)

- Rows 1–2: correction round. An approval counts only from an approval record whose status is not
  rejected, superseded or archived, and whose body states the approval of the crate in a fixed
  form the gate reads (the crate, its version); a prose mention does not count.
- Row 3: correction round. Feature requests from dev-dependencies count as always requested.
- Row 4: correction round. The ModelSpec name check is case-insensitive.
- Row 5: correction round. A lone or mismatched surrogate is a parse error, not a panic.
- Row 6: kept. The coordinator made the AGENTS.md edit from the implementor's proposal; the unit
  did not.
- Row 7: fixed by the coordinator in the unit: DESIGN.md revision 0.5 settles OQ-22 in § 3 (closure
  per entry crate, approval record, optional backend, no ModelSpec crate) and its changelog says so.
