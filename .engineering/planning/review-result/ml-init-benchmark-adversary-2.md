---
format: aep.planning-md/3
id: review-result:ml-init-benchmark-adversary-2
kind: review-result
status: active
title: 'Adversary pass 2: ML-init benchmark (wave a12, unit 1)'
relations:
- reviews: story:ml-init-benchmark
revision: 1
---
# Adversary pass 2: ML-init benchmark (wave a12, unit 1)

Reviewer: `aep:adversary` (Opus), 2026-10-25, on the uncommitted working tree of
`impl/ml-init-benchmark` at base d99f9f5 after correction 1. Report:
`a12/ml-init-benchmark/adversary-report-2.md` in the session's scratch (outside the repository).

## Verdict

Green: 0 red cases, 1 warning on a pre-existing doc sentence, 3 findings on the unit's own
tests. Cases executed 402 → 411 (8 cases in
`crates/patchback-cli/tests/adversary_benchmark_ml_init_2.rs`, 1 in
`crates/patchback-fit/tests/adversary_ml_init_benchmark_2.rs`, every one green on the tree).
Gate after the cases: `cargo test -p patchback-fit -p patchback-cli` 53 suites, 411 passed, 0
failed (510 s); dependency gate 39 passed; clippy clean; fmt clean. No tracked file edited.

## What was attacked and held

- The engine baseline byte for byte against a base build from `git archive d99f9f5`, two flag
  sets: every WAV and `clips.txt` equal; `benchmark.txt` and stdout equal except `wall_time_ms`.
- `StartBenchmark`'s refusal order (`spec/domains/fit.yaml`): `InvalidHoldoutSize` before
  `WeightArtifactRequired`, which wins over an unknown topology, `--max-renders 0`,
  `--max-restarts 0` and an existing `benchmark.txt`.
- Usage errors (exit 2) before any file is read; the weights file is reported before the
  statistics when both are missing.
- A foreign `modelspec` record in the artifact is written verbatim; a mutant writing the pin
  fails the new case.
- 3 and 5 clips: 14 column names, 15 fields per clip line, a partial clamp count (1 of 3, 2 of 5)
  equal to the library predictor's, the six medians recomputed from `clips.txt`.
- A prediction refused at clip 2 of 3 refuses the benchmark with exit 1 and leaves no `--out`.
- A binding wall-time bound stops both searches on their first candidate.
- The statistics bound (1 MiB) and a directory or empty file as `--weights`.
- Non-finite predicted values at all 8 indices refused as the predicted patch.

## Decisions for the coordinator

Da12-36 in approval-record:wave-a12 (the doc sentence corrected by the coordinator; the three
test findings covered by the pass-2 cases, which are committed with the unit).

```findings
- file: crates/patchback-cli/src/benchmark.rs
  line: 147
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: pre-existing
  message: the module doc says only wall_time_ms differs between two runs of the same flags, but under a binding --max-wall-time-ms the evaluations, losses, fit.wav and seeded.wav differed across runs on both the new and the base binary.
- file: crates/patchback-cli/tests/ml_init_benchmark.rs
  line: 588
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the_kind_and_file_flags_are_usage_checked passes against the base binary d99f9f5, which has no --kind, --weights or --normalization, because it asserts only exit 2 and a flag name in stderr.
- file: crates/patchback-cli/tests/ml_init_benchmark.rs
  line: 496
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: a mutant writing the repository pin into the modelspec record passes the unit's 6 and pass 1's 5 CLI cases, because every artifact is written with the pin; the pass-2 case with a foreign modelspec catches it.
- file: crates/patchback-fit/tests/ml_init_benchmark.rs
  line: 404
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: a_predicted_point_the_builder_refuses_is_refused asserts only is_err() for one NaN, so a refusal of the wrong kind or wording would stay green.
```
