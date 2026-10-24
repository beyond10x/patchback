---
format: aep.planning-md/3
id: review-result:bounded-search-fit-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: bounded derivative-free fit'
relations:
- reviews: story:bounded-search-fit
revision: 1
---
# Adversary pass 2: Bounded derivative-free fit (story:bounded-search-fit)

Wave a6, unit 1. `aep:adversary` on Opus against the corrected uncommitted change in tree
`patchback-impl-bounded-search-fit` (base 76bb7ec), 2026-10-25.

## Result

- verdict: red; cases executed 219 → 225, red 2; origin: introduced 4 (N1, N2, F3 and F4
  carried), pre-existing 0.
- Correction round 1 verified without loss on F1, F2, F5, F6: the first render is still
  unconditional (a 1 ms bound gives one log entry and `restarts 1`); `restart_point` matches
  `patchback_core::prng` against a published SplitMix64 vector and its doc states the
  derivation; the `<out>` refusal comes after `--input` and `--notes` and before the silence
  check, as `fit.rs` says; the two-fault cases pass; the defaults are unchanged. The
  adversary's grid case now checks 968 points from `restart_point`.
- Mutants on a scratch copy: `step < MIN_STEP` → `<=` still caught by the 65-evaluation case
  (49 evaluations under the mutant); the clock moved after the reference check is caught only
  by N1's case.
- N1 (warning, NEEDS-CHANGE): with the clock started before the reference analysis, a 1 ms
  bound records `wall_time_ms` 566 to 795 ms against a longest evaluation of 399 to 489 ms, so
  the documented "may exceed the bound by at most one evaluation" is false by about one
  reference analysis (157 to 169 ms). Red 6 of 6 runs on the tree; green 3 of 3 on a copy with
  the clock after the reference check.
- N2 (note): an `--out` that is a dangling symlink passes the pre-search check (`fs::metadata`
  follows the link), so the search runs (3.68 s for 20 renders) before `create_dir_all` refuses
  it with `File exists`.
- Not checked: the first implementor's test files before correction 1 (untracked, no copy);
  compared by count, name and file time only.

## Routed

- N1 → Da6-29: the clock stays at `fit` entry; the overshoot sentence becomes "the reference
  analysis plus one evaluation" in `search.rs`, `fit.rs`, the CLI reference and the changelog;
  the case is re-pinned to it (correction round 2).
- N2 → Da6-30: `--out` checked with `fs::symlink_metadata`; a symlink that does not resolve to
  a directory is refused before the search (correction round 2).
- F3, F4: carried, no change.

Report: wave a6 scratch `bounded-search-fit/adversary-report-2.md`. Agent cost 160,852 tokens,
43 tool uses, 900 s.

```findings
[
{"file":"crates/patchback-fit/tests/search_adversary.rs","line":371,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"with the clock now started before the reference analysis, a 1 ms bound records wall_time_ms 566-795 against a longest evaluation of 399-489 ms, so the documented \"may exceed the bound by at most one evaluation\" (search.rs Stopping, fit.rs wall_time_ms row, cli-doc.md) is false by about one reference analysis (157-169 ms)"},
{"file":"crates/patchback-cli/tests/fit_adversary.rs","line":360,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"an --out that is a dangling symlink passes the pre-search check because fs::metadata follows the link, so the whole search runs (3.68 s for 20 renders) before create_dir_all refuses it with File exists"},
{"file":"crates/patchback-fit/src/search.rs","line":460,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"carried F3, the correction made Da6-23 the documented rule without a code change, so with max_restarts 1 a natural end is always RestartsExhausted, pinned by the 65-evaluation case"},
{"file":"crates/patchback-fit/tests/search.rs","line":376,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"carried F4, the correction deleted a_rejected_candidate_names_its_cause with no replacement, so no case checks that a rejection in the log names its cause"}
]
```
