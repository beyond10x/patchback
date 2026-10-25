---
format: aep.planning-md/3
id: review-result:regressor-seeded-fit-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: regressor-seeded fit (wave a11, unit 1)'
relations:
- reviews: story:regressor-seeded-fit
revision: 1
---
# Adversary pass 2: regressor-seeded fit (story:regressor-seeded-fit)

Wave a11, unit 1, after correction round 1 (Da11-21 to Da11-23). `aep:adversary` on Opus against
the uncommitted change in tree `patchback-impl-seeded-fit` (base 2ef9cd0), 2026-10-25. Known and
not re-reported: the findings of pass 1 and the cases that closed them, the implementor's accepted
deviations (Da11-18 to Da11-20), the spec's missing refusals (Da11-5, Da11-6).

## Result

- verdict: red; cases executed 369 → 378, red 1; origin: introduced 3, pre-existing 0.
- Added `crates/patchback-fit/tests/adversary_seeded_fit_2.rs` (7 cases, 1 red) and
  `crates/patchback-cli/tests/adversary_fit_seeded_2.rs` (2 cases); changed nothing else. With
  `--no-fail-fast`: 47 suites, 378 executed, 377 passed, 1 failed, exit 101; clippy
  `-D warnings` and fmt exit 0.
- The diff since the pass-1 snapshot: every hunk in `search.rs`, the CLI's `fit.rs:69`,
  `regressor_seeded_fit_scenarios.rs` and the one rewritten case in `adversary_seeded_fit.rs` is
  named by correction report 1; nine other files byte-identical to the snapshot.
- F1 (warning, NEEDS-CHANGE, boundary): since Da11-21 the given analysis is never checked
  against the reference. The prediction no longer reads it and `check_reference` is skipped
  when it is given, so an analysis of 48,000 frames, or one at 44,100 Hz, admits a 21,600-frame
  reference and the seeded run starts. Pass 1 refused that through `extract_features`'s
  `ClipMismatch`. Library callers of `fit_request` reach it; the CLI does not (it analyses its
  own reference).
- F2 (warning, NEEDS-CHANGE, judgement): Da11-21 slices from sample 0, so silence before the
  first note stays in the first-note clip and shifts the frames against the note: a lead-in of
  4,096 frames changes 11 of 32 raw values, one of 1,000 frames 28 of 32 (largest |d| 1.953125).
  A first note not at sample 0 never gets the dataset's vector. Slicing `start..tail_end` with
  the span shifted to `0..end − start` is bit-equal to the dataset for a silent lead-in.
- F3 (note, NEEDS-CHANGE, judgement): refusal row 7 of the CLI's `fit.rs` module doc does not
  name the first-note analysis refusals (the held note of Da11-22; a first-note clip that is
  silent or pitch-ambiguous), which the scratch `cli-doc.md` states.
- Held: the Da11-21 constants come from `patchback_analysis`; `.min(frames)` and the clamped span
  are in place; the vector bit-equals the dataset's for three one-note renders; a second note on
  the first sample after the release window leaves it unchanged, one inside the window changes
  it; a foreign analysis of the same length changes nothing (the prediction does not read it);
  the Da11-22 boundary is 47,104 (a vector) and 47,105 (refused) in a 48,000-frame clip, ends
  48,000 and 60,000 refused through the binary with nothing written, the default fit runs all
  four; `--max-wall-time-ms 1` gives one evaluation and a `wall_time_ms` above the bound (seeded
  276 ms, default 251 ms); on a 10 s two-note reference (debug build) the whole-reference
  analysis is 1,146 ms, the first-note analysis 298 ms, one evaluation about 2,883 ms, so the
  seeded run's extra analysis is under one evaluation; the correction changed no expected value
  and `features_of` derives its vector; the default path byte-equal to the base for the five
  files; `fit_results.rs` unchanged; the mutants (no `.min(frames)`, an unclamped span end)
  are caught by pass 1's held case and the 47,104 case.

## Routed

- F1 → Da11-24: `fit_request` refuses a given analysis whose frame count or sample rate is not
  the reference's, before the prediction, as `PredictionFailed` (`NoValidCandidate`) naming both.
- F2 → Da11-25: the first-note clip starts at the note's start (`samples[start..tail_end]`, the
  span `0..end − start`).
- F3 → Da11-26: refusal row 7 names the first-note analysis refusals.

```findings
- file: crates/patchback-fit/src/search.rs
  line: 834
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: a given analysis is never checked against the reference since Da11-21 (frame count and rate); an analysis of a 48000-frame clip or of 44100 Hz admits a 21600-frame reference and the seeded run starts; pass 1 refused it via extract_features ClipMismatch
- file: crates/patchback-fit/src/search.rs
  line: 542
  category: judgement
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: leading silence before the first note stays in the slice and changes the vector (11 of 32 values at a hop-aligned shift of 4096, 28 of 32 at 1000, largest |d| 1.953125), so a first note not at sample 0 never gets the dataset's vector; slicing start..tail_end gives bit equality
- file: crates/patchback-cli/src/fit.rs
  line: 101
  category: judgement
  severity: note
  verdict: NEEDS-CHANGE
  origin: introduced
  message: refusal row 7 omits the first-note analysis refusals (held note Da11-22, silent or pitch-ambiguous first-note clip) that cli-doc.md, which names this doc as its source, states
```
