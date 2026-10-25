//! `patchback benchmark --out <dir> --examples <n> …` through the binary (story:engine-baseline):
//! a small engine baseline over a synthesized holdout completes with exit 0 and writes
//! `benchmark.txt`, `clips.txt` and the listening examples; the medians, the reduction and the
//! verdict agree with the per-clip records; the listening examples compare back to the recorded
//! losses; the refusals exit 1 with nothing written; two processes give the same files. The
//! `Benchmark` model and its scenarios are held by `crates/patchback-fit/tests/engine_baseline.rs`.
//!
//! The runs are small: two clips, a dozen renders a fit, one restart.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

use patchback_fit::LISTENING_EXAMPLE_COUNT;

/// The holdout stream's domain, `HOLDOUT_DOMAIN` of the verb (Da8-46): `--seed s` draws the
/// holdout from `SplitMix64::new(s ^ HOLDOUT_DOMAIN)`.
const HOLDOUT_DOMAIN: u64 = 0x686f_6c64_6f75_7431;

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-benchmark")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("output is UTF-8")
}

fn patchback(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(args)
        .output()
        .expect("the patchback binary runs")
}

/// The small run's flags after `--out <out>`.
const SMALL: &[&str] = &[
    "--examples",
    "2",
    "--seed",
    "7",
    "--max-renders",
    "12",
    "--max-restarts",
    "1",
];

/// The determinism pin needs no search quality: two renders a fit keep it short (Da8-38).
const TWO_RENDERS: &[&str] = &[
    "--examples",
    "2",
    "--seed",
    "7",
    "--max-renders",
    "2",
    "--max-restarts",
    "1",
];

fn benchmark_args(out: &Path, flags: &[&str]) -> Vec<String> {
    let mut args = vec![
        "benchmark".to_owned(),
        "--out".to_owned(),
        out.to_str().unwrap().to_owned(),
    ];
    args.extend(flags.iter().map(|s| (*s).to_owned()));
    args
}

fn benchmark(out: &Path, flags: &[&str]) -> Output {
    let args = benchmark_args(out, flags);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    patchback(&args)
}

/// Every file and directory under `dir`, relative to it, sorted.
fn listing(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, into: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            into.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
            if path.is_dir() {
                walk(root, &path, into);
            }
        }
    }
    let mut all = Vec::new();
    walk(dir, dir, &mut all);
    all.sort();
    all
}

/// A refusal: exit 1, nothing on stdout, one stderr line starting with `prefix`.
fn assert_refused(output: &Output, prefix: &str) -> String {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with(prefix), "{stderr}");
    stderr
}

/// The `benchmark.txt` record keys, in order, before the listening examples.
const BENCHMARK_KEYS: [&str; 19] = [
    "patchback-benchmark",
    "patchback",
    "kind",
    "topology",
    "seed",
    "example_count",
    "sample_rate",
    "block_size",
    "max_renders",
    "max_wall_time_ms",
    "max_restarts",
    "improvement_threshold",
    "prng",
    "default_median_loss",
    "default_search_median_loss",
    "median_loss_reduction_percent",
    "target_verdict",
    "default_search_median_evaluations",
    "wall_time_ms",
];

/// The `(key, value)` records of a `key value` text, in order.
fn records(text: &str) -> Vec<(String, String)> {
    assert!(text.ends_with('\n'), "{text:?}");
    text.lines()
        .map(|line| {
            let (key, value) = line.split_once(' ').unwrap_or_else(|| panic!("{line:?}"));
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

fn value<'a>(records: &'a [(String, String)], key: &str) -> &'a str {
    let found: Vec<&str> = records
        .iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(found.len(), 1, "{key}: {records:?}");
    found[0]
}

/// One `clip` line of `clips.txt`.
#[derive(Debug)]
struct Clip {
    index: usize,
    patch_seed: u64,
    fit_seed: u64,
    default_loss: f64,
    best_loss: f64,
    evaluations: i64,
    restarts: u32,
    stop_reason: String,
}

fn clips(text: &str) -> Vec<Clip> {
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("patchback-benchmark-clips 1"));
    assert_eq!(
        lines.next(),
        Some(
            "columns index patch_seed fit_seed default_loss best_loss evaluations restarts \
             stop_reason"
        )
    );
    assert!(text.ends_with('\n'));
    lines
        .map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            assert_eq!(fields.len(), 9, "{line:?}");
            assert_eq!(fields[0], "clip", "{line:?}");
            Clip {
                index: fields[1].parse().unwrap(),
                patch_seed: fields[2].parse().unwrap(),
                fit_seed: fields[3].parse().unwrap(),
                default_loss: fields[4].parse().unwrap(),
                best_loss: fields[5].parse().unwrap(),
                evaluations: fields[6].parse().unwrap(),
                restarts: fields[7].parse().unwrap(),
                stop_reason: fields[8].to_owned(),
            }
        })
        .collect()
}

/// The lower median (Da8-4), recomputed here.
fn lower_median<T: Copy + PartialOrd>(values: &[T]) -> T {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[(sorted.len() - 1) / 2]
}

/// The small run completes with exit 0; `benchmark.txt` and `clips.txt` read back with their
/// records in the documented order; the medians and the reduction recompute from `clips.txt`;
/// the verdict agrees with the reduction; stdout is the records from `default_median_loss` on.
#[test]
fn a_small_benchmark_completes_and_its_files_agree() {
    let dir = scratch("small");
    let out = dir.join("bench");
    let started = Instant::now();
    let output = benchmark(&out, SMALL);
    let elapsed = started.elapsed();
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    eprintln!("benchmark --examples 2: {elapsed:?}");

    let file = fs::read_to_string(out.join("benchmark.txt")).unwrap();
    let records = records(&file);
    let keys: Vec<&str> = records.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys[..BENCHMARK_KEYS.len()], BENCHMARK_KEYS);
    let examples: Vec<&str> = [
        "examples/0/reference.wav",
        "examples/0/default.wav",
        "examples/0/fit.wav",
        "examples/1/reference.wav",
        "examples/1/default.wav",
        "examples/1/fit.wav",
    ]
    .to_vec();
    let tail: Vec<(&str, &str)> = records[BENCHMARK_KEYS.len()..]
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        tail,
        examples
            .iter()
            .map(|path| ("listening_example", *path))
            .collect::<Vec<_>>()
    );
    for path in &examples {
        assert!(out.join(path).is_file(), "{path}");
    }
    assert_eq!(value(&records, "patchback-benchmark"), "1");
    assert_eq!(value(&records, "patchback"), env!("CARGO_PKG_VERSION"));
    assert_eq!(value(&records, "kind"), "engine-baseline");
    assert_eq!(value(&records, "topology"), "subtractive-mono/1");
    assert_eq!(value(&records, "seed"), "7");
    assert_eq!(value(&records, "example_count"), "2");
    assert_eq!(value(&records, "sample_rate"), "48000");
    assert_eq!(value(&records, "block_size"), "256");
    assert_eq!(value(&records, "max_renders"), "12");
    assert_eq!(value(&records, "max_wall_time_ms"), "60000");
    assert_eq!(value(&records, "max_restarts"), "1");
    assert_eq!(value(&records, "improvement_threshold"), "0.0001");
    assert_eq!(value(&records, "prng"), "xoshiro256**");

    let clips = clips(&fs::read_to_string(out.join("clips.txt")).unwrap());
    assert_eq!(clips.len(), 2);
    for (k, clip) in clips.iter().enumerate() {
        assert_eq!(clip.index, k);
        assert!(clip.evaluations >= 1 && clip.evaluations <= 12, "{clip:?}");
        assert_eq!(clip.restarts, 1, "{clip:?}");
        assert!(clip.best_loss <= clip.default_loss, "{clip:?}");
        assert!(
            ["RenderBudgetExhausted", "RestartsExhausted"].contains(&clip.stop_reason.as_str()),
            "{clip:?}"
        );
        assert_ne!(clip.patch_seed, clip.fit_seed);
    }
    let default_median = lower_median(&clips.iter().map(|c| c.default_loss).collect::<Vec<_>>());
    let search_median = lower_median(&clips.iter().map(|c| c.best_loss).collect::<Vec<_>>());
    let evaluations = lower_median(&clips.iter().map(|c| c.evaluations).collect::<Vec<_>>());
    let reduction = (default_median - search_median) / default_median * 100.0;
    assert_eq!(
        value(&records, "default_median_loss"),
        default_median.to_string()
    );
    assert_eq!(
        value(&records, "default_search_median_loss"),
        search_median.to_string()
    );
    assert_eq!(
        value(&records, "median_loss_reduction_percent"),
        reduction.to_string()
    );
    assert_eq!(
        value(&records, "default_search_median_evaluations"),
        evaluations.to_string()
    );
    let verdict = if reduction >= 20.0 { "met" } else { "not-met" };
    assert_eq!(value(&records, "target_verdict"), verdict);
    let wall: u64 = value(&records, "wall_time_ms").parse().unwrap();
    assert!(u128::from(wall) <= elapsed.as_millis(), "{wall} ms");

    let start = file.find("default_median_loss ").unwrap();
    assert_eq!(text(&output.stdout), file[start..]);
}

/// The holdout's seeds are exactly the documented derivation (Da8-37, Da8-46): the stream
/// `SplitMix64::new(7 ^ HOLDOUT_DOMAIN)` is walked from its start; each output is a patch seed
/// whose draw the dataset generator's pipeline classifies; a diagnostic draw consumes that one
/// output and is skipped; an admitted
/// draw is clip `k`'s patch seed and the next output its fit seed. `clips.txt` names exactly
/// those two outputs for every clip.
#[test]
fn the_seeds_follow_the_documented_stream() {
    let dir = scratch("seeds");
    let out = dir.join("bench");
    let output = benchmark(&out, SMALL);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let clips = clips(&fs::read_to_string(out.join("clips.txt")).unwrap());
    assert_eq!(clips.len(), 2);

    let pipeline = patchback_dataset::Pipeline::new();
    let mut stream = patchback_core::prng::SplitMix64::new(7 ^ HOLDOUT_DOMAIN);
    for clip in &clips {
        let patch_seed = loop {
            let candidate = stream.next_u64();
            let draw = patchback_dataset::PatchDraw::from_seed(candidate);
            match pipeline.run(&draw).expect("no draw is a fault") {
                patchback_dataset::Rendering::Sample(_) => break candidate,
                patchback_dataset::Rendering::Diagnostic(_) => {}
            }
        };
        let fit_seed = stream.next_u64();
        assert_eq!(clip.patch_seed, patch_seed, "{clip:?}");
        assert_eq!(clip.fit_seed, fit_seed, "{clip:?}");
    }
}

/// Da8-46: the holdout is not the training data. With the same seed, `generate-dataset`'s first
/// sample (train sample 0 of [`patchback_dataset::Generator`], run through the real pipeline at
/// the smallest admitted counts) and the benchmark's clip 0 come from different patch seeds, and
/// clip 0's fit seed is not the seed the dataset generator would draw its first patch from.
#[test]
fn the_holdout_is_not_the_training_data_of_the_same_seed() {
    let dir = scratch("holdout-domain");
    let out = dir.join("bench");
    let output = benchmark(&out, SMALL);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let clips = clips(&fs::read_to_string(out.join("clips.txt")).unwrap());

    let counts = patchback_dataset::SplitCounts {
        train: 16,
        validation: 16,
        test: 16,
    };
    let generated = patchback_dataset::Generator::new(7, counts, dir.join("dataset"))
        .run()
        .unwrap();
    let train = generated.samples(patchback_dataset::SplitRole::Train);
    let sample_0 = generated.patch_seed_of(train[0]);
    let first_dataset_seed = patchback_core::prng::SplitMix64::new(7).next_u64();
    for clip in &clips {
        assert_ne!(clip.patch_seed, sample_0, "{clip:?}");
        assert_ne!(clip.patch_seed, first_dataset_seed, "{clip:?}");
        assert_ne!(clip.fit_seed, first_dataset_seed, "{clip:?}");
    }
}

/// `patchback compare` of `examples/0/reference.wav` with `examples/0/fit.wav` prints clip 0's
/// best loss from `clips.txt`; with `default.wav`, its default loss.
#[test]
fn the_listening_examples_compare_back_to_the_recorded_losses() {
    let dir = scratch("listen");
    let out = dir.join("bench");
    let output = benchmark(&out, SMALL);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let clips = clips(&fs::read_to_string(out.join("clips.txt")).unwrap());
    let reference = out.join("examples/0/reference.wav");
    for (candidate, expected) in [
        ("fit.wav", clips[0].best_loss),
        ("default.wav", clips[0].default_loss),
    ] {
        let compared = patchback(&[
            "compare",
            "--reference",
            reference.to_str().unwrap(),
            "--candidate",
            out.join("examples/0").join(candidate).to_str().unwrap(),
        ]);
        assert_eq!(
            compared.status.code(),
            Some(0),
            "{}",
            text(&compared.stderr)
        );
        let printed = records(&text(&compared.stdout));
        assert_eq!(value(&printed, "loss"), expected.to_string(), "{candidate}");
    }
    let bytes = fs::metadata(&reference).unwrap().len();
    eprintln!(
        "clip 0: {} frames, {} s",
        (bytes - 44) / 4,
        (bytes - 44) as f64 / 4.0 / 48_000.0
    );
}

/// Only the first [`LISTENING_EXAMPLE_COUNT`] clips get listening examples.
#[test]
fn at_most_three_clips_get_listening_examples() {
    assert_eq!(LISTENING_EXAMPLE_COUNT, 3);
    let dir = scratch("four");
    let out = dir.join("bench");
    let output = benchmark(
        &out,
        &[
            "--examples",
            "4",
            "--seed",
            "11",
            "--max-renders",
            "3",
            "--max-restarts",
            "1",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let mut dirs: Vec<String> = fs::read_dir(out.join("examples"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    dirs.sort();
    assert_eq!(dirs, ["0", "1", "2"]);
    let file = fs::read_to_string(out.join("benchmark.txt")).unwrap();
    assert_eq!(
        file.lines()
            .filter(|l| l.starts_with("listening_example "))
            .count(),
        9
    );
    assert_eq!(
        clips(&fs::read_to_string(out.join("clips.txt")).unwrap()).len(),
        4
    );
}

/// `--examples 0` reaches the library and is refused `InvalidHoldoutSize`; nothing is written.
#[test]
fn an_empty_holdout_is_refused() {
    let dir = scratch("empty");
    let out = dir.join("bench");
    let stderr = assert_refused(
        &benchmark(&out, &["--examples", "0"]),
        "patchback: benchmark refused: patchback.fit.InvalidHoldoutSize: ",
    );
    assert!(stderr.contains("holdout"), "{stderr}");
    assert!(!out.exists());
}

/// An unknown `--topology` is refused `TopologyNotFound` naming it; nothing is written.
#[test]
fn an_unknown_topology_is_refused() {
    let dir = scratch("topology");
    let out = dir.join("bench");
    let stderr = assert_refused(
        &benchmark(&out, &["--examples", "1", "--topology", "no-such/1"]),
        "patchback: benchmark refused: patchback.fit.TopologyNotFound: ",
    );
    assert!(stderr.contains("\"no-such/1\""), "{stderr}");
    assert!(!out.exists());
}

/// A bound of 0 is refused `InvalidSearchBound` before any work; nothing is written.
#[test]
fn a_zero_bound_is_refused() {
    let dir = scratch("bound");
    let out = dir.join("bench");
    assert_refused(
        &benchmark(&out, &["--examples", "1", "--max-renders", "0"]),
        "patchback: benchmark refused: patchback.fit.InvalidSearchBound: ",
    );
    assert!(!out.exists());
}

/// `benchmark.txt`, `clips.txt` or `examples` already in `--out` is refused before any work;
/// the directory is left as it was.
#[test]
fn an_existing_output_is_refused_and_left_as_it_was() {
    for name in ["benchmark.txt", "clips.txt", "examples"] {
        let dir = scratch(&format!("existing-{name}"));
        let out = dir.join("bench");
        fs::create_dir_all(&out).unwrap();
        if name == "examples" {
            fs::create_dir(out.join(name)).unwrap();
            fs::write(out.join(name).join("keep"), "kept\n").unwrap();
        } else {
            fs::write(out.join(name), "kept\n").unwrap();
        }
        let before = listing(&out);
        let stderr = assert_refused(
            &benchmark(&out, SMALL),
            "patchback: benchmark refused: cannot write ",
        );
        assert!(
            stderr.contains(&format!("{:?}", out.join(name))),
            "{name}: {stderr}"
        );
        assert!(stderr.contains("already exists"), "{stderr}");
        assert_eq!(listing(&out), before, "{name}");
        let kept = if name == "examples" {
            out.join(name).join("keep")
        } else {
            out.join(name)
        };
        assert_eq!(fs::read_to_string(kept).unwrap(), "kept\n");
    }
}

/// An `--out` that is a file is refused, the file unchanged.
#[test]
fn an_out_that_is_a_file_is_refused() {
    let dir = scratch("file");
    let out = dir.join("bench");
    fs::write(&out, "a file\n").unwrap();
    assert_refused(
        &benchmark(&out, SMALL),
        "patchback: benchmark refused: cannot write ",
    );
    assert_eq!(fs::read_to_string(&out).unwrap(), "a file\n");
}

/// Two processes with the same flags write byte-identical `clips.txt` and WAVs, and
/// `benchmark.txt` differing at most in `wall_time_ms`.
#[test]
fn two_runs_of_the_same_flags_write_the_same_files() {
    let dir = scratch("determinism");
    let first = dir.join("first");
    let second = dir.join("second");
    for out in [&first, &second] {
        let output = benchmark(out, TWO_RENDERS);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    }
    assert_eq!(listing(&first), listing(&second));
    for name in listing(&first) {
        if name == "benchmark.txt" || first.join(&name).is_dir() {
            continue;
        }
        assert_eq!(
            fs::read(first.join(&name)).unwrap(),
            fs::read(second.join(&name)).unwrap(),
            "{name}"
        );
    }
    let without_wall = |path: &Path| -> BTreeMap<usize, String> {
        fs::read_to_string(path.join("benchmark.txt"))
            .unwrap()
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.starts_with("wall_time_ms "))
            .map(|(i, line)| (i, line.to_owned()))
            .collect()
    };
    assert_eq!(without_wall(&first), without_wall(&second));
}

/// A write that fails after the fits removes everything the run wrote. The failure is a file
/// size limit (`ulimit -f 16`, with `SIGXFSZ` ignored so the write fails with `EFBIG`):
/// `clips.txt` is written below it, the first WAV (about 100 KB or more) is not. Exit 1, one
/// stderr line naming the WAV, `--out` and every directory the run created gone.
#[cfg(unix)]
#[test]
fn a_write_that_fails_removes_everything_the_run_wrote() {
    let dir = scratch("failed-write");
    let out = dir.join("results").join("nested");
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg("ulimit -f 16 && trap '' XFSZ && exec \"$0\" \"$@\"")
        .arg(env!("CARGO_BIN_EXE_patchback"))
        .args(benchmark_args(&out, SMALL))
        .output()
        .expect("sh runs");
    let wav = out.join("examples").join("0").join("reference.wav");
    assert_refused(
        &output,
        &format!("patchback: benchmark refused: cannot write {wav:?}: "),
    );
    assert!(!dir.join("results").exists(), "{:?}", listing(&dir));
    assert!(dir.is_dir());
}
