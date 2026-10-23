---
format: aep.planning-md/3
id: review-result:workspace-scaffold-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on the workspace scaffold
relations:
- reviews: story:workspace-scaffold
revision: 1
---
# Adversary pass 1 on story:workspace-scaffold

Reviewer: `aep:adversary` (Opus), wave a1, 2026-10-23. Subject: unit commit
`bb49c1c4424af18125ee6890b5d909a078da1235` on `impl/workspace-scaffold`.

Verdict: CONFIRMED. Cases executed 28→33, red 5 (all in `crates/patchback-cli/tests/dependencies.rs`).
Origin: introduced 7, pre-existing 0.

```findings
- file: crates/patchback-cli/tests/dependencies.rs
  line: 101
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: a dependencies key under a bare [target.<cfg>] header or a top-level dotted dependencies key is never read, so a registry crate passes the gate.
- file: crates/patchback-cli/tests/dependencies.rs
  line: 161
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: a package rename in [workspace.dependencies] is not checked, so patchback-fft.workspace = true pointing at package rustfft is admitted.
- file: crates/patchback-cli/tests/dependencies.rs
  line: 108
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: trailing comments are not stripped, so a registry crate whose comment contains path = passes.
- file: crates/patchback-cli/tests/dependencies.rs
  line: 157
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the check is not transitive and uses a fixed crate list, so depending on patchback-docs (clap, serde_json) or an unlisted crates/ crate passes.
- file: crates/patchback-docs/src/main.rs
  line: 44
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: patchback-docs main.rs, reference.rs and status.rs were reformatted although the brief lists them as not the unit's, because the base was not rustfmt-clean and the workspace fmt gate demanded it.
- file: crates/patchback-cli/src/main.rs
  line: 57
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: patch --help and patch -h are reported as an unknown command, and render --help exits 3 instead of printing usage.
```

## Routing (coordinator)

- Rows 1–4 (dependency checker): correction round. The implementor replaces the manifest-text
  parser with a std-only walk of `Cargo.lock` from the eight application crates, refusing any
  package with a `source =` line or a name outside `patchback-*`; the adversary's five red cases
  stay and must go green.
- Row 5 (patchback-docs reformat): kept. The change is formatting only, the base `69e87fd` already
  failed `cargo fmt --check`, and the wave gate runs it on the workspace.
- Row 6 (sub-command help): `--help`/`-h` after any recognised command prints the usage to stdout
  and exits 0, with a test per command. Coordinator decision; no specification settles it.
