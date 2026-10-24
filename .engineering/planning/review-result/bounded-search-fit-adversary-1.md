---
format: aep.planning-md/3
id: review-result:bounded-search-fit-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: bounded derivative-free fit'
relations:
- reviews: story:bounded-search-fit
revision: 1
---
# Adversary pass 1: Bounded derivative-free fit (story:bounded-search-fit)

Wave a6, unit 1. `aep:adversary` on Opus against the implementor's uncommitted change in tree
`patchback-impl-bounded-search-fit` (base 76bb7ec), 2026-10-24.

## Result

- verdict: red; cases executed 209 → 218 (patchback-fit 49 → 55, patchback-cli 160 → 163),
  red 3; origin: introduced 6, pre-existing 0.
- Added `crates/patchback-fit/tests/search_adversary.rs` (6 cases) and
  `crates/patchback-cli/tests/fit_adversary.rs` (3 cases); changed nothing else.
- F1 (blocker, NEEDS-CHANGE): with a 1 ms wall-time bound the recorded `wall_time_ms` is 173
  (0.25 s note) and 388 (0.9 s note), and the binary prints 173: the first render is never
  checked against the bound, a started render is not interrupted, and the clock starts after
  the reference analysis. The story's obligation says wall time never exceeds its bound.
- F2 (warning): when the render budget ends where a restart ends, the next restart is counted
  although it rendered nothing (`restarts 2`, log holds restart 0 only; `search.rs:452`, `:531`).
- F3 (note): with `max_restarts` 1 a converged run reports `RestartsExhausted`, so
  `ImprovementBelowThreshold` is unreachable with one restart; code and module doc agree, the
  unit brief's sentence differs.
- F4 (note, INFEASIBLE): `a_rejected_candidate_names_its_cause` checks a constant's `Display`
  and never runs the search; no completed a6 fit can hold a rejected entry.
- F5 (note): an `--out` naming a regular file is refused only after the whole search
  (4416 ms for 20 renders; up to 60 s with the defaults).
- F6 (note): restart seeds `seed ^ r` collide across (seed, r) pairs (seed 1 restart 2 and
  seed 2 restart 1 start from the same point).
- Mutant the implementor's suite misses: `step < MIN_STEP` → `<=` at `search.rs:439`; the
  adversary's `a_restart_that_cannot_improve_spends_one_start_and_four_sweeps` kills it.
- Held under attack: outcome order against `fit.yaml`; byte-identical `patch.patch` across block
  sizes 256, 512 and 3 and across seeds for restart 0; all 968 visitable points pass `validate`
  (`RendererFault` unreachable); the normalized vector stays in `[0, 1]`; the D8 fault order with
  two faults at once; bounds of 0 refused as `InvalidSearchBound`; the demo holds off the step
  grid (resonance_q 0.65, seed 3: loss 0.021532814785200735 → 0.012082857524406226).

## Routed

- F1 → Da6-22: the bound means no render starts after it, the first candidate is always
  evaluated, the clock starts at `fit` entry; the story's obligation is amended; the two red
  cases are re-pinned to the rule.
- F2, F5 → correction round 1 (Da6-25 for the refusal's place in the order).
- F3 → Da6-23: the implementor's D2 is the rule; no code change.
- F4 → the test is deleted in correction round 1.
- F6 → Da6-24: restart seeds through `SplitMix64::new(seed)`.

Report: wave a6 scratch `bounded-search-fit/adversary-report-1.md`. Agent cost 190,852 tokens,
43 tool uses, 1251 s.

```findings
[
{"file":"crates/patchback-fit/tests/search_adversary.rs","line":91,"category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the story obliges wall time never to exceed its bound, yet a 1 ms bound records 173 ms (0.25 s note) and 388 ms (0.9 s note), and the binary prints wall_time_ms 173, because the first render is unchecked and a started render is never interrupted"},
{"file":"crates/patchback-fit/tests/search_adversary.rs","line":151,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"when the render budget runs out exactly where a restart ends, the next restart is counted although it evaluated nothing, so restarts is 2 for a log holding only restart 0"},
{"file":"crates/patchback-fit/src/search.rs","line":460,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"with max_restarts 1 a converged run reports RestartsExhausted although its last sweep improved by less than the threshold, which diverges from the unit brief's stated stop rule and makes ImprovementBelowThreshold unreachable with one restart"},
{"file":"crates/patchback-fit/tests/search.rs","line":376,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a_rejected_candidate_names_its_cause only checks that a constant's Display is non-empty and never runs the search, and no completed fit with a6 inputs can hold a rejected entry to test instead"},
{"file":"crates/patchback-cli/src/fit.rs","line":221,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"an --out that names a regular file passes the existing-patch check and is refused only after the whole search (4416 ms for 20 renders, up to 60 s with the defaults)"},
{"file":"crates/patchback-fit/src/search.rs","line":205,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"deriving restart seeds as seed ^ r makes seed 1 restart 2 and seed 2 restart 1 start from the same point"}
]
```
