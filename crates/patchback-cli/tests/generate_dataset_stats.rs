//! `normalization.stats` from `patchback generate-dataset` through the binary
//! (story:normalization-stats, Da8-15, Da8-16, Da8-30): written before the dense files and
//! `dataset.meta`; its `rows` is the training split's count; its means and population standard
//! deviations are those of the training samples' `f64` features (Da8-13), which no file holds,
//! so this file draws the same generation in process (`patchback_dataset::Generator`, seed and
//! counts as given) and recomputes them, two passes, within `1e-12` relative to the largest
//! magnitude of the column; the `x.f32` of every split holds each sample's z-scores under the
//! file's statistics, `((x − mean) / std) as f32` with divisor 1 where the standard deviation is
//! 0, bit for bit. An existing `normalization.stats` is refused before the generation and
//! nothing is written; a failed write of it removes what the run created; two processes write it
//! byte for byte the same.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use patchback_dataset::{Generator, SplitCounts, SplitRole};

const COLUMNS: usize = 32;

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-generate-dataset-stats")
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

fn generate(seed: &str, samples: &str, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args([
            "generate-dataset",
            "--seed",
            seed,
            "--samples",
            samples,
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("the patchback binary runs")
}

/// Every path under `dir`, relative to it, files and directories, sorted.
fn tree(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, paths: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            paths.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
            );
            if path.is_dir() && !path.is_symlink() {
                walk(root, &path, paths);
            }
        }
    }
    let mut paths = Vec::new();
    walk(dir, dir, &mut paths);
    paths.sort();
    paths
}

/// The value of the `key value` record `key` of `text`.
fn record<'a>(text: &'a str, key: &str) -> &'a str {
    text.lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(' '))
        .unwrap_or_else(|| panic!("no {key} record in\n{text}"))
}

/// The bits of each value of `x.f32` of the split directory `dir`, least significant byte first.
fn x_bits(out: &Path, dir: &str) -> Vec<u32> {
    let bytes = fs::read(out.join(dir).join("x.f32")).unwrap();
    let (values, rest) = bytes.as_chunks::<4>();
    assert!(rest.is_empty(), "{dir}/x.f32: a whole number of values");
    values
        .iter()
        .map(|value| u32::from_le_bytes(*value))
        .collect()
}

/// Whether `a` and `b` agree within `1e-12` relative to `scale`, the largest magnitude of the
/// column they are statistics of: the two-pass sums here and the library's may round apart in
/// their last bits, and a mean near 0 of values far from it has no relative precision of its own.
fn close(a: f64, b: f64, scale: f64) -> bool {
    a == b || (a - b).abs() <= 1e-12 * scale
}

/// The statistics of a 64-sample generation (32 / 16 / 16) and the z-scored `x.f32` of each
/// split, against the same generation drawn in process.
#[test]
fn a_generation_writes_the_training_statistics_and_z_scored_dense_files() {
    let dir = scratch("written");
    let out = dir.join("data");
    let started = Instant::now();
    let output = generate("42", "64", &out);
    eprintln!("generate-dataset --samples 64: {:?}", started.elapsed());
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));

    let stats = text(&fs::read(out.join("normalization.stats")).unwrap());
    let lines: Vec<&str> = stats.lines().collect();
    assert_eq!(lines.len(), 5 + COLUMNS, "{stats}");
    assert!(stats.ends_with('\n'));
    let meta = text(&fs::read(out.join("dataset.meta")).unwrap());
    let train = record(&meta, "train");
    assert_eq!(train, "32", "64 samples: 32 train, 16 validation, 16 test");
    assert_eq!(
        lines[..5],
        [
            "patchback-normalization-stats 1",
            "feature_contract features/1",
            "split train",
            format!("rows {train}").as_str(),
            "std population",
        ]
    );
    assert_eq!(record(&meta, "feature_contract"), "features/1");
    let read = patchback_analysis::read_stats(&stats).expect("the library reads it");
    assert_eq!(read.rows().to_string(), train);
    assert_eq!(patchback_analysis::write_stats(&read), stats, "canonical");

    let generated = Generator::new(42, SplitCounts::from_samples(64), out.clone())
        .run()
        .unwrap();
    let rows: Vec<[f64; COLUMNS]> = generated
        .samples(SplitRole::Train)
        .iter()
        .map(|sample| sample.x)
        .collect();
    assert_eq!(rows.len().to_string(), train);
    let n = rows.len() as f64;
    for (i, line) in lines[5..].iter().enumerate() {
        let fields: Vec<&str> = line.split(' ').collect();
        assert_eq!(fields.len(), 4, "{line}");
        assert_eq!((fields[0], fields[1]), ("feature", i.to_string().as_str()));
        let mean: f64 = fields[2].parse().unwrap();
        let std: f64 = fields[3].parse().unwrap();
        let expected_mean = rows.iter().map(|row| row[i]).sum::<f64>() / n;
        let expected_std = (rows
            .iter()
            .map(|row| (row[i] - expected_mean).powi(2))
            .sum::<f64>()
            / n)
            .sqrt();
        let scale = rows.iter().map(|row| row[i].abs()).fold(0.0, f64::max);
        assert!(
            close(mean, expected_mean, scale),
            "{i}: {mean} {expected_mean} (scale {scale})"
        );
        assert!(
            close(std, expected_std, scale),
            "{i}: {std} {expected_std} (scale {scale})"
        );
    }

    let (means, stds) = (read.means(), read.stds());
    let divisor = |j: usize| if stds[j] == 0.0 { 1.0 } else { stds[j] };
    for role in SplitRole::ALL {
        let expected: Vec<u32> = generated
            .samples(role)
            .iter()
            .flat_map(|sample| {
                (0..COLUMNS).map(|j| (((sample.x[j] - means[j]) / divisor(j)) as f32).to_bits())
            })
            .collect();
        assert_eq!(
            x_bits(&out, role.dir()),
            expected,
            "{role}: z-scored under the training statistics"
        );
    }

    let mut expected = vec!["dataset.meta".to_owned(), "normalization.stats".to_owned()];
    for split in ["train", "val", "test"] {
        expected.push(split.to_owned());
        for file in ["order.u32", "samples.meta", "x.f32", "y.f32"] {
            expected.push(format!("{split}/{file}"));
        }
    }
    expected.sort();
    assert_eq!(tree(&out), expected);
}

#[test]
fn two_processes_write_byte_identical_statistics() {
    let dir = scratch("twice");
    let first = dir.join("first");
    let second = dir.join("second");
    assert_eq!(generate("7", "48", &first).status.code(), Some(0));
    assert_eq!(generate("7", "48", &second).status.code(), Some(0));
    let a = fs::read(first.join("normalization.stats")).unwrap();
    let b = fs::read(second.join("normalization.stats")).unwrap();
    assert_eq!(a, b);
    let other = dir.join("other");
    assert_eq!(generate("8", "48", &other).status.code(), Some(0));
    assert_ne!(
        a,
        fs::read(other.join("normalization.stats")).unwrap(),
        "another seed draws other training rows"
    );
}

/// An existing `normalization.stats` is refused by the `--out` pre-check, before the generation:
/// it keeps its bytes and the directory is left as it was.
#[test]
fn an_existing_normalization_stats_is_never_overwritten_and_nothing_is_written() {
    let dir = scratch("existing");
    let out = dir.join("data");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("normalization.stats"), "kept\n").unwrap();
    let started = Instant::now();
    let output = generate("42", "48", &out);
    let elapsed = started.elapsed();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let stderr = text(&output.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with("patchback: generate-dataset refused: cannot write ")
            && stderr.contains(&format!("{:?}", out.join("normalization.stats")))
            && stderr.contains("already exists"),
        "{stderr}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "refused after {elapsed:?}, so not by the pre-check"
    );
    assert_eq!(
        fs::read_to_string(out.join("normalization.stats")).unwrap(),
        "kept\n"
    );
    assert_eq!(tree(&out), ["normalization.stats"]);
}

/// A write of `normalization.stats` that fails (a dangling symbolic link, which the pre-check does
/// not see and the exclusive create refuses) is the run's first write, so nothing else is
/// written: no split directory, no dense file, no `dataset.meta`.
#[cfg(unix)]
#[test]
fn a_failed_statistics_write_removes_everything_the_run_created() {
    let dir = scratch("failed-write");
    let out = dir.join("data");
    fs::create_dir_all(&out).unwrap();
    std::os::unix::fs::symlink(dir.join("nowhere"), out.join("normalization.stats")).unwrap();
    let output = generate("42", "48", &out);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with("patchback: generate-dataset refused: cannot write ")
            && stderr.contains(&format!("{:?}", out.join("normalization.stats"))),
        "{stderr}"
    );
    assert_eq!(tree(&out), ["normalization.stats"]);
    assert!(!dir.join("nowhere").exists());
}
