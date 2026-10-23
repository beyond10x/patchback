---
format: aep.planning-md/3
id: review-result:workspace-scaffold-adversary-2
kind: review-result
status: active
title: Adversary pass 2 on the workspace scaffold
relations:
- reviews: story:workspace-scaffold
revision: 1
---
# Adversary pass 2 on story:workspace-scaffold

Reviewer: `aep:adversary` (Opus), wave a1, 2026-10-23, the last of the two passes. Subject: the
correction round, `bb49c1c..698e196` on `impl/workspace-scaffold`.

Verdict: NEEDS-CHANGE. Cases executed 44→45, red 1 (`crates/patchback-cli/tests/adversary2.rs`).
Origin: introduced 1. The lock walk and the help handling withstood the attack (lock entries with
version and source suffixes, duplicate names, `[patch]` and `[replace]`, four mutants, offline
fallback, help placement).

```findings
- file: crates/patchback-cli/tests/adversary2.rs
  line: 83
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: task check, the only thing CI runs, never runs cargo test for patchback-cli, so the dependency gate and the help tests never run in CI.
- file: crates/patchback-cli/src/main.rs
  line: 51
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: a help flag after `--` is treated as a request for usage rather than as an operand.
- file: crates/patchback-cli/tests/dependencies.rs
  line: 144
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: a source-less patchback-* path crate outside crates/ that is not an application crate is admitted, and no test refuses extra workspace members.
- file: crates/patchback-cli/tests/dependencies.rs
  line: 471
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the scratch workspace under CARGO_TARGET_TMPDIR is only removed on the success path, so a panic before it leaves the directory behind.
```

## Routing (coordinator)

- Row 1: fixed by the coordinator in `24e05c2`. `Taskfile.yml` is outside the implementor's
  files: task `rust-test` (`cargo test --locked --workspace`) is called from `task check`;
  `AGENTS.md` lists it. The adversary's case is kept and is green.
- Rows 2 and 3: INFEASIBLE, no change. No command takes operands yet, and every member lives
  under `crates/`; a later story that adds operands or a member outside `crates/` owns them.
- Row 4: no change. A leftover directory appears only after a failing cargo run, inside `target/`.
