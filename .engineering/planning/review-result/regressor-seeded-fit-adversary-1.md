---
format: aep.planning-md/3
id: review-result:regressor-seeded-fit-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: regressor-seeded fit (wave a11, unit 1)'
relations:
- reviews: story:regressor-seeded-fit
revision: 1
---
# Adversary pass 1: regressor-seeded fit (story:regressor-seeded-fit)

Wave a11, unit 1. `aep:adversary` on Opus against the uncommitted change in tree
`patchback-impl-seeded-fit` (base 2ef9cd0), 2026-10-25. Known and not re-reported: the three
implementor deviations (Da11-18 to Da11-20), the artifact and statistics refusals the spec does
not declare (Da11-6).

## Result

- verdict: red; cases executed 361 → 369 (patchback-fit and patchback-cli), red 2; origin:
  introduced 3, pre-existing 0.
- Added `crates/patchback-fit/tests/adversary_seeded_fit.rs` (5 cases, 2 red) and
  `crates/patchback-cli/tests/adversary_fit_seeded.rs` (3 cases); changed nothing else. With
  `--no-fail-fast`: 45 suites, 369 executed, 367 passed, 2 failed, exit 101; clippy
  `-D warnings` and fmt exit 0.
- F1 (warning, CONFIRMED, acceptance): the pitch feature (index 31) is the whole clip's f0
  estimate, so on a two-note reference the first note's normalized vector differs from the
  single-note one at index 31 (1.0 against 1.0551; f0 110.01 Hz against 164.81 Hz). The dataset
  extracts over one note's own clip (`crates/patchback-dataset/src/generate.rs:386-391`).
- F2 (warning, CONFIRMED, boundary): the Da11-8 clamp never yields a vector: a first note whose
  `end_sample` is at or past the reference's end has an empty release window,
  `FeatureNotComputable` (feature 16, "the window from sample 48000 to 48000 holds no frame
  centre") follows, and the seeded fit refuses `NoValidCandidate` where the default fit of the
  same inputs runs.
- F3 (note, CONFIRMED, judgement): the `wall_time_ms` doc row names the reference analysis as
  what may exceed the bound; on a seeded run the analysis is outside the clock and the
  prediction is inside it.
- Held: the refusal order (bounds before `AnalysisRequired`, mismatch before silence, both
  contracts wrong names the weight artifact); the default path byte-equal to the base for
  `run.meta`, `patch.patch`, `notes.events`, `trace.events` and `reconstructed.wav` (stdout and
  `report.txt` differ in `out` and `wall_time_ms` only); `std 0` divides by 1; wrong row counts
  and versions refused by `read_stats`; a statistics file of 1 MiB + 1 byte refused, a CRLF one
  accepted and digested raw; a hidden `f32::MAX` overflow refused as `NoValidCandidate`
  "output 0 is NaN" through the binary; the seed files answered between `--input` and `--out`;
  trailing silence leaves the features unchanged; the training-path features reproduced.
- Noted: the adversary compared the implementor's `default-base/` and `default-after/` and did
  not build the base binary itself.

## Routed

- F1 → Da11-21: the prediction's vector is extracted from the first note's own clip (the
  reference's samples up to the note's release window end) through `analyze_and_extract`, the
  dataset's call; the given analysis admits the reference and is no longer the prediction's
  source. The adversary's files stay.
- F2 → Da11-22: the refusal stays and is documented; the adversary's case is rewritten to
  assert it.
- F3 → Da11-23: the `wall_time_ms` and `--max-wall-time-ms` doc rows name the first-note
  analysis and the prediction inside the fit's clock.

```findings
- file: crates/patchback-fit/src/search.rs
  line: 524
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the seeded features' pitch value (index 31) is the whole clip's f0, so a later louder note of another pitch changes the first note's prediction (1.0 vs 1.0551)
- file: crates/patchback-fit/src/search.rs
  line: 513
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the Da11-8 clamp never yields a vector, because a first note reaching the clip's end has an empty release window and the seeded fit is refused NoValidCandidate while the default fit of the same inputs runs
- file: crates/patchback-cli/src/fit.rs
  line: 69
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the wall_time_ms row says the bound may be exceeded by the reference analysis, but on a seeded run the analysis is outside the clock and the prediction is inside it
```
