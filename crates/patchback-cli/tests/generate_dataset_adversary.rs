//! Adversary cases for `patchback generate-dataset` (story:dataset-generation, wave a6, pass 1):
//! the row order of `samples.meta` against the SplitMix64 stream of the seed, the diagnostic
//! counts against the draws the files imply, the seed extremes, and the refusal of a `--samples`
//! too large for the split counts' type. Generations are 48 samples.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_core::prng::{SplitMix64, Xoshiro256StarStar};

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-generate-dataset-adversary")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn generate(seed: &str, samples: &str, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(["generate-dataset", "--seed", seed, "--samples", samples])
        .arg("--out")
        .arg(out)
        .output()
        .expect("the patchback binary runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The `(patch_seed, midi_note, gate_samples)` rows of a split's `samples.meta`.
fn rows(out: &Path, split: &str) -> Vec<(u64, u64, u64)> {
    let list = text(&fs::read(out.join(split).join("samples.meta")).unwrap());
    list.lines()
        .skip(1)
        .map(|line| {
            let f: Vec<u64> = line.split(' ').map(|v| v.parse().unwrap()).collect();
            (f[0], f[1], f[2])
        })
        .collect()
}

fn meta_count(meta: &str, key: &str) -> u64 {
    meta.lines()
        .find_map(|line| line.strip_prefix(&format!("{key} ")))
        .unwrap_or_else(|| panic!("{key} in {meta}"))
        .parse()
        .unwrap()
}

/// D-a6-8 from the files alone: every row's seed is an output of SplitMix64(42); the rows of
/// train, then validation, then test are in stream order; each row's note and gate are the
/// seed's own xoshiro256** draws after eight parameters; and the draws the files imply (the
/// stream position of the last test row, plus one) are the samples plus the diagnostic counts.
#[test]
fn adversary_sample_rows_are_in_stream_order_and_the_diagnostics_are_the_skipped_draws() {
    let dir = scratch("stream-order");
    let out = dir.join("data");
    let output = generate("42", "48", &out);
    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let mut mix = SplitMix64::new(42);
    let position: HashMap<u64, usize> = (0..4 * 48).map(|i| (mix.next_u64(), i)).collect();
    let mut last: Option<usize> = None;
    let mut samples = 0u64;
    for split in ["train", "val", "test"] {
        let rows = rows(&out, split);
        assert_eq!(rows.len(), 16, "{split}");
        for (seed, midi, gate) in rows {
            let at = *position
                .get(&seed)
                .unwrap_or_else(|| panic!("{split}: seed {seed} is not drawn from seed 42"));
            assert!(
                last.is_none_or(|before| at > before),
                "{split}: seed {seed} at stream position {at} after {last:?}"
            );
            last = Some(at);
            samples += 1;

            let mut prng = Xoshiro256StarStar::new(seed);
            for _ in 0..8 {
                prng.next_f64();
            }
            let expected_midi = 36 + (prng.next_f64() * 49.0).floor() as u64;
            let expected_gate = (0.25 * 6f64.powf(prng.next_f64()) * 48_000.0).round() as u64;
            assert_eq!(
                (midi, gate),
                (expected_midi, expected_gate),
                "{split} {seed}"
            );
        }
    }
    let draws = last.unwrap() as u64 + 1;
    let meta = text(&fs::read(out.join("dataset.meta")).unwrap());
    let diagnostics: u64 = ["silence", "noise", "clipping"]
        .iter()
        .map(|k| meta_count(&meta, &format!("diagnostic_{k}")))
        .sum();
    assert_eq!(draws, samples + diagnostics, "{meta}");
}

/// The extremes of `--seed` generate, and `dataset.meta` records the seed as given.
#[test]
fn adversary_seed_zero_and_seed_max_generate() {
    let dir = scratch("seed-extremes");
    for seed in ["0", "18446744073709551615"] {
        let out = dir.join(format!("data-{seed}"));
        let output = generate(seed, "48", &out);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let meta = text(&fs::read(out.join("dataset.meta")).unwrap());
        assert_eq!(meta.lines().nth(1), Some(format!("seed {seed}").as_str()));
        for split in ["train", "val", "test"] {
            assert_eq!(rows(&out, split).len(), 16, "{seed} {split}");
        }
    }
}

/// `--samples 32` leaves a train split of 0: refused, nothing created.
#[test]
fn adversary_a_train_split_of_zero_is_refused() {
    let dir = scratch("train-zero");
    let out = dir.join("data");
    let output = generate("42", "32", &out);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with(
            "patchback: generate-dataset refused: patchback.dataset.SplitCountNotBatchMultiple: "
        ),
        "{stderr}"
    );
    assert!(stderr.contains("Train split of 0 samples"), "{stderr}");
    assert!(!out.exists());
}

/// D-a6-10 at a `--samples` of 2^64 - 16: validation and test are positive multiples of 16, and
/// so is train (`samples - 2 * validation`, both multiples of 16), but train does not fit the
/// model's `Integer` (i64). Re-pinned to Da6-26: `--samples` admits `0..=i64::MAX`, so this count
/// is a usage error (exit 2, clap's message), never a SplitCountNotBatchMultiple refusal naming a
/// train count the rule does not give.
#[test]
fn adversary_a_refused_train_count_is_the_count_the_rule_gives() {
    let samples: u64 = u64::MAX - 15;
    let held_out = (u128::from(samples) * 4_992 / 60_000 / 16 * 16).max(16);
    let train = u128::from(samples) - 2 * held_out;
    assert_eq!(train % 16, 0, "train is a positive multiple of 16");
    assert_eq!(samples.to_string(), "18446744073709551600");

    let dir = scratch("huge-samples");
    let out = dir.join("data");
    let output = generate("42", &samples.to_string(), &out);
    assert!(!out.exists());
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("--samples"), "{stderr}");
    assert!(!stderr.contains("SplitCountNotBatchMultiple"), "{stderr}");
}
