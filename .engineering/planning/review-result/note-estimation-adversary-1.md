---
format: aep.planning-md/3
id: review-result:note-estimation-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on note estimation
relations:
- reviews: story:note-estimation
revision: 2
---
unit: story:note-estimation, commit 3ed9472 (base 1623311), working tree of impl/note-estimation plus one untracked test file
verdict: red
cases: executed 659→671, red 10
origin: introduced 10 / pre-existing 0 / undecided 0
wrote-outside-worktree: the adversary scratch directory c1/note-estimation/adv1/{probe-reference.log,probe-shapes.log,probe-phrases.sh,adversary-alone.log,suite.log}
needs-coordinator: yes

The estimator's constants are fitted to the pinned seed. On 0x5eed_0c19 every Dc1-9 rate comes out at exactly 1.0000. On 12 other seeds of the same generator, 9 have a quantity outside tolerance that still carries a confidence at or above its threshold. Some of those are in estimations the estimator accepts, so they would be written to `estimation.txt`. Two seeds miss the 95 % detection bound because a monophonic phrase is refused as `PolyphonicInput`.

The coordinator needs to decide what to do with the Dc1-9 harness: it passes on one seed only. A harness that ran several seeds would have caught this.

## 1. Diff
`git --no-pager diff --stat` is empty because my file is untracked. `git status --short`:
```
?? crates/patchback-analysis/tests/adversary_1.rs
```
That is the only change, and it is a test file. Release builds of the test binary also went into the worktree's own `target/release` (6.6 MB, git-ignored).

## 2. Cases added
All are in `crates/patchback-analysis/tests/adversary_1.rs`. They were run as that file alone before the suite (`cargo test -p patchback-analysis --test adversary_1`): 10 red, 2 green, 2 ignored. The two ignored tests are the exploration helpers `probe_rates` and `probe_phrase`.

- **`reference_generator_phrases_on_unpinned_seeds_carry_no_confident_quantity_outside_tolerance`**: red. 10 phrases from the reference generator (an exact copy, which reproduces the pinned seed's 155 notes), seeds 1, 3, 4, 6, 7, 9, 11 and 12.
  ```
  13 confident quantities outside tolerance: [
      "seed 1 phrase 7 note 1 (11480, 20809, 37, 0.9607794): onset error -1597, confidence 1.000",
      "seed 3 phrase 6 note 3 (56273, 66826, 67, 0.70010203): gate error 2110, confidence 0.633",
      "seed 6 phrase 31 note 2 (28479, 39298, 82, 0.51255935): tail error -13307, confidence 1.000",
      "seed 6 phrase 31: spurious note at 29326 with onset confidence 1.000",
      "seed 7 phrase 10 note 1 (22528, 39485, 46, 0.85712963): onset error -1723, confidence 0.581", ...
  ```
- **`an_accepted_estimation_of_a_reference_phrase_carries_no_confident_quantity_outside_tolerance`**: red. The same phrases, counting only estimations the estimator accepted.
  ```
  "seed 6 phrase 20 note 1 ...: onset error -603, confidence 0.511",
  "seed 7 phrase 10 note 0 ...: gate error -1723, confidence 0.581",
  "seed 7 phrase 10 note 1 ...: onset error -1723, confidence 0.581",
  "seed 7 phrase 17 note 3 ...: onset error 300, confidence 0.685",
  "seed 11 phrase 20 note 1 ...: onset error 851, confidence 0.576",
  "seed 12 phrase 28 note 3 ...: onset error -558, confidence 0.582",
  ```
- **`monophonic_reference_phrases_are_not_refused_as_polyphonic`**: red. Seed 8 phrase 8 and seed 9 phrase 30 are both refused as `PolyphonicInput`. Probe: in seed 8 the trigger is the zero-gap pair MIDI 36 → 65. Each note alone, and every other pair, is accepted.
- **`the_reference_generator_on_seed_8_finds_at_least_95_percent_of_notes`**: red, `detection 0.9452; refused ["seed 0x8 phrase 8: patchback.analysis.PolyphonicInput"]`.
- **`a_same_pitch_retrigger_with_no_gap_is_within_tolerance_or_not_confident`**: red. Three MIDI 57 notes with no gap, sustain 0.5, attack 5 ms. The estimation is accepted, and both retriggers are placed 1,305 samples early at confidence 1.0:
  ```
  "constructed note 1 (19200, 33600, 57, 0.8): onset error -1305, confidence 1.000",
  "constructed note 0 (4800, 19200, 57, 0.8): gate error -1305, confidence 0.952", ...
  ```
- **`zero_gap_same_pitch_repeats_carry_no_confident_quantity_outside_tolerance`**: red, 7 violations. Onsets are 1,386 to 1,672 samples early at confidence 1.000.
- **`a_legato_octave_change_is_within_tolerance_or_not_confident`**: red, from the one-sided change point. An octave down (58 → 46) is accepted with `onset error -1722, confidence 0.607`. 46 → 58, 69 → 57 and 57 → 69 are within tolerance.
- **`long_releases_carry_no_confident_quantity_outside_tolerance`**: red, for releases of 300 ms to 1.5 s:
  ```
  "seed 1 phrase 19 note 2 ...: gate error 18286, confidence 0.806",
  "seed 1 phrase 19 note 2 ...: tail error 12308, confidence 0.994",
  "seed 1 phrase 14 note 3 ...: gate error 7375, confidence 0.990", ...
  ```
- **`a_phrase_over_a_noise_floor_10_db_below_the_silence_threshold_is_estimated`**: red. The 3-note `PHRASE` from `tests/estimate.rs` is accepted when clean. With white noise at -70 dBFS RMS added it is refused as `PolyphonicInput`:
  ```
  refused: more than one note sounds at once; polyphony is outside the MVP and is refused, not guessed away
  ```
- **`a_dyad_whose_second_tone_is_12_db_down_is_refused_as_polyphonic`**: red. A dyad whose upper tone is 12 dB down is still refused, but 3 of the 6 dyads get the wrong error, `LowConfidenceNotes` instead of `PolyphonicInput`: `MIDI 40 + 1`, `MIDI 40 + 7`, `MIDI 64 + 1`.
- **`a_single_low_saw_note_of_105_ms_is_not_refused_as_polyphonic`**: green. Single 105 ms notes at MIDI 36 to 40 are not refused.
- **`an_undetected_octave_dyad_reports_the_lower_pitch_or_is_not_confident`**: green. Two saws an octave apart, at 0.5/0.5 and 0.2/0.8, are always accepted as one note at the lower MIDI number with pitch confidence about 0.998. That is what Dc1-3 allows, and the pitch reported is the correct period.

## 3. Suite run, after the cases existed
Command: `flock <wave lock> env CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0 cargo test --locked -p patchback-analysis -p patchback-cli -p patchback-core --no-fail-fast`
```
     Running tests/adversary_1.rs
test result: FAILED. 2 passed; 10 failed; 2 ignored; 0 measured; 0 filtered out; finished in 7.22s
error: 1 target failed:
EXIT=101
```
Every other test binary passed, including `estimate_reference.rs` (7 passed) and `estimate.rs` (10 and 9 passed). Totals: 661 passed, 10 failed, 671 executed. The "before" figure of 659 is 671 minus the 12 executed in `adversary_1`, taken from the same run with that binary subtracted. I made no earlier suite run.

Before writing any case I measured rates in release builds with the `probe_rates` helper, 32 phrases per seed. Full numbers are in `probe-shapes.log`.

**The reference shape**, seeds 1 to 12:
- Pinned seed 0x5eed0c19: 1.0000 on every rate.
- Unpinned seeds: onset 0.957 to 0.994, gate 0.971 to 0.993.
- Calibration violations on 9 of 12 seeds.
- Detection 0.9452 (seed 8) and 0.9459 (seed 9).
- 13 to 23 of each seed's 32 phrases are accepted; the rest are refused for low confidence.

**Other shapes**, seeds 1 to 4:

| Shape | What changes | Where it misses Dc1-9 |
|---|---|---|
| short | gates of 45 to 100 ms | detection 0.83 to 0.87, from false `PolyphonicInput` refusals |
| legato-repeat | zero-gap same-pitch repeats allowed | onset 0.85 to 0.89; 25 to 36 violations per seed, 13 to 23 of them in accepted estimations |
| long-release | releases of 300 to 1,500 ms | gate 0.78 to 0.83 |
| low | MIDI 36 to 43 | onset down to 0.86, confident-inside down to 0.877; violations on every seed |
| quiet | velocities of 0.1 to 1.0 | violations on every seed |
| high | MIDI 77 to 84 | no rate missed |

**How representative these inputs are:**
- The reference-shape seeds are Dc1-9's own distribution, so those misses are representative.
- Zero-gap same-pitch repeats are inside the wording of Dc1-9 ("zero gaps only with an attack of 20 ms or less, repeated same-pitch notes included"). The generator excludes them.
- Long releases, short gates and quiet notes are not excluded by Dc1-9, which fixes no gate, release or velocity range. They are common in electronic music.
- The noise floor is outside Dc1-9 but is the story's stated outcome ("for external monophonic audio").

## 4. Judgement findings (commit 3ed9472 plus the test file)
- **Noise floor.** Any noise above 1e-4 amplitude (a peak of -80 dBFS) merges the whole clip into one region starting at sample 0 (`sounding_regions`, `src/estimate.rs:665-695`). The noise before the first note then becomes a pitchless "note". The polyphony test runs on it, takes its lowest noise peak as the fundamental and refuses the clip (`src/estimate.rs:1531-1549`). Where the noise does not reach that path, `silence_onset` still gives onset confidence 0 at sample 0 (`:958`). What reaches it: any external recording. CONFIRMED, introduced.
- **Unchecked file contents.** The estimation id covers the clip and the configuration but not the notes. `bind` checks only that id: a hand-edited note line still reads and binds, and so does an estimation of another clip passed to `fit --input`. The reader also accepts `f0_hz 0` together with a non-zero `midi_note`, and confidences below their thresholds, which the writer never produces (`src/estimation_file.rs:279-319`, `crates/patchback-cli/src/estimate.rs:238-259`). No Dc1 decision requires these checks. INFEASIBLE as a failing case; note.
- **Wrong refusal name.** A dyad whose second tone is 12 dB down gets `LowConfidenceNotes` rather than `PolyphonicInput`, so the diagnostic contradicts the spec's order, which puts polyphony before low confidence. CONFIRMED, introduced; warning.

## 5. Attacked and not broken
- High notes at MIDI 77 to 84 with drive up to 18 dB: no Dc1-9 miss.
- Single short low notes (105 ms, MIDI 36 to 40): not refused as polyphonic.
- Octave-up and other legato octave changes (46 → 58, 69 → 57, 57 → 69): within tolerance.
- An undetected octave dyad reports the lower, correct pitch.
- The unknown-pitch encoding (`f0_hz 0`, `midi_note 0`) never reaches a written file: pitch confidence 0 always refuses the estimation, and `F0_MIN_HZ` of 50 Hz keeps a real pitch above MIDI 31.
- Writer line lengths: every written confidence is at least 0.5, so a line can never exceed the 512-byte bound.
- `render` precedence: patch, then unknown estimation, then rate. A rate check only runs on a found sequence, as the comment in `render.rs:508-517` states.
- Determinism: single-threaded f64 code, and the existing byte-identity CLI test holds.

## 6. Paths written outside the worktree
- `the adversary scratch directory c1/note-estimation/adv1/probe-reference.log`
- `the adversary scratch directory c1/note-estimation/adv1/probe-shapes.log`
- `the adversary scratch directory c1/note-estimation/adv1/probe-phrases.sh`
- `the adversary scratch directory c1/note-estimation/adv1/adversary-alone.log`
- `the adversary scratch directory c1/note-estimation/adv1/suite.log`

## 7. Findings

```findings
[
  {"file": "crates/patchback-analysis/tests/adversary_1.rs", "line": 519, "category": "property", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The reference generator on seeds other than the pinned 0x5eed_0c19 yields 13 quantities outside tolerance at or above their confidence thresholds (onset -1597 at 1.0, tail -13307 at 1.0, a spurious note at 1.0), and the pinned seed measuring exactly 1.0 on every rate shows the constants are fitted to it (test: reference_generator_phrases_on_unpinned_seeds_carry_no_confident_quantity_outside_tolerance)"},
  {"file": "crates/patchback-analysis/tests/adversary_1.rs", "line": 533, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "Accepted estimations of reference-generator phrases carry confident wrong onsets and gate ends (seed 7 phrase 10 is 1723 samples early at 0.581), so estimation.txt and notes.events would hold them (test: an_accepted_estimation_of_a_reference_phrase_carries_no_confident_quantity_outside_tolerance)"},
  {"file": "crates/patchback-analysis/tests/adversary_1.rs", "line": 554, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "Monophonic reference phrases are refused as PolyphonicInput (seed 8 phrase 8 at the zero-gap MIDI 36 to 65 change, and seed 9 phrase 30), against Dc1-3 (test: monophonic_reference_phrases_are_not_refused_as_polyphonic)"},
  {"file": "crates/patchback-analysis/tests/adversary_1.rs", "line": 775, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The unchanged reference harness on seed 8 measures detection 0.9452, below the Dc1-9 bound of 0.95, and seed 9 measures 0.9459 (test: the_reference_generator_on_seed_8_finds_at_least_95_percent_of_notes)"},
  {"file": "crates/patchback-analysis/tests/adversary_1.rs", "line": 604, "category": "boundary", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "A same-pitch retrigger with no gap, which Dc1-9 allows and the generator never draws, is accepted with both onsets 1305 samples early at confidence 1.0 and gate ends wrong at 0.95 (test: a_same_pitch_retrigger_with_no_gap_is_within_tolerance_or_not_confident)"},
  {"file": "crates/patchback-analysis/tests/adversary_1.rs", "line": 613, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "Phrases with zero-gap same-pitch repeats carry onsets 1386 to 1672 samples early at confidence 1.0, and that shape measures onset rates of 0.85 to 0.89 against the 0.95 bound (test: zero_gap_same_pitch_repeats_carry_no_confident_quantity_outside_tolerance)"},
  {"file": "crates/patchback-analysis/tests/adversary_1.rs", "line": 661, "category": "boundary", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The one-sided change point in change_point (src/estimate.rs:1094-1143) places a legato octave-down onset 1722 samples early at confidence 0.607 in an accepted estimation (test: a_legato_octave_change_is_within_tolerance_or_not_confident)"},
  {"file": "crates/patchback-analysis/tests/adversary_1.rs", "line": 627, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "Releases of 300 ms to 1.5 s give gate errors up to 18286 samples at confidence 0.81 to 0.99 and gate rates of 0.78 to 0.83 against the 0.90 bound (test: long_releases_carry_no_confident_quantity_outside_tolerance)"},
  {"file": "crates/patchback-analysis/tests/adversary_1.rs", "line": 687, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "A noise floor at -70 dBFS RMS merges the clip into one region from sample 0, and the pitchless noise before the first note makes polyphony refuse an otherwise accepted monophonic phrase as PolyphonicInput (src/estimate.rs:665-695, 1531-1549) (test: a_phrase_over_a_noise_floor_10_db_below_the_silence_threshold_is_estimated)"},
  {"file": "crates/patchback-analysis/tests/adversary_1.rs", "line": 724, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "Dyads whose second tone is 12 dB down are refused as LowConfidenceNotes instead of PolyphonicInput in 3 of 6 cases, contradicting the spec's polyphony-first order (test: a_dyad_whose_second_tone_is_12_db_down_is_refused_as_polyphonic)"},
  {"file": "crates/patchback-cli/src/estimate.rs", "line": 238, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "bind checks only the estimation id, which covers the clip and configuration but not the notes, so an edited note line, an f0 of 0 with a non-zero MIDI number, below-threshold confidences or an estimation of another clip passed to fit --input all bind; no Dc1 decision requires more (test: none)"}
]
```

The home-directory paths in the header and section 6 were replaced by descriptions by the coordinator, because the security gate refuses home paths in commits.
