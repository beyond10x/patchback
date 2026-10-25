---
format: aep.planning-md/3
id: review-result:docs-progress-and-demos-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: Progress and Listen pages (wave a12, unit 2)'
relations:
- reviews: story:docs-progress-and-demos
revision: 1
---
# Adversary pass 1: Progress and Listen pages (wave a12, unit 2)

Reviewer: `aep:adversary` (Opus), 2026-10-25, on the uncommitted working tree of
`impl/docs-progress-and-demos` at base 0e0f027. Report:
`a12/docs-demos/adversary-report-1.md` in the session's scratch (outside the repository).

## Verdict

Red: 2 red cases of 12 added (`crates/patchback-docs/tests/adversary_docs_demos.rs`), cases
executed 33 → 45. Both red cases build a state nothing is shown to reach (INFEASIBLE in reach),
and both are corrected in correction 1 (Da12-38). Gate after the cases: unit tests 33 passed;
the adversary file 10 passed, 2 failed; clippy exit 0; fmt exit 0; dependency gate 39 passed;
`generate --check` exit 0 in 6,563 ms on a loaded machine; two debug renders and a release
render byte-identical; release `demos fit` (72,901 ms) byte-identical to the committed
`website/data/demos.json`. No tracked file edited.

## What was attacked and held

- Drift: six one-byte input changes (patch, notes, weights payload and header, statistics, an
  appended newline), a story status move, a new release heading and a displayed loss digit are
  each refused by `generate --check`, `generate` and `render`, naming the file; nothing written.
- The release rule on a synthetic CHANGELOG: prefix and suffix slugs, a wrapped mention,
  `(stories a, b and c)`, `story:x` at a line end, a story named twice → the newest, only under
  Unreleased, same day → "(by date)", after every release → Unreleased, proposed, active and
  unknown statuses → "—", quoting and escaping, `epic:patch-studio` → the Patch studio section.
- The store reader: a missing epic, missing front matter, `epic:` or `story:` in a title refused
  by name; counts per section and status equal the store (41 stories); rows ordered by status
  then date; no planning id on either page.
- The Listen page against `demos.json`, `voice.patch`, `note.events` and
  `crates/patchback-core/src/parameter.rs`; `PROVENANCE` against the recorded run; the weights
  checksum and statistics digest recomputed.
- Audio: 16 takes PCM16, 48 kHz, mono, under 96,000 frames, peak below 32,767; two renders
  byte-identical; the 16 files equal the 16 `require()` paths; a stale take is replaced.
- Wiring: sidebars, Taskfile, pages.yml order, .gitignore; `Cargo.lock` changed in the
  `patchback-docs` stanza only; `float_roundtrip` reaches no other crate.

## Decisions for the coordinator

Da12-38 and Da12-39 in approval-record:wave-a12.

```findings
- file: crates/patchback-docs/src/demos.rs
  line: 557
  category: contract-drift
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: demos.json is bound to its input files only, so a recovered vector that no longer maps to its recorded physical values (or any later engine change) renders audio that disagrees with the page while render and generate --check exit 0
- file: crates/patchback-docs/src/progress.rs
  line: 249
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: the by-date fallback compares CHANGELOG dates as strings, so a non-padded date like 2026-9-5 counts as after 2026-10-25
- file: crates/patchback-docs/src/demos.rs
  line: 1205
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: committed_weights_match_their_provenance only parses the dataset seed, split sizes, steps and batch, so a wrong value in PROVENANCE stays green
- file: crates/patchback-docs/src/main.rs
  line: 110
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: demos fit resolves its default --out against the working directory rather than --root
```
