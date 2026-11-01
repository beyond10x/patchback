---
format: aep.planning-md/3
id: review-result:note-estimation-adversary-2
kind: review-result
status: active
title: Adversary pass 2 on note estimation
relations:
- reviews: story:note-estimation
revision: 1
---
unit: story:note-estimation, commits 3ed9472 + e3fb8d3 (base 1623311), working tree of impl/note-estimation plus two untracked test files
verdict: red
cases: executed 673→693, red 14
origin: introduced 14 / pre-existing 0 / undecided 0
wrote-outside-worktree: 9 files in the c1/note-estimation/adv2 scratch directory (listed in section 6)
needs-coordinator: yes. The pooled confident-inside rate sits right at its 0.90 bound, so any 16-seed sample is a coin flip. Shapes inside Dc1-9's wording but outside Dc1-13 need a decision on whether they bind.

## 1. Diff
`git --no-pager diff --stat` is empty because both files are untracked. `git status --short`:
```
?? crates/patchback-analysis/tests/adversary_2.rs
?? crates/patchback-cli/tests/adversary_2.rs
```
Both are test files. No implementation file was touched. `rustfmt` ran on these two files only, and no tracked file changed.

## 2. Cases added
They were run alone with `cargo test -p patchback-analysis --test adversary_2` before the suite: 18 tests, 12 red. After that first run I added three things, then ran the file alone again: 19 executed, 13 red, 1 ignored. The additions:
- the `a_gaussian_noise_floor…` case, run alone first and red;
- an ignored `probe_draw` helper;
- a diagnostic print of which violations fall in accepted estimations. It changes no assertion.

The CLI case was run alone: 1 test, red.

**Reference generator on unseen seeds 17 to 32** (16 × 32 phrases, 2,511 notes):
- `the_reference_distribution_on_unseen_seeds_17_to_32_meets_every_dc1_9_bound_and_calibration_per_seed`: red.
  ```
  [
      "confident inside < 0.90: 0.8996",
  ]
  ```
  - Every other rate is met (pooled detection 0.9960, spurious 0.0036, onset 0.9764, MIDI 0.9952, pitch 0.9952, gate 0.9728, tail 0.9948).
  - 0 calibration violations on every one of the 16 seeds.
  - 989 of 9,852 in-tolerance quantities are unconfident: onset 230, pitch 2, gate 406, tail 351. The limit is 985.
- `no_reference_phrase_on_unseen_seeds_17_to_32_is_refused_as_polyphonic`: red, `"seed 29 phrase 27"`. That phrase is MIDI 40, 37, 37, 48, 54 with gaps, attack 33 ms.

**Stress shapes (Dc1-13) on unseen seeds 5 to 8:**
- `short_gates_on_unseen_seeds_5_to_8_hold_calibration_and_polyphony`: red.
  ```
  "seed 6 phrase 15 note 1 (9183, 12654, 39, 0.58449394): onset error 369, confidence 0.526",
  "seed 7 phrase 24 note 4 (28420, 31944, 42, 0.6197642): onset error -988, confidence 0.900",
  "short-gate: monophonic phrases refused as PolyphonicInput: [\"seed 5 phrase 30\", \"seed 6 phrase 18\"]"
  ```
  Seed 5 phrase 30 is a zero-gap legato MIDI 49 → 50.
- `low_notes_on_unseen_seeds_5_to_8_hold_calibration_and_polyphony`: red.
  ```
  "seed 6 phrase 4: spurious note at 30326 with onset confidence 0.545",
  "seed 7 phrase 0 note 1 (20930, 32444, 38, 0.57824993): onset error 666, confidence 0.619",
  ```
- Green on seeds 5 to 8: long-release, quiet, zero-gap-repeat and noise-floor.

**The noise-relative silence threshold:**
- `a_gaussian_noise_floor_at_minus_70_dbfs_holds_calibration_and_polyphony`: red. This is Dc1-13's floor, white noise at -70 dBFS RMS, but Gaussian.
  - The generator's noise is uniform, so its peaks stay at or below 1.73 σ and never cross the 4 × floor quiet threshold.
  - 16 violations, 15 of them spurious notes at onset confidence 1.000, for example `"seed 1 phrase 0: spurious note at 96418 with onset confidence 1.000"`.
  - Pooled spurious rate 0.1739. Only 18 of 128 phrases are accepted, against 41 with uniform noise.
- `a_noise_floor_4_db_below_the_silence_threshold_holds_calibration_and_polyphony` (uniform noise at -64 dBFS): red, `"seed 2 phrase 1: spurious note at 48307 with onset confidence 0.998"`.

**Shapes inside Dc1-9's wording that the generator does not draw** (seeds 1 to 4, calibration and polyphony asserted, rates not bounded):
- `dark_cutoffs_of_100_to_800_hz_hold_calibration_and_polyphony`: red, 6 violations.
  - One is in an **accepted** estimation, so it would be written: `"seed 4 phrase 20 note 3 (51645, 62311, 54, 0.765721): pitch error MIDI 54 for 54 (-12.5 cents), confidence 0.929"`.
  - Another: `"seed 3 phrase 13 note 0 (2714, 18343, 84, …): pitch error MIDI 50 for 84 (-3370.0 cents), confidence 0.859"`.
- `plucked_notes_with_a_sustain_below_0_3_hold_calibration_and_polyphony`: red, 18 violations, none in an accepted estimation.
  - Gate errors are 13,116 to 16,381 samples at confidence 0.598 to 0.746, for example `"seed 4 phrase 19 note 0 (4794, 22340, 71, …): gate error -16381, confidence 0.746"`. The sustain there is 0.0009, so the gate end cannot be seen in the audio at all.
  - One spurious note at 1.000.
  - The 13 tail violations at 0.96 to 1.0 come from the harness's tail truth. When the note is already below -60 dBFS at its gate end, the truth is the gate end, but the estimator places the tail where the decay actually crossed the threshold. **Those 13 are a gap in the truth definition, not a defect I can claim.**
- `gaps_of_half_a_millisecond_to_10_ms_hold_calibration_and_polyphony`: red, `"seed 3 phrase 19 note 1 (14043, 31935, 53, 0.9263912): gate error -1223, confidence 0.940"`.
- `drives_of_18_to_24_db_hold_calibration_and_polyphony`: red, `tail error -17462, confidence 0.545` and `spurious note at 96376 with onset confidence 0.545`.
- `long_decays_hold_calibration_and_polyphony`: red, `"seed 1 phrase 10 note 7 (114339, 119308, 59, …): onset error 265, confidence 0.830"`.

**Dc1-3 with drawn voices and roots:**
- `drawn_dyads_a_minor_second_major_third_tritone_or_fifth_apart_are_refused_as_polyphonic`: red. `7 of 64 dyads not refused as PolyphonicInput`; all 7 are `LowConfidenceNotes`. Five are minor seconds (MIDI 65+1 twice, 83+1, 71+1, 78+1) and two are major thirds (67+4, 42+4). None was accepted.
- `a_single_drawn_note_is_never_refused_as_polyphonic`: green (128 notes).

**Determinism:**
- `concurrent_estimations_of_one_clip_write_identical_bytes`: green.

**The estimation file (Dc1-14):**
- `an_estimation_whose_frame_count_was_changed_is_refused`: red, `an estimation with frame_count 118080 for a clip of 59040 samples reads: Ok(118080)`.

**CLI** (`crates/patchback-cli/tests/adversary_2.rs`):
- `fit_refuses_a_given_estimation_of_another_clip_even_when_the_notes_name_none`: red.
  ```
  assertion `left == right` failed: fit of other.wav with the estimation of phrase.wav exited Some(0); stderr ""
  ```

## 3. Suite run, after the cases existed
Command: `flock <wave lock> env CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0 cargo test --locked -p patchback-analysis -p patchback-cli -p patchback-core --no-fail-fast`. Wall time 90 s.
```
     Running tests/adversary_2.rs (target/debug/deps/adversary_2-573c51bd5ba7506f)
test result: FAILED. 6 passed; 13 failed; 1 ignored; 0 measured; 0 filtered out; finished in 25.12s
     Running tests/adversary_2.rs (target/debug/deps/adversary_2-05750ee482e87042)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
error: 2 targets failed:
EXIT=101
```
- Totals: 679 passed, 14 failed, 693 executed.
- "Before" is 673: 693 minus the 20 executed in my two files.
- Every other binary passed, including `estimate_reference`, `estimate_stress` and `adversary_1`.
- `cargo clippy --locked -p patchback-analysis -p patchback-cli --test adversary_2 -- -D warnings`: EXIT=0.

**Extra probe** with the committed ignored `probe` (32 phrases a seed) on further unseen seeds:

| Seeds | Confident-inside | Calibration violations | `PolyphonicInput` refusals |
|---|---|---|---|
| 17–32 | 0.8996 | 0 | 1 |
| 33–48 | 0.9074 | 0 | 0 |
| 49–64 | 0.8985 | 0 | 0 |
| 65–80 | 0.8969 | 0 | 1 (seed 79 phrase 17) |
| 17–80 pooled | 0.9006 (4,004 unconfident of 40,294) | 0 | 2 |

All other Dc1-9 rates are met in every block.

## 4. Judgement findings (working tree at e3fb8d3 plus the two files)
- **Confident-inside rate.** It is about 0.900 ± 0.004 per 16-seed block. The committed harness's 0.9001 on seeds 1 to 16 passes by sampling luck, not by margin; 3 of 4 fresh blocks miss.
  - What reaches it: Dc1-12's own rule applied to another draw from the same distribution. NEEDS-CHANGE, introduced.
- **Calibration.** It holds on the reference out of sample: 0 violations in 64 unseen seeds. It does not hold on two Dc1-13 shapes out of sample (short-gate, low), nor on Gaussian noise at Dc1-13's level.
  - Almost every violation sits in a phrase refused as `LowConfidenceNotes`. Only the dark-cutoff pitch error reaches an accepted estimation.
- **`fit --estimation` with a sequence that names no estimation** (`crates/patchback-cli/src/fit.rs:440`). The file is read and validated, but its `clip_sha256` is never compared with `--input`.
  - What reaches it: a user passing `--estimation` with a hand-written `notes.events`. CONFIRMED, warning.
  - The literal fix: run `check_estimation_clip` on every read estimation, not only on the bound one.
- **`frame_count` is covered by neither digest nor id** (`crates/patchback-analysis/src/estimation_file.rs:269`). Nothing downstream reads it except the `tail ≤ frame_count` check.
  - What reaches it: nothing found, only a hand edit. INFEASIBLE, note. It contradicts the module doc's "a file whose clip … was changed reads as refused".

## 5. Attacked and not broken
- The calibration rule on the reference distribution, seeds 17 to 80: 0 violations.
- Every Dc1-9 rate other than confident-inside, on every block of unseen seeds.
- Long-release, quiet, zero-gap-repeat and uniform noise floor at -70 dBFS on seeds 5 to 8: 0 violations and no polyphonic refusals.
- A single drawn note across MIDI 36 to 84: never refused as polyphonic (128 cases).
- Concurrent and repeated estimation of one clip: identical text.
- `estimate` and `fit` read WAVs through the same `decode` path, so the clip SHA-256 of one file agrees between them.
- The order of the input guards matches `AnalyzeAudio` in `analysis.yaml`. The render path answers `unknown-estimation` after `unknown-patch` and before the rate, as `score.yaml` orders it.

## 6. Paths written outside the worktree
All under `the adversary scratch directory c1/note-estimation/adv2/`:
- `analysis-alone.log`
- `analysis-alone-2.log`
- `gaussian-alone.log`
- `cli-alone.log`
- `probe.sh`
- `probe.log`
- `probe-draw.log`
- `suite.log`
- `clippy.log`

Test binaries and CLI scratch went into the worktree's own `target/`. My session lease is released.

## 7. Findings

```findings
[
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 61, "category": "property", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "On unseen reference seeds 17 to 32 pooled confident-inside is 0.8996 against the 0.90 bound (989 unconfident of 9852, limit 985), and 33-48, 49-64, 65-80 measure 0.9074, 0.8985, 0.8969, so the bound sits at the expected value and the in-sample 0.9001 is sampling luck (test: the_reference_distribution_on_unseen_seeds_17_to_32_meets_every_dc1_9_bound_and_calibration_per_seed)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 79, "category": "property", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "Monophonic reference phrases on unseen seeds are still refused as PolyphonicInput (seed 29 phrase 27; seed 79 phrase 17 by probe), against Dc1-3 (test: no_reference_phrase_on_unseen_seeds_17_to_32_is_refused_as_polyphonic)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 137, "category": "property", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The Dc1-13 short-gate shape on unseen seeds 5 to 8 refuses two monophonic phrases as PolyphonicInput (a zero-gap MIDI 49 to 50 legato among them) and carries onsets off by 369 at 0.526 and -988 at 0.900 (test: short_gates_on_unseen_seeds_5_to_8_hold_calibration_and_polyphony)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 142, "category": "property", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The Dc1-13 low shape on unseen seeds 5 to 8 carries a spurious note at onset confidence 0.545 and an onset off by 666 at 0.619 (test: low_notes_on_unseen_seeds_5_to_8_hold_calibration_and_polyphony)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 353, "category": "boundary", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "Gaussian white noise at Dc1-13's -70 dBFS RMS breaks the noise-relative quiet threshold, tuned to the generator's uniform noise: 15 spurious notes at onset confidence 1.000, spurious rate 0.1739, 18 of 128 phrases accepted (test: a_gaussian_noise_floor_at_minus_70_dbfs_holds_calibration_and_polyphony)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 164, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "Uniform noise at -64 dBFS, 4 dB below the silence threshold, yields a spurious note at onset confidence 0.998 (test: a_noise_floor_4_db_below_the_silence_threshold_holds_calibration_and_polyphony)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 508, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "7 of 64 equal-level dyads from drawn voices (five minor seconds, two major thirds) are refused as LowConfidenceNotes instead of PolyphonicInput, so polyphony is still not decided before confidence (test: drawn_dyads_a_minor_second_major_third_tritone_or_fifth_apart_are_refused_as_polyphonic)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 483, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "Cutoffs of 100 to 800 Hz, inside the voice contract and Dc1-9's varied cutoff, give an accepted estimation with f0 12.5 cents off at pitch confidence 0.929, and MIDI 50 for 84 at 0.859 in a refused one (test: dark_cutoffs_of_100_to_800_hz_hold_calibration_and_polyphony)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 463, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "Sustains below 0.3 give gate ends off by 13116 to 16381 samples at confidence 0.598 to 0.746 and a spurious note at 1.000, all in refused phrases; the 13 tail violations come from the harness's tail truth, not the estimator (test: plucked_notes_with_a_sustain_below_0_3_hold_calibration_and_polyphony)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 451, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "Gaps of 0.5 to 10 ms after attacks up to 40 ms, inside the reference wording, give a gate end off by 1223 samples at confidence 0.940 (test: gaps_of_half_a_millisecond_to_10_ms_hold_calibration_and_polyphony)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 493, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "Drives of 18 to 24 dB give a tail off by 17462 samples and a spurious note, both at confidence 0.545, in a refused phrase (test: drives_of_18_to_24_db_hold_calibration_and_polyphony)"},
  {"file": "crates/patchback-analysis/tests/adversary_2.rs", "line": 473, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "Decays of 300 ms to 2 s give an onset off by 265 samples at confidence 0.830 (test: long_decays_hold_calibration_and_polyphony)"},
  {"file": "crates/patchback-analysis/src/estimation_file.rs", "line": 269, "category": "contract-drift", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "frame_count is covered by neither notes_sha256 nor the id, so a doubled frame_count reads, against the module doc's claim that a changed clip reads as refused; only a hand edit reaches it (test: an_estimation_whose_frame_count_was_changed_is_refused)"},
  {"file": "crates/patchback-cli/src/fit.rs", "line": 440, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "fit reads and validates a given --estimation even when the notes name none but never compares its clip_sha256 with --input, so an estimation of another clip is fitted with exit 0, against Dc1-14 (test: fit_refuses_a_given_estimation_of_another_clip_even_when_the_notes_name_none)"}
]
```

The home-directory path in section 6 was replaced by a description by the coordinator, because the security gate refuses home paths in commits.
