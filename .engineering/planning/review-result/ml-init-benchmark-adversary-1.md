---
format: aep.planning-md/3
id: review-result:ml-init-benchmark-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: ML-init benchmark (wave a12, unit 1)'
relations:
- reviews: story:ml-init-benchmark
revision: 1
---
# Adversary pass 1: ML-init benchmark (wave a12, unit 1)

Reviewer: `aep:adversary` (Opus), 2026-10-25, on the uncommitted working tree of
`impl/ml-init-benchmark` at base d99f9f5. Report:
`a12/ml-init-benchmark/adversary-report-1.md` in the session's scratch (outside the repository).

## Verdict

Green: 0 red cases, 2 notes. Cases executed 394 → 402 (8 adversary cases in
`crates/patchback-fit/tests/adversary_ml_init_benchmark.rs` and
`crates/patchback-cli/tests/adversary_benchmark_ml_init.rs`, every one green on its first run).
Gate after the cases: `cargo test -p patchback-fit -p patchback-cli` 51 suites, 402 passed, 0
failed (480 s); clippy clean; fmt clean; dependency gate 39 passed. No tracked file edited.

## What was attacked and held

- The spec's order of refusals in `RecordMlInitResult` (kind-mismatch before wrong-state, both
  directions), and a baseline result on a completed ML-init benchmark (`BenchmarkKindMismatch`).
- Da12-3: inputs read once; swapping the weights and the statistics 0.5 s into a 4-clip run changed
  no byte, and the `weights` and `normalization` records name the original files.
- Da12-4: arm 4 runs with the clip's `fit_seed` and its restart 1 starts where arm 2's does
  (`--max-restarts 2`, threshold 1e9).
- Da12-16: arm 3 is resized as arm 1 is; `patchback compare` of `predicted.wav` and `seeded.wav`
  against the reference gives back `raw_loss` and `seeded_best_loss`, including a clamped
  prediction (3 of 8 outputs clamped, `clamped_predictions 1 of 1`).
- Medians recomputed from `clips.txt` at 1 and 4 clips; 3 example directories and 15
  `listening_example` lines at 4 clips; reruns byte-equal apart from `wall_time_ms`.
- Refusals: a corrupt artifact is refused before `InvalidHoldoutSize` and before an existing
  `benchmark.txt`, in under 5 s with `--examples 200`; 31-row statistics give
  `NormalizationStatsNotFound` with `read_stats`'s message verbatim.
- The domain accounting fails a mutant that drops `BenchmarkStateConflict` (by code reading).
- 40 admitted holdout clips (5 seeds × 8) pass the analysis and the seeded fit, and render the
  same at block sizes 7 and 256.

## Decisions for the coordinator

Da12-28 and Da12-29 in approval-record:wave-a12.

```findings
- file: crates/patchback-cli/src/benchmark.rs
  line: 751
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the Da12-7 analysis-refusal clip path cannot be reached from the generator, because the pipeline admits only draws whose analysis under the identical config and samples succeeded (40 of 40 measured).
- file: crates/patchback-cli/tests/ml_init_benchmark.rs
  line: 53
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: every ML-init run in the unit's suite uses --max-restarts 1, so a wrong fit seed on arm 4 would stay green; the adversary's paired-restart case now covers it.
```
