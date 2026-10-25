---
format: aep.planning-md/3
id: review-result:docs-progress-and-demos-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: Progress and Listen pages (wave a12, unit 2)'
relations:
- reviews: story:docs-progress-and-demos
revision: 1
---
# Adversary pass 2: Progress and Listen pages (wave a12, unit 2)

Reviewer: `aep:adversary` (Opus), 2026-10-25, on the uncommitted working tree of
`impl/docs-progress-and-demos` at base 0e0f027 after correction 1. Report:
`a12/docs-demos/adversary-report-2.md` in the session's scratch (outside the repository).

## Verdict

Red: 3 red cases of 7 added (`crates/patchback-docs/tests/adversary_docs_demos_2.rs`), cases
executed 48 → 55. All three build a state nothing is shown to reach (INFEASIBLE in reach,
severity note) and are corrected in correction 2 (Da12-41). Gate after the cases: unit tests
36 passed; pass-1 file 12 passed; pass-2 file 4 passed, 3 failed; clippy exit 0; fmt exit 0
after rustfmt on its own file; dependency gate 39 passed; `generate --check` exit 0 in
1,248 ms; two renders byte-identical; one release `demos fit` under `--root` byte-identical to
the committed `website/data/demos.json` (16,268 bytes). No tracked file edited.

## What was attacked and held

- The take digests are the SHA-256 of each rendered file's PCM `data` chunk, walked
  independently; exactly 4 lower-case hex keys per voice; `listen.mdx` lists the same 16.
- Five ways of changing a digest (one hex digit, 63 characters, non-hex, missing, a number):
  `render`, `generate` and `--check` each refuse naming that take only, saying `task
  docs-demos`; no audio directory, no generated file changes.
- Dates: non-padded months and days compare as numbers; month 13 and a letter in the date are
  refused by name; two same-day releases resolve to the lower one; a story implemented before
  every release and one after every release.
- `render --out` resolves against `--root` through the binary, from another working directory,
  creating missing directories; an absolute `--out` stays absolute. The release `demos fit` did
  the same for its `--out`.
- The provenance test asserts every recorded literal.
- Fit numbers: the pre-correction `demos.json` equals the implementor's first run; the current
  file adds 24 digest lines and removes none; a fresh fit equals it.
- Pass 1's 12 cases still green; `listen.mdx` differs from before correction 1 only by the
  appended digest table.

## Decisions for the coordinator

Da12-41 in approval-record:wave-a12.

```findings
- file: crates/patchback-docs/src/demos.rs
  line: 594
  category: contract-drift
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: render_takes binds the takes to the vectors but not to the physical values the listen page prints beside them, so an edited fit's physical value passes generate, --check and render
- file: crates/patchback-docs/src/progress.rs
  line: 236
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: a versioned CHANGELOG heading with an empty date (which release-check accepts) is shown as Unreleased instead of refused by name
- file: crates/patchback-docs/src/progress.rs
  line: 48
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: read_day accepts a two-digit year, so 26-10-30 reads as the year 26 and that release is never matched by date
- file: crates/patchback-docs/src/main.rs
  line: 718
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the relative --out unit test calls under_root itself, so a dispatch in main that drops under_root stays green without a binary-level case
```
